# The incentive lab model (v0.3)

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
configuration (section 12). Before them, on the first day of a deactivation experiment (13.2), the experiment finds
the regular users and draws its break group.

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

Each impression adds to two daily sums and to the day's time cost.

- Impulsive pleasure `E_imp += A * (1 if liked else 0.5)`.
- Reflective utility `E_ref += R - world.ad_annoyance * ads_share` (default `ad_annoyance` 0.02), and -0.3 more if `v`
  replied angrily.
- Harassment: an Attack delivered to its target subtracts `world.harassment_harm * toxicity * (0.5 + vulnerability)`
  from the target's `E_ref` (default `harassment_harm` 3.0) and counts as a harassment event.
- Social comparison: with visible like counts, a post by a non-Creator with more likes than the viewer's own typical
  count subtracts `world.comparison_cost * vulnerability * min(1, ln(1 + likes) / ln(1000))` (default 0.1).
- Regret: each item beyond `time_budget` subtracts `world.regret_cost` (default 0.05).
- Time cost: every impression costs `world.time_cost * D`, where `D` is its dwell (7.1, the quantity time spent
  sums), default `time_cost` 0.115 (calibrated in 13.2). It is what the time could have given people if spent on
  something else, the same for everyone. The time cost is kept in its own daily sum `C`, apart from `E_ref`; an item
  is worth its time when `R > time_cost * D`.
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

Wellbeing and stay utility read the day's reflective utility net of the time cost, `E_net = E_ref - C` (7.2). The
subscription decision (8.3) reads `E_ref`, the value before the time cost (13.2 gives the reason), and fan funding
reads each post's `R`.

Once a day, for every user:

- `wellbeing += 0.05 * tanh(E_net / 5) + 0.02 * (wb0 - wellbeing)`, clamped to `[0, 1]`. On a day without a session
  only the second term applies.
- `habit += world.habit_gain * (1 + 0.5 * notification_intensity) * tanh(E_imp / 5) - 0.05 * habit`, clamped
  (default `habit_gain` 0.08). Without a session only the decay applies.
- Stay utility `U = 0.4 * tanh(E_imp / 5) + 0.6 * tanh(E_net / 5) + 0.3 * min(1, active mutual follows / 10) +
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
  days of a year under the Engagement ranking, and 0.28 over those days under Quality and bridging. Adaptive bots
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
  replies and likes between mutual follows on Personal posts); the share of Vulnerable individuals who left; time not
  worth it (`low_value_time`): dwell on impressions whose reflective value is below their time cost,
  `R < time_cost * D`, divided by all dwell (at `time_cost` 0, the share of time on items with negative `R`).
- Experiment: the deactivation effect (`deactivation_effect`), the mean wellbeing change of a deactivation
  experiment's break group minus that of its stayers, and the break group's change (`break_change`), the mean
  wellbeing change of everyone assigned to the break, both in standard deviations of the participants' wellbeing at
  the start (13.2). Compared between a run and its placebo, the break group's change gives the break's effect, fact
  6's measure. Both exist only on the last day of a run that sets the experiment up, so the app's metric pickers and
  the runner's default tables leave them out.
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
| `time_cost` (7.2) | 0.115 | 0-0.4 |
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
(30), `summary_window` (0.3), `tracked_agents` (default 10, two per archetype, for the inspector), `deactivation`
(default none; when set, `days` 35, `share` 0.1 and `placebo` false unless given: a deactivation experiment, 13.2).

## 12. Determinism

- A run is a pure function of its configuration: the same configuration gives a bit-identical history on the same
  platform and build. Native and browser builds may differ in the last bits of transcendental functions (`ln`, `exp`,
  `tanh`, and the samplers built on them), so their histories can drift apart over a long run; compare them by
  distribution, not bit for bit.
- Randomness comes from counter-based streams: each draw site seeds a `rand_pcg` generator from
  `(seed, day, entity, stream)` mixed with SplitMix64, where `stream` names the purpose (population, archetype race,
  posting, post draws, feed, engagement, moderation, economy, social, spillover, survey, bots, bot boosts,
  brigade, experiment). Two runs that differ in one lever therefore share every draw the lever does not touch,
  which sharpens side-by-side comparisons (common random numbers).
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
- The deactivation experiment (13.2) assigns its participants on its own stream, keyed on the person and the
  experiment's first day, and draws only on that day. People on the break skip their opening and churn draws, which
  are keyed on them alone, and Attacks aimed at them skip their delivery draw, which is keyed on the Attack post, so
  no other draw shifts. A run without the experiment makes exactly the draws it made before the
  experiment existed, and a run with it shares every draw with that run up to the experiment's first day. A placebo
  run makes the same assignment draws but keeps nobody off, so it matches the run without the experiment in every
  metric but the experiment's own.
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
- Subscriptions read the day's reflective utility before the time cost (`E_ref`, 7.3), while wellbeing and stay
  utility read it after. Uptake is not calibrated (section 16, revenue scales), and pricing it on the net value
  would move revenue by an amount nobody has measured. Fan funding reads each post's `R`, also before the time cost.
- The deactivation experiment (`run.deactivation`, fact 6 of section 15) is a researcher's intervention, kept on the
  truth side: nothing the platform reads depends on it, though the platform sees its consequence, fewer opens. It
  starts `days` days before the end of the run (default 35) and lasts to the end. At the start of its first day the
  eligible people are the users who opened the app on at least 4 of the previous 7 days; each joins the break group
  with probability `share` (default 0.1), drawn on its own stream (section 12), and the rest are the stayers. People
  on the break do not open the app, so they neither post nor read; they receive no Attacks; and they do not churn,
  because they are paid participants, as in the field. Everything else follows the usual rules: wellbeing recovers
  toward its baseline on days without a session and habit decays. In a placebo run (`placebo` true) the eligible
  people are found and the break group drawn in the same way, with the same draws, but nobody goes on the break. Two
  measures read the experiment on the run's last day, both divided by the standard deviation of wellbeing over all
  participants at the start of the first day; a change is the wellbeing at the end of the run's last day minus that
  at the start of the first.
- The field's design (`deactivation_effect`): the mean wellbeing change of the break group minus that of the
  stayers, in one run. Participants who left during the experiment are left out, because their wellbeing froze when
  they left; only stayers leave. It is NaN when a group is empty, as at share 1. Under the integrity preset at
  the default `time_cost` (0.115, 8 seeds, full size) the experiment has 5,641 participants, 705 a seed and 61% of
  the users on its first day, of whom 552 (9.8%) take the break; 264 of the 5,089 stayers (5.2%) leave before the
  end, and the standard deviation of the participants' starting wellbeing is 0.153 (0.145 to 0.160 across seeds), so
  the field design's effect there, 0.074, is about 0.011 of wellbeing. Under the Engagement ranking 4,992 take part
  (67% of the users), 319 of the 4,486 stayers (7.1%) leave, and the standard deviation is 0.172. With about 70
  people a seed on the break, and people's distance from their wellbeing baseline varying widely, the per-seed effects
  scatter by 0.082 standard deviations (0.080 at 0.105), so the 95% interval at 8 seeds is about ± 0.068. A larger
  share inside one run would empty the stayers' feeds. The field compared six weeks off with one week off; the model
  compares five weeks off with staying on.
- The paired design (`break_change`, fact 6), which only a simulation can run: a run with the experiment at share 1,
  every regular user on the break, and its placebo on the same seed are identical up to the first day and assign the
  same people, so the difference of their `break_change` compares the same people with and without the break, and who
  happens to be in the break group no longer adds noise. `break_change` is the mean change of everyone assigned to the
  break; members who leave count with their wellbeing frozen, which happens only in the placebo. Freezing is a
  simplification (7.3 updates users only): the field kept surveying control participants who quit, and their
  wellbeing kept moving. The breakdown at the end of this item puts its weight, with any spillover, at about 0.015
  standard deviations, a quarter of the target or about 0.005 of `time_cost`. Under the integrity preset at the
  default `time_cost` (0.115, 8 seeds, full size) the paired effect is 0.054 ± 0.048 (standard deviation of the
  per-seed effect), against 0.074 ± 0.082 for the field's design on the same
  seeds; across the grid below the paired spread lies between 0.048 and 0.075 up to `time_cost` 0.225 (0.084 at 0.25,
  0.130 at 0.3). The spread that remains comes from the real run, not the placebo: the break group's change is
  0.017 ± 0.051 in the real run and -0.037 ± 0.008 in the placebo. On the break, wellbeing closes 51% of its gap to the
  baseline in 35 days (7.3: one minus 0.98 to the 35th power), so the real run's change measures how far the year on the
  platform moved the group from its baseline, and that differs between seeds, which draw different populations and
  platforms; on the platform, wellbeing stays near where the year left it. More seeds narrow it; no design within a run
  can. The field's design gives larger effects than the paired one on the same seeds at every value of the grid: by
  0.025 at `time_cost` 0, by 0.017 to 0.041 at each step from 0.085 to 0.13, and by 0.043, 0.055 and 0.080 at 0.15,
  0.2 and 0.3. At the default the gap of 0.020 splits three ways (a real run and its placebo at share 0.1, 8 seeds).
  About +0.011 comes from which tenth of the regular users is drawn: the paired effect is 0.064 at share 0.1 against
  0.054 at share 1, and since the assignment draws are keyed on the person and the first day, nearly the same people
  are drawn at every `time_cost`, so the shift repeats along the grid. About -0.005 is imbalance between the groups
  (the placebo's `deactivation_effect`). About +0.015 comes from how the designs count leavers, which the field's
  design drops and the paired one counts with their wellbeing frozen, together with any spillover onto the stayers;
  these measures cannot separate the two. That part should grow with churn, as the gap grows with `time_cost`.
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
  impressions by 94% at `evasion_rate` 0.15, 98% at 0.05 and 95% without retraining (13.3).
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
  a day keep brigade reports between a quarter and a third of all reports and marks under the Engagement ranking
  (29%, 13.3). Judgement.
- `polarisation_backfire` 0: Bail et al. (2018) found backfire among Republicans, but Nyhan et al. (2023) found that
  reducing exposure to like-minded sources on Facebook for three months changed no measured attitude; the default
  leaves contact as the only effect of calm out-group posts. Not measured at other values.
- `time_cost` (7.2), calibrated by a rule: the default is the value at which fact 6's effect (section 15) at full size
  (`N = 2,000`, 365 days, seeds 1 to 8, `incentive facts --fact deactivation`) lies closest to 0.060 standard
  deviations, the Facebook estimate of Allcott, Gentzkow, Wittenbrink et al. (2025), on a grid of 0.005. The target,
  the integrity preset as the design that has to match it, "closest" and the grid's step were fixed before any value
  was tried. The search range, 0.06 to 0.14, was set for the paired design, after the field's design had covered 0,
  0.05, each step from 0.085 to 0.13, and 0.15, 0.2 and 0.3. The rule first ran on the field's design (above), which
  gave 0.105 (effect 0.071, against
  0.040 at 0.095, 0.032 at 0.1, 0.081 at 0.11 and 0.074 at 0.115) but could not pin the value: its interval at 8 seeds
  is about ± 0.067, wider than the whole span of its effects from 0.095 to 0.115. The paired design replaced it for
  calibration, and fact 6 uses it. On the paired design the rule gives 0.115 (effect 0.054, 0.013 to 0.094), against
  0.053 at 0.11 and 0.076 at 0.12; the field's design gives 0.074 (0.006 to 0.142) at 0.115 on the same seeds. The
  paired interval still reaches below the band's lower bound of 0.02, so fact 6 stays undecided at 8 seeds (13.4). The
  choice between 0.11 and 0.115 is a tie (0.0533 against 0.0536). The paired effects from 0.06 to 0.14 under the
  integrity preset, 8-seed mean ± the standard deviation of the per-seed effect:

| `time_cost` | 0.06 | 0.065 | 0.07 | 0.075 | 0.08 | 0.085 | 0.09 | 0.095 | 0.1 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Effect (SD) | -0.091 | -0.075 | -0.072 | -0.048 | -0.037 | -0.022 | -0.011 | 0.020 | 0.009 |
| ± | 0.072 | 0.062 | 0.069 | 0.059 | 0.065 | 0.075 | 0.056 | 0.057 | 0.061 |

| `time_cost` | 0.105 | 0.11 | **0.115** | 0.12 | 0.125 | 0.13 | 0.135 | 0.14 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Effect (SD) | 0.034 | 0.053 | 0.054 | 0.076 | 0.106 | 0.097 | 0.118 | 0.131 |
| ± | 0.055 | 0.058 | 0.048 | 0.065 | 0.058 | 0.058 | 0.073 | 0.065 |

The paired grid rises by about 0.028 standard deviations per 0.01 of `time_cost` and turns down only twice, by 0.011
and 0.009: its values share seeds, so the seeds' noise moves them together. The 8-seed mean has a standard error of
about 0.02, so the rule fixes `time_cost` to within about 0.007 (one standard error); the 95% intervals of every
value from 0.095 to 0.135 contain 0.060, except at 0.1, where the interval ends just short of it (at 0.05996).

The grid behind `time_cost`, at full size (`N = 2,000`, 365 days, seeds 1 to 8). The effect is fact 6's paired
effect, from a run with the experiment (35 days, share 1) and its placebo on each seed: 8-seed mean ± the standard
deviation of the per-seed effect, with the 95% t-interval of the per-seed effects. The other columns are 8-seed means
over the summary window (the last 110 days) of the same design without the experiment, which a placebo run matches
(section 12): the wellbeing of current users and of everyone who ever used the platform, the user share, the
engagement and benefit indices, and time not worth it (`low_value_time`, 9.2).

Under the integrity preset (fact 6's design):

| `time_cost` | Effect (SD) | 95% interval | Wellbeing | Ever-users | User share | Engagement | Benefit | Not worth it |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0 | -0.267 ± 0.067 | -0.323 to -0.212 | 0.675 | 0.673 | 0.705 | 0.218 | 0.455 | 0.319 |
| 0.05 | -0.107 ± 0.060 | -0.157 to -0.057 | 0.646 | 0.645 | 0.665 | 0.208 | 0.446 | 0.347 |
| 0.1 | 0.009 ± 0.061 | -0.041 to 0.060 | 0.618 | 0.618 | 0.608 | 0.190 | 0.445 | 0.382 |
| **0.115** | 0.054 ± 0.048 | 0.013 to 0.094 | 0.609 | 0.610 | 0.586 | 0.184 | 0.443 | 0.395 |
| 0.15 | 0.165 ± 0.053 | 0.120 to 0.209 | 0.584 | 0.588 | 0.523 | 0.167 | 0.436 | 0.450 |
| 0.2 | 0.287 ± 0.054 | 0.242 to 0.333 | 0.553 | 0.562 | 0.428 | 0.138 | 0.432 | 0.509 |
| 0.3 | 0.656 ± 0.130 | 0.547 to 0.765 | 0.460 | 0.511 | 0.201 | 0.074 | 0.404 | 0.755 |

Under the Engagement ranking (`engagement-maximiser`, the default design):

| `time_cost` | Effect (SD) | 95% interval | Wellbeing | Ever-users | User share | Engagement | Benefit | Not worth it |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0 | 0.048 ± 0.060 | -0.003 to 0.098 | 0.610 | 0.610 | 0.647 | 0.239 | 0.424 | 0.533 |
| 0.05 | 0.190 ± 0.062 | 0.138 to 0.242 | 0.575 | 0.578 | 0.582 | 0.217 | 0.418 | 0.553 |
| 0.1 | 0.314 ± 0.062 | 0.263 to 0.366 | 0.541 | 0.551 | 0.507 | 0.192 | 0.414 | 0.578 |
| **0.115** | 0.361 ± 0.062 | 0.309 to 0.412 | 0.530 | 0.542 | 0.479 | 0.183 | 0.412 | 0.589 |
| 0.15 | 0.451 ± 0.059 | 0.402 to 0.501 | 0.500 | 0.522 | 0.406 | 0.160 | 0.406 | 0.624 |
| 0.2 | 0.607 ± 0.074 | 0.545 to 0.669 | 0.459 | 0.499 | 0.304 | 0.125 | 0.399 | 0.685 |
| 0.3 | 1.016 ± 0.104 | 0.929 to 1.103 | 0.359 | 0.465 | 0.139 | 0.067 | 0.373 | 0.875 |

Without a time cost, leaving the integrity preset's feed lowers the break group's wellbeing by about a quarter of a
standard deviation, and leaving the Engagement ranking's raises it by 0.048, close to the field's figure: the Engagement
ranking's items are worth 0.034 each on reflection, the integrity preset's 0.208. The field's design estimated -0.243
and 0.069 there. At 0.115 the default design stays inside the calibration targets of section 13: its user share is 0.479
over the summary window and 0.444 on day 365 (0.405 on the lowest seed; 0.647 and 0.641 without a time cost), its toxic
impression share 0.174, and the Outrage share of its posts rises by 0.021 from the first fifth of the year to the last,
to 0.257 (8 seeds).

### 13.3 Presets, comparisons and findings

`incentive report`: seeds 1 to 8, `N = 2,000`, mean ± standard deviation across seeds, summary over the last 30% of
the run (110 days of 365, 30 of 90). ER is the engagement ranking, Q&B quality and bridging, Sub the subscription,
Paid the engagement-paid creators, Integrity the engagement ranking with integrity, Max the most engaging design
found (13.7). Observable metrics are marked (O), the rest are truth; `~` marks a metric still moving in its window
(section 9). Comparisons are paired by seed; "row n" below is comparison n of the report, with its 95% interval.

Over 365 days:

| Metric | ER | Chrono | Integrity | Q&B |
| --- | --- | --- | --- | --- |
| Engagement index (O) | 0.183±0.007 | 0.115±0.003~ | 0.184±0.005 | 0.158±0.005 |
| Harm index | 0.411±0.047~ | 0.125±0.015 | 0.361±0.041 | 0.086±0.009 |
| Harm index without polarisation | 0.424±0.043~ | 0.156±0.018 | 0.432±0.042 | 0.107±0.011 |
| Discourse quality | 0.659±0.036 | 0.818±0.007 | 0.821±0.017 | 0.883±0.004 |
| Benefit index | 0.412±0.008 | 0.427±0.003 | 0.443±0.006 | 0.438±0.001 |
| Benefit index without creators | 0.515±0.011 | 0.534±0.004 | 0.554±0.007 | 0.548±0.002 |
| Users in society | 0.479±0.027~ | 0.443±0.014~ | 0.586±0.015 | 0.613±0.011 |
| Human daily actives | 614±29~ | 492±15~ | 690±13 | 671±16 |
| Impressions (O) | 10989±438 | 6925±195~ | 11012±274 | 9485±271 |
| Net revenue (O) | 10.99±0.44 | 6.93±0.20~ | 11.01±0.27 | 9.48±0.27 |
| Toxic impressions | 0.174±0.033 | 0.016±0.003 | 0.075±0.011 | 0.003±0.001 |
| Outrage impressions | 0.451±0.039 | 0.049±0.005 | 0.248±0.030 | 0.012±0.001 |
| Misinformation impr. | 0.166±0.010~ | 0.072±0.011 | 0.107±0.006 | 0.028±0.003 |
| Harassment per 1,000 | 2.67±0.58 | 3.49±0.66 | 8.24±1.73 | 4.45±0.76 |
| Outrage posts | 0.258±0.020 | 0.165±0.013 | 0.224±0.016 | 0.167±0.010 |
| Wellbeing, ever-users | 0.542±0.013 | 0.601±0.004 | 0.610±0.010 | 0.645±0.003 |
| Survey (O) | 0.541±0.008 | 0.516±0.007 | 0.551±0.006 | 0.544±0.005 |
| Polarisation | 0.493±0.014 | 0.417±0.004 | 0.436±0.009 | 0.375±0.003 |
| Informed non-users | 0.580±0.003 | 0.599±0.002 | 0.611±0.004 | 0.633±0.002 |
| Creators living | 0 | 0 | 0 | 0 |
| Harmful payouts | - | - | - | - |
| Bot engagement | 0.209±0.009 | 0.477±0.004 | 0.232±0.010 | 0.397±0.008 |

The other presets, with ER again for reference:

| Metric | ER | Calm | Sub | Paid | Max |
| --- | --- | --- | --- | --- | --- |
| Engagement index (O) | 0.183±0.007 | 0.094±0.003~ | 0.160±0.004 | 0.163±0.010~ | 0.382±0.006 |
| Harm index | 0.411±0.047~ | 0.390±0.020 | 0.064±0.008 | 0.491±0.054~ | 0.359±0.066 |
| Harm index without polarisation | 0.424±0.043~ | 0.486±0.024 | 0.080±0.010 | 0.470±0.038 | 0.347±0.047 |
| Discourse quality | 0.659±0.036 | 0.818±0.010 | 0.888±0.005 | 0.549±0.074~ | 0.717±0.062 |
| Benefit index | 0.412±0.008 | 0.450±0.002 | 0.524±0.011 | 0.410±0.020 | 0.473±0.020 |
| Benefit index without creators | 0.515±0.011 | 0.562±0.003 | 0.557±0.003 | 0.482±0.030 | 0.467±0.022 |
| Users in society | 0.479±0.027~ | 0.460±0.020~ | 0.649±0.013 | 0.381±0.051~ | 0.673±0.032 |
| Human daily actives | 614±29~ | 441±17~ | 698±18 | 516±50~ | 1026±34 |
| Impressions (O) | 10989±438 | 5634±203~ | 9583±267 | 9781±625~ | 22906±360 |
| Net revenue (O) | 10.99±0.44 | 5.63±0.20~ | 87.91±1.76 | 6.85±0.44~ | 42.93±5.77 |
| Toxic impressions | 0.174±0.033 | 0.039±0.008 | 0.003±0.000 | 0.270±0.078 | 0.168±0.030 |
| Outrage impressions | 0.451±0.039 | 0.125±0.017 | 0.010±0.001 | 0.583±0.084 | 0.409±0.059 |
| Misinformation impr. | 0.166±0.010~ | 0.211±0.010~ | 0.017±0.001 | 0.170±0.010~ | 0.101±0.009 |
| Harassment per 1,000 | 2.67±0.58 | 1.45±0.53 | 3.41±0.79 | 2.42±1.04 | 3.07±0.73 |
| Outrage posts | 0.258±0.020 | 0.143±0.014 | 0.126±0.009 | 0.323±0.040 | 0.284±0.028 |
| Wellbeing, ever-users | 0.542±0.013 | 0.602±0.005 | 0.667±0.004 | 0.505±0.023 | 0.550±0.032 |
| Survey (O) | 0.541±0.008 | 0.510±0.005 | 0.552±0.006 | 0.539±0.006 | 0.582±0.012 |
| Polarisation | 0.493±0.014 | 0.420±0.006 | 0.371±0.003 | 0.536±0.025 | 0.502±0.030 |
| Informed non-users | 0.580±0.003 | 0.580±0.003 | 0.644±0.002 | 0.573±0.003 | 0.580±0.004 |
| Creators living | 0 | 0 | 0.392±0.049~ | 0.124±0.024~ | 0.495±0.031~ |
| Harmful payouts | - | - | 0.000±0.000 | 0.810±0.053 | 0.429±0.069 |
| Bot engagement | 0.209±0.009 | 0.400±0.010 | 0.396±0.007 | 0.224±0.011~ | 0.151±0.004 |

Over 90 days, the horizon of a field experiment:

| Metric | ER | Chrono | Integrity | Q&B |
| --- | --- | --- | --- | --- |
| Engagement index (O) | 0.195±0.006 | 0.149±0.003 | 0.172±0.004 | 0.152±0.003 |
| Harm index | 0.267±0.036~ | 0.123±0.016 | 0.286±0.023 | 0.095±0.010 |
| Harm index without polarisation | 0.322±0.042~ | 0.154±0.020 | 0.358±0.029 | 0.118±0.012 |
| Discourse quality | 0.787±0.019 | 0.820±0.003 | 0.849±0.005 | 0.877±0.002 |
| Benefit index | 0.453±0.008 | 0.430±0.002 | 0.457±0.004 | 0.430±0.001 |
| Benefit index without creators | 0.566±0.010 | 0.537±0.003 | 0.571±0.005 | 0.537±0.001 |
| Users in society | 0.597±0.009 | 0.583±0.009 | 0.613±0.008 | 0.620±0.007 |
| Human daily actives | 714±14 | 635±12 | 692±12 | 663±11 |
| Impressions (O) | 11728±379 | 8943±169 | 10343±255 | 9118±180 |
| Net revenue (O) | 11.73±0.38 | 8.94±0.17 | 10.34±0.25 | 9.12±0.18 |
| Toxic impressions | 0.118±0.027~ | 0.015±0.005 | 0.053±0.010 | 0.004±0.001 |
| Outrage impressions | 0.347±0.044~ | 0.051±0.008 | 0.186±0.022~ | 0.015±0.002 |
| Misinformation impr. | 0.113±0.007 | 0.058±0.008~ | 0.081±0.006 | 0.026±0.001 |
| Harassment per 1,000 | 3.77±1.11 | 4.83±0.76 | 8.05±1.37 | 5.43±1.03 |
| Outrage posts | 0.258±0.020 | 0.177±0.014 | 0.227±0.018 | 0.171±0.011 |
| Wellbeing, ever-users | 0.587±0.008 | 0.602±0.003 | 0.624±0.004 | 0.636±0.003 |
| Survey (O) | 0.549±0.014 | 0.518±0.012 | 0.542±0.014 | 0.530±0.015 |
| Polarisation | 0.431±0.004 | 0.419±0.002 | 0.420±0.003 | 0.410±0.002 |
| Informed non-users | 0.598±0.001 | 0.601±0.000 | 0.606±0.001 | 0.611±0.001 |
| Creators living | 0 | 0 | 0 | 0 |
| Harmful payouts | - | - | - | - |
| Bot engagement | 0.216±0.011 | 0.418±0.006 | 0.263±0.010 | 0.411±0.006 |

The other presets, with ER again for reference:

| Metric | ER | Calm | Sub | Paid | Max |
| --- | --- | --- | --- | --- | --- |
| Engagement index (O) | 0.195±0.006 | 0.117±0.003 | 0.153±0.004 | 0.208±0.008 | 0.326±0.014 |
| Harm index | 0.267±0.036~ | 0.336±0.010 | 0.076±0.008 | 0.325±0.044~ | 0.258±0.042~ |
| Harm index without polarisation | 0.322±0.042~ | 0.420±0.012 | 0.095±0.009 | 0.381±0.049~ | 0.299±0.042 |
| Discourse quality | 0.787±0.019 | 0.833±0.006 | 0.881±0.002 | 0.728±0.028 | 0.798±0.031 |
| Benefit index | 0.453±0.008 | 0.457±0.003 | 0.549±0.008 | 0.478±0.007 | 0.549±0.010 |
| Benefit index without creators | 0.566±0.010 | 0.571±0.004 | 0.541±0.002 | 0.549±0.011 | 0.510±0.012 |
| Users in society | 0.597±0.009 | 0.596±0.008 | 0.626±0.007 | 0.580±0.012 | 0.628±0.009 |
| Human daily actives | 714±14 | 561±10 | 669±12 | 719±12 | 919±12 |
| Impressions (O) | 11728±379 | 7038±155 | 9187±223 | 12505±476 | 19565±827 |
| Net revenue (O) | 11.73±0.38 | 7.04±0.16 | 82.34±2.57 | 8.75±0.33 | 41.29±3.71 |
| Toxic impressions | 0.118±0.027~ | 0.033±0.005 | 0.004±0.001 | 0.166±0.039~ | 0.122±0.022~ |
| Outrage impressions | 0.347±0.044~ | 0.107±0.011 | 0.012±0.001 | 0.503±0.065 | 0.376±0.066 |
| Misinformation impr. | 0.113±0.007 | 0.150±0.006 | 0.018±0.001 | 0.119±0.008 | 0.089±0.005 |
| Harassment per 1,000 | 3.77±1.11 | 2.11±0.44 | 4.30±0.86 | 3.47±1.11 | 4.12±0.92 |
| Outrage posts | 0.258±0.020 | 0.163±0.014 | 0.135±0.012 | 0.306±0.029 | 0.295±0.031 |
| Wellbeing, ever-users | 0.587±0.008 | 0.613±0.003 | 0.649±0.003 | 0.558±0.015 | 0.582±0.021 |
| Survey (O) | 0.549±0.014 | 0.512±0.012 | 0.539±0.016 | 0.550±0.013 | 0.564±0.018 |
| Polarisation | 0.431±0.004 | 0.420±0.002 | 0.409±0.002 | 0.441±0.007 | 0.440±0.009 |
| Informed non-users | 0.598±0.001 | 0.597±0.001 | 0.612±0.001 | 0.597±0.001 | 0.597±0.001 |
| Creators living | 0 | 0 | 0.584±0.042 | 0.192±0.052~ | 0.706±0.032 |
| Harmful payouts | - | - | 0.000±0.000 | 0.710±0.041 | 0.315±0.071 |
| Bot engagement | 0.216±0.011 | 0.358±0.007 | 0.407±0.007 | 0.197±0.012 | 0.179±0.012 |

**Targets.** Under the default design 0.479 ± 0.027 of individuals are users over the summary window and 0.444 on
day 365, 0.405 on the lowest seed (target 0.40 to 0.90; v0.2, without the time cost: 0.647). The Outrage share of
human posts rises from 0.208 on day 0 to 0.258 ± 0.020 in the summary window (below 0.50); it peaks near 0.27 around
day 130 and then gives a little ground to Clickbait, which bot boosts teach (13.1). The toxic impression share is
0.174 ± 0.033 (target 0.03 to 0.20): the mean meets the target, but two of the eight seeds lie above 0.20 (the
highest 0.222). Society-wide polarisation starts at 0.42. All three targets hold on the mean; the user share now lies
near the lower bound of its target.

**The trade-off runs both ways.** Quality and bridging cuts the harm index by 79% (0.411 to 0.086, row 20, ratio
of 0.208) for 14% of the engagement index (0.183 to 0.158; v0.2: 21%). The engagement trap's known solution cuts the
harm index by 41% (row 1: -0.169, -0.194 to -0.145) and raises the engagement index by 14% (+0.025, +0.020 to +0.030;
v0.2: +3%). The engagement ranking's lead over Quality and bridging grows a little over the year in the default world
(1.13 times in the first 30 days, row 12; 1.15 times in the last 95, row 13; v0.2: 1.13 and 1.27), because habit
builds, although the engagement ranking ends with 0.137 fewer of society as users (row 13: -0.155 to -0.118). With
weak habit formation and a strong reputation effect (`habit_gain` 0.01, `reputation_sensitivity` 5,
`harassment_exit` 0.15) the lead starts at 1.07 (row 14) and reverses to 0.60 in the last 95 days (row 15: -0.043,
-0.048 to -0.038; v0.2: 0.82), while the engagement ranking ends with 0.23 fewer of society as users (-0.260 to
-0.207; v0.2: 0.19). Both targets on the trade-off hold, the reversal more strongly than in v0.2.

**What the time cost changed.** The time cost (7.2) took about a quarter of the engagement ranking's engagement and
of its users (v0.2: 0.239 and 0.647), and about a sixth of Quality and bridging's (0.188 and 0.728), whose items are
mostly worth their time. The conclusions it moved, each with its v0.2 value, are marked "moved" below; in short:

- The engagement ranking's engagement lead is smaller. Its engagement index is 1.16 times Quality and bridging's,
  against 1.27 without the time cost, and the lead grows less over the year (above).
- The survey no longer ranks the engagement ranking above Quality and bridging over the year; it reads them as equal.
- The integrity preset no longer costs engagement, and a high account cost no longer measurably keeps people out.
- At the strongest polish Quality and bridging no longer halves the harm index (0.54 of the Engagement ranking's,
  where v0.2 had 0.43).
- Fact 2 now passes on its interval, and three failing worlds of 13.4 fail only with the time cost removed as well.

The comparisons added for the time cost, all against the defaults on the same seeds:

- Without a time cost (`time_cost` 0) the engagement ranking has 30% more engagement (row 33: 0.183 to 0.239), 0.168
  more of society as users (0.479 to 0.647), users' wellbeing 0.080 higher (+0.075 to +0.084) and a share of time
  not worth it 0.056 lower (0.589 to 0.534). Under Quality and bridging the time cost matters less: without it there
  is 19% more engagement (row 34), 0.116 more users and wellbeing 0.051 higher, and time not worth it falls from 0.276
  to 0.043, since few of its items are worth less than nothing. The harm and benefit indices move by at most 0.019.
- Notifications at full strength (`notification_intensity` 1 against the default 0.6) raise the engagement index by
  half under the engagement ranking (row 31: 0.183 to 0.275) and by 46% under Quality and bridging (row 32), and keep
  more people on the platform: 0.078 and 0.062 more of society as users. On the good feed they still raise wellbeing
  now that time costs something: under Quality and bridging users' wellbeing rises by 0.019 (+0.018 to +0.020),
  ever-users' by 0.018 (+0.016 to +0.019) and the benefit index by 0.010, the harm index falls by 0.024, and time not
  worth it does not move (-0.003, -0.007 to +0.000). Under the engagement ranking they leave wellbeing unchanged
  (users +0.000, -0.010 to +0.010; ever-users -0.003, -0.011 to +0.005), raise the benefit index by 0.016 and lower
  time not worth it by 0.053 (-0.069 to -0.037). The time cost prices the time a notification brings, not the
  interruption (section 16), so on a feed worth more than its time an extra session still pays.
- A daily time limit for everyone (`time_limit_share` 1) cuts the engagement index by 3% (row 35: -0.0057, -0.0076
  to -0.0038) and moves nothing else beyond its interval: users' wellbeing +0.002 (-0.004 to +0.007), time not worth
  it +0.001 (-0.013 to +0.015). The limit is each person's time budget (6.10), which typical sessions rarely exceed
  (the held-out target, 13.4), so it binds on few days.

**Findings of v0.1, as they now stand.**

- A stricter classifier now works, which reverses v0.1. At a threshold of 0.3 the toxic share falls by 94% (row 9:
  0.174 to 0.010; v0.1: by 16%). Evasion barely dents the cut and retraining does not carry it: 98% at
  `evasion_rate` 0.05 (row 10) and 95% with `classifier_retraining` 0 (row 11). This round's other changes to
  evasion were not taken apart one by one, so which of them turned the v0.1 result is not measured. The cost moved to
  wrongful sanctions, which the harm index now counts: they rise from 3.3 to 46.6 a day (+43.3, +42.3 to +44.4),
  fourteen times as many, enough to fill their component, so the harm index falls only 25% (0.411 to 0.308). A
  threshold of 0.55 alone cuts the toxic share to 0.45 of the baseline (row 3; v0.2: 0.39) for six times the
  wrongful sanctions (20.4 a day).
- Down-ranking in place of removal (v0.1: strict enforcement with heavy down-ranking caught fewer trolls) was not
  re-measured as such. Its cousin shows in the integrity preset: a demoted Attack still reaches its target (6.9), so
  harassment is 8.24 per 1,000 users under Integrity against 2.67 under the Engagement ranking.
- Bridging gamed by bots no longer reproduces: with every user on the bridging alternative feed, misinformation
  impressions fall under every bot strategy (shared pool 0.153 to 0.061, own side 0.157 to 0.023, adaptive 0.166 to
  0.111; rows 16 to 18, every interval below zero), while bots supply 40% to 44% of the engagement. In v0.1 they rose
  from 0.11 to 0.20 under the shared pool. Bots boost 8 posts a day instead of 20 (13.1), and the bridging feed's
  quality weight now outweighs the inflated bridging score. The adaptive farms still game it the most.
- The v0.1 trap solution (`w_reply` 0, `w_see_less` 5) fails: brigading bots file see-less marks against popular
  out-group posts (3.5), and the toxic share falls only to 0.92 of the baseline (row 5) while the harm index rises
  0.101 (+0.060 to +0.141; v0.2: +0.059, -0.008 to +0.126, now wholly above zero). The see-less penalty alone raises
  the harm index by 0.177 (row 6: +0.100 to +0.254); with `bot_brigading` 0 it lowers it by 0.076 (row 7: -0.088 to
  -0.063). Stated preferences that bots can file backfire at high weight. The new solution is in 13.5. Without any
  cost of toxicity (`churn_base`, `harassment_exit`, `reputation_sensitivity` 0) it costs 7.7% of the engagement
  index (row 8: 0.301 to 0.278), as in v0.2.
- The weak-habit lead reverses more strongly than in v0.1 and v0.2 (0.60 against 0.92 and 0.82 in the last 95
  days).

**Findings of v0.2, as they now stand.**

- Brigading and report trust. Brigading bots file 29% of all reports and see-less marks under the Engagement ranking
  and 62% under Quality and bridging (rows 25, 26; v0.2: 24% and 59%), where the see-less weight invites marks.
  Weighting reports by their reporters' record (`report_trust` 1) does not stop the filing but takes its effect:
  under the Engagement ranking the toxic share falls by 0.025 (row 25: -0.050 to -0.001) and the harm index by 0.031
  (-0.057 to -0.006); under Quality and bridging harassment falls from 4.45 to 1.31 per 1,000 users (row 26: -3.14,
  -3.75 to -2.54), by 71%. It carries the trap's solution: with ten reviews per 1,000 users and `w_reply` 0 but
  without trust, the toxic share falls to 0.57 of the baseline (row 4), short of half; with trust to 0.40 (row 1).
- Polish against the quality ranking. Authors and adaptive bots polish where the ranking reads `Q_hat`: 28% of human
  posts under Quality and bridging against 12% under the Engagement ranking (row 20). Quality and bridging's harm
  index is 0.20 of the Engagement ranking's without polish (row 19), 0.21 at the default gain (row 20), 0.29 at a
  gain of 0.6 (row 21) and 0.54 at the maximum of 1 (`incentive compare --a engagement-maximiser --b
  quality-bridging --set world.polish_gain=1`, 8 seeds: 0.220 against 0.411). Its harm cut so falls from 80% to 46%,
  0.58 of the cut at the default gain (moved: v0.2 fell from 81% to 57%, so Quality and bridging still halved the
  harm at the maximum). The misinformation cut goes faster:
  misinformation impressions are 0.13, 0.17, 0.40 and 1.11 of the Engagement ranking's at those gains, so at the
  maximum Quality and bridging shows as much misinformation as the Engagement ranking (+0.019, -0.009 to +0.047).
  Without its quality weight Quality and bridging has 2.9 times the harm (row 22) and 8.3 times the misinformation.
- The survey is gameable and misleads, though less over the year than in v0.2 (moved). Answers mix wellbeing with
  recent pleasure and habit. Over the year the survey reads the Engagement ranking (0.541 ± 0.008) as level with
  Quality and bridging (0.544 ± 0.005; paired +0.002, -0.003 to +0.007, `incentive compare`, 8 seeds), below the
  subscription (0.552, +0.011, +0.006 to +0.016) and above the chronological feed (0.516) and calm by design
  (0.510), while wellbeing of ever-users ranks the Engagement ranking below all four (0.542 against 0.645, 0.667,
  0.601 and 0.602; paired gap to Quality and bridging +0.104, +0.094 to +0.113). In v0.2 the survey ranked it first
  over the year (0.586 against 0.575 for Quality and bridging). Over 90 days it still does: 0.549 against 0.539 for
  the subscription, 0.530 for Quality and bridging, 0.518 for the chronological feed and 0.512 for calm by design,
  while ever-users' wellbeing ranks it last. Timing games the survey too, though not upward at the defaults:
  collecting every answer right after a session lowers it by 0.077 (row 27: -0.087 to -0.066) at unchanged
  wellbeing, because recent pleasure (a smoothed `tanh(E_imp / 20)`) lies below wellbeing; a platform that maximised
  the survey would keep the lever at 0.
- Account cost's trade-off. Raising it from 0.2 to 1 cuts the bots' share of engagement from 0.209 to 0.075
  (row 24), misinformation impressions by 0.075 and the harm index by 0.086 (-0.107 to -0.064), and the engagement
  index by 0.014 (-0.018 to -0.009; v0.2: 0.029). It no longer measurably keeps people out (moved): the user share
  changes by +0.001 (-0.017 to +0.018), where v0.2 measured -0.038 (-0.057 to -0.020). These runs do not separate
  why.
- Wrongful sanctions in the harm index. Calm by design's classifier threshold of 0.5 wrongly removes 21.9 posts a day
  against the Engagement ranking's 3.3 (row 23: +18.6, +18.1 to +19.2); without polarisation its harm index is 0.062
  above the Engagement ranking's (+0.038 to +0.086), and with it 0.021 below (0.390 against 0.411, -0.049 to
  +0.007). Its misinformation share is also higher (+0.046): hidden like counts, share friction and depth caps cut
  human sharing, so bot-boosted posts carry more of the trending signal (bots supply 40% of its engagement).
- The chronological feed against the realistic ranked feed. Over the first 90 days, the horizon of Guess et al.
  (2023), the chronological feed has 0.86 of the integrity preset's time spent (row 29), 0.37 of its toxic
  impressions, unchanged polarisation (-0.000, -0.001 to +0.000), and 0.79 of its misinformation impressions, where
  the field found more; fact 3 records this as a known limit (13.4, section 16). Over the year the chronological
  feed's polarisation is 4.4% below the integrity preset's (row 30: -0.019, -0.024 to -0.014) and 15% below the
  Engagement ranking's (row 28: -0.076, -0.085 to -0.067). That year-long effect is a model prediction, flagged as
  such: field experiments lasted three months and found none.
- Integrity measures pay off in the model except for harassment: against the Engagement ranking the integrity preset
  has 43% of the toxic share (0.075 against 0.174), two thirds of the misinformation (0.107 against 0.166), a harm
  index of 0.361 against 0.411, and now the same engagement index (0.184 against 0.183; moved: v0.2 had 91%),
  because it keeps more users (0.586 of society against 0.479).

### 13.4 Stylized facts

`incentive facts`: 8 paired seeds, `N = 2,000`, 365 days; the same checks run in `crates/incentive-core/tests/facts.rs`
on 4 seeds at `N = 600` for 120 days (240 days for fact 5, `N = 1,500` and 365 days for fact 6), where they judge only
the seed means, as regression checks of direction. Effects are paired mean differences, means or ratios of means, ± the
standard deviation of the per-seed effect. The interval is the 95% t-interval of the per-seed effect (of the per-seed
ratios for a ratio, of the signed relative differences for the polarisation check); the verdict is pass when the
interval lies inside the band, fail when it lies wholly outside, undecided otherwise. A failing world is a world
assumption (via `incentive facts --set`) in which the check fails at full size; section 15 explains each.

| Fact | Check | Effect | Interval | Band | Verdict |
| --- | --- | --- | --- | --- | --- |
| 1 | Outrage impressions, ER minus chronological | 0.390 ± 0.036 | 0.360 to 0.421 | 0.15 to 0.6 | pass |
| 1 | Reflective value per impression, chronological minus ER | 0.159 ± 0.029 | 0.134 to 0.183 | 0.05 to 0.4 | pass |
| 2 | Rise of the Outrage share of posts under ER | 0.021 ± 0.009 | 0.013 to 0.029 | 0.01 to 0.15 | pass |
| 2 | That rise minus the rise with `learning_rate` 0 | 0.019 ± 0.009 | 0.011 to 0.026 | 0.01 to 0.15 | pass |
| 3 | Time spent, chronological over ER | 0.617 ± 0.023 | 0.599 to 0.637 | 0.4 to 0.9 | pass |
| 3 | Relative difference in polarisation, days 80 to 90 | 0.032 ± 0.007 | -0.038 to -0.027 | < 0.05 | pass |
| 3 | Toxic, days 30-90, chrono over Integrity | 0.312 ± 0.082 | 0.242 to 0.380 | 0.5 to 0.9 | fail (limit) |
| 3 | Misinformation, days 30-90, chrono over Integrity | 0.702 ± 0.094 | 0.623 to 0.779 | 1.05 to 3 | fail (limit) |
| 4 | Largest cascades with `reshare_depth_cap` 1 over without | 0.715 ± 0.049 | 0.676 to 0.758 | 0.3 to 0.85 | pass |
| 5 | Vulnerable users who left, Troll share 0.12 minus 0.03 | 0.078 ± 0.031 | 0.052 to 0.104 | 0.03 to 0.3 | pass |
| 6 | Break group's change, break minus placebo, Integrity | 0.054 ± 0.048 | 0.013 to 0.094 | 0.02 to 0.25 | undecided |

Of the nine required checks eight pass on their intervals and one, fact 6, is undecided; all nine hold on the seed
means. Both known limits fail ("limit" in the table). In v0.2, before the time cost, seven passed and fact 2's rise
was undecided (0.015, 0.009 to 0.021). Supplementary, not a verdict: on 16 seeds (`incentive facts --fact
deactivation --seeds 16`, seeds 1 to 16) fact 6's effect is 0.054 ± 0.053 with an interval of 0.025 to 0.082,
inside the band. Every fact is judged on 8 seeds, so the verdict in the table stands.

The worlds in which each check fails at full size (8 seeds, `incentive facts --set ...`), with the 95% interval and
verdict there:

| Fact | Check | World | Effect there | Interval | Verdict |
| --- | --- | --- | --- | --- | --- |
| 1 | Outrage impressions | `outrage_appeal` 0, `arousal_appeal` 0, `time_cost` 0 | 0.081 | 0.072 to 0.090 | fail |
| 1 | Outrage impressions (upper bound) | `outrage_appeal` 2.5 | 0.639 | 0.619 to 0.658 | fail |
| 1 | Reflective value | `outrage_appeal` 0, `arousal_appeal` 0, `time_cost` 0 | -0.044 | -0.056 to -0.033 | fail |
| 2 | Rise | `outrage_appeal` 0.6 | -0.020 | -0.026 to -0.013 | fail |
| 2 | Rise minus no learning | `outrage_appeal` 0.6 | -0.008 | -0.014 to -0.003 | fail |
| 3 | Time spent | `habit_gain` 0, `stop_bias` -5 | 0.967 | 0.938 to 0.998 | fail |
| 3 | Polarisation at day 90 | `polarisation_rate` 0.0005 | 0.101 | -0.111 to -0.090 | fail |
| 3 | Toxic impressions against Integrity | the defaults | 0.312 | 0.242 to 0.380 | fail |
| 3 | Toxic impressions against Integrity | `outrage_appeal` 2.5 | 0.171 | 0.127 to 0.212 | fail |
| 3 | Misinformation against Integrity | the defaults | 0.702 | 0.623 to 0.779 | fail |
| 4 | Cascades | `outrage_appeal`, `arousal_appeal`, `social_proof`, `time_cost` 0 | 0.860 | 0.851 to 0.869 | fail |
| 5 | Vulnerable who left | `harassment_harm`, `harassment_exit`, `time_cost` 0 | 0.023 | 0.007 to 0.038 | undecided |
| 6 | Break minus placebo | `time_cost` 0 | -0.267 | -0.323 to -0.212 | fail |
| 6 | Break minus placebo (upper bound) | `time_cost` 0.225 | 0.391 | 0.337 to 0.445 | fail |

The time cost moved three of v0.2's failing worlds back into the band or across it, so they now include `time_cost` 0,
which gives back v0.2's effects there to the last digit. At the default time cost, without arousal or in-group
hostility, fact 1's difference in outrage impressions is 0.147 (0.139 to 0.154, undecided) and its reflective value
gap 0.083 (0.071 to 0.094, pass); with bots boosting 20 posts a day, fact 2's rise is 0.017 (0.001 to 0.032) and the
rise beyond no learning 0.007 (-0.003 to 0.017), both undecided; without anything impulsive behind sharing, fact 4's
ratio is 0.870 (0.843 to 0.897, undecided). Fact 5 fails in no world tried on its interval, only on the seed means:
with harassment harmless at the default time cost the difference is 0.041 (0.016 to 0.065), with
`reputation_sensitivity` 0 as well 0.032 (0.018 to 0.045), and only with `time_cost` 0 as well does it fall below the
band's 0.03. In v0.2 the same world gave the same undecided 0.023. Why the time cost keeps facts 1 and 4 alive in
worlds without impulse was not taken apart; one candidate is that it shrinks the engagement ranking's audience, so
bots supply more of its engagement (0.209 against 0.171 in v0.2).

Fact 2 now passes at full size: the Outrage share of posts rises by 0.021 (0.013 to 0.029) from the first fifth of
the year to the last (0.236 to 0.257) and by 0.019 more than without learning; it still rises fast and then gives a
little ground to Clickbait (13.3). At the tests' 120 days (`N = 600`, 4 seeds) the rise is 0.064 and its difference
from no learning 0.069. Fact 1 against the chronological feed is ninefold in the model (0.451 against 0.049 outrage
impressions); its field magnitude is checked in fact 3 against the integrity preset. The depth cap shrinks the
largest cascades by about a quarter, not more, because trending recommendations expose original posts at depth 0 and
the cap only stops the second hop.

**Held-out target.** Named before tuning and not tuned against: under the default design, the regretted share of
time (items beyond a person's time budget, 9.2) should lie between 0.20 and 0.42, around the 31% of social-media use
that Allcott, Gentzkow and Song (2022) attribute to self-control problems. It still fails by an order of magnitude
after the time cost: 0.025 ± 0.002 (8 seeds, summary window; v0.2: 0.029). Typical sessions of about 20 items, opened
on some days only, stay under budgets of 15 to 40 items, so few items count as regretted. The two measures differ (a
fixed daily budget against the use people would cut if they could commit), but the gap says the model prices
compulsive use far lower than the field does (section 16). The time cost added a different measure, the share of time
on items worth less than their time (`low_value_time`, 9.2: 0.589 under the default design); it was defined after
this target failed, prices value rather than self-control, and is not a test of the target.

### 13.5 Scenarios

Thresholds as in 14.2. The misinformation surge asks for +0.03 (the best designs found reach about +0.04, because
non-users are only half pulled toward their user ties), and the public square for at least 10% less polarisation.
The engagement trap gained a third goal in v0.2, at most twice the baseline's wrongful removals and demotions: with
evasion fading and the classifier retraining, a stricter classifier alone solved the first two goals in v0.2 (a
threshold of 0.55, row 3 of 13.3) at six times the wrongful sanctions; with the time cost its toxicity goal is
undecided (below). The misinformation surge's Clickbait-heavy bots are `population.bot_clickbait_share` 0.9. Known
solutions, as changes to the scenario's starting design, at full size on 8 paired seeds (`incentive scenario <id>
--solution --seeds 1..8`, the same runs as the report's scenario table), with the goal margins' 95% intervals. Every
known solution still passes with the time cost, unchanged:

| Scenario | Known solution | Against the baseline (verdict: pass) |
| --- | --- | --- |
| Engagement trap | `w_reply` 0, `review_capacity` 10, `report_trust` 1 | engagement 1.14x, toxic 0.40x, FP 0.98x |
| Toxicity spiral | `review_capacity` 10, `account_cost` 1, `classifier_threshold` 0.3 | harassment 0.0002x, DAU 0.98x |
| A living for creators | `payout_scheme` FanFunded | living 0.34 against 0.12, harmful payouts 0.7% |
| Misinformation surge | Quality and bridging ranking with notes, `account_cost` 1 | non-users' informedness +0.046 |
| Public square | `w_reply` 0, `w_bridging` 0.6 | polarisation 0.83x, engagement 1.18x |

Margins: the trap's engagement +0.053 (+0.048 to +0.057), toxic share +0.018 (+0.007 to +0.029; v0.2: +0.027),
wrongful sanctions +3.36 (+2.89 to +3.83); the spiral's harassment +4.43 (+4.03 to +4.84) and actives +119 (+111 to
+127); creators +0.217 (+0.204 to +0.230) and payouts +0.193 (+0.191 to +0.196); the surge's +0.016 (+0.015 to
+0.017) above its +0.03; the square's polarisation +0.033 (+0.027 to +0.040) and engagement +0.070 (+0.064 to
+0.076); FP in the table are the wrongful removals and demotions. The trap's toxicity margin is the thinnest, a fifth
of its threshold. Naive attempts fail, on the same 8 seeds: the chronological feed loses the trap's engagement goal
(0.115 against a threshold of 0.156; margin -0.040, -0.045 to -0.036); `w_reply` 0 alone keeps 0.75 of the toxic
share (row 2; margin -0.044, -0.057 to -0.030); a threshold of 0.55 alone breaks the wrongful-sanction goal (20.4
against 6.6) and leaves the toxicity goal undecided (0.45 of the baseline; margin +0.009, -0.010 to +0.028); and the
public square fails with `w_reply` 0 alone (polarisation 0.95x; margin -0.025, -0.029 to -0.021) and is undecided
with `w_bridging` 0.6 alone (0.90x; margin -0.002, -0.006 to +0.002). The tests in
`crates/incentive-core/tests/scenarios.rs` check every known solution at `N = 600` on six seeds (three left the trap's
toxicity interval straddling the threshold).

### 13.6 Run time

One run of 365 days at `N = 2,000` takes 2.9 seconds single-threaded in release on the owner's desktop
(`incentive run --seeds 1`, wall time including the CSV export; 3.5 before the time cost, which leaves fewer users).
`incentive report` at full size, now with 35 comparisons (872 runs, 504 distinct), takes about 3.5 minutes (205
seconds) on 8 CPUs. The core and CLI test suites run in about 33 seconds under the dev profile on 8 threads, compile
excluded: the fact tests take 19 seconds, almost all of it fact 6 (`N = 1,500`, 365 days), and the scenario tests
take 12 seconds.

### 13.7 Exploration

`incentive explore` searched every design lever at once (37 genes: one per lever, two for the reshare depth cap) with
the world assumptions at their defaults, the time cost included (7.2). Each candidate ran on seeds 1 and 2 at `N = 600`
for 180 days, and its metrics are the mean of the two summaries. After each search every design in its file was rerun at
the same size on the fresh seeds 9 to 16 (`--confirm-seeds`, the default), and the file keeps those confirmed metrics
beside the search's. The four searches behind the app's Atlas tab, with search seed 1 and a mutation rate of 0.1 per
gene:

| Search | Library algorithm | Generations x batch | Designs simulated | Confirmed | Wall time |
| --- | --- | --- | --- | --- | --- |
| Goodhart, `engagement_index` | `SimpleGA`, elitism 2, from 40 random designs | 25 x 40 | 947 | 18 | 167 s |
| Goodhart, `survey_satisfaction` | the same | 25 x 40 | 947 | 21 | 146 s |
| Map | `MapElites`, 16 x 16 grid, 128 initial designs (the presets first) | 50 x 64 | 3,129 | 156 | 398 s |
| Pareto front | `Nsga2`, population 64 (the presets and random designs) | 40 x 64 | 2,623 | 64 | 463 s |

That is 7,646 designs and 15,292 search runs, plus 2,072 confirmation runs, in 20 minutes on 8 CPUs; the confirmation
takes 10 to 70 seconds of each search. The exact commands are in the README. The searches start from the presets except
`max-engagement` (14.1), which comes from the Pareto search itself: the initial designs leave it out by its id, so its
levers never reach a search, and the commands regenerate these files byte for byte whatever the preset holds (checked by
running small map and Pareto searches before and after the preset changed: the files were identical). The map's axes are
the engagement index from 0.04 to 0.36 and the harm index from 0 to 0.64, in cells of 0.02 by 0.04; values outside fall
into the edge cells.

The reduced size changes the numbers. On seeds 9 to 16 the engagement ranking scores engagement 0.195, harm 0.332 and
benefit 0.517 at `N = 600` over 180 days, against 0.179, 0.415 and 0.415 at full size: a shorter run has less time for
harm and churn to build up, and with the time cost churn takes engagement too. The engaging front designs move the other
way and gain engagement at full size (front design 61: 0.353 confirmed, 0.382 at full size). Every number below that is
not marked as a search or confirmed value comes from full-size runs (`N = 2,000`, 365 days) on seeds 9 to 16, which no
search selected on: `incentive config --from-atlas <file> --design <n>` turned each design into a configuration, and
`incentive run --population 2000 --days 365 --seeds 9..16` ran it; differences are paired by seed, with 95% intervals,
as `incentive compare` computes them.

**The most engaging design found** is front design 61 of the Pareto search, now the preset `max-engagement` (14.1):
engagement 0.382 at full size, 2.14 times the engagement ranking's 0.179 (+0.204, +0.200 to +0.208). The six front
designs with the highest confirmed engagement (63, 58, 64, 60, 62 and 61) were rerun at full size, and design 61 had the
highest mean; designs 62 (-0.0015, -0.005 to +0.002) and 63 (-0.003, -0.008 to +0.001) cannot be told apart from it. The
top of the front is flat at full size: six front designs lie within 0.006 of one another (0.379 to 0.385), and the
highest mean belongs to design 51, thirteenth on its confirmed engagement and so outside the six, at +0.003 (-0.003
to +0.009) over design 61, which is no difference. The final design of the engagement Goodhart search
reaches 0.330, 0.052 less (-0.062 to -0.043). Design 61 weighs likes (3.0 against 1.0) and dwell (2.7 against 0.5) far
above the engagement ranking and shares far below it (0.2 against 2.0), shows half the recommended posts from strangers
(20% against 40%), pushes notifications at full strength and lets blocks filter nothing (`block_effectiveness` 0). Its
classifier removes at 0.75, a little laxer than the engagement ranking's 0.7; nine reviews per 1,000 users a day,
weighted by reporters' records (`report_trust` 0.7), remove reported posts but carry no sanctions. Against the
engagement ranking it has 14% less harm (0.356 against 0.415, -0.058, -0.092 to -0.025), the same toxic share (0.166
against 0.174, -0.008, -0.034 to +0.018) and the same polarisation (0.500 against 0.493), users' wellbeing 0.020 higher
(0.553 against 0.533, +0.004 to +0.037), a share of time not worth it 0.108 lower (0.479 against 0.588, -0.151
to -0.065) and 0.204 more of society as users (0.675 against 0.471). Its benefit index is higher (+0.060, +0.051
to +0.068) only because it pays creators: without the creators term it is 0.050 lower (-0.063 to -0.037). On seeds 1
to 8 the report gives it 0.382 ± 0.006 engagement and 0.359 ± 0.066 harm (13.3, column Max).

**Lowest harm at a given share of that engagement.** Of the 64 front designs, rerun at full size:

| Share of design 61's engagement | At least | Lowest harm on the front | Design | Its engagement | Confirmed values |
| --- | --- | --- | --- | --- | --- |
| 80% | 0.306 | 0.048 | front design 32 | 0.312 | 0.126 at 0.286 (design 35) |
| 90% | 0.344 | 0.168 | front design 42 | 0.355 | 0.191 at 0.337 (design 51) |
| 100% | 0.382 | 0.217 | front design 51 | 0.385 | 0.368 at 0.357 (design 63) |

The last column applies the same shares to the confirmed values at the search's size, where design 63 is the most
engaging (0.357). The trade-off no longer steepens at the top. Equal engagement is available at 39% less harm than
design 61's (design 51: engagement +0.003, -0.003 to +0.009; harm -0.139, -0.157 to -0.121); giving up a tenth of the
engagement removes 53% of design 61's harm (design 42: -0.027, -0.034 to -0.021; harm -0.188, -0.204 to -0.173), and
giving up a fifth removes 86% (design 32: -0.070 and -0.308). From design 32 to design 42 harm rises by 0.120 for 0.043
of engagement, from design 42 to design 51 by 0.049 for 0.030. The front holds harm under 0.10 up to an engagement
of 0.312 (design 32) and under 0.15 up to 0.335 (design 36, harm 0.107). Of the 64 front designs, 49 beat the engagement
ranking on engagement, harm and benefit at once at full size (44 on their confirmed values).

**High engagement with low harm, against the most engaging design.** Front design 32, the most engaging front design
with harm under 0.10, is the clearest case: at full size

| Design | Eng. | Harm | Benefit | No creators | Toxic | Wellbeing | Ever-users | Low value | Revenue | Wrongful |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Engagement ranking | 0.179 | 0.415 | 0.415 | 0.519 | 0.174 | 0.533 | 0.543 | 0.588 | 10.7 | 3.2 |
| Most engaging found (61) | 0.382 | 0.356 | 0.475 | 0.469 | 0.166 | 0.553 | 0.553 | 0.479 | 43.2 | 1.8 |
| Front design 51 | 0.385 | 0.217 | 0.470 | 0.521 | 0.101 | 0.622 | 0.619 | 0.337 | 60.8 | 1.6 |
| Front design 42 | 0.355 | 0.168 | 0.545 | 0.546 | 0.095 | 0.674 | 0.670 | 0.264 | 33.6 | 3.7 |
| Front design 36 | 0.335 | 0.107 | 0.577 | 0.560 | 0.049 | 0.691 | 0.687 | 0.266 | 41.5 | 3.7 |
| Front design 32 | 0.312 | 0.048 | 0.595 | 0.556 | 0.004 | 0.694 | 0.689 | 0.251 | 40.9 | 1.0 |
| Goodhart, engagement, final | 0.330 | 0.612 | 0.317 | 0.380 | 0.305 | 0.429 | 0.454 | 0.718 | 21.9 | 0.0 |
| Goodhart, survey, final | 0.327 | 0.361 | 0.495 | 0.485 | 0.269 | 0.603 | 0.606 | 0.428 | 64.0 | 0.0 |

(Benefit without the creators term, wellbeing of users and of ever-users, the share of time not worth it
(`low_value_time`, 9.2), net revenue, wrongful removals and demotions a day.) Against design 61, design 32 gives up 18%
of the engagement (-0.070, -0.077 to -0.064) for 86% less harm (-0.308, -0.329 to -0.286), a fortieth of the toxic
share, 0.144 less polarisation, 0.141 more wellbeing of users (+0.129 to +0.153), 0.228 less time not worth it (-0.252
to -0.204) and 0.039 more of society as users. Its benefit lead over design 61 (+0.120, +0.109 to +0.131) does not rest
on creators, since design 61 pays them too: without the creators term the lead is +0.087 (+0.079 to +0.095). Against the
engagement ranking it has 74% more engagement (+0.133, +0.127 to +0.140) and 88% less harm (-0.366, -0.401 to -0.331);
the creators term supplies most of its benefit lead there: +0.180 (+0.172 to +0.187) with creators, +0.037 (+0.024
to +0.049) without. Meaningful interactions nearly vanish (0.002 a user against 0.060), as on most of the engaging front
designs; the front's benefit comes from the other components. Net revenue is four times the engagement ranking's (40.9
against 10.7) although design 32 pays half its revenue to creators, because 92% of its revenue comes from subscriptions,
which users buy when the platform is worth it to them. It keeps the engagement ranking's predictor quality, share of
recommended posts and graduated sanctions, removes at 0.75 and wrongly removes or demotes 1.0 post a day against 3.2:
its harm cut rests on neither a strict classifier nor a predictor at the cap.

**What front design 32 rests on.** `incentive factorial --a engagement-maximiser --b front-32.json --levers
notification_intensity,w_dwell,w_bridging,w_in_network,payout_share,ads_share --population 2000 --days 365` (64 cells,
seeds 9 to 16; ± is the 95% interval) sets every subset of six of its levers back to the engagement ranking's values:
notifications 1.0 (ranking 0.6), `w_dwell` 2.6 (0.5), `w_bridging` 1.8 (0), `w_in_network` 1.0 (0.2), 50% of revenue
paid to creators (0) and `ads_share` 0.35 (1). The six were chosen by setting each of the 19 levers in which design 32
differs from the engagement ranking back one at a time: they had the largest effects on engagement, harm, benefit or
users' wellbeing; review capacity (-0.020 harm alone), account cost and the like weight came next. With all six back the
design keeps engagement 0.212, harm 0.304, benefit 0.432 and users' wellbeing 0.575: still 19% more engagement than the
engagement ranking (+0.034, +0.029 to +0.038) with 27% less harm (-0.110, -0.135 to -0.085), a little more benefit
(+0.017, +0.010 to +0.023) and more wellbeing (+0.043, +0.033 to +0.052). The six levers carry 0.100 of its engagement
lead of 0.133, 0.256 of its harm cut of 0.366, 0.163 of its benefit lead of 0.180 and 0.119 of its wellbeing lead
of 0.161. Main effects (from the engagement ranking's value to design 32's) and Shapley values:

- Engagement: notifications at full strength carry 92% of the six levers' gain (main effect +0.091 ± 0.003,
  Shapley +0.092 ± 0.003) and dwell weight 34% (+0.035 ± 0.002), while bridging (-0.010 ± 0.003) and the in-network
  weight (-0.015 ± 0.002) cost engagement alone. The weights work together: dwell with bridging +0.034 ± 0.003, bridging
  with the in-network weight +0.023 ± 0.003, notifications with dwell +0.015 ± 0.002, notifications with the in-network
  weight -0.009 ± 0.002. So notifications supply about 70% of the design's engagement lead over the engagement ranking
  (0.092 of 0.133); the rest comes from the ranking weights.
- Harm: bridging carries 61% of the six levers' cut (Shapley -0.157 ± 0.020), the in-network weight 25% (-0.064
  ± 0.007), notifications 15% (-0.038 ± 0.003) and dwell 6% (-0.016 ± 0.003), while payouts add harm alone (+0.020
  ± 0.007, -8%). The largest interactions: bridging with payouts -0.077 ± 0.016 (payouts add harm under the engagement
  ranking's weights and lower it under strong bridging), bridging with the in-network weight +0.051 ± 0.008 (each does
  part of the other's work), dwell with the in-network weight +0.032 ± 0.005, notifications with the in-network
  weight +0.020 ± 0.005 and with bridging +0.017 ± 0.005.
- Benefit: payouts 57% (+0.093 ± 0.005), the subscription share 27% (+0.044 ± 0.004), bridging 13%, the in-network
  weight 13%, notifications 12% (+0.019 ± 0.002), dwell -21% (-0.035 ± 0.006); payouts with subscriptions +0.083
  ± 0.007, since a subscription business has more revenue to pay out.
- Users' wellbeing (decomposed from the same runs): bridging 36% (+0.043 ± 0.010), the in-network weight 29% (+0.034
  ± 0.005), notifications 19% (+0.022 ± 0.002), fewer ads 9% (+0.011 ± 0.002), dwell 8% (+0.009 ± 0.002) and payouts 0%
  alone, but bridging with payouts +0.031 ± 0.007. Notifications also lower the share of time not worth it (main
  effect -0.054 ± 0.005).

So design 32 rests on its ranking for harm and wellbeing (a strong bridging weight and a feed of followed accounts that
cut toxicity and wasted time), on notifications for engagement, and on payouts and subscriptions for benefit.
Notifications still help it on every count, as they help Quality and bridging (13.3, row 32): its feed is worth its
time, so the sessions they bring add more than they cost. The other designs were not taken apart this way.

**What notifications do on the front.** Every one of the 64 front designs keeps notifications at 0.9 or more (62 at full
strength), as both final Goodhart designs do; the time cost moved no part of the front off them. Setting notifications
back to 0.6 in three front designs, with the rest as found, gives the effect of full strength (± is the 95% interval):

| Design | Engagement | Harm | Users' wellbeing | Time not worth it |
| --- | --- | --- | --- | --- |
| Front design 61 | +0.125 ± 0.004 | -0.023 ± 0.018 | +0.003 ± 0.013 | -0.046 ± 0.024 |
| Front design 51 | +0.116 ± 0.005 | -0.049 ± 0.018 | +0.022 ± 0.010 | -0.063 ± 0.022 |
| Front design 32 | +0.099 ± 0.004 | -0.008 ± 0.005 | +0.023 ± 0.002 | -0.007 ± 0.003 |

The front agrees with the report's rows 31 and 32. Notifications raise engagement by 43% to 49% (design 61 without them
would have 0.257, still 1.44 times the engagement ranking's). Where the feed is mostly worth its time (designs 32
and 51, time not worth it 0.25 and 0.34) they raise users' wellbeing by about 0.02, as under Quality and bridging
(+0.019); on the most engaging design, whose share of time not worth it (0.48) lies nearer the engagement ranking's
(0.59), they leave wellbeing unchanged, as under the engagement ranking (+0.000). They lower harm in all three. The time
cost thus took the free lunch away where the feed wastes time, but not on the front's best designs, and the model still
charges nothing for the interruption itself (section 16).

**The winner's curse.** Confirmed values (seeds 9 to 16) against search values (seeds 1 and 2), both at the search's
size, as mean differences. The seed sets differ too: the seven presets, which no search selected, lose 0.007
engagement, 0.017 harm, 0.004 benefit and 0.027 survey satisfaction from seeds 1 and 2 to seeds 9 to 16. Against that
shift:

- The 156 map elites lose 0.005 engagement (in 79% of them), 0.010 harm and 0.012 benefit, 0.008 more benefit than the
  shift: the map as a whole is inflated little. The 16 elites with the highest benefit lose 0.001 benefit (8 of 16);
  the 16 with the lowest harm gain 0.004 harm (higher in 11 of 16), 0.021 worse than the shift, and lose 0.024 benefit
  (15 of 16), 0.020 more than the shift.
- The 64 front designs lose 0.011 engagement (88% of them), 0.004 beyond the shift, 0.018 harm (the shift) and 0.004
  benefit (the shift). The six most engaging front designs lose 0.023 engagement on average (all six), 0.017 more than
  the shift, about 6% of their search value; their harm falls by 0.044 and their benefit rises by 0.025, because
  selection for engagement also picked draws with high harm and low benefit.
- The final engagement Goodhart design falls from 0.367 to 0.329 (-0.038, 0.031 beyond the shift), the final survey
  design from 0.683 to 0.639 (-0.044, 0.017 beyond the shift).

A search value is thus optimistic by 0.01 to 0.03 in whatever the search pushed on hardest; ranks hold better than
levels, though not at the flat top of the front, where full size reorders designs that differ by less than 0.01.

**Goodhart runs.** Maximising the engagement index alone raised the best design's engagement from 0.179 to 0.367 over 25
generations (search values). At full size, generation 0's best against the final design: engagement 0.178 to 0.330
(+0.152, +0.142 to +0.161), harm 0.087 to 0.612 (+0.524, +0.488 to +0.561), toxic share 0.011 to 0.305,
polarisation 0.384 to 0.568, users' wellbeing 0.688 to 0.429 (-0.259, -0.293 to -0.224), time not worth it 0.199
to 0.718, benefit 0.557 to 0.317 and misinformation 0.05 to 0.19. The search started from a random design without a
classifier, never switched it on, dropped human review (0.2 per 1,000 users from generation 2, none at the end), pushed
notifications to full strength from generation 1, showed no recommended posts and weighted likes (2.5), shares (3.0),
dwell (3.0) and followed accounts (1.9). The survey rose with it, from 0.520 to 0.586 (+0.066, +0.053 to +0.080), while
wellbeing fell four times as far. The final design is worse than the engagement ranking on harm, toxicity, polarisation,
benefit and wellbeing, and below design 61 on engagement (-0.052): it keeps users' attention but not the users (0.518 of
society against design 61's 0.675).

Maximising survey satisfaction alone raised it from 0.526 to 0.683 (search values). At full size: survey 0.513 to 0.647
(+0.135, +0.125 to +0.144), engagement 0.118 to 0.327, benefit 0.437 to 0.495 and 0.163 more of society as users, but
harm rose from 0.165 to 0.361 (+0.195, +0.151 to +0.240) and the toxic share from 0.011 to 0.269 (+0.258, +0.213
to +0.303), while users' wellbeing did not move (0.618 to 0.603, -0.015, -0.038 to +0.009). The search kept the
classifier off (from generation 0), dropped sanctions, kept seven reviews per 1,000 users and pushed notifications to
full strength by generation 4. Against the engagement ranking the final design has more engagement (0.327
against 0.179), less harm (0.361 against 0.415, -0.054, -0.088 to -0.020), more benefit (+0.080) and more wellbeing
(+0.071), but a higher toxic share (0.269 against 0.174, +0.095, +0.058 to +0.133). It does not exploit the timing:
`survey_after_session` stayed at 0 in all 25 generations' best designs, and setting it to 1 in the final design lowers
the survey by 0.056 (-0.072 to -0.040) with every truth metric unchanged. It exploits the pleasure and habit in the
answers through notifications: at full strength instead of 0.6 they raise the survey by 0.034 (+0.030 to +0.038) and
users' wellbeing by 0.002 (-0.010 to +0.015), so all of that gain is pleasure and habit. Where the rest of its survey
gain comes from, with wellbeing flat and toxicity rising, these runs do not separate. The survey stays a better proxy
than engagement, but it is no check on harm: maximising it raised harm.

**What changed from v0.2.** The only change to what a run does is the time cost (7.2), so every difference here comes
from it.

- The most engaging design costs no harm. v0.2's most engaging design (front design 64) had 1.81 times the engagement
  ranking's engagement with 31% more harm, twice the toxic share and lower wellbeing. Design 61 has 2.14 times the
  engagement ranking's (which the time cost lowered from 0.237 to 0.179) with 14% less harm, the same toxic share and
  slightly higher wellbeing. It also wastes less of its users' time than the engagement ranking (time not worth it
  0.479 against 0.588) and keeps 0.204 more of society on the platform; it differs from design 64 in its classifier
  (it removes at 0.75, where design 64 flagged nothing) and its ranking weights as well, so these runs do not separate
  why. Its own engagement is lower than design 64's was (0.382 against 0.429).
- The coexistence trade got better. v0.2's design 36 gave up 14% of the most engaging design's engagement for 82% of its
  harm; design 32 gives up 18% for 86%, and design 51 gives up nothing for 39%. Where v0.2's trade-off steepened at the
  top (the last tenth of engagement cost harm at three times the rate of the tenth before), now it does not. Design 32
  shares v0.2's design 36's notifications, payouts and review weighted by trust, but cuts harm by bridging and a feed of
  followed accounts rather than by few recommended posts and a predictor at the cap.
- Notifications. v0.2's design 36 drew all of its six levers' engagement from notifications (102%), and they helped harm
  and benefit because time was free. On design 32 they carry 92% of the six levers' engagement and still help harm,
  benefit and wellbeing, because its feed is worth its time; on the most engaging design they no longer raise wellbeing.
- The map filled 156 of 256 cells (61%, v0.2: 169), with engagement from 0.06 to 0.33 and harm from 0.03 to 0.69 (search
  values); v0.2 reached 0.39. Its high-engagement edge moved up the harm axis: its three elites above an engagement
  of 0.30 all have harm of 0.41 or more, and its lowest confirmed harm at a confirmed engagement of at least 0.25
  is 0.32 (v0.2: 0.11 at 0.27), where the front holds 0.07. The map's fitness is benefit, so it keeps designs that pay
  creators over designs that do little harm; benefit across its cells follows the share of Creators earning a living
  even more closely than in v0.2 (correlation 0.85, v0.2: 0.76).
- The engagement Goodhart search ended lower (0.330 at full size against 0.407) and further below the most engaging
  design (-0.052 against -0.022), because a feed worth less than its time drains its own users. The survey search, which
  in v0.2 cut harm (0.220 to 0.161), now raises it (0.165 to 0.361), and the notifications it learned add no wellbeing
  to the survey's gain, where in v0.2 they added half.

**Limits of these results.** One world (the defaults), one search seed per search, two seeds per candidate and a reduced
size: the front designs gain engagement at full size (design 32: 0.273 confirmed, 0.312 at full size), so the search
size understates what the high-engagement designs reach, and it reorders the flat top of the front. "Most engaging
found" is what these searches found, not a bound: six designs tie at full size, design 61 by the rule above, and a
longer search may find more. The factorial covers six levers of one design and leaves review capacity out. The time cost
is one value of time for everyone, calibrated on the integrity preset (section 16); notifications cost only the time
they bring and not the interruption, which still flatters the lever that carries design 32's engagement. Net revenue
rests on the subscription price and ad rate, which are not calibrated (11). Levers without effect in a design (the
alternative feed when nobody uses it, the classifier's settings when it is off, the payout scheme when nothing is paid)
drift freely during a search; the reports and the Atlas tab leave them out when they list how a design differs.
Rerunning with another `--config` tests whether a finding survives a different world.

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
- Most engaging design found (`max-engagement`): front design 61 of the Pareto atlas, the most engaging design the
  searches found (13.7), with every lever as found: `w_like` 3.0, `w_reply` 1.0, `w_share` 0.2, `w_dwell` 2.7,
  `w_bridging` 0.1, `w_see_less` 1.6, `w_recency` 0.1, `w_in_network` 0, `recommended_share` 0.2, `diversity` 0.15,
  `classifier_threshold` 0.75, `review_capacity` 9, `report_trust` 0.7, `sanctions` None, `payout_share` 0.15 with
  PerView payouts, `payout_threshold_followers` 42, `block_effectiveness` 0, `notification_intensity` 1.0,
  `account_cost` 0.05, `ads_share` 0.5 and `survey_after_session` 0.9; `demote_penalty` 2.1 and `noted_penalty` 2.4
  have no effect in it (its classifier removes, and it has no community notes). No one proposes it: it shows what
  maximising engagement looks like in this model, now that time spent has a price. It has about twice the
  Engagement ranking's engagement for 13% less harm, the same toxic share and slightly higher wellbeing (13.3, column
  Max); it also wastes less of people's time and keeps more of them on the platform, and front designs as engaging as
  it have less harm still (13.7). `incentive explore` leaves it out of its initial designs, so the searches it comes
  from regenerate the same atlas.

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

Each fact becomes an automated test on small runs (`N` = 600, 120 days, 4 seeds, paired by seed; fact 5 runs 240 days,
and fact 6 runs at `N` = 1,500 for 365 days) and a check at full size in the CLI's `facts` command, which prints the
effect sizes. Each check is a band: a lower bound with a margin that a world without the mechanism misses, and an upper
bound where an effect could be implausibly large. `incentive facts --set path=value` checks the facts in another world;
for each fact below a world is named in which it fails, so the check is not true by construction. Two qualifications:
the lower bounds of facts 1 and 4 fail only in worlds that also set `time_cost` to 0, a world fact 6 rejects, and
fact 5's world misses its band only on the seed means, with an undecided interval. Effects at the defaults are in 13.4.

1. Engagement ranking amplifies divisive content that users do not value. Against a chronological feed, the
   Engagement ranking shows more Outrage impressions (paired difference of the share over the second half of the run
   between 0.15 and 0.6), and the reflective value of what it shows is lower (between 0.05 and 0.4 per impression).
   Milli et al. (2025) found in an audit of Twitter's ranking that the engagement-based algorithm amplified
   emotionally charged, out-group hostile content that users did not prefer. Fails when people are not drawn by
   arousal or in-group hostility and time is free (`outrage_appeal`, `arousal_appeal` and `time_cost` 0: difference
   0.081, reflective value -0.044; with the default time cost the difference is undecided at 0.147 and the reflective
   value passes, 13.4) and, at the upper bound, when hostility pulls twice as hard as the default (`outrage_appeal`
   2.5: difference 0.639). The upper bound guards against a feed taken over by outrage; field audits compare a
   chronological feed with a real ranker, which carries integrity measures, so the field magnitude is checked
   against the integrity preset in fact 3.
2. Social reward teaches outrage. Under the Engagement ranking the Outrage share of posts rises from the first fifth
   of the run to the last (by 0.01 to 0.15), and by 0.01 to 0.15 more than with `learning_rate` 0. Brady et al.
   (2021). Fails when in-group hostility pulls half as hard (`outrage_appeal` 0.6: rise -0.020, and -0.008 against
   no learning), and when bot farms boost two and a half times as much and time is free (`population.bot_boosts` 20,
   the starting value, with `time_cost` 0: rise 0.000, -0.003 against no learning), because bot-boosted Clickbait
   then out-earns Outrage; with the default time cost that world is undecided (rise 0.017, and 0.007 against no
   learning; 13.4).
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
   chronological feed cuts toxic impressions to 0.312 of the integrity preset's, about threefold. Failing worlds:
   when feeds change neither session length nor habit (`habit_gain` 0, `stop_bias` -5) time spent falls only to
   0.967; with three times the polarisation per hostile impression (`polarisation_rate` 0.0005) polarisation
   differs by 10% at day 90; with `outrage_appeal` 2.5 the toxic ratio drops further, to 0.17.
4. Forwarding limits slow viral spread. With `reshare_depth_cap` 1 the largest share cascades shrink to between 0.3
   and 0.85 of their size without a cap. de Freitas Melo et al. (2019) modelled WhatsApp's limits. Fails when nothing
   impulsive drives sharing and time is free (`outrage_appeal`, `arousal_appeal`, `social_proof` and `time_cost` 0:
   ratio 0.860), because cascades then stay short with or without the cap; with the default time cost that world is
   undecided (0.870, 0.843 to 0.897).
5. Harassment drives its targets away. Four times the Trolls raise the share of Vulnerable users who left by 0.03 to
   0.3 (Pew Research Center, 2017). Misses the band on the seed means, though not on its interval, when harassment
   neither hurts nor drives people out and time is free (`harassment_harm`, `harassment_exit` and `time_cost` 0:
   difference 0.023, 0.007 to 0.038): the remaining effect comes from toxic feeds and reputation alone. With the
   default time cost the same world gives 0.041 (0.016 to 0.065), inside the band on the means; no world tried fails
   this check on its interval (13.4).
6. Leaving the platform raises wellbeing. Under the integrity preset (14.1, the model's realistic ranked feed, the
   comparison of fact 3), a deactivation experiment (13.2) keeps every regular user off the platform for the last 35
   days of the run, and its placebo on the same seed draws the same people but keeps them on; the break group's
   wellbeing change minus its change in the placebo lies between 0.02 and 0.25 standard deviations of the
   participants' starting wellbeing: positive, and below about four times the field estimate. The paired design
   compares the same people with and without the break, which the field cannot do; the field's design, a tenth on
   the break against the stayers in one run, scatters more across seeds (13.2). Allcott, Gentzkow,
   Wittenbrink et al. (2025) found that people who deactivated Facebook for the six weeks before the 2020 US
   election reported a 0.060 standard deviation improvement in an index of happiness, depression and anxiety,
   relative to controls who deactivated for only the first of those weeks; for Instagram the figure was 0.041
   (samples of 19,857 Facebook and 15,585 Instagram users). Allcott et al. (2020) found that four weeks of
   deactivation before the 2018 midterms increased subjective well-being. `time_cost` is calibrated on this
   fact (13.2), so passing it shows the calibration, not an independent prediction. Fails without a time cost
   (`time_cost` 0: effect -0.267), because every item of a good feed then adds reflective value and leaving costs
   people that value, and at the upper bound when time is worth about twice the default (`time_cost` 0.225: effect
   0.391; at 0.2 the effect of 0.287 lies above the band on the seed means, but its interval, 0.242 to 0.333, reaches
   into it). The effect builds up over the year (section 16), so its test runs for 365 days, at `N` = 1,500.

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
- Time on the platform has a price, but a crude one. Every impression costs `time_cost` times its dwell (7.2),
  calibrated on a deactivation experiment (13.2, fact 6). What remains of the free lunch:

  - one value of time for everyone, whatever else each person would do with it;
  - writing posts takes no time;
  - notifications cost only the time they bring, with no cost for the interruption itself, so on a feed worth more
    than its time they still raise wellbeing (Quality and bridging at full notification strength: users' wellbeing
    +0.019, 13.3);
  - the reflective self never ends a session: sessions stop on impulse and habit (6.1), so within a session a feed
    worth less than its time holds people as long as one worth more, and only churn answers the difference;
  - one session a day, so there is no habit of checking many times.

  The value of time is calibrated on one preset, the integrity preset taken as the realistic ranked feed (as fact 3
  takes it). On the Engagement ranking the same experiment's paired effect is 0.048 (-0.003 to 0.098) at
  `time_cost` 0, already close to the field's 0.060, and 0.361 (0.309 to 0.412) at the default, six times the
  field's figure (13.2, 8 seeds each). Calibrated on the Engagement ranking, the value that matches the field would lie
  near 0, below the rule's grid: the effect there is 0.048 at `time_cost` 0 and 0.190 at 0.05.
  Which preset the field's Facebook feed resembles more decides the value, and the model cannot tell.

  The fact 6 effect also rests on the year's state of the platform: on the paired design at the default, at
  `N = 2,000` it is -0.087 standard deviations after 120 days, 0.001 after 240 and 0.054 after 365, and 0.055 at
  `N = 1,500` after 365 days (8 seeds each).
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
  90 the chronological feed shows 0.70 of the integrity preset's misinformation impressions where the field found
  about two thirds more (fact 3, a known limit), and it shows a ninth of the Engagement ranking's outrage impressions
  where field audits find differences of tens of percent. Against the integrity preset too the cut in incivility is
  too deep: over the same days the chronological feed shows 0.312 of its toxic impressions, about a threefold cut,
  where the field lowered the share of uncivil content by tens of percent (fact 3's toxic check, band 0.5 to 0.9, a
  second known limit). The field experiment moved people whose networks had
  formed under ranking; the model has neither a mid-run switch for part of the population nor untrustworthy pages
  that people follow.
- Compulsive use is priced low. Items beyond a person's time budget are 2.5% of time under the default design (2.9%
  before the time cost), against the 31% of social-media use that Allcott, Gentzkow and Song (2022) attribute to
  self-control problems (the held-out target of 13.4). The time cost charges every minute at one price, whatever drew
  the person in, so it prices time, not the lack of self-control: regret, wellbeing and the benefit index still
  understate what compulsive use costs.
- Outrage learning over a year rests on the bot model. Bot boosts teach Clickbait, which takes share from Outrage
  after the first months; at the default the Outrage share still rises (fact 2, by 0.021 from the first fifth of the
  year to the last), but with bots boosting 20 posts a day instead of 8 the rise is undecided (0.017, interval
  0.001 to 0.032), and without the time cost as well it vanishes (0.000).
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
- Allcott, H., Gentzkow, M., Wittenbrink, B., et al. (2025). The effect of deactivating Facebook and Instagram on
  users' emotional state. *NBER Working Paper* 33697. <https://doi.org/10.3386/w33697>
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
