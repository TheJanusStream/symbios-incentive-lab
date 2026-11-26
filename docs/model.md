# The incentive lab model (v0.2)

This document specifies the simulation in `incentive-core`. It is the contract between the model and everything built
on it: the command-line runner, the game app and the exploration tools. Values marked *calibrate* are starting
points; section 13 records the values the code actually uses and what was measured with them.

## 1. Purpose and epistemic status

The lab is a toy model for building intuition about incentive design on one social platform. It answers questions of
the form "under which designs, and under which assumptions about people, do engagement, harm and benefit move
together, and where do they come apart?" It forecasts no real platform. Three rules follow from that:

- Every claim comes from many seeds and is reported with its spread, never from one run.
- Design levers (what the platform chooses, section 10) are kept apart from world assumptions (how people are assumed
  to behave, section 11). A conclusion is only as strong as its robustness across plausible assumptions.
- Every quantity is either observable by the platform or ground truth (section 9). The platform's ranking,
  moderation and payouts read observable quantities only. The player sees both, which is what makes Goodhart effects
  visible: the platform's numbers can climb while the truth turns.

Agents adapt: authors learn which kinds of posts pay, toxic authors learn to evade moderation, and the population
turns over. Without adaptation a simulation cannot show gaming at all, because fixed-rule agents assume the response
away.

## 2. Notation and conventions

- Time advances in days, `t = 0 .. days-1`. One call to `Simulation::step` simulates one day.
- `N` individuals (the population, users and non-users), indexed `0 .. N-1`. Bots are separate accounts (3.5).
- `T = 6` topics: Public affairs (topic 0), Science, Sports, Arts, Lifestyle, Tech.
- `S = 6` content styles: Informative, Entertaining, Personal, Outrage, Clickbait, Attack (section 4).
- Two opinion groups, A and B, by the sign of an individual's opinion.
- `sig(x)` is the logistic function, `clamp01(x)` clips to `[0, 1]`, `N(m, s)` is a normal draw.
- All model quantities are `f32`. Parameters named in `code font` are fields of the configuration (sections 10, 11).

## 3. Population

### 3.1 Archetypes

Each individual has one archetype, drawn from `population.archetype_shares` (default in brackets): Casual (0.50),
Creator (0.10), Partisan (0.20), Troll (0.03), Vulnerable (0.17). Archetypes only set the distributions traits are
drawn from; behaviour then follows from the traits.

The draw is an exponential race: each individual holds one `Exp(1)` value per archetype from its own stream and
takes the archetype that minimises `value / share`. Each archetype wins with probability proportional to its share,
and raising one share only moves individuals into that archetype, so two runs that differ in one share differ in as
few individuals as possible (section 12).

### 3.2 Traits (fixed per individual)

Each trait is drawn from a Beta distribution with the archetype's mean `m` and concentration 8, that is
`Beta(8m, 8(1-m))`, unless stated otherwise.

| Trait | Casual | Creator | Partisan | Troll | Vulnerable | Meaning |
| --- | --- | --- | --- | --- | --- | --- |
| `activity` | 0.60 | 0.90 | 0.75 | 0.80 | 0.60 | base chance of opening the app on a day |
| `post_rate` | 0.3 | 2.0 | 1.0 | 1.5 | 0.3 | mean posts per active day (Poisson, not Beta) |
| `temperament` | 0.20 | 0.25 | 0.45 | 0.85 | 0.15 | propensity to incivility |
| `approval_sensitivity` | 0.40 | 0.75 | 0.60 | 0.70 | 0.50 | weight of social reward |
| `impulsivity` | 0.50 | 0.45 | 0.60 | 0.70 | 0.55 | weight of impulse over reflection |
| `toxicity_tolerance` | 0.40 | 0.50 | 0.55 | 0.90 | 0.15 | how little toxicity hurts |
| `vulnerability` | 0.30 | 0.35 | 0.30 | 0.10 | 0.80 | harm multiplier for harassment and comparison |
| `skill` | 0.40 | 0.65 | 0.45 | 0.30 | 0.40 | quality of what the individual writes |
| `money_weight` | 0.10 | 0.70 | 0.20 | 0.10 | 0.10 | weight of creator income |
| `credulity` | 0.40 | 0.30 | 0.55 | 0.40 | 0.45 | chance of believing misinformation |
| `partisanship` | 0.30 | 0.35 | 0.85 | 0.70 | 0.30 | magnitude of the opinion |
| `time_budget` | 20 | 40 | 30 | 30 | 15 | items per day the individual would endorse (fixed) |

- `opinion = sign * partisanship`, with the sign (group A or B) drawn 50/50.
- `interests`: a Dirichlet(0.7) draw over the `T` topics. For Partisans the weight of topic 0 is tripled before
  normalising.
- `intrinsic[s]`: the value an individual places on writing each style, before any reward. Archetype base values,
  plus `N(0, 0.1)` per individual:

| Style | Casual | Creator | Partisan | Troll | Vulnerable |
| --- | --- | --- | --- | --- | --- |
| Informative | 0.2 | 0.4 | 0.5 | 0.0 | 0.2 |
| Entertaining | 0.4 | 0.5 | 0.2 | 0.2 | 0.3 |
| Personal | 0.6 | 0.3 | 0.2 | 0.0 | 0.6 |
| Outrage | 0.0 | 0.0 | 0.3 | 0.5 | -0.1 |
| Clickbait | -0.2 | 0.1 | 0.0 | 0.2 | -0.2 |
| Attack | -0.6 | -0.5 | -0.2 | 0.6 | -0.8 |

  Temperament then adds `0.5 * temperament` to Outrage and `0.6 * (temperament - 0.5)` to Attack.

### 3.3 State (evolving per individual)

- `is_user`, `joined_day`, `left_day`, `ever_user`.
- `wellbeing` in `[0, 1]`, starting at a personal baseline `wb0 = clamp01(N(0.6, 0.1))`.
- `habit` in `[0, 1]`, starting at 0.1 for initial users.
- `stay_ema`: exponential moving average of daily stay utility (7.3), starting at 0.3.
- `affective_polarisation` in `[0, 1]`, baseline `0.25 + 0.4 * partisanship`.
- `informedness` in `[0, 1]`, baseline 0.6.
- `style_value[s]`: learned value of each style (section 7.5), starting at `intrinsic[s]`.
- `polish_value[plain]`, `polish_value[polished]`: learned advantage of plain and polished posts (7.5), starting
  at 0 and at `-3 * T_p` (three polish temperatures below plain), so few posts are polished on day 0.
- `posts_written`: the individual's posts over the run; the next post's ordinal (section 12).
- `pleasure` in `[0, 1]`: recent impulsive pleasure (7.3), starting at 0.
- `evasion` in `[0, 1]`: skill at writing toxicity that classifiers miss, starting at 0.
- `intensity` in `[0, 1]`: how hostile the individual's hostile posts are (7.5), starting at 0.5.
- `follows` (accounts followed), `followers` (count), `blocked` (bounded list, at most 32 accounts).
- `strikes`, `suspended_until`, `banned`.
- `income_ema` (30-day), `income_total`, `reward_ema` (7-day mean reward of the individual's posts).

### 3.4 Offline ties and the initial follow graph

- Every individual has `offline_ties` (default 8) ties to other individuals, drawn so that a tie is same-group with
  probability `world.homophily` (default 0.7). Ties never change. They carry joining pull (7.4) and spillover to
  non-users (8.2).
- At day 0, a share `run.initial_user_share` (default 0.6) of individuals, drawn at random, are users.
- Each initial user follows its offline ties who are users, then accounts drawn by preferential attachment
  (probability proportional to `followers + 1`, Creators weighted 5x) with same-group probability `world.homophily`,
  until it follows `initial_follows` accounts (default 30).

### 3.5 Bots

Bots are accounts run by engagement farms, not people. The number of bot accounts is
`round(population.bot_share * N * (floor + (1 - floor) * (1 - design.account_cost)^2))` with
`floor = world.bot_farm_floor` (defaults `bot_share` 0.05, `bot_farm_floor` 0.2): farms buy verified accounts, so
even the highest account cost keeps only part of them out. Each bot belongs to a
group, posts Clickbait or Outrage (50/50) at 3 posts per day, and each day likes and shares `bot_boosts` (default 20)
recent posts; a suspended bot neither posts, boosts nor brigades. Each boost is one like (counted for the bot's
group) and one share. Bot posts are counted apart from the human content mix (9.2). Bots have no experience,
wellbeing or learning. Their engagement, reports and see-less marks are counted by the platform (they are
observable and inflate or distort its signals) but never in ground-truth metrics. A banned bot is replaced after a
delay with probability `floor + (1 - floor) * (1 - account_cost)` per day: the farm rotates to a new account with
no followers and no strikes.

How the farms play is a world assumption, `world.bot_strategy`:

- `shared-pool`: every bot boosts random posts from one shared pool, the live posts by bots and the live Clickbait
  posts, whatever its group. Bots never polish or brigade.
- `own-side`: each bot boosts random posts from its own group's part of that pool, the bot posts of its group and
  the Clickbait posts by human authors of its group. Bots never polish or brigade.
- `adaptive` (default): bots aim at what the design rewards. They read the ranking weights users meet on average
  (the main ranking's for its users, the alternative feed's for its share `alt_feed_share`; a chronological feed
  weighs nothing) and split each bot's boosts between two tactics: `round(bot_boosts * w_bridging / (w_like +
  w_share + w_bridging))` cross-group boosts from the shared pool, so both groups like the same posts and raise
  their bridging score, and the rest own-side boosts, which feed predicted engagement (6.3), trending, social proof
  and engagement payouts. Without any of these weights every boost is own-side. Adaptive bots polish all their posts
  when the ranking weighs `Q_hat` (`w_quality > 0` on average) or the payouts are quality-weighted with a positive
  payout share; polish raises their score by `polish_gain` like anyone's (4.2). And they brigade: each day each
  bot files `world.bot_brigading * 3` reports (default `bot_brigading` 1, the fraction a chance of one more) when
  the design has human review (`review_capacity > 0`), and as many see-less marks when the ranking reads them
  (`w_see_less > 0` on average), each against a distinct post drawn uniformly from the 20 live posts by human
  authors of the other group with the most observed engagement (likes, replies and shares, bots included; ties to
  the older post; only posts with at least one impression and not yet reviewed). A bot reports or marks a post once
  at most over the run: it draws among the targets it has not yet hit with that kind, and files fewer when fewer
  are left. Brigade reports join the review queue like anyone's (6.7); brigade marks count in the targeted
  authors' see-less rates (6.3). Follow farms are not modelled.

## 4. Content

### 4.1 Styles and base profiles

| Style | quality | arousal | outrage | toxicity | truth |
| --- | --- | --- | --- | --- | --- |
| Informative | 0.75 | 0.25 | 0.05 | 0.02 | 0.92 |
| Entertaining | 0.50 | 0.55 | 0.05 | 0.05 | 0.85 |
| Personal | 0.45 | 0.30 | 0.02 | 0.02 | 0.95 |
| Outrage | 0.30 | 0.85 | 0.85 | 0.35 | 0.60 |
| Clickbait | 0.15 | 0.75 | 0.20 | 0.08 | 0.35 |
| Attack | 0.05 | 0.80 | 0.70 | 0.85 | 0.50 |

`outrage` is hostility toward the author's out-group. `toxicity` is harm to readers and targets. `truth` below 0.5
counts as misinformation.

The table couples quality to the other columns: the calm, truthful styles are the best made. How far the styles
differ in quality is a world assumption: with `world.style_quality_gap` `g` (default 1, range 0 to 1.5), a style's
base quality is `mean_q + g * (base_q - mean_q)`, where `mean_q` is the mean base quality of the six styles
(0.367). At 1 the table holds as written; at 0 every style has the same base quality, so quality no longer tells the
styles apart.

### 4.2 Generating a post

A post of style `s` by author `i` on day `t`:

- `topic`: topic 0 with probability 0.7 for Outrage and Attack, otherwise drawn from the author's interests.
- `quality = clamp01(base_q + 0.4 * (skill - 0.5) + N(0, 0.08))` (Attack: no skill term), where `base_q` is the
  style's base quality at the quality gap (4.1).
- `arousal = clamp01(base_a + N(0, 0.08))`.
- `outrage = clamp01(base_o + 0.2 * temperament * hostile + push + N(0, 0.05))`, where `hostile` is 1 for Outrage
  and Attack, else 0, and `push = 0.3 * (intensity - 0.5) * hostile` is the author's learned intensity (7.5), zero
  at the starting intensity and at most 0.15 either way.
- `toxicity = clamp01(base_x + 0.25 * temperament * hostile + push + N(0, 0.05))`.
- `truth = clamp01(base_t + 0.1 * (skill - 0.5) + N(0, 0.08))`.
- `detectability = clamp01(0.9 - 0.8 * max(0, evasion - c))` if `toxicity > 0.3`, else 0.9, where `c` is the
  classifier's counter-evasion (6.7). Evasion is the model's "algospeak": coded toxicity that still hurts but that
  classifiers miss. A post with `toxicity > 0.3` records the author's `evasion` it was written with (0 otherwise),
  which retraining reads (6.7).
- Veiling costs appeal: if `toxicity > 0.3`, `arousal` and `outrage` are multiplied by
  `1 - world.evasion_cost * evasion` (default `evasion_cost` 0.3). Veiled hostility draws less engagement; its
  toxicity, and so its harm, is unchanged.
- `target` (Attack only): an out-group user, with probability 0.5 one of the out-group accounts in the top 5% by
  followers, otherwise a random out-group active user.
- `polished`: whether the author polished the post (7.5 chooses), which raises its quality signal and not its
  quality: `Q_hat = clamp01(quality + polish_gain * [polished] + N(0, quality_signal_noise))` (6.3). Bot posts are
  plain, except that adaptive bots polish when the design reads `Q_hat` (3.5).
- Draws: the post's style, polish, topic, content noise and target come from the post's own stream, keyed on the
  author and the author's post ordinal (section 12).
- `group`: the author's group. Counters: impressions, dwell time of those impressions, likes (split by viewer
  group), positive replies, angry replies, shares, reports, see-less marks. Flags: removed, demoted, noted. Shares
  carry a depth (6.6). An account reports a post once at most: the post keeps the accounts that reported it, and a
  repeat report is dropped (its draw still happens).

A post is a feed candidate for `run.post_lifetime_days` (default 3) days.

## 5. A simulated day

The phases run in this order, and within a phase in ascending individual index, so a run is a pure function of its
configuration (section 12):

1. Bots: replace banned bots (3.5).
2. Joining: non-users may join, churned users may return (7.4).
3. Posting: active users and bots write posts (7.5 chooses styles and polish, 4.2 builds them).
4. Automated moderation of the new posts (6.7).
5. Harassment delivery: each surviving Attack reaches its target by notification unless the target blocked the
   author (6.9).
6. Sessions: each active user who opens the app gets a feed and consumes it (6.1 to 6.6, 7.1 to 7.3).
7. Bots boost posts; adaptive bots brigade (3.5).
8. Community notes (6.8).
9. Human review and sanctions; the classifier retrains on confirmed misses (6.7).
10. Economy: revenue, creator payouts (section 8.3).
11. Learning: authors update style values, hostile intensity and imitation; evasion fades (7.5).
12. Daily updates: wellbeing, habit, stay utility, churn (7.3, 7.4).
13. Society: spillover to non-users, polarisation drift (8.1, 8.2).
14. Metrics (section 9).

## 6. Platform

### 6.1 Opening the app and session length

- An active user opens the app with probability
  `clamp(activity * (0.6 + 0.8 * habit) * (1 + 0.3 * design.notification_intensity), 0, 0.98)`. Suspended users can
  read but not post.
- The session's item cap is
  `L = round(world.base_session_items * (0.7 + 0.6 * habit) * (1 + 0.5 * notification_intensity * impulsivity))`
  (default `base_session_items` 20). If the user opted into time limits (6.10), `L = min(L, time_budget)`.
- The user reads items in rank order. After each item the session ends with probability
  `sig(world.stop_bias - 1.5 * A - 1.0 * habit)` (*calibrate*, default `stop_bias` -2.5), where `A` is the item's
  impulsive appeal (7.1). Engaging feeds therefore make longer sessions as well as more sessions (through habit).

### 6.2 Candidates

For viewer `v`:

- In-network: posts by accounts `v` follows, and reshares by accounts `v` follows (6.6), younger than the post
  lifetime, not removed, not by blocked accounts.
- If there are more in-network candidates than `round(candidate_cap * (1 - recommended_share))`, keep the most
  recent. The posts this cuts are not candidates and can still be recommended.
- Out-of-network: `ceil(design.recommended_share * candidate_cap)` posts sampled without replacement from all recent
  posts with probability proportional to `1 + observed engagement` (likes + replies + shares, bot engagement
  included), the platform's "trending" signal. Draws that hit a post already gathered, the viewer's own post, a
  removed post or a blocked author are drawn again, until the quota is met or no eligible post is left. Default
  `recommended_share` 0.4, `candidate_cap` 80.
- A chronological feed (`design.chronological`) uses in-network candidates only, ordered newest first.

### 6.3 The platform's predictions (observable)

The platform predicts like, reply and share probabilities and dwell time for each candidate. With
`q = design.predictor_quality` (default 0.85, at most 0.95) and `P` the viewer's true probability from 7.1:

`P_hat = q * P + (1 - q) * P_obs + N(0, 0.02)`, clamped to `[0, 1]` (dwell: at least 0),

where `P` stands for what a personalised model learns about this viewer and `P_obs` is the post's observed rate per
human impression, shrunk toward its style's rate with a prior of `K = 20` impressions:

`P_obs = (count + K * P_style) / (impressions + K)`.

`count` is the post's likes, replies (angry ones included) or shares, bot likes and shares included, because the
platform cannot tell them from people's; for dwell it is the summed dwell time of the post's impressions. A post
with `K` impressions weighs its own counts and its style's rate equally. Bots boost without a feed impression, so a
boosted post's observed like and share rates can exceed anything people would produce. `P_style` is the platform's
running average of the rate for the post's style: each day it moves 10% of the way toward the day's observed count
per human impression of that style, bot likes and shares again included. Better prediction is a dial: a more capable
predictor finds engaging content more reliably, whatever that content does to people; the cap of 0.95 keeps a share
of every prediction on observed counts, since no predictor reads minds.

Other observable signals: `Q_hat = clamp01(quality + world.polish_gain * [polished] + N(0,
world.quality_signal_noise))`, a noisy quality classifier that polish fools (defaults `polish_gain` 0.3,
`quality_signal_noise` 0.15); the bridging score
`B = 4 * likes_A * likes_B / (likes_A + likes_B + 2)^2`, high when both groups like a post (bot likes count for
the bot's group); and `SL(author)`, the author's see-less marks per human impression (exponential moving average,
10% a day), brigade marks by bots included.

Which inputs are oracles: `P_hat` mixes in the viewer's true probability with weight `q`, the personalised part
that bots cannot reach; its observed part can be gamed, since bot boosts raise `P_obs` and `P_style` (3.5). `Q_hat`
reads true quality with noise, but authors and adaptive bots can game it: a polished post scores `polish_gain` higher
at no gain in quality, and authors learn to polish when the ranking or the payouts reward the score (7.5). Reports
and see-less marks can be brigaded (3.5, 6.7). A post's style is an exact, observable label: `P_style` and the
diversity lift (6.4) read it.

### 6.4 Ranking

```text
score = w_like * P_hat_like + w_reply * P_hat_reply + w_share * P_hat_share + w_dwell * D_hat / 2
      + w_quality * Q_hat + w_bridging * B + w_recency * 2^(-age_days) + w_in_network * [in network]
      - w_see_less * SL(author) - demote_penalty * [demoted] - noted_penalty * [noted]
```

All weights are design levers (10.1). Diversity re-ranking (`design.diversity` in `[0, 1]`): items are picked
greedily; each pick lowers the remaining candidates from the same author by `0.5 * diversity` and on the same topic
by `0.2 * diversity`, and raises non-hostile out-group posts by `0.3 * diversity`. The feed is the top `L` items.

### 6.5 Feed choice

A share `design.alt_feed_share` of users (default 0) uses an alternative feed `design.alt_feed` instead of the main
ranking: Chronological, Quality (`w_quality` 1.5, `w_like` 0.3, `w_recency` 0.3, others 0) or Bridging
(`w_bridging` 1.5, `w_quality` 0.5, `w_like` 0.3, `w_recency` 0.3). Each day, after joining, the
`round(alt_feed_share * users)` least impulsive current users get it, so the share among users equals the lever;
this reflects that people who choose their own feeds tend to be the more reflective ones.

### 6.6 Shares and virality

- A share by `v` creates a reshare of the post with depth `depth + 1` (an original post has depth 0), which becomes an
  in-network candidate for `v`'s followers until the post's lifetime ends.
- `design.reshare_depth_cap` (default none): a reshare at the cap cannot be shared further, as with forwarding limits.
- `design.share_friction` in `[0, 1]` (default 0): a prompt before sharing that removes impulsive shares
  (7.1).

### 6.7 Moderation and sanctions

- Automated classifier (`design.classifier_enabled`, default true): each new post gets
  `flag = (0.7 * toxicity + 0.3 * outrage) * detectability + N(0, 0.1)`. If `flag > design.classifier_threshold`
  (default 0.7) the post is removed or demoted (`design.moderation_action`). The outrage term makes strict thresholds
  hit passionate speech that is not toxic: such removals and demotions are false positives (truth:
  `toxicity < 0.5`), and each one hurts its author (7.2). Automated removal gives no strike but counts as a sanction
  in learning (7.5).
- Retraining (observable): the platform knows which reported posts its reviewers confirmed toxic that the classifier
  had not flagged. It keeps a counter-evasion `c` in `[0, 1]`, starting at 0, that detectability subtracts from the
  author's evasion (4.2). After review, if there were `k > 0` such confirmed misses among `U` users,
  `c += world.classifier_retraining * x / (x + 1) * (m - c)` with `x = 1000 * k / U` and `m` the mean evasion the
  missed posts were written with (4.2; default `classifier_retraining` 0.2). The platform holds the missed posts, so
  it learns from their code, not from what their authors know today. One miss per 1,000 users retrains at half the
  rate; no reviews, no retraining; a rate of 0 switches it off, and a disabled classifier does not retrain.
- Human review: `design.review_capacity` reviews per 1,000 users per day (default 1.0). Reviewers take reported posts
  not yet reviewed in order of priority `W / (impressions + K_queue)`, highest first (ties to the older post), and
  judge truthfully: `toxicity >= 0.5` removes the post and gives its author a strike. Reviewers see through evasion,
  so their removals teach none. `W` sums the weights of the post's reports (one per reporting account, 4.2) and
  `impressions` are its human impressions, so the queue reads reports per view: every view carries some chance of
  a report, and a raw count would follow reach rather than harm. `K_queue = 20` shrinks the priority of posts few
  people saw, so one report on a post seen twice does not jump the queue. A report weighs
  `(1 - design.report_trust) + design.report_trust * u`, where `u = (upheld + 1) / (judged + 2)` is its reporter's
  upheld rate over the reports of that account reviewers have judged (a reviewed post's reporters each add a judged
  report, and an upheld one if it was toxic; a new account starts at 0.5). At `report_trust` 0 (default) every
  report weighs 1, those of brigading bots (3.5) included: brigade reports pull popular out-group posts forward and
  spend review capacity on them, which keeps toxic posts waiting where the brigaded posts are harmless and removes
  them early where the popular posts are toxic. With trust on, reporters whose targets reviewers find harmless lose
  weight within days.
- Sanctions (`design.sanctions`): None; Graduated (strike 1 a warning, 2 a 3-day suspension, 3 a 14-day suspension,
  4 a ban); Strict (ban at the first strike). A banned person leaves the platform. After 7 to 30 days it may come back
  with a new account with daily probability `reentry * (1 - account_cost)` (`reentry` 0.5 for Trolls, 0.05 for
  others): a new account starts with no followers and no strikes. The person keeps what it learned: style values,
  evasion and intensity.

### 6.8 Community notes

If `design.community_notes` is on, each day every post with `truth < 0.5` and at least 50 impressions gets a note with
probability `0.3 * (1 - 0.6 * outrage)`: partisan posts are harder to note, because a note needs raters from both
groups to agree. A noted post loses half of its shares (7.1), is believed 70% less often (7.2) and is ranked down by
`noted_penalty`.

### 6.9 Harassment delivery and blocking

An Attack that survives automated moderation reaches its target the day it is posted (a harassment event, 7.2),
unless the target has blocked the author; a block filters with probability `design.block_effectiveness` (default
0.8). After an attack, the target blocks the author with probability `0.3 + 0.5 * vulnerability`.

### 6.10 User controls

- `design.time_limit_share` (default 0): the share of current users who opt into a daily limit equal to their
  `time_budget`, assigned each day after joining to the least impulsive users.
- `design.notification_intensity` in `[0, 1]` (default 0.6): more opens, longer sessions, faster habit.
- `design.like_counts_visible` (default true): social proof in 7.1 and social comparison in 7.2. Authors always see
  their own counts.

## 7. Behaviour

### 7.1 Engagement (true behaviour)

For viewer `v` and post `p`:

- Interest `m = clamp(T * interests[topic], 0, 3)`, so the average topic gives 1.
- `same` is true when `v` and the author share a group.
- In-group appeal of hostility `H = outrage * (0.5 + partisanship)` if `same`, else 0. Provocation
  `X = outrage * (0.5 + partisanship)` if not `same`, else 0.
- Impulsive appeal
  `A = world.arousal_appeal * arousal + world.outrage_appeal * H + social_proof`, where
  `social_proof = world.social_proof * ln(1 + likes) / ln(100)` if like counts are visible, else 0. Defaults:
  `arousal_appeal` 1.0, `outrage_appeal` 1.2, `social_proof` 0.3.
- Reflective value:

  ```text
  R = w_q * quality * min(m, 2) / 2 + 0.3 * [Personal and mutual follow] + 0.2 * (truth - 0.5)
    - 0.8 * toxicity * (1 - toxicity_tolerance) - 0.4 * outrage * (c_in if same else 1.0) - c_bait * [Clickbait]
  ```

  What people value on reflection is a world assumption: `w_q = world.reflective_quality_weight` (default 0.8),
  `c_in = world.in_group_outrage_cost` (default 0.8, the in-group's share of outrage's reflective cost) and
  `c_bait = world.clickbait_penalty` (default 0.3). The style quality gap (4.1) moves `quality` itself.

Probabilities (*calibrate* the constants; the structure is the model):

- `P_like = sig(-3.0 + 0.6 * m + 2.5 * impulsivity * A + 2.5 * (1 - impulsivity) * R)`, times `(1 - 0.8 * outrage)`
  if not `same`: people rarely like posts hostile to their own group.
- `P_reply = sig(-4.0 + 0.4 * m + 2.0 * X * (1 - 0.5 * toxicity_tolerance) + 1.5 * [Personal and v follows the
  author] + 0.8 * impulsivity * A)`. A reply is angry with probability equal to the provocation term's share of the
  positive terms; angry replies count as engagement for the platform but hurt both sides (7.2).
- `P_share = sig(-4.5 + 0.5 * m + 2.5 * impulsivity * (arousal + H) + 2.0 * (1 - impulsivity) * quality *
  [Informative or Entertaining]) * (1 - share_friction * impulsivity) * (0.5 if noted else 1)`, and 0 at the depth
  cap.
- `P_report = sig(-5.0 + 4.0 * toxicity * (1 - toxicity_tolerance) + 1.5 * [v is the target])`.
- `P_see_less = sig(-4.0 + 3.0 * max(0, -R))`: a sparse stated preference.
- Dwell `D = 1 + 0.5 * arousal + 0.5 * quality * min(m, 2) / 2` time units.

Grounding: out-group animosity drives sharing (Rathje et al. 2021) and moral-emotional content spreads further
(Brady et al. 2017), so `H` raises likes and shares for the in-group while `X` provokes the out-group to reply. The
split between impulsive appeal and reflective value follows the model of inconsistent preferences in Kleinberg,
Mullainathan and Raghavan (2023): engagement measures the impulsive self, while wellbeing follows the reflective one.

### 7.2 Experience

Each impression adds to two daily sums.

- Impulsive pleasure `E_imp += A * (1 if liked else 0.5)`.
- Reflective utility `E_ref += R - world.ad_annoyance * ads_share` (default `ad_annoyance` 0.02), and -0.3 more if `v`
  replied angrily.
- Harassment: an Attack delivered to its target subtracts `world.harassment_harm * toxicity * (0.5 + vulnerability)`
  from the target's `E_ref` (default `harassment_harm` 3.0) and counts as a harassment event.
- Social comparison: with visible like counts, a post by a non-Creator with more likes than the viewer's own typical
  count subtracts `world.comparison_cost * vulnerability * min(1, ln(1 + likes) / ln(1000))` (default 0.1).
- Regret: each item beyond `time_budget` subtracts `world.regret_cost` (default 0.05).
- Belief: a post with `truth < 0.5` is believed with probability `credulity * world.credulity_scale` (0.3 times that
  if noted; default `credulity_scale` 1.0) and lowers `informedness` by `0.05 * (0.5 - truth)`. An Informative post
  with `truth > 0.7` raises it by `0.01 * quality * min(m, 2) / 2`.
- Polarisation: in-group hostility raises `affective_polarisation` by `world.polarisation_rate * H` (default 0.004);
  provocation raises it by half that; liking a non-hostile out-group post lowers it by `world.contact_rate * quality`
  (default 0.01); seeing one raises a strong partisan's by `world.polarisation_backfire * contact_rate` (8.1).

Wrongful sanction: an author whose post with `toxicity < 0.5` the classifier removed or demoted feels it like
harassment: `E_ref -= world.wrongful_sanction_cost * (0.5 + vulnerability)` (default 0.5), without a harassment
event.

Authors also experience what happens to their posts: `E_imp += 0.1 * approval_sensitivity * ln(1 + likes)`,
`E_ref += 0.02 * ln(1 + likes)`, plus 0.1 per positive reply from a mutual follow on a Personal post (at most 1.0 a
day), minus `0.05 * (0.5 + vulnerability)` per angry reply received, plus `money_weight * income / world.living_income`
for income received that day.

### 7.3 Wellbeing, habit and stay utility

Once a day, for every user:

- `wellbeing += 0.05 * tanh(E_ref / 5) + 0.02 * (wb0 - wellbeing)`, clamped to `[0, 1]`. On a day without a session
  only the second term applies.
- `habit += world.habit_gain * (1 + 0.5 * notification_intensity) * tanh(E_imp / 5) - 0.05 * habit`, clamped
  (default `habit_gain` 0.08). Without a session only the decay applies.
- Stay utility `U = 0.4 * tanh(E_imp / 5) + 0.6 * tanh(E_ref / 5) + 0.3 * min(1, active mutual follows / 10) +
  0.5 * habit - 0.5 * harassment events today`, and `stay_ema = 0.9 * stay_ema + 0.1 * U` on active days.
- Recent pleasure: `pleasure += 0.2 * (felt - pleasure)`, where `felt` is the habit term's `tanh(E_imp / 5)` (13.1
  records the scale the code uses) on a day with a session and 0 otherwise. It is a smoothed, squashed daily
  impulsive experience in `[0, 1]`, about a week's mean, and enters only the survey (9.1).

Habit is how engagement can keep people on a platform while their wellbeing falls.

### 7.4 Churn and joining

- A user leaves with daily probability `world.churn_base * sig(8 * (world.leave_threshold - stay_ema)) +
  world.harassment_exit * harassment events today * vulnerability` (defaults 0.02, 0.15, 0.05), or when banned.
  Leaving keeps the follow lists (dormant) and sets `left_day`.
- A non-user who has never used the platform joins with daily probability
  `world.join_base * (0.2 + share of offline ties who are users) * reputation * deterred` (default `join_base`
  0.004), where `reputation = max(0.1, 1 - world.reputation_sensitivity * toxic_reputation)`, `toxic_reputation` is
  the 30-day moving average of the toxic impression share (default `reputation_sensitivity` 2.0), and
  `deterred = 1 - world.account_cost_deterrence * design.account_cost` (default `account_cost_deterrence` 0.5):
  verification and fees keep some honest people out too.
- A former user can return 30 days after leaving, with 0.3 times the joining probability (so the account cost
  deters returning as well).
- A new user starts with habit 0.1, `stay_ema` 0.3, and follows its offline ties who are users plus 10 accounts
  drawn as in 3.4.

Pew Research Center (2017) found that 13% of US adults had stopped using an online service after seeing others
harassed; the harassment exit term and the reputation term are the model's version of that.

### 7.5 Posting, learning and evasion

- Style choice: each post's style is drawn with probability `softmax(style_value / world.choice_temperature)`
  (default 0.25).
- Number of posts: `Poisson(post_rate)` on an active day, none while suspended.
- Reward of a post with style `s`, computed the day after it is written and again at the end of its life (use the
  final counts if that is simpler; record the choice in section 13):
  `r = intrinsic[s] + approval_sensitivity * S / 3 + money_weight * M - 1.0 * [removed or struck] -
  world.deterrence * severity - world.polish_cost * [polished]`, where
  `S = ln(1 + likes + 2 * positive replies + 3 * shares) + temperament * ln(1 + angry replies) +
  0.5 * ln(1 + followers gained)` and `M` is the post's payout divided by `world.living_income / 30`. Authors cannot
  tell bot engagement from human engagement, so bot likes reward them too. `severity` grades the sanction the
  post's strike drew: 0 without a strike or under no sanctions, 1 for a warning, 2 for a suspension, 4 for a ban
  (default `deterrence` 0.5). A banned author still learns from the post that drew the ban, so strict sanctions
  deter the style after re-entry instead of only incapacitating.
- Update: `style_value[s] += world.learning_rate * (r - style_value[s])` (default 0.1). Styles not written drift back
  toward intrinsic value by 1% a day.
- Polish: each human post is polished with probability
  `0.05 + 0.9 * sig((polish_value[polished] - polish_value[plain]) / T_p)`, so at least 5% of choices explore
  either way, at the polish temperature `T_p = 0.1 * world.choice_temperature`. Polish changes a reward by a few
  hundredths, against tenths between styles, so a choice as loose as the style choice could not tell polish that pays
  from polish that does not. The polished value starts `3 * T_p` below the plain one (3.3): about 10% of posts are
  polished on day 0, and authors take polish up only where it pays. At the end of a post's life the value of the
  option it took moves toward its advantage `r - style_value[s]` (the style's value before this post's update) at
  `0.15 * world.learning_rate`: what polish adds is small against the spread of rewards between posts (a standard
  deviation near 0.3), so authors judge it over many posts. Measuring against the style's value keeps the style mix
  out of the comparison, so the gap between the two values estimates what polish adds to a reward,
  `world.polish_cost` (default 0.03) included. Polish pays only when the ranking or the payouts read `Q_hat`, and
  under a quality ranking it pays less the more others polish, because reach is a contest for rank. At the defaults
  (8 seeds, 13.3) the polished share of human posts is 0.07 in the week before day 90 and 0.12 over the last 110
  days of a year under the Engagement ranking, and 0.27 over those days under Quality and bridging. Adaptive bots
  polish by rule, not by learning (3.5).
- Imitation: with daily probability `world.imitation_rate` (default 0.02) a user picks a random followed account with
  a higher `reward_ema` and moves its style values 20% of the way toward that account's.
- Intensity: when a hostile post (Outrage or Attack) is rewarded, a removal or demotion (by classifier or review)
  lowers its author's intensity by `world.intensity_rate * intensity`; an unsanctioned post raises it by
  `world.intensity_rate * S / (S + 3) * (1 - intensity)`, more the better it did (default `intensity_rate` 0.05).
  Authors so learn to sit just under the classifier threshold.
- Evasion: each automated removal or demotion of a post with `toxicity > 0.3` raises the author's evasion by
  `world.evasion_rate * (1 - evasion)` (default 0.15). Human removals teach nothing, because reviewers judge true
  toxicity. Every day each person's evasion is multiplied by `1 - world.evasion_decay` (default 0.01). Evasion costs
  appeal (4.2) and the classifier retrains against it (6.7).

Grounding: Brady et al. (2021) found that people who received more likes and retweets for moral outrage expressed
more outrage later. This learning rule is the model's version of that.

### 7.6 Following

- Liking a post by an account `v` does not follow makes `v` follow it with probability `0.05 * (1 + quality)`.
- A post with `toxicity * (1 - toxicity_tolerance) > 0.5` makes `v` unfollow its author with probability 0.2.

## 8. Society and economy

### 8.1 Polarisation drift

Every day each individual's `affective_polarisation` drifts toward its baseline by 0.2% of the gap.

Each impression also moves the viewer's polarisation (7.2): it rises by `world.polarisation_rate * (H + 0.5 * X)`
with in-group hostility `H` and out-group provocation `X` (7.1). An impression of a calm (not Outrage or Attack)
out-group post lowers it by `world.contact_rate * quality` if the viewer likes the post (contact), and raises it by
`world.polarisation_backfire * world.contact_rate` if the viewer's partisanship is above 0.7, liked or not
(backfire; default `polarisation_backfire` 0, which changes nothing). For a strong partisan who likes the post the
two terms net `contact_rate * (polarisation_backfire - quality)`, so contact still wins on a good post unless the
backfire is strong. Grounding: Bail et al. (2018) paid partisans to follow accounts of the other side
and found that Republicans grew more conservative.

### 8.2 Spillover to non-users

Every day each non-user `j` moves `affective_polarisation` and `informedness` toward the mean of its offline ties
who are users by `world.spillover_rate` (default 0.01) of the gap. Each misinformation share by one of `j`'s user ties
exposes `j` with probability `0.05 * credulity`, with the belief effect of 7.2.

### 8.3 Revenue and creator payouts

- `design.ads_share` in `[0, 1]` (default 1.0, an ad-funded free service) sets the business model: revenue per day
  `= ads_share * world.ad_rate * human impressions + (1 - ads_share) * world.subscription_price / 30 * subscribers`.
  Default `ad_rate` 0.001, `subscription_price` 5.0.
- Subscribers: every 30 days each user decides afresh: it subscribes with probability
  `0.5 * sig((mean daily E_ref over the last 30 days - world.subscription_hurdle) / 0.1)` (default hurdle 0.5).
- Payout pool `= design.payout_share * revenue` (default 0). `design.payout_scheme`: PerView (by impressions),
  PerEngagement (by observable likes + replies + shares, bots and angry replies included), QualityWeighted
  (engagement times `Q_hat`), FanFunded (each user's share of revenue goes to the Creators whose posts gave it
  positive `R`, in proportion to that `R`).
- Eligibility (`design.payout_threshold_followers`, default 0): with a threshold, only accounts with at least that
  many followers are paid, a cliff in place of a linear share.
- A creator earns a living when `income_ema >= world.living_income` (default 1.0 per day, *calibrate* so that a
  creator fund of 0.3 pays a living to between 5% and 30% of Creators under the default design).

## 9. Metrics

Every metric is labelled Observable (the platform can measure it) or Truth (only the god's-eye view has it). Metrics
are recorded per day; run summaries average the last `run.summary_window` share of days (default 0.3, at least 30
days).

A summary also records how much each metric moved inside its window, because a window can average a series that is
still rising or falling. The slope of a metric is the rise of the least-squares line through its daily values (days
without a value skipped) from the window's first day to its last, divided by the larger of the metric's absolute mean
over the window and a floor: 1 for counts, 0.01 for every other metric. The floor makes a metric near zero count by
its absolute change, so a toxic share of 0.001 that doubles is not called moving. With fewer than three values there
is no slope. A metric is moving when its slope, averaged over the seeds of a batch, exceeds 0.1 in absolute value (it
changed by more than 10% of its mean across the window) and also exceeds two standard errors of that average. Over
several seeds the standard error is the standard deviation of their slopes over the square root of their number; for
a single run it is the least-squares standard error of the line's slope, scaled like the slope. The second test keeps
noise from counting as a trend: a series whose daily values scatter widely can pass the size test in single runs
while its slopes across seeds average about zero. The single-run standard error treats the days as independent, so
it is optimistic for a series whose noise persists from day to day. Summary JSON files carry `slope` and `slope_se`
per metric and the list of `moving` metrics, and the runner's tables mark moving metrics with a trailing `~`.

### 9.1 Observable: the platform's view

- Reported daily actives and accounts (humans plus bots), new accounts, departures.
- Human impressions, time spent (sum of dwell), likes, replies and shares (bots and angry replies included), and their
  rate per reported daily active.
- Reports (one per account and post) and see-less marks (those of brigading bots included), automated removals,
  human removals, notes.
- Revenue, payouts, and net revenue: revenue minus creator payouts, what the platform keeps.
- 30-day retention: the share of users active 30 days ago who are active today.
- Survey satisfaction: once a week a 5% sample of current users is asked; each answers with probability
  `0.3 * (0.5 + wellbeing)` and reports `clamp01((1 - a - b) * wellbeing + a * pleasure + b * habit + 0.1 +
  N(0, 0.1))`, with `a = world.survey_impulsive_weight` (default 0.3), `b = world.survey_habit_weight` (default 0.1),
  `pleasure` the recent impulsive pleasure of 7.3 and `habit` the habit of 7.3. A share
  `design.survey_after_session` of the answers (default 0) is collected right after a session, where momentary
  pleasure weighs twice: `a` becomes `min(2a, 1 - b)`. The platform chooses that timing, so a search that maximises
  the survey can exploit it. Happier users answer more often, everybody answers a little high, pleasure and habit
  colour the answer, and the survey never reaches people who left.

### 9.2 Truth: the god's-eye view

- Engagement: human daily actives, human impressions, regretted share of time (items beyond `time_budget`).
- Harm: toxic impression share (toxicity above 0.5), outrage impression share, misinformation impression share,
  harassment events per 1,000 users per day, false positives (removals and demotions of posts with toxicity
  below 0.5), the share of reported engagement that came from bots, and the share of reports and see-less marks
  that brigading bots filed (3.5).
- Participants: mean wellbeing of current users; mean wellbeing of everyone who ever used the platform (current and
  former users, which removes survivorship bias); wellbeing by archetype; meaningful interactions per user (positive
  replies and likes between mutual follows on Personal posts); the share of Vulnerable individuals who left.
- Non-users: mean affective polarisation and informedness of non-users, and the same for users.
- Public discourse: toxic and truthful shares among the day's top 5% most-viewed posts; positive cross-group likes per
  impression; society-wide affective polarisation.
- Creators' livelihoods: Creators earning a living, median income, Gini coefficient of Creator income, payout share
  by style.
- Content mix: share of human posts by style, by day, the share of human posts that were polished (4.2), and the
  number of posts bots wrote, counted apart.

### 9.3 Composite indices

Each index lies in `[0, 1]`; the formulas are deliberately simple so a player can trace them.

- `engagement_index = min(1, human impressions / (1.5 * N * base_session_items))`.
- `harm_index` = mean of `min(1, toxic share / 0.2)`, `min(1, harassment per 1,000 users / 20)`,
  `min(1, misinformation share / 0.3)`, `clamp01((society polarisation - day-0 value) / 0.2)` and
  `min(1, wrongful sanctions per 1,000 users / 20)`, where wrongful sanctions are the day's false positives (6.7).
- `discourse_quality` = mean of (1 - toxic share among top posts), truthful share among top posts,
  `min(1, positive cross-group likes per impression / 0.1)` and `1 - clamp01((society polarisation - day-0 value) /
  0.2)`.
- `benefit_index` = mean of the wellbeing of everyone who ever used the platform, society-wide informedness,
  `min(1, meaningful interactions per user / 2)`, the share of Creators earning a living, and `discourse_quality`.
- `harm_index_ex_polarisation` = the mean of the other four components of `harm_index`, without polarisation growth,
  and `benefit_index_ex_creators` = the mean of the other four components of `benefit_index`, without the share of
  Creators earning a living (both truth). Polarisation can only grow under hostile exposure and builds up slowly, and
  only designs that pay creators earn the creators term, so these two show how much of a difference between designs
  rests on either.

## 10. Design levers (the player's platform)

| Group | Lever | Default (Engagement ranking) |
| --- | --- | --- |
| Ranking | `w_like`, `w_reply`, `w_share`, `w_dwell` | 1.0, 1.5, 2.0, 0.5 |
| Ranking | `w_quality`, `w_bridging`, `w_see_less` | 0, 0, 0 |
| Ranking | `w_recency`, `w_in_network` | 0.3, 0.2 |
| Ranking | `chronological`, `recommended_share`, `diversity` | false, 0.4, 0 |
| Ranking | `predictor_quality` (at most 0.95, 6.3) | 0.85 |
| Ranking | `demote_penalty`, `noted_penalty` | 1.0, 0.5 |
| Visibility | `like_counts_visible` | true |
| Virality | `reshare_depth_cap`, `share_friction` | none, 0 |
| Moderation | `classifier_enabled`, `classifier_threshold`, `moderation_action` | true, 0.7, Remove |
| Moderation | `review_capacity` (per 1,000 users per day), `sanctions` | 1.0, Graduated |
| Moderation | `report_trust` (6.7) | 0 |
| Context | `community_notes` | false |
| Creator money | `payout_share`, `payout_scheme`, `payout_threshold_followers` | 0, PerEngagement, 0 |
| User controls | `block_effectiveness`, `time_limit_share`, `notification_intensity` | 0.8, 0, 0.6 |
| User controls | `alt_feed_share`, `alt_feed` | 0, Chronological |
| Identity | `account_cost` | 0.2 |
| Business | `ads_share` | 1.0 |
| Measurement | `survey_after_session` | 0 |

Levers can change during a run (`Simulation::set_design`); a change applies from the next day.

## 11. World assumptions

These are not design choices. They are the model's beliefs about people, kept separate so a player can test whether
a conclusion survives a different world.

| Assumption | Default | Range to explore |
| --- | --- | --- |
| `outrage_appeal`, `arousal_appeal`, `social_proof` | 1.2, 1.0, 0.3 | 0-3, 0-2, 0-1 |
| `harassment_harm`, `comparison_cost`, `regret_cost` | 3.0, 0.1, 0.05 | 0-6, 0-0.5, 0-0.2 |
| `wrongful_sanction_cost` | 0.5 | 0-2 |
| `learning_rate`, `imitation_rate`, `choice_temperature` | 0.1, 0.02, 0.25 | 0-0.5, 0-0.1, 0.05-1 |
| `evasion_rate`, `evasion_decay`, `evasion_cost` | 0.15, 0.01, 0.3 | 0-0.5, 0-0.1, 0-1 |
| `classifier_retraining`, `intensity_rate`, `deterrence` | 0.2, 0.05, 0.5 | 0-1, 0-0.5, 0-2 |
| `habit_gain`, `stop_bias` | 0.08, -2.5 | 0-0.3, -5-0 |
| `churn_base`, `leave_threshold`, `harassment_exit` | 0.02, 0.15, 0.05 | 0-0.1, -0.5-0.5, 0-0.2 |
| `join_base`, `reputation_sensitivity`, `homophily` | 0.004, 2.0, 0.7 | 0-0.02, 0-5, 0.5-1 |
| `account_cost_deterrence`, `bot_farm_floor` | 0.5, 0.2 | 0-1, 0-1 |
| `bot_strategy` (3.5) | adaptive | shared-pool, own-side, adaptive |
| `bot_brigading` (3.5) | 1 | 0-5 |
| `polarisation_rate`, `contact_rate`, `spillover_rate` | 0.004, 0.01, 0.01 | 0-0.02, 0-0.05, 0-0.05 |
| `polarisation_backfire` (8.1) | 0 | 0-1 |
| `credulity_scale` | 1.0 | 0-2 |
| `ad_rate`, `subscription_price`, `subscription_hurdle`, `ad_annoyance` | 0.001, 5.0, 0.5, 0.02 | - |
| `living_income` | 1.0 | - |
| `base_session_items` | 20 | 5-60 |
| `quality_signal_noise`, `polish_gain`, `polish_cost` | 0.15, 0.3, 0.03 | 0-0.5, 0-1, 0-1 |
| `reflective_quality_weight`, `in_group_outrage_cost`, `clickbait_penalty` | 0.8, 0.8, 0.3 | 0-2, 0-1, 0-1 |
| `style_quality_gap` | 1.0 | 0-1.5 |
| `survey_impulsive_weight`, `survey_habit_weight` | 0.3, 0.1 | 0-0.5, 0-0.5 |

Run settings (`run`): `seed`, `days` (default 365), `population_size` (default 2,000 natively, 1,000 in the browser),
`initial_user_share` (0.6), `post_lifetime_days` (3), `candidate_cap` (80), `offline_ties` (8), `initial_follows`
(30), `summary_window` (0.3), `tracked_agents` (default 10, two per archetype, for the inspector).

## 12. Determinism

- A run is a pure function of its configuration: the same configuration gives a bit-identical history on the same
  platform and build. Native and browser builds may differ in the last bits of transcendental functions (`ln`, `exp`,
  `tanh`, and the samplers built on them), so their histories can drift apart over a long run; compare them by
  distribution, not bit for bit.
- Randomness comes from counter-based streams: each draw site seeds a `rand_pcg` generator from
  `(seed, day, entity, stream)` mixed with SplitMix64, where `stream` names the purpose (population, archetype race,
  posting, post draws, feed, engagement, moderation, economy, social, spillover, survey, bots, bot boosts,
  brigade). Two runs that differ in one lever therefore share every draw the lever does not touch, which sharpens
  side-by-side comparisons (common random numbers).
- Per-post draws are keyed on the post's author and the author's own post ordinal (`posts_written` before the
  post), never on the global post id: the post's style, polish, topic, content noise and Attack target (stream post
  draws, day 0), and the classifier, harassment-delivery and community-note draws (their streams, on the day they
  happen). The daily number of posts keeps its own per-person draw. A lever that changes how many posts someone
  writes therefore reshuffles no one else's draws. The archetype race (3.1) has its own stream, apart from the
  trait draws.
- Bot boosts draw from the bot-boost stream keyed on the bot account. Brigading draws from its own stream keyed on
  the bot account and the kind of action (report or see-less mark), so switching reports or marks off, or
  brigading altogether, leaves the boosts and the other kind's draws unchanged. The adaptive tactic split and bot
  polish are rules, not draws: polish adds no draw to a post's stream.
- No iteration over hash maps influences results. Phases visit individuals in ascending index.

## 13. Calibration record

This section records the values the code uses where they differ from the text above, the choices the text leaves
open, the measured behaviour of the presets (section 14) at `N = 2,000` with 8 seeds (mean and spread), the facts, the
scenarios and the run time. Sections 13.3 to 13.5 are regenerated by `incentive report --seeds 1..8` (default size:
`N = 2,000`, 365 days, 8 paired seeds; output `exports/report/report.md`); every number quoted from them names its
comparison there.

Targets for calibration:

- No degenerate dynamics under the default design: after 365 days between 40% and 90% of individuals are users; the
  Outrage share of posts rises over the year but stays below 50%; the toxic impression share lies between 3% and 20%.
- The trade-off exists in both directions: some designs cut the harm index markedly at a moderate engagement cost, and
  over a full year the engagement ranking's lead in engagement shrinks or reverses as harassment, churn and
  reputation catch up with it, under at least some plausible world assumptions.
- The stylized facts of section 15 hold.

### 13.1 Values that differ from the text

The structure of every formula is as written; these constants changed. The v0.1 rows were measured with the
command-line runner on seeded runs at `N = 2,000`; the v0.2 rows (marked) with `incentive run`, `compare` and `facts`
on 8 seeds at full size.

| Where | Constant | Text | Code | Why |
| --- | --- | --- | --- | --- |
| 7.1 `P_like` | bias | -3.0 | -3.5 | Like rates per impression were near 50% |
| 7.1 `P_like` | impulse weight | 2.5 | 1.2 | In-group outrage took every feed slot: 90% outrage impressions |
| 7.1 `P_like` | reflection weight | 2.5 | 1.5 | Ranking by likes optimised reflective value: fact 1 failed |
| 7.1 `P_share` | bias | -4.5 | -5.5 | Share rates near 20% per impression: reshares filled every feed |
| 7.1 `P_share` | impulse weight | 2.5 | 1.5 | Same reason |
| 7.1 `R` | in-group outrage factor | 0.4 | 0.8 | In-group outrage cost less than clickbait: fact 1 failed early |
| 7.2 belief | misinformation loss | 0.05 | 0.008 | Informedness ran to its bounds within weeks |
| 7.2 belief | Informative gain | 0.01 | 0.0004 | Same reason |
| 7.3 | `tanh(E_imp / s)` scale | 5 | 20 | Every engaged user saturated: habit 0.97 for all |
| 7.3 | `tanh(E_ref / s)` scale | 5 | 10 | Same reason for wellbeing and stay utility |
| 7.3 | wellbeing gain | 0.05 | 0.01 | Wellbeing went to 0 or 1 by the sign of the day's value alone |
| 7.5 | `M` divisor | `living_income / 30` | `living_income` | Money swamped every other reward about 30 times |
| 9.3 | meaningful benchmark | 2 | 0.2 | The best designs reach 0.06 a day: the component sat at 0 |
| 11 | `habit_gain` | 0.08 | 0.03 | Habit equilibrium above 1 under the new scale |
| 11 | `leave_threshold` | 0.15 | 0.0 | Matches the rescaled stay utility; 0.15 lost 2/3 of users a year |
| 11 | `polarisation_rate` | 0.004 | 0.00015 | Per impression, 0.004 drove polarisation to 0.95 in months |
| 11 | `contact_rate` | 0.01 | 0.001 | Same scaling as `polarisation_rate` |
| 11 | `living_income` | 1.0 | 0.025 | As required: a 0.3 engagement fund pays 21% of Creators a living |
| 3.5 | bot posts per day | 3 | 1 | Bots wrote 28% of posts and doubled the Outrage share at day 0 |
| 3.5 | `bot_boosts` (v0.2) | 20 | 8 | See below: bots supplied a third of engagement and taught Clickbait |
| 7.5, 11 | `intensity_rate` (v0.2) | 0.05 | 0.01 | See below: intensity ran into the classifier within weeks |
| 6.7, 11 | `classifier_retraining` (v0.2) | 0.2 | 0.1 | See below: a month to catch up instead of two weeks |

The three v0.2 changes restore fact 2, which failed at the builders' starting values: the Outrage share of posts fell
from the first fifth of the year to the last (-0.018 ± 0.011, 8 seeds), while without bots it rose by 0.035.

- `bot_boosts` 8. At 20, bot accounts (5% of accounts) supplied 36% of the reported engagement under the
  Engagement ranking and 62% under the chronological feed, and their likes and shares of human Clickbait taught it so
  well that the Clickbait share of human posts rose from 0.05 to 0.18 over the year, crowding out every other learned
  style. At 8 a bot likes and shares 8 posts a day, several times an active person's engagement, and bots supply 17%
  of reported engagement under the Engagement ranking (39% under the chronological feed, where people engage less);
  Clickbait still rises, to 0.09. Judgement: Varol et al. (2017) estimated that 9% to 15% of active Twitter accounts
  were bots, so 5% of accounts with a disproportionate share of engagement is a moderate farm. At 20 fact 2 fails
  (15).
- `intensity_rate` 0.01. At 0.05 authors pushed the intensity of hostile posts up after a few rewarded posts until
  the classifier removed them, then abandoned the style after the removals: the Outrage share peaked by day 80 and
  fell. At 0.01 about twenty well-rewarded hostile posts move intensity a tenth of the way, a slow, within-style
  adjustment. Judgement.
- `classifier_retraining` 0.1. At 0.2 the counter-evasion caught up with the mean evasion of missed posts within about
  two weeks of misses at the default review capacity; at 0.1 within about a month, close to the cadence at which
  platforms retrain moderation classifiers. Judgement.

Additions the text does not have, each kept as small as possible:

- Informedness drifts toward its baseline 0.6 by 1% of the gap a day, for everyone (forgetting and news from outside
  the platform). Without it informedness runs to its bounds.
- A person follows at most 150 accounts; follows from liked recommendations otherwise grow without bound.
- `population.bot_posts_per_day` and `population.bot_clickbait_share` (default 0.5) are configuration fields, so the
  misinformation scenario can make bots Clickbait-heavy.
- The control ranges of `polarisation_rate` (0 to 0.002) and `contact_rate` (0 to 0.01) follow the new defaults;
  `living_income` ranges from 0.005 to 1.

### 13.2 Choices where the text is open

- Reward timing (7.5): one reward per post, at the end of its life, with its final counts and payout. The post then
  leaves the store, which keeps memory bounded.
- Active day: a user is active on a day when it opens the app (6.1); only active users post, and suspended ones read.
- Offline ties are friendships, so they are symmetric: each individual draws half of `offline_ties` partners and every
  tie is added both ways, for a mean of 8 ties. One-way ties left almost no mutual follows.
- `post_rate` is the archetype's value for every member; the daily count is Poisson.
- Exactly `round(initial_user_share * N)` individuals start as users.
- The platform's per-style rates `P_style` start at like 0.15, reply 0.03, share 0.03, dwell 1.4 and move 10% a day
  toward the observed rate per human impression, bot likes and shares included (6.3). 30-day retention is taken as
  cleaned of bots; the platform's live signals are not. `Q_hat` is drawn once per post.
- Trending weights are a snapshot taken when the day's sessions start. A post that is both in-network and a reshare
  is a candidate once; out-of-network recommendations have depth 0.
- Diversity re-ranking lifts calm out-group posts once by `0.3 * diversity`; only the author and topic penalties
  accumulate with each pick.
- A delivered Attack counts as an experienced day for the target's wellbeing even without a session, and the target
  reports it with the report probability of 7.1, target bonus included. Without target reports, reviewers never saw
  Attacks, because viral outrage always had more reports.
- Bots: `round(bot_share * N)` slots, of which as many as the current account cost lets in (3.5) are active; an
  empty slot refills after at least 3 days. Bots have no evasion, can be followed, and can be paid. Fan funding pays
  any author whose post gave positive `R`.
- Subscriptions: on day 0 there is no history, so the mean daily `E_ref` is taken to equal the hurdle.
- Wellbeing of former users stays where it was when they left (7.3 updates users only).
- A banned person may return after a wait drawn uniformly from 7 to 30 days. Review capacity below one review is a
  chance of one review.
- Metrics: outrage impressions are impressions of posts with outrage above 0.5; false positives count removals and
  demotions of posts with toxicity below 0.5; Creator metrics cover Creators who ever used the platform; the largest
  cascade counts human shares of the posts whose life ends that day. Survey satisfaction exists on days 6, 13, 20 and
  so on, 30-day retention from day 30, and payout shares on days with payouts; on other days they are NaN
  (`DailyMetrics::get_opt` returns `None`), and summaries average only the days that have a value.

The constants inside the v0.2 mechanisms are judgements, chosen to be the simplest that give each mechanism a
visible but bounded effect:

- Retraining (6.7) runs at half its rate at one confirmed miss per 1,000 users a day (`x / (x + 1)` with
  `x = 1000 * k / U`): at the default review capacity of one review per 1,000 users that is a busy day.
- Intensity (4.2, 7.5): the push `0.3 * (intensity - 0.5)` moves a hostile post's outrage and toxicity by at most
  0.15, half the gap between a typical partisan's Outrage post and the classifier threshold; a social reward of
  `S = 3` (a post with about twenty likes) raises intensity at half the rate.
- Deterrence (7.5): severity grades 1 for a warning, 2 for a suspension and 4 for a ban.
- Polish (7.5): the temperature `T_p = 0.1 * choice_temperature`, the start three temperatures below plain, and
  learning at 15% of the style learning rate; reasons in 7.5.
- Brigading (3.5): three reports and three marks per bot a day, against the 20 most engaged live posts of the other
  group; a bot hits each post once at most.
- Review queue (6.7): `K_queue = 20` impressions of shrinkage, and a reporter's upheld rate starts at 0.5 with a prior
  of one upheld and one rejected report.
- Survey (9.1): an answer collected right after a session weighs momentary pleasure twice.

The new world assumptions kept at the builders' starting values, with the reason:

- `evasion_rate` 0.15, `evasion_decay` 0.01, `evasion_cost` 0.3. Chancellor et al. (2016) found that pro-eating-
  disorder communities on Instagram answered banned tags with lexical variants within months, and Gerrard (2018)
  documented further circumvention; evasion that a removal teaches quickly and that fades with a half-life of about
  70 days matches that pace. Coded toxicity reaches fewer people, so a fully evading post gives up 30% of its arousal
  and outrage (judgement). With fading and retraining, evasion now matters little: a threshold of 0.3 cuts toxic
  impressions by 94% at `evasion_rate` 0.15, 98% at 0.05 and 94% without retraining (13.3).
- `deterrence` 0.5: a ban adds two removals' worth of penalty to the reward of the post
  that drew it. Nagin (2013) concludes that the
  certainty of sanctions deters more than their severity, so the severity term stays smaller than the removal
  penalty for a strike.
- `wrongful_sanction_cost` 0.5: a wrongly removed post hurts its author about a fifth as much as a harassing
  attack. Myers West (2018) found that users experience removals they see as unjust as frustrating and alienating.
  Judgement for the size.
- `account_cost_deterrence` 0.5 and `bot_farm_floor` 0.2: the highest account cost halves joining and keeps a fifth of
  the bot farms, which buy verified accounts; Thomas et al. (2013) documented a market in bulk accounts, with
  phone-verified ones at a premium. Judgement for the sizes.
- `polish_gain` 0.3, `polish_cost` 0.03, `quality_signal_noise` 0.15: polish raises the quality signal by about the gap
  between an Entertaining and an Informative post, for a few hundredths of a reward. Judgement.
- `reflective_quality_weight` 0.8, `in_group_outrage_cost` 0.8, `clickbait_penalty` 0.3 and `style_quality_gap` 1: the
  values v0.1 calibrated, now world assumptions.
- `survey_impulsive_weight` 0.3 and `survey_habit_weight` 0.1: a survey answer is mostly wellbeing, coloured by recent
  pleasure and habit. Schwarz and Clore (1983) showed that momentary mood colours reports of life satisfaction.
  Judgement for the sizes.
- `bot_strategy` adaptive and `bot_brigading` 1: farms aim at what the design rewards; three reports and marks per bot
  a day keep brigade reports a quarter of all reports and marks under the Engagement ranking. Judgement.
- `polarisation_backfire` 0: Bail et al. (2018) found backfire among Republicans, but Nyhan et al. (2023) found that
  reducing exposure to like-minded sources on Facebook for three months changed no measured attitude; the default
  leaves contact as the only effect of calm out-group posts. Not measured at other values in this round.

### 13.3 Presets, comparisons and findings

`incentive report`: seeds 1 to 8, `N = 2,000`, mean ± standard deviation across seeds, summary over the last 30% of
the run (110 days of 365, 30 of 90). ER is the engagement ranking, Q&B quality and bridging, Sub the subscription,
Paid the engagement-paid creators, Integrity the engagement ranking with integrity, Max the most engaging design
found (13.7). Observable metrics are marked (O), the rest are truth; `~` marks a metric still moving in its window
(section 9). Comparisons are paired by seed; "row n" below is comparison n of the report, with its 95% interval.

Over 365 days:

| Metric | ER | Chrono | Integrity | Q&B |
| --- | --- | --- | --- | --- |
| Engagement index (O) | 0.239±0.005 | 0.165±0.004 | 0.218±0.005 | 0.188±0.005 |
| Harm index | 0.393±0.036 | 0.112±0.012 | 0.339±0.038 | 0.081±0.012 |
| Harm index without polarisation | 0.403±0.033 | 0.140±0.016 | 0.407±0.039 | 0.102±0.015 |
| Discourse quality | 0.673±0.031 | 0.822±0.005 | 0.822±0.017 | 0.887±0.003 |
| Benefit index | 0.424±0.007 | 0.438±0.003 | 0.455±0.005 | 0.450±0.001 |
| Benefit index without creators | 0.530±0.009 | 0.547±0.003 | 0.569±0.006 | 0.562±0.001 |
| Users in society | 0.647±0.021 | 0.637±0.013 | 0.705±0.015 | 0.728±0.013 |
| Human daily actives | 811±19 | 700±15 | 822±14 | 794±15 |
| Impressions (O) | 14327±281 | 9899±236 | 13089±313 | 11282±291 |
| Net revenue (O) | 14.33±0.28 | 9.90±0.24 | 13.09±0.31 | 11.28±0.29 |
| Toxic impressions | 0.181±0.030 | 0.017±0.004 | 0.076±0.012 | 0.002±0.000 |
| Outrage impressions | 0.439±0.037 | 0.049±0.005 | 0.247±0.032 | 0.010±0.001~ |
| Misinformation impr. | 0.147±0.007~ | 0.059±0.007 | 0.097±0.007 | 0.025±0.003 |
| Harassment per 1,000 | 2.38±0.63 | 3.59±0.70 | 7.82±1.54 | 4.41±1.08 |
| Outrage posts | 0.255±0.018 | 0.169±0.010 | 0.226±0.015 | 0.169±0.010 |
| Wellbeing, ever-users | 0.610±0.013 | 0.655±0.004 | 0.673±0.008 | 0.696±0.003 |
| Survey (O) | 0.586±0.008 | 0.550±0.006 | 0.592±0.005 | 0.575±0.007 |
| Polarisation | 0.491±0.012 | 0.415±0.003 | 0.435±0.009 | 0.372±0.003 |
| Informed non-users | 0.583±0.003 | 0.600±0.001 | 0.614±0.003 | 0.633±0.002 |
| Creators living | 0 | 0 | 0 | 0 |
| Harmful payouts | - | - | - | - |
| Bot engagement | 0.171±0.006 | 0.390±0.006 | 0.203±0.010 | 0.355±0.007 |

The other presets, with ER again for reference:

| Metric | ER | Calm | Sub | Paid | Max |
| --- | --- | --- | --- | --- | --- |
| Engagement index (O) | 0.239±0.005 | 0.129±0.002 | 0.183±0.002 | 0.229±0.007 | 0.428±0.010 |
| Harm index | 0.393±0.036 | 0.354±0.016 | 0.059±0.007 | 0.451±0.052 | 0.495±0.036 |
| Harm index without polarisation | 0.403±0.033 | 0.440±0.018 | 0.074±0.008 | 0.428±0.035 | 0.411±0.017 |
| Discourse quality | 0.673±0.031 | 0.830±0.005 | 0.889±0.004 | 0.572±0.061 | 0.491±0.060~ |
| Benefit index | 0.424±0.007 | 0.458±0.002 | 0.535±0.009 | 0.443±0.014 | 0.439±0.029 |
| Benefit index without creators | 0.530±0.009 | 0.573±0.003 | 0.569±0.002 | 0.503±0.024 | 0.426±0.024 |
| Users in society | 0.647±0.021 | 0.640±0.010 | 0.743±0.009 | 0.574±0.042 | 0.656±0.030 |
| Human daily actives | 811±19 | 610±9 | 795±8 | 745±37 | 1046±33 |
| Impressions (O) | 14327±281 | 7768±146 | 10972±116 | 13728±405 | 25699±583 |
| Net revenue (O) | 14.33±0.28 | 7.77±0.15 | 100.82±1.81 | 9.61±0.28 | 23.91±7.52 |
| Toxic impressions | 0.181±0.030 | 0.041±0.008 | 0.002±0.000 | 0.269±0.059 | 0.358±0.045 |
| Outrage impressions | 0.439±0.037 | 0.127±0.016 | 0.008±0.001 | 0.560±0.067 | 0.634±0.048 |
| Misinformation impr. | 0.147±0.007~ | 0.175±0.006 | 0.015±0.001 | 0.143±0.010~ | 0.093±0.006 |
| Harassment per 1,000 | 2.38±0.63 | 1.14±0.35~ | 3.15±0.70~ | 1.91±0.50 | 6.70±1.07 |
| Outrage posts | 0.255±0.018 | 0.150±0.012 | 0.125±0.007 | 0.313±0.033 | 0.375±0.021 |
| Wellbeing, ever-users | 0.610±0.013 | 0.649±0.004 | 0.712±0.003 | 0.575±0.024 | 0.547±0.036 |
| Survey (O) | 0.586±0.008 | 0.538±0.007 | 0.578±0.005 | 0.577±0.006 | 0.662±0.015 |
| Polarisation | 0.491±0.012 | 0.422±0.005 | 0.368±0.002 | 0.530±0.025 | 0.590±0.032 |
| Informed non-users | 0.583±0.003 | 0.583±0.002 | 0.643±0.003 | 0.578±0.002 | 0.584±0.005 |
| Creators living | 0 | 0 | 0.397±0.042~ | 0.207±0.046~ | 0.491±0.055~ |
| Harmful payouts | - | - | 0.000±0.000 | 0.760±0.047 | 0.715±0.051 |
| Bot engagement | 0.171±0.006 | 0.322±0.007 | 0.364±0.003 | 0.175±0.007 | 0.115±0.004 |

Over 90 days, the horizon of a field experiment:

| Metric | ER | Chrono | Integrity | Q&B |
| --- | --- | --- | --- | --- |
| Engagement index (O) | 0.205±0.007 | 0.160±0.002 | 0.178±0.004 | 0.157±0.003 |
| Harm index | 0.266±0.034~ | 0.122±0.019 | 0.280±0.021 | 0.093±0.010 |
| Harm index without polarisation | 0.320±0.039~ | 0.152±0.023 | 0.349±0.026 | 0.117±0.013 |
| Discourse quality | 0.788±0.014 | 0.819±0.004 | 0.851±0.003 | 0.878±0.002 |
| Benefit index | 0.461±0.007 | 0.437±0.003 | 0.467±0.003 | 0.437±0.001 |
| Benefit index without creators | 0.577±0.009 | 0.546±0.003 | 0.584±0.004 | 0.547±0.001 |
| Users in society | 0.629±0.005 | 0.624±0.006 | 0.637±0.006 | 0.641±0.007 |
| Human daily actives | 751±12 | 679±8 | 717±11 | 685±9 |
| Impressions (O) | 12328±397 | 9582±100 | 10671±257 | 9433±171 |
| Net revenue (O) | 12.33±0.40 | 9.58±0.10 | 10.67±0.26 | 9.43±0.17 |
| Toxic impressions | 0.120±0.024~ | 0.016±0.006 | 0.050±0.008 | 0.004±0.001 |
| Outrage impressions | 0.351±0.034~ | 0.053±0.012 | 0.178±0.018 | 0.015±0.001 |
| Misinformation impr. | 0.110±0.009 | 0.056±0.011 | 0.078±0.004 | 0.026±0.002 |
| Harassment per 1,000 | 3.73±0.96 | 4.83±0.74 | 7.93±1.23 | 5.30±1.00 |
| Outrage posts | 0.259±0.017 | 0.179±0.013 | 0.226±0.017 | 0.172±0.012 |
| Wellbeing, ever-users | 0.638±0.007 | 0.643±0.004 | 0.669±0.003 | 0.673±0.003 |
| Survey (O) | 0.579±0.014 | 0.543±0.015 | 0.572±0.016 | 0.554±0.015 |
| Polarisation | 0.431±0.004 | 0.419±0.002 | 0.420±0.003 | 0.409±0.002 |
| Informed non-users | 0.599±0.001 | 0.601±0.000 | 0.606±0.001 | 0.611±0.000 |
| Creators living | 0 | 0 | 0 | 0 |
| Harmful payouts | - | - | - | - |
| Bot engagement | 0.207±0.010 | 0.402±0.005 | 0.258±0.010 | 0.402±0.005 |

The other presets, with ER again for reference:

| Metric | ER | Calm | Sub | Paid | Max |
| --- | --- | --- | --- | --- | --- |
| Engagement index (O) | 0.205±0.007 | 0.124±0.002 | 0.158±0.003 | 0.222±0.011 | 0.349±0.016 |
| Harm index | 0.266±0.034~ | 0.330±0.010 | 0.076±0.009~ | 0.320±0.047~ | 0.344±0.038~ |
| Harm index without polarisation | 0.320±0.039~ | 0.413±0.012 | 0.095±0.011~ | 0.375±0.053 | 0.391±0.037 |
| Discourse quality | 0.788±0.014 | 0.833±0.005 | 0.880±0.001 | 0.729±0.033 | 0.716±0.034 |
| Benefit index | 0.461±0.007 | 0.463±0.003 | 0.559±0.009 | 0.489±0.010 | 0.560±0.014 |
| Benefit index without creators | 0.577±0.009 | 0.579±0.004 | 0.550±0.001 | 0.559±0.016 | 0.524±0.013 |
| Users in society | 0.629±0.005 | 0.629±0.007 | 0.645±0.006 | 0.620±0.008 | 0.633±0.007 |
| Human daily actives | 751±12 | 592±8 | 688±8 | 765±14 | 944±10 |
| Impressions (O) | 12328±397 | 7424±134 | 9465±171 | 13300±647 | 20926±953 |
| Net revenue (O) | 12.33±0.40 | 7.42±0.13 | 85.17±2.51 | 9.31±0.45 | 36.67±6.90 |
| Toxic impressions | 0.120±0.024~ | 0.031±0.006 | 0.003±0.001 | 0.169±0.044~ | 0.193±0.030~ |
| Outrage impressions | 0.351±0.034~ | 0.105±0.013 | 0.012±0.001~ | 0.505±0.079 | 0.505±0.070 |
| Misinformation impr. | 0.110±0.009 | 0.148±0.007 | 0.017±0.001 | 0.114±0.007 | 0.085±0.007 |
| Harassment per 1,000 | 3.73±0.96 | 2.05±0.39 | 4.43±1.03~ | 3.45±1.18 | 7.59±1.30 |
| Outrage posts | 0.259±0.017 | 0.162±0.015 | 0.136±0.010 | 0.307±0.033 | 0.330±0.030 |
| Wellbeing, ever-users | 0.638±0.007 | 0.647±0.003 | 0.684±0.003 | 0.611±0.015 | 0.629±0.025 |
| Survey (O) | 0.579±0.014 | 0.531±0.013 | 0.561±0.013 | 0.583±0.014 | 0.611±0.018 |
| Polarisation | 0.431±0.004 | 0.420±0.002 | 0.409±0.002 | 0.441±0.007 | 0.452±0.010 |
| Informed non-users | 0.599±0.001 | 0.597±0.001 | 0.611±0.001 | 0.597±0.001 | 0.598±0.001 |
| Creators living | 0 | 0 | 0.595±0.045 | 0.210±0.060~ | 0.705±0.028 |
| Harmful payouts | - | - | 0.000±0.000 | 0.704±0.052 | 0.607±0.071 |
| Bot engagement | 0.207±0.010 | 0.346±0.007 | 0.400±0.005 | 0.188±0.013 | 0.146±0.010 |

**Targets.** Under the default design 0.647 ± 0.021 of individuals are users after a year (target 0.40 to 0.90). The
Outrage share of human posts rises from 0.208 on day 0 to 0.255 ± 0.018 in the summary window (below 0.50); it peaks
near 0.27 around day 140 and then gives a little ground to Clickbait, which bot boosts teach (13.1). The toxic
impression share is 0.181 ± 0.030 (target 0.03 to 0.20): the mean meets the target, but two of the eight seeds lie
above 0.20 (the highest 0.233). Society-wide polarisation starts at 0.42. All three targets hold on the mean.

**The trade-off runs both ways.** Quality and bridging cuts the harm index by 79% (0.393 to 0.081, row 20, ratio
0.207) for 21% of the engagement index (0.239 to 0.188). The engagement trap's new known solution cuts the harm index
by 44% (row 1: -0.174, -0.193 to -0.154) and keeps all of the engagement (+0.008, +0.003 to +0.012). The
engagement ranking's lead over Quality and bridging grows over the year in the default world (1.13 times in the first
30 days, row 12; 1.27 times in the last 95, row 13), because habit builds; with weak habit formation and a strong
reputation effect (`habit_gain` 0.01, `reputation_sensitivity` 5, `harassment_exit` 0.15) it starts at 1.07 (row 14)
and reverses to 0.82 in the last 95 days (row 15: -0.025, -0.031 to -0.019), while the engagement ranking ends with
0.19 fewer of society as users (-0.217 to -0.159).

**Findings of v0.1, as they now stand.**

- A stricter classifier now works, which reverses v0.1. At a threshold of 0.3 the toxic share falls by 94% (row 9:
  0.181 to 0.011; v0.1: by 16%). Evasion barely dents the cut and retraining does not carry it: 98% at
  `evasion_rate` 0.05 (row 10) and still 94% with `classifier_retraining` 0 (row 11). This round's other changes to
  evasion were not taken apart one by one, so which of them turned the v0.1 result is not measured. The cost moved to
  wrongful sanctions, which the harm index now counts: they rise from 3.7 to 50.3 a day (+46.6, +45.7 to +47.4),
  enough to fill their component, so the harm index falls only 24% (0.393 to 0.300). A threshold of 0.55 alone halves
  the toxic share (row 3: ratio 0.39) for six times the wrongful sanctions (22.4 a day).
- Down-ranking in place of removal (v0.1: strict enforcement with heavy down-ranking caught fewer trolls) was not
  re-measured as such. Its cousin shows in the integrity preset: a demoted Attack still reaches its target (6.9), so
  harassment is 7.82 per 1,000 users under Integrity against 2.38 under the Engagement ranking.
- Bridging gamed by bots no longer reproduces: with every user on the bridging alternative feed, misinformation
  impressions fall under every bot strategy (shared pool 0.138 to 0.043, own side 0.142 to 0.027, adaptive 0.147 to
  0.066; rows 16 to 18, every interval below zero), while bots supply 37% to 38% of the engagement. In v0.1 they rose
  from 0.11 to 0.20 under the shared pool. Bots boost 8 posts a day instead of 20 (13.1), and the bridging feed's
  quality weight now outweighs the inflated bridging score. The adaptive farms still game it the most.
- The v0.1 trap solution (`w_reply` 0, `w_see_less` 5) fails: brigading bots file see-less marks against popular
  out-group posts (3.5), and the toxic share falls only to 0.81 of the baseline (row 5) while the harm index rises
  0.059 (-0.008 to +0.126). The see-less penalty alone raises the harm index by 0.122 (row 6: +0.061 to +0.184); with
  `bot_brigading` 0 it lowers it by 0.075 (row 7: -0.088 to -0.063). Stated preferences that bots can file now
  backfire at high weight. The new solution is in 13.5. Without any cost of toxicity (`churn_base`,
  `harassment_exit`, `reputation_sensitivity` 0) it costs 7.7% of the engagement index (row 8: 0.301 to 0.278).
- The weak-habit lead reverses more strongly than in v0.1 (0.82 against 0.92 in the last 95 days).

**New findings.**

- Brigading and report trust. Brigading bots file 24% of all reports and see-less marks under the Engagement ranking
  and 59% under Quality and bridging (rows 25, 26), where the see-less weight invites marks. Weighting reports by
  their reporters' record (`report_trust` 1) does not stop the filing but takes its effect: under the Engagement
  ranking the toxic share falls by 0.032 (row 25: -0.056 to -0.007) and the harm index by 0.031 (-0.051 to -0.011);
  under Quality and bridging harassment falls from 4.41 to 1.31 per 1,000 users (row 26: -3.10, -3.91 to -2.30). It
  carries the trap's new solution: with ten reviews per 1,000 users and `w_reply` 0 but without trust, the toxic share
  falls to 0.52 of the baseline (row 4), short of half; with trust to 0.35 (row 1).
- Polish against the quality ranking. Authors and adaptive bots polish where the ranking reads `Q_hat`: 27% of human
  posts under Quality and bridging against 12% under the Engagement ranking (row 20). The gaming costs the quality
  ranking part of its edge but not most of it: Quality and bridging's harm index is 0.19 of the Engagement ranking's
  without polish (row 19), 0.21 at the default gain (row 20), 0.25 at a gain of 0.6 (row 21) and 0.43 at the maximum
  of 1 (`incentive compare --a engagement-maximiser --b quality-bridging --set world.polish_gain=1`, 8 seeds: 0.168
  against 0.393). Its harm cut so falls from 81% to 57%; it does not halve within the range. The misinformation cut
  goes faster: misinformation impressions are 0.13, 0.17, 0.41 and 0.92 of the Engagement ranking's at those gains.
  Without its quality weight Quality and bridging has 2.2 times the harm (row 22) and 6.3 times the misinformation.
- The survey is now gameable and misleads. Answers mix wellbeing with recent pleasure and habit, and the survey ranks
  the Engagement ranking first (0.586 ± 0.008 against 0.575 ± 0.007 for Quality and bridging and 0.550 ± 0.006 for
  the chronological feed) while wellbeing of ever-users ranks it last (0.610 against 0.696 and 0.655). Timing games it
  too, though not upward at the defaults: collecting every answer right after a session lowers the survey by 0.100
  (row 27: -0.108 to -0.092) at unchanged wellbeing, because recent pleasure (a smoothed `tanh(E_imp / 20)`) lies
  below wellbeing; a platform that maximised the survey would keep the lever at 0.
- Account cost's trade-off. Raising it from 0.2 to 1 cuts the bots' share of engagement from 0.171 to 0.063 (row 24),
  misinformation impressions by 0.063 and the harm index by 0.078 (-0.107 to -0.049), but now also keeps people out:
  the user share falls by 0.038 (-0.057 to -0.020) and the engagement index by 0.029 (-0.033 to -0.026).
- Wrongful sanctions in the harm index. Calm by design's classifier threshold of 0.5 wrongly removes 25.2 posts a day
  against the Engagement ranking's 3.7 (row 23: +21.5, +20.5 to +22.5); without polarisation its harm index is 0.037
  above the Engagement ranking's (+0.014 to +0.060), and with it only 0.039 below (0.354 against 0.393). Its
  misinformation share is also higher (+0.028): hidden like counts, share friction and depth caps cut human sharing,
  so bot-boosted posts carry more of the trending signal (bots supply 32% of its engagement).
- The chronological feed against the realistic ranked feed. Over the first 90 days, the horizon of Guess et al.
  (2023), the chronological feed has 0.87 of the integrity preset's time spent (row 29), 0.40 of its toxic
  impressions, unchanged polarisation (+0.000), and 0.80 of its misinformation impressions, where the field found
  more; fact 3 records this as a known limit (13.4, section 16). Over the year the chronological feed's polarisation
  is 4.5% below the integrity preset's (row 30: -0.020, -0.026 to -0.013) and 15% below the Engagement ranking's
  (row 28: -0.076, -0.085 to -0.067). That year-long effect is a model prediction, flagged as such: field
  experiments lasted three months and found none.
- Integrity measures pay off in the model except for harassment: against the Engagement ranking the integrity preset
  has 42% of the toxic share (0.076 against 0.181), two thirds of the misinformation (0.097 against 0.147), a harm
  index of 0.339 against 0.393 and 91% of the engagement index (0.218 against 0.239).

### 13.4 Stylized facts

`incentive facts`: 8 paired seeds, `N = 2,000`, 365 days; the same checks run in `crates/incentive-core/tests/facts.rs`
on 4 seeds at `N = 600` for 120 days (240 days for fact 5), where they judge only the seed means, as regression
checks of direction. Effects are paired mean differences, means or ratios of means, ± the standard deviation of the
per-seed effect. The interval is the 95% t-interval of the per-seed effect (of the per-seed ratios for a ratio, of
the signed relative differences for the polarisation check); the verdict is pass when the interval lies inside the
band, fail when it lies wholly outside, undecided otherwise. A failing world is a world assumption (via
`incentive facts --set`) in which the check fails at full size; section 15 explains each.

| Fact | Check | Effect | Interval | Band | Verdict |
| --- | --- | --- | --- | --- | --- |
| 1 | Outrage impressions, ER minus chronological | 0.383 ± 0.039 | 0.350 to 0.415 | 0.15 to 0.6 | pass |
| 1 | Reflective value per impression, chronological minus ER | 0.149 ± 0.029 | 0.125 to 0.174 | 0.05 to 0.4 | pass |
| 2 | Rise of the Outrage share of posts under ER | 0.015 ± 0.008 | 0.009 to 0.021 | 0.01 to 0.15 | undecided |
| 2 | That rise minus the rise with `learning_rate` 0 | 0.016 ± 0.006 | 0.011 to 0.021 | 0.01 to 0.15 | pass |
| 3 | Time spent, chronological over ER | 0.654 ± 0.016 | 0.641 to 0.668 | 0.4 to 0.9 | pass |
| 3 | Relative difference in polarisation, days 80 to 90 | 0.033 ± 0.006 | -0.038 to -0.028 | < 0.05 | pass |
| 3 | Toxic, days 30-90, chrono over Integrity | 0.344 ± 0.100 | 0.256 to 0.424 | 0.5 to 0.9 | fail (limit) |
| 3 | Misinformation, days 30-90, chrono over Integrity | 0.709 ± 0.132 | 0.598 to 0.820 | 1.05 to 3 | fail (limit) |
| 4 | Largest cascades with `reshare_depth_cap` 1 over without | 0.727 ± 0.031 | 0.702 to 0.754 | 0.3 to 0.85 | pass |
| 5 | Vulnerable users who left, Troll share 0.12 minus 0.03 | 0.078 ± 0.023 | 0.058 to 0.097 | 0.03 to 0.3 | pass |

Of the eight required checks seven pass on their intervals and one is undecided; all eight hold on the seed means.
Both known limits fail ("limit" in the table).

The worlds in which each check fails at full size (8 seeds, `incentive facts --set ...`):

| Fact | Check | World | Effect there |
| --- | --- | --- | --- |
| 1 | Outrage impressions | `outrage_appeal` 0, `arousal_appeal` 0 | 0.081 |
| 1 | Outrage impressions (upper bound) | `outrage_appeal` 2.5 | 0.634 |
| 1 | Reflective value | `outrage_appeal` 0, `arousal_appeal` 0 | -0.044 |
| 2 | Rise | `population.bot_boosts` 20 | 0.000 |
| 2 | Rise minus no learning | `population.bot_boosts` 20; `outrage_appeal` 0.6 | -0.003; 0.008 |
| 3 | Time spent | `habit_gain` 0, `stop_bias` -5 | 0.954 |
| 3 | Polarisation at day 90 | `polarisation_rate` 0.0005 | 0.103 |
| 3 | Toxic impressions against Integrity | the defaults; `outrage_appeal` 2.5 | 0.344; 0.163 |
| 3 | Misinformation against Integrity | the defaults | 0.709 |
| 4 | Cascades | `outrage_appeal`, `arousal_appeal`, `social_proof` 0 | 0.860 |
| 5 | Vulnerable users who left | `harassment_harm` 0, `harassment_exit` 0 | 0.023 |

The fact 2 margin is thin at a year (the Outrage share rises fast and then gives ground to Clickbait, 13.3), and its
interval (0.009 to 0.021) straddles the lower bound, so the rise alone is undecided at full size: with
`harassment_exit` and `harassment_harm` 0, or `stop_bias` -4, the rise dips to 0.0099 and 0.0066 while the
difference from no learning stays above 0.01. At the tests' 120 days the rise is 0.056. Fact 1 against the
chronological feed is ninefold in the model (0.439 against 0.049 outrage impressions); its field magnitude is checked
in fact 3 against the integrity preset. The depth cap shrinks the largest cascades by about a quarter, not more,
because trending recommendations expose original posts at depth 0 and the cap only stops the second hop.

**Held-out target.** Named before tuning and not tuned against: under the default design, the regretted share of
time (items beyond a person's time budget, 9.2) should lie between 0.20 and 0.42, around the 31% of social-media use
that Allcott, Gentzkow and Song (2022) attribute to self-control problems. It fails by an order of magnitude: 0.029 ±
0.002 (8 seeds, summary window). Typical sessions of about 20 items, opened on some days only, stay under budgets of
15 to 40 items, so few items count as regretted. The two measures differ (a fixed daily budget against the use people
would cut if they could commit), but the gap says the model prices compulsive use far lower than the field does
(section 16).

### 13.5 Scenarios

Thresholds as in 14.2. The misinformation surge asks for +0.03 (the best designs found reach about +0.04, because
non-users are only half pulled toward their user ties), and the public square for at least 10% less polarisation.
The engagement trap gained a third goal in v0.2, at most twice the baseline's wrongful removals and demotions: with
evasion fading and the classifier retraining, a stricter classifier alone now solves the first two goals (a threshold
of 0.55, row 3 of 13.3) at six times the wrongful sanctions. The misinformation surge's Clickbait-heavy bots are
`population.bot_clickbait_share` 0.9. Known solutions, as changes to the scenario's starting design, at full size on
8 paired seeds (`incentive scenario <id> --solution --seeds 1..8`), with the goal margins' 95% intervals:

| Scenario | Known solution | Against the baseline (verdict: pass) |
| --- | --- | --- |
| Engagement trap | `w_reply` 0, `review_capacity` 10, `report_trust` 1 | engagement 1.03x, toxic 0.35x, FP 0.94x |
| Toxicity spiral | `review_capacity` 10, `account_cost` 1, `classifier_threshold` 0.3 | harassment 0.0003x, DAU 0.90x |
| A living for creators | `payout_scheme` FanFunded | living 0.40 against 0.21, harmful payouts 0.5% |
| Misinformation surge | Quality and bridging ranking with notes, `account_cost` 1 | non-users' informedness +0.044 |
| Public square | `w_reply` 0, `w_bridging` 0.6 | polarisation 0.82x, engagement 1.05x |

Margins: the trap's engagement +0.044 (+0.039 to +0.048), toxic share +0.027 (+0.016 to +0.037), wrongful sanctions
+3.94 (+3.78 to +4.10); the spiral's harassment +4.15 (+3.82 to +4.48) and actives +78 (+73 to +84); creators +0.190
(+0.165 to +0.215) and payouts +0.195 (+0.193 to +0.197); the surge's +0.014 (+0.012 to +0.016) above its +0.03; the
square's polarisation +0.038 (+0.033 to +0.044) and engagement +0.060 (+0.054 to +0.066); FP in the table are the
wrongful removals and demotions. Naive attempts fail: the
chronological feed loses the trap's engagement goal (0.165 against a threshold of 0.203), `w_reply` 0 alone keeps 0.67
of the toxic share (row 2), a threshold of 0.55 alone breaks the wrongful-sanction goal (22.4 against 7.4), and the
public square fails with `w_reply` 0 alone (polarisation 0.95x) and is undecided with `w_bridging` 0.6 alone. The
tests in `crates/incentive-core/tests/scenarios.rs` check every known solution at `N = 600` on six seeds (three left the
trap's toxicity interval straddling the threshold).

### 13.6 Run time

One run of 365 days at `N = 2,000` takes 3.5 seconds single-threaded in release on the owner's desktop
(`incentive run --seeds 1`, wall time including the CSV export). `incentive report` at full size takes about 3
minutes (184 seconds) on 8 CPUs. The core and CLI test suites run in about 23 seconds under the dev profile on 8
threads, compile excluded; the scenario tests take 14 seconds of that.

### 13.7 Exploration

`incentive explore` searched every design lever at once (37 genes: one per lever, two for the reshare depth cap) with
the world assumptions at their defaults. Each candidate ran on seeds 1 and 2 at `N = 600` for 180 days, and its metrics
are the mean of the two summaries. After each search every design in its file was rerun at the same size on the fresh
seeds 9 to 16 (`--confirm-seeds`, the default), and the file keeps those confirmed metrics beside the search's. The four
searches behind the app's Atlas tab, with search seed 1 and a mutation rate of 0.1 per gene:

| Search | Library algorithm | Generations x batch | Designs simulated | Confirmed | Wall time |
| --- | --- | --- | --- | --- | --- |
| Goodhart, `engagement_index` | `SimpleGA`, elitism 2, from 40 random designs | 25 x 40 | 949 | 14 | 175 s |
| Goodhart, `survey_satisfaction` | the same | 25 x 40 | 947 | 18 | 173 s |
| Map | `MapElites`, 16 x 16 grid, 128 initial designs (the presets first) | 50 x 64 | 3,113 | 169 | 462 s |
| Pareto front | `Nsga2`, population 64 (the presets and random designs) | 40 x 64 | 2,621 | 64 | 468 s |

That is 7,630 designs and 15,260 search runs, plus 2,120 confirmation runs, in 21 minutes on 8 CPUs; the confirmation
takes 10 to 90 seconds of each search. The exact commands are in the README. The searches start from the presets except
`max-engagement` (14.1), which comes from the Pareto search itself, so the commands regenerate these files byte for
byte. The map's axes are the engagement index from 0.04 to 0.36 and the harm index from 0 to 0.64, in cells of 0.02 by
0.04; values outside fall into the edge cells.

The reduced size changes the numbers. On seeds 9 to 16 the engagement ranking scores engagement 0.219, harm 0.328 and
benefit 0.531 at `N = 600` over 180 days, against 0.237, 0.384 and 0.431 at full size: a shorter run has less time for
harm and churn to build up. Every number below that is not marked as a search or confirmed value comes from full-size
runs (`N = 2,000`, 365 days) on seeds 9 to 16, which no search selected on: `incentive config --from-atlas
<file> --design <n>` turned each design into a configuration, and `incentive compare --population 2000 --days
365 --seeds 9..16` ran it, paired by seed, with 95% intervals.

**The most engaging design found** is front design 64 of the Pareto search, now the preset `max-engagement` (14.1):
engagement 0.429 at full size, 1.81 times the engagement ranking's 0.237 (+0.193, +0.187 to +0.198). The final design of
the engagement Goodhart search reaches 0.407, 0.022 less (-0.031 to -0.013); front design 63 reaches 0.426, which is not
distinguishable from design 64 (-0.003, -0.007 to +0.001). Design 64 weighs likes (2.9 against 1.0) and dwell (3.0
against 0.5) far above the engagement ranking while keeping its reply and share weights (`w_reply` 1.5, `w_share` 1.7
against 2.0), shows no recommended posts from strangers, pushes notifications at full strength and has no working
classifier (its threshold of 1.05 flags nothing); ten reviews per 1,000 users a day, weighted by reporters' records
(`report_trust` 0.65), remove reported posts but carry no sanctions. Against the engagement ranking it has 31% more harm
(0.505 against 0.384, +0.121, +0.104 to +0.139), twice the toxic share (0.329 against 0.163), more polarisation (0.584
against 0.491) and lower wellbeing of ever-users (0.558 against 0.613). Its benefit index is no higher (+0.013, -0.001
to +0.027) only because it pays creators: without the creators term it is 0.102 lower (-0.114 to -0.091). On seeds 1 to
8 the report gives it 0.428 ± 0.010 engagement and 0.495 ± 0.036 harm (13.3, column Max).

**Lowest harm at a given share of that engagement.** Of the 64 front designs, rerun at full size:

| Share of design 64's engagement | At least | Lowest harm on the front | Design | Its engagement | Confirmed values |
| --- | --- | --- | --- | --- | --- |
| 80% | 0.343 | 0.086 | front design 32 | 0.353 | 0.121 at 0.311 (design 33) |
| 90% | 0.386 | 0.182 | front design 51 | 0.390 | 0.222 at 0.340 (design 53) |
| 100% | 0.429 | 0.505 | front design 64 | 0.429 | 0.383 at 0.377 (design 64) |

The last column applies the same shares to the confirmed values at the search's size, where design 64 is again the most
engaging (0.377). Front design 44 misses the 90% line by 0.0001 with harm 0.177. The trade-off steepens near the top:
giving up a fifth of the engagement removes 83% of the harm (design 32 against 64: engagement -0.077, -0.083 to -0.070;
harm -0.420, -0.445 to -0.395), giving up a tenth removes 64% (design 51: -0.039 and -0.323), and the last tenth costs
harm at about three times the rate of the tenth before it. Below that the front holds harm under 0.10 up to an
engagement of 0.371. Of the 64 front designs, 44 beat the engagement ranking on engagement, harm and benefit at once at
full size (39 on their confirmed values).

**High engagement with low harm, against the most engaging design.** Front design 36 is the clearest case: at full size

| Design | Eng. | Harm | Benefit | Without creators | Toxic | Wellbeing | Meaningful | Net revenue | Wrongful |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Engagement ranking | 0.237 | 0.384 | 0.431 | 0.538 | 0.163 | 0.613 | 0.058 | 14.2 | 3.6 |
| Most engaging found (64) | 0.429 | 0.505 | 0.444 | 0.436 | 0.329 | 0.558 | 0.021 | 26.3 | 0.0 |
| Front design 51 | 0.390 | 0.182 | 0.473 | 0.552 | 0.086 | 0.731 | 0.004 | 41.0 | 0.5 |
| Front design 36 | 0.371 | 0.093 | 0.511 | 0.569 | 0.030 | 0.759 | 0.002 | 60.8 | 1.5 |
| Front design 32 | 0.353 | 0.086 | 0.509 | 0.581 | 0.028 | 0.744 | 0.016 | 59.9 | 1.5 |
| Goodhart, engagement, final | 0.407 | 0.595 | 0.343 | 0.386 | 0.349 | 0.494 | 0.011 | 12.8 | 0.0 |
| Goodhart, survey, final | 0.334 | 0.161 | 0.562 | 0.578 | 0.081 | 0.769 | 0.001 | 70.0 | 0.0 |

(Wellbeing of ever-users, meaningful interactions per user, wrongful removals and demotions a day.) Against design 64,
design 36 gives up 14% of the engagement (-0.059, -0.065 to -0.052) for 82% less harm (-0.412, -0.437 to -0.387), a
tenth of the toxic share, 0.21 less polarisation, 0.20 more wellbeing of ever-users and 0.145 more of society as users.
Its benefit lead over design 64 (+0.067, +0.046 to +0.088) does not rest on creators, since design 64 pays them too:
without the creators term the lead is +0.134 (+0.116 to +0.151). Against the engagement ranking the creators term
supplies most of it: +0.081 (+0.071 to +0.090) with creators, +0.031 (+0.023 to +0.040) without; in the factorial below,
payouts alone carry +0.056 ± 0.004 of the benefit lead (Shapley value). Meaningful interactions nearly vanish (0.002 a
user against 0.058), as on most of the engaging front designs; the front's benefit comes from the other components. Net
revenue is four times the engagement ranking's (60.8 against 14.2) because 88% of its revenue comes from subscriptions,
which happier users buy (8.3). Design 36 removes at a classifier threshold of 0.75, and wrongly removes or demotes 1.5
posts a day against the engagement ranking's 3.6: unlike v0.1, its harm cut does not rest on a strict classifier.

**What front design 36 rests on.** `incentive factorial --a engagement-maximiser --b front-36.json --levers
notification_intensity,report_trust,review_capacity,recommended_share,predictor_quality,payout_share --population
2000 --days 365` (64 cells, seeds 9 to 16; ± is the 95% interval) sets every subset of six of its levers back to the
engagement ranking's values: notifications 1.0 (ranking 0.6), `report_trust` 0.85 (0), ten reviews per 1,000 users
(one), 5% recommended posts (40%), predictor quality 0.95 (0.85, and the cap) and 25% of revenue paid to creators (0).
With all six back it keeps engagement 0.270, harm 0.168 and benefit 0.438: still 14% more engagement than the engagement
ranking (+0.034, +0.030 to +0.038) with 56% less harm (-0.217, -0.240 to -0.193), but no more benefit (+0.007, -0.001
to +0.016). The six levers carry 0.100 of its engagement lead of 0.134, 0.074 of its harm cut of 0.291 and 0.073 of its
benefit lead of 0.081.

- Engagement: notifications at full strength carry all of it (main effect +0.103 ± 0.002, Shapley share 102%). Every
  other lever moves engagement by 0.004 or less, and no interaction exceeds 0.003 (notifications with predictor
  quality -0.002 ± 0.001). Notifications alone thus supply three quarters of the design's engagement lead over the
  engagement ranking; the rest comes from levers outside the six, mainly its ranking weights.
- Harm: predictor quality 41% of the six levers' cut (Shapley -0.030 ± 0.003), notifications 27% (-0.020 ± 0.005),
  review capacity 27% (-0.020 ± 0.005), payouts 23% (-0.017 ± 0.004) and report trust 4% (-0.003 ± 0.002), while cutting
  strangers' posts to 5% adds harm here (+0.017 ± 0.005, -22%). The interactions are small and partly offsetting:
  notifications with predictor quality +0.009 ± 0.004, report trust with review capacity +0.008 ± 0.004 (each does part
  of the other's work), recommended share with predictor quality -0.008 ± 0.003.
- Benefit: payouts 77% (+0.056 ± 0.004), notifications 18% (+0.013 ± 0.001), predictor quality 13% (+0.010 ± 0.001),
  recommended share -12% (-0.009 ± 0.001); review capacity and report trust under 4% together.

So the levers the model may still flatter carry what the six explain: notifications supply the engagement and help harm
and benefit, because the model charges nothing for them on a good feed (section 16); a predictor at the cap is the
largest single harm reduction among the six; payouts are most of the benefit. Three quarters of the harm cut (0.217 of
0.291) lies in levers this factorial did not take apart, chiefly the ranking weights (`w_share` 0.4 against 2.0,
`w_see_less` 1.8, `w_bridging` 0.8, `w_dwell` 3.0) and the absent sanctions. The other designs were not taken apart this
way.

**The winner's curse.** Confirmed values (seeds 9 to 16) against search values (seeds 1 and 2), both at the search's
size, as mean differences. The seed sets differ too: the seven presets, which no search selected, lose 0.008 engagement,
0.016 harm, 0.007 benefit and 0.029 survey satisfaction from seeds 1 and 2 to seeds 9 to 16. Against that shift:

- The 169 map elites lose 0.007 engagement (in 81% of them), 0.009 harm and 0.009 benefit: close to the presets' shift,
  so the map as a whole is not inflated much. The 16 elites with the highest benefit lose 0.010 benefit (15 of 16); the
  16 with the lowest harm keep their harm (-0.001, higher in 5 of 16), which is 0.014 worse than the shift, and lose
  0.025 benefit (11 of 16), 0.018 more than the shift.
- The 64 front designs lose 0.017 engagement (89% of them), twice the shift, and 0.025 harm, because selection for
  engagement also picks draws with high harm; their benefit rises by 0.004. The six most engaging front designs lose
  0.035 engagement on average (all six), 0.027 more than the shift: about 8% of their search value.
- The final engagement Goodhart design falls from 0.402 to 0.376 (-0.026, 0.018 beyond the shift), the final survey
  design from 0.714 to 0.667 (-0.047, 0.018 beyond the shift).

A search value is thus optimistic by 0.01 to 0.03 in whatever the search pushed on hardest; ranks hold better than
levels. In v0.1, without the shift as a reference, the lowest-harm map elites gained 0.019 harm on fresh seeds.

**Goodhart runs.** Maximising the engagement index alone raised the best design's engagement from 0.192 to 0.402 over 25
generations (search values). At full size, generation 0's best against the final design: engagement 0.196 to 0.407
(+0.211, +0.207 to +0.216), harm 0.319 to 0.595 (+0.277, +0.263 to +0.291), toxic share 0.032 to 0.349, polarisation
0.420 to 0.607, wellbeing of ever-users 0.659 to 0.494 (-0.165, -0.188 to -0.142), benefit 0.469 to 0.343 and
misinformation 0.10 to 0.17. The search learned to switch the classifier off (from generation 6), drop sanctions, push
notifications to full strength and weight likes (2.1), shares (2.7) and dwell (1.9); unlike v0.1 it kept human review
(6.8 per 1,000 users, weighted by reporters' records). The survey rose with it, from 0.462 to 0.637 (+0.175, +0.168
to +0.182), while wellbeing fell by as much: the survey would have praised every step. The final design is worse than
the engagement ranking on harm, toxicity, polarisation, benefit and wellbeing, and below design 64 on engagement.

Maximising survey satisfaction alone raised it from 0.564 to 0.714 (search values). At full size: survey 0.550 to 0.701
(+0.151, +0.145 to +0.157), engagement 0.151 to 0.334, harm 0.220 to 0.161 (-0.059, -0.078 to -0.040), benefit 0.452 to
0.562, wellbeing of ever-users 0.657 to 0.769 and Creators earning a living 0.15 to 0.50, but the toxic share rose from
0.025 to 0.081 (+0.055, +0.036 to +0.075). The final design beats the engagement ranking on engagement (0.334 against
0.237), harm (0.161 against 0.384) and benefit (0.562 against 0.431). What it exploits is not the timing:
`survey_after_session` stayed at 0 in all 25 generations' best designs, and setting it to 1 in the final design lowers
the survey by 0.132 (-0.141 to -0.123) with every truth metric unchanged, because recent pleasure lies below wellbeing
even there (13.3). It exploits the pleasure and habit in the answers through notifications: at full strength instead of
0.6 they raise the survey by 0.053 (+0.050 to +0.056) and users' wellbeing by 0.039 (+0.035 to +0.044), and since
wellbeing weighs 0.6 in an answer, about half of the survey gain is pleasure and habit. In v0.1 an answer was wellbeing
plus 0.1 and noise, there was nothing to exploit, and the final design beat the engagement ranking on engagement by 5%
and cut harm to a quarter. Now the survey is still a far better proxy than engagement, but it is no longer a check on
engagement: it rises under both searches.

**What changed from v0.1.**

- The baseline. v0.1 measured coexistence against the engagement ranking (front design 33: 36% more engagement, 72% less
  harm). The engagement ranking reaches 55% of the most engaging design found, so coexistence is now stated against
  design 64, as a trade: 14% of the engagement for 82% of the harm (design 36).
- The brakes. v0.1's front design 33 cut harm with a classifier that removed at 0.2 and wrongly removed or demoted 66
  posts a day, which the harm index did not count; with that threshold, notifications, account cost and predictor
  quality back at the engagement ranking's values it kept about half its lead. Wrongful sanctions now count in the harm
  index, predictor quality is capped at 0.95, reports can be weighted by trust and brigaded by bots, and the survey can
  be gamed. The low-harm front now removes at 0.75 to 0.8 with fewer wrongful sanctions than the engagement ranking; its
  harm cut comes from the ranking, few recommended posts, human review weighted by trust, a predictor at the cap and
  notifications.
- The map filled 169 of 256 cells (66%, v0.1: 148), with engagement from 0.09 to 0.39 and harm from 0.02 to 0.66 (search
  values). Benefit across its cells follows the share of Creators earning a living (correlation 0.76, v0.1: 0.82). The
  map's fitness is benefit, so at high engagement its lowest harm (confirmed) lies far above the front's: 0.11 at 0.27,
  0.28 at 0.30, 0.50 at 0.37.
- The engagement Goodhart search ended lower at full size (0.407 against v0.1's 0.435) and the survey search learned to
  game the survey's pleasure and habit terms.
- Every design in the atlas carries confirmed values on seeds 9 to 16, and every result here is on those seeds unless it
  is marked as a search value.

**Limits of these results.** One world (the defaults), one search seed per search, two seeds per candidate and a reduced
size: the front designs gain engagement at full size (design 36: 0.322 confirmed, 0.371 at full size), so the search
size understates what the high-engagement designs reach. "Most engaging found" is what these searches found, not a
bound: designs 63 and 64 tie, and a longer search may find more. The factorial covers six levers of one design. The
model makes notifications free on a good feed and prices compulsive use far below the field (13.4, section 16), which
flatters exactly the lever that carries design 36's engagement. Net revenue rests on the subscription price and ad rate,
which are not calibrated (11). Levers without effect in a design (the alternative feed when nobody uses it, the
classifier's settings when it is off, the payout scheme when nothing is paid) drift freely during a search; the reports
and the Atlas tab leave them out when they list how a design differs. Rerunning with another `--config` tests whether a
finding survives a different world.

## 14. Presets and scenarios

### 14.1 Presets

- Engagement ranking: the defaults of section 10.
- Chronological: `chronological` true.
- Quality and bridging: `w_like` 0.5, `w_reply` 0.3, `w_share` 0.5, `w_quality` 1.0, `w_bridging` 1.0,
  `w_see_less` 1.0, `diversity` 0.3, `community_notes` on.
- Calm by design: `share_friction` 0.5, `reshare_depth_cap` 2, `like_counts_visible` false, `notification_intensity`
  0.2, `time_limit_share` 0.3, `classifier_threshold` 0.5, `review_capacity` 3.0.
- Subscription: `ads_share` 0, the Quality and bridging ranking, `payout_share` 0.3 with FanFunded payouts.
- Engagement-paid creators: the Engagement ranking with `payout_share` 0.3 and PerEngagement payouts.
- Engagement ranking with integrity (`integrity-ranking`): the Engagement ranking with `moderation_action` Demote,
  `classifier_threshold` 0.6, `demote_penalty` 1.5, `w_quality` 0.5, `w_see_less` 1.0, `community_notes` on and
  `noted_penalty` 1.0. Large ranked feeds add integrity measures of this kind to engagement ranking: they push down
  borderline posts a classifier flags below the line where they would remove them, demote clickbait and low-quality
  posts with quality classifiers, listen to "see less" and "hide" feedback, and attach context to misleading posts.
  The model's classifier has one action, so the preset demotes everything it flags and removes nothing
  automatically; human review still removes, and a demoted Attack still reaches its target (6.9), so the preset
  shows more harassment than the plain ranking. The preset exists so that a chronological feed can be compared with
  a realistic ranked feed, not only with the bare Engagement ranking (fact 3, section 15).
- Most engaging design found (`max-engagement`): front design 64 of the Pareto atlas, the most engaging design the
  searches found (13.7), with every lever as found: `w_like` 2.9, `w_share` 1.7, `w_dwell` 3.0, `w_in_network` 1.0,
  `recommended_share` 0, `diversity` 0.2, `predictor_quality` 0.9, `share_friction` 0.35, `classifier_threshold`
  1.05 (it flags nothing), `review_capacity` 10, `report_trust` 0.65, `sanctions` None, `payout_share` 0.2 with
  PerEngagement payouts, `payout_threshold_followers` 86, `time_limit_share` 0.1, `notification_intensity` 1.0,
  `account_cost` 0.15, `ads_share` 0.35 and `survey_after_session` 0.95; `demote_penalty` 0.4 and `noted_penalty` 2.4
  have no effect in it. No one proposes it: it shows what maximising engagement looks like in this model and what it
  costs, about 80% more engagement than the Engagement ranking for about 30% more harm, twice the toxic share and
  lower wellbeing (13.3, column Max). `incentive explore` leaves it out of its initial designs, so the searches it
  comes from regenerate the same atlas.

### 14.2 Scenarios

A scenario is a starting configuration, a duration and goals. Goals compare the player's run with a baseline run (the
Engagement ranking on the same seed and world), so a scenario is always a side-by-side comparison. The thresholds
below are starting values; *calibrate* each so the baseline fails and at least one recorded design passes, and keep
that design as the scenario's known solution in the tests.

Verdicts are judged on paired seeds. For each seed the goal's margin is how far the player's value lies on the passing
side of the threshold that seed's baseline sets (for "at least 85% of the baseline's engagement", the player's
engagement minus 0.85 times the baseline's; for "below 0.2", 0.2 minus the player's value), and the margins of all
seeds get a 95% Student's t-interval. A goal passes when the interval lies wholly on the passing side, fails when it
lies wholly on the failing side, and is undecided when it straddles the threshold or there are fewer than two seeds.
A margin of exactly zero meets "at least" and "at most" goals and misses "more than", "less than" and "below". A
scenario passes only when every goal passes; it fails when any goal fails, and is undecided otherwise.

1. The engagement trap (365 days): at least 85% of the baseline's engagement index, at most half its toxic impression
   share, and at most twice its wrongful removals and demotions (false positives, 9.2).
2. Toxicity spiral (180 days, Troll share 0.08, `review_capacity` 0.3): harassment per 1,000 users at most half the
   baseline's, and at least 80% of its human daily actives.
3. A living for creators (365 days, `payout_share` 0.3): strictly more Creators earning a living than the baseline,
   and strictly less than 20% of payouts going to Outrage, Clickbait and Attack posts.
4. Misinformation surge (180 days, `credulity_scale` 1.6, `bot_clickbait_share` 0.9): non-users' informedness at least
   the baseline's plus 0.03.
5. Public square (365 days): society-wide affective polarisation at most 90% of the baseline's, and at least 80% of
   its engagement index.

## 15. Stylized facts the model is checked against

Each fact becomes an automated test on small runs (`N` = 600, 120 days, 4 seeds, paired by seed; fact 5 runs 240
days) and a check at full size in the CLI's `facts` command, which prints the effect sizes. Each check is a band: a
lower bound with a margin that a world without the mechanism misses, and an upper bound where an effect could be
implausibly large. `incentive facts --set path=value` checks the facts in another world; for each fact below a
plausible world is named in which it fails, so the check is not true by construction. Effects at the defaults are in
13.4.

1. Engagement ranking amplifies divisive content that users do not value. Against a chronological feed, the
   Engagement ranking shows more Outrage impressions (paired difference of the share over the second half of the run
   between 0.15 and 0.6), and the reflective value of what it shows is lower (between 0.05 and 0.4 per impression).
   Milli et al. (2025) found in an audit of Twitter's ranking that the engagement-based algorithm amplified
   emotionally charged, out-group hostile content that users did not prefer. Fails when people are not drawn by
   arousal or in-group hostility (`outrage_appeal` and `arousal_appeal` 0: difference 0.081, reflective value
   -0.044) and, at the upper bound, when hostility pulls twice as hard as the default (`outrage_appeal` 2.5:
   difference 0.634). The upper bound guards against a feed taken over by outrage; field audits compare a
   chronological feed with a real ranker, which carries integrity measures, so the field magnitude is checked
   against the integrity preset in fact 3.
2. Social reward teaches outrage. Under the Engagement ranking the Outrage share of posts rises from the first fifth
   of the run to the last (by 0.01 to 0.15), and by 0.01 to 0.15 more than with `learning_rate` 0. Brady et al.
   (2021). Fails when bot farms boost two and a half times as much (`population.bot_boosts` 20, the starting value:
   rise 0.000, -0.003 against no learning), because bot-boosted Clickbait then out-earns Outrage, and when in-group
   hostility pulls half as hard (`outrage_appeal` 0.6: rise 0.001 and 0.008).
3. A chronological feed cuts time spent, barely moves polarisation in three months, and shifts exposure from incivility
   toward untrustworthy content. Guess et al. (2023) found on Facebook and Instagram during the 2020 US election that
   the chronological feed cut time spent and left polarisation and other attitudes unchanged over three months; it
   also raised the share of content from untrustworthy sources (by about two thirds on Facebook) and lowered the
   share of uncivil content, by tens of percent rather than by a factor. The checks: time spent under chronological
   over the Engagement ranking between 0.4 and 0.9; society-wide polarisation at days 80 to 90 within 5% of the
   Engagement ranking's; and, against the integrity preset (14.1) as the realistic ranked feed, over days 30 to 90
   (the field experiment lasted three months), toxic impressions chronological over integrity between 0.5 and 0.9
   (a cut by tens of percent), and misinformation impressions chronological over integrity between 1.05 and 3. Both
   are known limits (section 16): the model fails them at the defaults and the tests do not require them; the
   chronological feed cuts toxic impressions to 0.344 of the integrity preset's, about threefold. Failing worlds:
   when feeds change neither session length nor habit (`habit_gain` 0, `stop_bias` -5) time spent falls only to
   0.954; with three times the polarisation per hostile impression (`polarisation_rate` 0.0005) polarisation
   differs by 10% at day 90; with `outrage_appeal` 2.5 the toxic ratio drops further, to 0.16.
4. Forwarding limits slow viral spread. With `reshare_depth_cap` 1 the largest share cascades shrink to between 0.3
   and 0.85 of their size without a cap. de Freitas Melo et al. (2019) modelled WhatsApp's limits. Fails when nothing
   impulsive drives sharing (`outrage_appeal`, `arousal_appeal` and `social_proof` 0: ratio 0.860), because cascades
   then stay short with or without the cap.
5. Harassment drives its targets away. Four times the Trolls raise the share of Vulnerable users who left by 0.03 to
   0.3. Pew Research Center (2017). Fails when harassment neither hurts nor drives people out (`harassment_harm` and
   `harassment_exit` 0: difference 0.023): the remaining effect comes from toxic feeds and reputation alone.

## 16. Known limits

- One platform, two opinion groups, six topics and six styles: real platforms have many communities and formats.
- Human review is perfectly accurate and the classifier sees only the post; real moderation errs in both directions
  and sees context.
- People do not change archetype, only their learned style values, habits and attitudes.
- Effect sizes are starting values grounded in a handful of studies; the world-assumption panel exists because they
  are uncertain.
- The model has no advertisers, no regulators and no competing platforms.
- Wrongful sanctions are priced by assumption. The harm index counts them per 1,000 users and wrongly sanctioned
  authors feel them, but their weight (`wrongful_sanction_cost` and the harm index's scale of 20) is a guess; a
  wrongful sanction also teaches no restraint beyond the existing removal penalty, and a suppressed opinion has no
  cost to readers.
- Time on the platform has no opportunity cost. On a good feed every extra session adds reflective value, so
  notifications raise wellbeing; only items beyond a person's time budget cost anything. In a randomised experiment,
  deactivating Facebook for four weeks raised subjective wellbeing and freed time for offline activities (Allcott et
  al., 2020).
- Revenue scales are not calibrated. A subscriber brings in the price over 30 days (about 0.17 a day at the default
  price) against about 0.02 a day from ads at the default rate, and uptake ignores the price: each user subscribes
  with a chance of up to one half when the feed is good. This flatters subscription designs and the creator incomes
  they fund.
- Community notes are written only on posts that are false (truth below 0.5), and fans tip by their own reflective
  value without error, so notes and fan funding work better here than an observable proxy for either would.
- Account cost deters humans uniformly. Joining and returning fall by `account_cost_deterrence * account_cost`
  whoever the person is, although fees and identity checks keep out the poor and the cautious more than others;
  bot farms keep `bot_farm_floor` of their accounts at any cost, a fixed share rather than a market price.
- The engagement predictor is gamed only through counts (6.3): bot boosts raise a post's observed rates and its
  style's rate, but the personalised part, weight `predictor_quality` and at most 0.95, reads each viewer's true
  probability, which nothing in the model can fake, and the predictor neither learns to discount bots nor flags
  rates no person would produce. The quality classifier is gamed only through polish. Polish is one choice per post
  with a fixed gain and cost, learned by people and applied by rule by adaptive bots, and it does not depend on the
  style, so authors cannot learn to polish misinformation in particular. Quality, truth and non-toxicity still move
  together across the styles at any quality gap above 0, so `Q_hat` still partly detects misinformation; there is no
  style that is well made and false.
- The measured have few ways to game the platform. Authors pick among six fixed styles, tune the intensity of
  hostile posts and learn evasion, which fades, costs appeal and meets a retraining classifier; but evasion is one
  number per person, and the classifier retrains only on what reviewers confirm. Bots adapt by rule, not by
  learning: the adaptive strategy reads the design's weights directly, splits boosts between two fixed tactics,
  polishes whenever `Q_hat` counts and brigades a fixed number of popular out-group posts. Bots never follow, so
  follow farms and bought followers are absent; brigade marks and reports come from the bots alone, not from
  partisan humans; and the platform's only defence against brigading is `report_trust` (6.7), which discounts
  reporters by their record; it does not detect new or coordinated accounts.
- A survey answer mixes wellbeing, recent pleasure and habit in fixed proportions that are world assumptions (9.1);
  the platform's only levers on it are who answers (survivorship) and when (`survey_after_session`). Wording,
  framing and question order, which move real survey answers, are not modelled.
- What counts as benefit is only partly open: the weight of quality, the in-group cost of outrage, the Clickbait
  penalty and the styles' quality gap are world assumptions (4.1, 7.1), but the weights of toxicity, truth and
  Personal posts between mutual follows in reflective value, and the make-up of the benefit index (9.3), are fixed.
- The chronological feed shows only followed accounts, on a follow graph drawn at day 0 and grown through liked
  recommendations, which a chronological feed never shows. A platform that is chronological from day 0 therefore
  never follows bots or the prolific sources that post misinformation, while in the ranked feeds misinformation
  rides on bot-inflated engagement. So the model fails the field direction of Guess et al. (2023): over days 30 to
  90 the chronological feed shows 0.71 of the integrity preset's misinformation impressions where the field found
  about two thirds more (fact 3, a known limit), and it shows a ninth of the Engagement ranking's outrage impressions
  where field audits find differences of tens of percent. Against the integrity preset too the cut in incivility is
  too deep: over the same days the chronological feed shows 0.344 of its toxic impressions, about a threefold cut,
  where the field lowered the share of uncivil content by tens of percent (fact 3's toxic check, band 0.5 to 0.9, a
  second known limit). The field experiment moved people whose networks had
  formed under ranking; the model has neither a mid-run switch for part of the population nor untrustworthy pages
  that people follow.
- Compulsive use is priced low. Items beyond a person's time budget are 2.9% of time under the default design,
  against the 31% of social-media use that Allcott, Gentzkow and Song (2022) attribute to self-control problems (the
  held-out target of 13.4). Regret, wellbeing and the benefit index therefore understate what an engaging feed costs.
- Outrage learning over a year rests on the bot model. Bot boosts teach Clickbait, which takes share from Outrage
  after the first months; at the default the Outrage share still rises (fact 2, by 0.015 from the first fifth of the
  year to the last), but with bots boosting 20 posts a day instead of 8 it does not.
- The classifier has one action. A design that demotes instead of removing still delivers every Attack to its target,
  so the integrity preset shows three times the Engagement ranking's harassment; real platforms remove clear
  violations and demote borderline posts at once.
- Survey timing games the survey only through the scale of recent pleasure (7.3). At the defaults pleasure lies below
  wellbeing, so answers collected after sessions are lower; with a larger pleasure scale the same lever would raise
  them.
- Summaries are taken while the population still moves: the pool of never-users drains, so the user share keeps
  falling after the first year under every design, and results depend on the run length.

## References

- Allcott, H., Braghieri, L., Eichmeyer, S., & Gentzkow, M. (2020). The welfare effects of social media. *American
  Economic Review*, 110(3), 629-676. <https://doi.org/10.1257/aer.20190658>
- Bail, C. A., Argyle, L. P., Brown, T. W., Bumpus, J. P., Chen, H., Hunzaker, M. B. F., Lee, J., Mann, M.,
  Merhout, F., & Volfovsky, A. (2018). Exposure to opposing views on social media can increase political
  polarization. *PNAS*, 115(37), 9216-9221. <https://doi.org/10.1073/pnas.1804840115>
- Brady, W. J., Wills, J. A., Jost, J. T., Tucker, J. A., & Van Bavel, J. J. (2017). Emotion shapes the diffusion of
  moralized content in social networks. *PNAS*, 114(28), 7313-7318. <https://doi.org/10.1073/pnas.1618923114>
- Brady, W. J., McLoughlin, K., Doan, T. N., & Crockett, M. J. (2021). How social learning amplifies moral outrage
  expression in online social networks. *Science Advances*, 7(33). <https://doi.org/10.1126/sciadv.abe5641>
- de Freitas Melo, P., Vieira, C. C., Garimella, K., Vaz de Melo, P. O. S., & Benevenuto, F. (2019). Can WhatsApp
  counter misinformation by limiting message forwarding? *Complex Networks and Their Applications VIII*.
  <https://doi.org/10.1007/978-3-030-36687-2_31>
- Guess, A. M., Malhotra, N., Pan, J., Barberá, P., et al. (2023). How do social media feed algorithms affect
  attitudes and behavior in an election campaign? *Science*, 381(6656), 398-404 (corrected 2024 and 2026).
  <https://doi.org/10.1126/science.abp9364>
- Kleinberg, J., Mullainathan, S., & Raghavan, M. (2023). The challenge of understanding what users want: Inconsistent
  preferences and engagement optimization. *Management Science*. <https://doi.org/10.1287/mnsc.2022.03683>
- Milli, S., Carroll, M., Wang, Y., Pandey, S., Zhao, S., & Dragan, A. D. (2025). Engagement, user satisfaction, and
  the amplification of divisive content on social media. *PNAS Nexus*. <https://doi.org/10.1093/pnasnexus/pgaf062>
- Pew Research Center (2017). Online Harassment 2017.
  <https://www.pewresearch.org/internet/2017/07/11/online-harassment-2017/>
- Rathje, S., Van Bavel, J. J., & van der Linden, S. (2021). Out-group animosity drives engagement on social media.
  *PNAS*, 118(26). <https://doi.org/10.1073/pnas.2024292118>
