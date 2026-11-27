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
configuration (section 12). Before them, on the first day of an experiment (a deactivation, a notifications-off or a
feed experiment, 13.2), the experiment finds the regular users and draws its treated group; after phase 13, a
notifications-off experiment logs each person's time on the platform and sessions over its baseline week and its
own days, a feed experiment logs each person's use and exposure over its days, and a break that ends before the run
takes its measures on its last day and logs its group's time on the platform after it.

1. Bots: replace banned bots (3.5).
2. Joining: non-users may join, churned users may return (7.4).
3. Posting: active users and bots write posts (7.5 chooses styles and polish, 4.2 builds them).
4. Automated moderation of the new posts (6.7).
5. Harassment delivery: each surviving Attack reaches its target by notification unless the target blocked the
   author (6.9).
6. Sessions: each active user who opens the app gets the day's feed and reads it in one or more sessions
   (6.1 to 6.6, 7.1 to 7.3).
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
  `clamp(activity * (0.6 + 0.8 * habit) * (1 + 0.15 * notification_intensity), 0, 0.98)`. Suspended users can
  read but not post. The notification factor was `1 + 0.3 * notification_intensity` in v0.3; fact 7, the
  notifications-off week, shrank it (13.1).
- The session's item cap is `L = round(world.base_session_items * (0.7 + 0.6 * habit))` (default
  `base_session_items` 20). If the user opted into time limits (6.10), `L = min(L, time_budget)`. Until lab #30 it
  also had a factor `1 + 0.5 * notification_intensity * impulsivity`, removed because no field figure supports it, in
  either world (13.1).
- `notification_intensity` here, in the extra sessions below and in the habit gain of 7.3 is the person's own: the
  design's lever for everyone, except people whose notifications a notifications-off experiment switched off
  (13.2), for whom it is 0 in every term.
- The user reads items in rank order. After each item the session ends with probability
  `sig(world.stop_bias - 1.5 * A - 1.0 * habit)` (*calibrate*, default `stop_bias` -2.5), where `A` is the item's
  impulsive appeal (7.1). Engaging feeds therefore make longer sessions as well as more sessions (through habit).
- Extra sessions. A user who opened the app today also comes back `K_h ~ Poisson(world.recheck_rate * habit)` times out
  of habit and `K_n ~ Poisson(world.notification_pull * notification_intensity)` times because a notification pulled
  them back (each draw at most 30). By default both rates are 0, so a user has one session a day, the first above, and
  nothing below is drawn. The checking habits world (11) sets `recheck_rate` 27.5 and `notification_pull` 3, fitted to
  field figures for sessions a day and the share of sessions a notification starts (13.2); the cap of 30 then binds
  only for the most habituated, since at habit 1 the habitual draw has mean 27.5. People who did not open the app have
  no extra sessions, and neither do people on a deactivation experiment's break (13.2).
- The day's feed is built once, as long as all of the day's sessions together: `L + (K_h + K_n) * L_x` items, where
  an extra session's cap is `L_x = max(1, round(world.extra_session_share * L))` (default share 0.1, a judgement: a
  check shows about two items, 13.2).
  The first session reads from the top of the list as above; the habitual sessions follow, then the
  notification-driven ones, each continuing down the ranked list after the last item seen that day, with its own cap
  `L_x` and the same stop rule. No item is shown twice in a day, and no new posts arrive within a day (posting
  happens once, in phase 3), so a session that finds the list used up sees nothing; it still counts as a session,
  and a notification-driven one is still charged its interruption cost. In the checking world the candidate pool
  binds, so some of the counted sessions see nothing, the notification-driven ones most often since they come last;
  their share was not measured (13.2).
- The first session's ranking and draws do not depend on the extra sessions: the feed's order does not depend on its
  length, and the extra sessions draw after the first on the same streams.

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
  `recommended_share` 0.4, `candidate_cap` 80. With one session a day a daily active reads about a fifth of a feed's
  candidates; the checking habits world (11) reads about three fifths of them, so its ranking selects less (13.2).
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
  `time_budget`, assigned each day after joining to the least impulsive users. The limit counts the items of all the
  day's sessions (6.1).
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

Each impression adds to two daily sums and to the day's time cost; each notification-driven session adds to the
day's interruption cost.

- Impulsive pleasure `E_imp += A * (1 if liked else 0.5)`.
- Reflective utility `E_ref += R - world.ad_annoyance * ads_share` (default `ad_annoyance` 0.02), and -0.3 more if `v`
  replied angrily. Subscribers to the ad-free tier (8.3) see no ads, so for them the annoyance term is 0.
- Harassment: an Attack delivered to its target subtracts `world.harassment_harm * toxicity * (0.5 + vulnerability)`
  from the target's `E_ref` (default `harassment_harm` 3.0) and counts as a harassment event.
- Social comparison: with visible like counts, a post by a non-Creator with more likes than the viewer's own typical
  count subtracts `world.comparison_cost * vulnerability * min(1, ln(1 + likes) / ln(1000))` (default 0.1).
- Regret: each item beyond `time_budget` subtracts `world.regret_cost` (default 0.05).
- Time cost: every impression costs `world.time_cost * D`, where `D` is its dwell (7.1, the quantity time spent
  sums), default `time_cost` 0.06 (calibrated in 13.2). It is what the time could have given people if spent on
  something else, the same for everyone. The time cost is kept in its own daily sum `C`, apart from `E_ref`; an item
  is worth its time when `R > time_cost * D`.
- Interruption cost: each notification-driven extra session (6.1) costs `world.interruption_cost` (default 0, not
  calibrated yet), the broken concentration of being pulled back, on top of the time its items take. It is kept in its
  own daily sum `I`, apart from `E_ref` and `C`. Habitual returns cost no interruption. By default there are no extra
  sessions, so the cost acts only in a world that turns them on, such as the checking habits world (11).
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

Wellbeing and stay utility read the day's reflective utility net of the time cost and the interruption cost,
`E_net = E_ref - C - I` (7.2). The subscription decision (8.3) reads `E_ref`, the value before both, with the ad
annoyance taken out and the annoyance the tier would spare added, so the escape from ads counts as a motive on top
of the ad-free value (13.2 gives the reason), and fan funding reads each post's `R`.

Once a day, for every user:

- `wellbeing += 0.05 * tanh(E_net / 5) + 0.02 * (wb0 - wellbeing)`, clamped to `[0, 1]`. On a day without a session
  only the second term applies.
- `habit += world.habit_gain * (1 + 0.5 * notification_intensity) * tanh(E_imp / 5) - 0.05 * habit`, clamped
  (default `habit_gain` 0.08), with the person's own notification intensity (6.1: 0 for someone whose notifications
  a notifications-off experiment switched off). Without a session only the decay applies.
- Stay utility `U = 0.4 * tanh(E_imp / 5) + 0.6 * tanh(E_net / 5) + 0.3 * min(1, active mutual follows / 10) +
  0.5 * habit - 0.5 * harassment events today`, and `stay_ema = 0.9 * stay_ema + 0.1 * U` on active days.
- Recent pleasure: `pleasure += 0.2 * (felt - pleasure)`, where `felt` is the habit term's `tanh(E_imp / 5)` (13.1
  records the scale the code uses) on a day with a session and 0 otherwise. It is a smoothed, squashed daily
  impulsive experience in `[0, 1]`, about a week's mean, and enters only the survey (9.1).

Once a day, for every non-user (former users, banned people and people who never joined), wellbeing recovers toward
the baseline by the same rule, `wellbeing += 0.02 * (wb0 - wellbeing)`, as on a user's day without a session. Someone
who never joined stays at the baseline; a former user's wellbeing closes 45% of its gap in 30 days and 95% in five
months. Nothing else of a non-user changes here.

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

Revenue has two sources, ads and an ad-free tier offered alongside them under every design (lab #16). The price and
the uptake scale are calibrated to Snap's figures for 2025 (13.2) at the final defaults, with one session a day.

- Ads: `design.ads_share` in `[0, 1]` (default 1.0) is the ad load, the share of a full load of ads, shown to everyone
  who does not subscribe. Each impression that carries ads earns `ads_share * world.ad_rate` (default `ad_rate`
  0.001, the same whatever advertisers want) and costs its reader `ad_annoyance * ads_share` (7.2).
- The ad-free tier: a subscriber sees no ads, so their impressions earn no ad revenue and they feel no ad annoyance,
  and pays `world.subscription_price` per 30 days (default 1.87), whatever the ad load. With `ads_share` 0 the tier
  spares nothing, and only the platform's value draws people to it, as support.
- Revenue per day `= ads_share * ad_rate * impressions of non-subscribers + subscription_price / 30 * subscribers`.
- Uptake: every 30 days each user decides afresh, and subscribes with probability
  `min(1, s * sig((V - world.subscription_hurdle) / 0.1) * (p_ref / p)^e)`, where:

  - `V` is the platform's ad-free value plus the ads the tier escapes, counted as a motive of its own: the mean
    daily `E_ref` over the last 30 days, before the time cost and the interruption cost (7.3), without the ad
    annoyance the person felt (that is the feed's ad-free value), plus the annoyance the tier would spare at the
    current ad load and the person's mean items a day over those days, `ad_annoyance * ads_share * items / 30`. The
    ad-free value already contains the ads' absence, so measured from what a non-subscriber feels, the escape counts
    twice: once in the ad-free value and once as the motive. Taking the felt annoyance out makes a subscriber and a
    non-subscriber with the same feed value it alike. On day 0 there is no history, so `V` is taken to equal the
    hurdle (default 0.5, unchanged);
  - `s = world.subscription_uptake` (default 0.0585), the uptake scale, calibrated so that 5% of daily actives
    subscribe under the default design (R2, 13.2);
  - `p` is `subscription_price` and `p_ref` its default, 1.87, fixed in the code, so a world with another price
    moves uptake;
  - `e = world.subscription_elasticity` (default 1), the price elasticity of uptake: at 1, uptake halves when the
    price doubles. No measured price elasticity was found, so 1 is a judgement, explored from 0.5 to 2.

  The default price is 3.8 times the yearly revenue per reported daily active under the default design, converted
  to 30 days (R1, 13.2). People decide on their own value; the platform sees revenue and uptake, never `V`.
- Payout pool `= design.payout_share * revenue` (default 0). `design.payout_scheme`: PerView (by impressions),
  PerEngagement (by observable likes + replies + shares, bots and angry replies included), QualityWeighted
  (engagement times `Q_hat`), FanFunded (each user's share of the revenue that user brought, the ad revenue of the
  impressions that showed them ads or, for a subscriber, the tier's price over 30 days, goes to the Creators whose
  posts gave them positive `R`, in proportion to that `R`).
- Eligibility (`design.payout_threshold_followers`, default 0): with a threshold, only accounts with at least that
  many followers are paid, a cliff in place of a linear share.
- A creator earns a living when `income_ema >= world.living_income` (default 1.0 per day, *calibrate* so that a
  creator fund of 0.3 pays a living to between 5% and 30% of Creators under the default design; 13.1).

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
- Sessions per reported daily active (`sessions_per_active`): the day's human sessions (6.1), the first and the extra
  ones, plus one session for each active bot, over reported daily actives; exactly 1 when nobody has extra sessions.
  The share of sessions started by notifications (`notification_session_share`): notification-driven sessions over
  all of them, bots' included. The platform knows which session a notification started, and an app open that finds
  nothing new still counts.
- Candidates per feed (`candidates_per_feed`): the mean number of candidates gathered for each feed served today
  (6.2), before ranking cuts the feed to its length; at most `candidate_cap`.
- Reports (one per account and post) and see-less marks (those of brigading bots included), automated removals,
  human removals, notes.
- Revenue, payouts, and net revenue: revenue minus creator payouts, what the platform keeps. Revenue splits into ad
  revenue (`ad_revenue`) and the ad-free tier's (`subscription_revenue`); subscribers per reported daily active
  (`subscriber_share`) is the tier's uptake (8.3).
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
  former users, which removes survivorship bias), each at their current wellbeing, so a former user counts with
  the wellbeing they have recovered since leaving (7.3) as a field survey of former users would find it; wellbeing
  by archetype; meaningful interactions per user (positive
  replies and likes between mutual follows on Personal posts); the share of Vulnerable individuals who left; time not
  worth it (`low_value_time`): dwell on impressions whose reflective value is below their time cost,
  `R < time_cost * D`, divided by all dwell (at `time_cost` 0, the share of time on items with negative `R`).
- Experiment: the deactivation effect (`deactivation_effect`), the mean wellbeing change of a deactivation experiment's
  break group minus that of its stayers, and the break group's change (`break_change`), the mean wellbeing change of
  everyone assigned to the break, both in standard deviations of the participants' wellbeing at the start (13.2).
  Compared between a run and its placebo, the break group's change gives the break's effect, fact 6's measure. Beside
  it, the break group's changes in affective polarisation (`break_polarisation`) and informedness
  (`break_informedness`), measured the same way in standard deviations of the participants' values at the start, and the
  same two changes unscaled, in the quantities' own units (`break_polarisation_raw`, `break_informedness_raw`). All six
  are taken at the end of the break's last day, which is the run's last day unless the break ends earlier
  (`ends_before_end`, 13.2); then the break group's mean daily time on the platform from the break's end to the run's
  end (`post_break_dwell`) is measured too, every member counted, with no time on a day without a session or after
  leaving. The notifications-off experiment's changes (`notifications_off_time`, `notifications_off_sessions`,
  `notifications_off_wellbeing`): the treated group's mean change in daily time on the platform and in sessions a day,
  from each participant's mean over the week before to their mean over the experiment's days, and in wellbeing, each in
  standard deviations of the participants' baseline (13.2); compared between a run and its placebo, they give fact 7's
  measures. The feed experiment's measures (13.2), over its days, for everyone assigned to the treated group, leavers
  counted with no use after leaving: mean daily time on the platform (`feed_experiment_dwell`) and mean daily engagement
  actions, likes, replies and shares given (`feed_experiment_engagement`); the toxic, misinformation and same-group
  shares of their impressions (`feed_experiment_toxic_share`, `feed_experiment_misinformation_share`,
  `feed_experiment_same_group_share`), the mean toxicity of the posts they wrote (`feed_experiment_post_toxicity`) and
  the mean truth of the posts they shared (`feed_experiment_shared_truth`), each pooled over the whole group's items;
  and their mean changes in affective polarisation, informedness and wellbeing from the start of the first day to the
  end of the last (`feed_experiment_polarisation`, `feed_experiment_informedness`, `feed_experiment_wellbeing`), in
  standard deviations of all participants' values at the start, and the polarisation and informedness changes unscaled
  (`feed_experiment_polarisation_raw`, `feed_experiment_informedness_raw`). The model's spread of informedness across
  participants is small, so a change in its standard deviations comes out large; the unscaled changes sit beside the
  scaled ones. A hide-flagged experiment's dose, the share of the treated's impressions whose classifier score exceeds
  its threshold (`feed_experiment_flagged_share`), pooled likewise, is counted in a real run and in its placebo; it is
  read in the placebo, where the treated see those posts (in a real run they see none). Compared between a run and its
  placebo the measures give the experiment's effects: relative changes for the per-day measures and shares, differences
  for the changes in standard deviations. All of these exist only on the last day of a run that sets their experiment
  up, so the app's metric pickers and the runner's default tables leave them out.
- Diagnostics: descriptive measures kept for checking the model against field findings, not goals or harms. They
  draw nothing and exist in every run, but have no value on a day without their subject, and the app's metric
  pickers and the runner's default tables leave them out.

  - Reach by partisanship, over the posts that end their life today: `partisan_half_reach`, the mean lifetime human
    impressions per post of authors in the more partisan half by the `partisanship` trait (3.2), and
    `extreme_reach_ratio`, that of the most partisan tenth of authors over that of the middle fifth (from 40% to
    60%). Authors are ranked over the whole population, once, so the halves hold the same people in every design;
    bots' posts have no partisanship and are left out.
  - The out-group shares of the in-network stages, after Bakshy et al. (2015): of the in-network candidates gathered
    for the day's feeds (6.2, after the in-network cap, before ranking cuts the feed;
    `in_network_outgroup_candidates`), of in-network impressions (`in_network_outgroup_impressions`), and of the
    likes on in-network impressions (`in_network_outgroup_likes`). An out-group post is one whose author's group
    differs from the viewer's. Their ratios are metrics too: impressions over candidates
    (`outgroup_ranking_ratio`), what ranking and stopping do to the mix, and likes over impressions
    (`outgroup_like_ratio`), what people's choices do.
  - The share of a noted post's human shares made before its note (`noted_shares_before_note`), pooled over the
    noted posts that end their life today; the human share count is kept when the note is attached (6.8).
  - Human shares per impression of Outrage posts over those of the non-hostile styles, Informative, Entertaining and
    Personal (`outrage_share_ratio`), from the day's counts.
  - The mean truth of the posts people shared today (`shared_truth`), bots' shares left out.

- Non-users: mean affective polarisation and informedness of non-users, and the same for users.
- Public discourse: toxic and truthful shares among the day's top 5% most-viewed posts; positive cross-group likes per
  impression; society-wide affective polarisation.
- Creators' livelihoods: Creators earning a living, median income, Gini coefficient of Creator income, payout share
  by style.
- Content mix: share of human posts by style, by day, the share of human posts that were polished (4.2), and the
  number of posts bots wrote, counted apart.

### 9.3 Composite indices

Each index lies in `[0, 1]`; the formulas are deliberately simple so a player can trace them.

- `engagement_index = min(1, human impressions / (1.5 * N * base_session_items))`: 1 when everyone in the society
  sees one and a half typical sessions' worth of items a day. At the defaults the default design scores 0.153 and the
  most engaging design found (14.1) 0.258 (v0.3: 0.183 and 0.382). In the checking habits world (11) a daily active
  reads about three times the items, and the index rises to 0.749 under the default design and saturates under the
  most engaging design, at 1 on every day of every seed's summary window, so the index cannot rank the most engaging
  designs there; lab #30 had set the benchmark to 6 typical sessions for that world, and hub #23 returned it to 1.5
  with the default of one session a day (13.1).
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
| Business | `ads_share`: the ad load of everyone who does not subscribe (8.3) | 1.0 |
| Measurement | `survey_after_session` | 0 |

Levers can change during a run (`Simulation::set_design`); a change applies from the next day.

Every design offers the ad-free tier of 8.3 alongside its ads; `ads_share` sets only the ad load of those who do not
take it. The tier's price is a world assumption (11), not a lever: the lab compares designs at one price.

## 11. World assumptions

These are not design choices. They are the model's beliefs about people, kept separate so a player can test whether
a conclusion survives a different world. The interruption cost of 7.2 stays at 0 by judgement, an explorable world
assumption (13.2). Since lab #16 the subscription price and the uptake scale are calibrated to Snap's figures (R1 and
R2, 13.2) and the price elasticity is a judgement.

Checking habits, a documented world setting. By default people open the app at most once a day (6.1). Lab #30 fitted
the extra sessions of 6.1 to field figures (13.2): `recheck_rate` 27.5 and `notification_pull` 3, with
`extra_session_share` 0.1 by judgement, give about 15 sessions per daily active, 11% of them started by notifications.
To turn the setting on:

```text
--set world.recheck_rate=27.5 --set world.notification_pull=3 --set world.extra_session_share=0.1
```

It is not the default because it triples a day's reading while every per-impression rate of the model was calibrated at
about 16 items a day: under the default design fact 2 fails, the Outrage share of impressions halves and the bot farms'
share of the signals falls by half or more (13.2 lists its effects at the final defaults). The owner's decision of
hub #23 (2026-11-26) keeps it as a world to explore, with its side effects, rather than the default.

| Assumption | Default | Range to explore |
| --- | --- | --- |
| `outrage_appeal`, `arousal_appeal`, `social_proof` | 1.2, 1.0, 0.3 | 0-3, 0-2, 0-1 |
| `harassment_harm`, `comparison_cost`, `regret_cost` | 3.0, 0.1, 0.05 | 0-6, 0-0.5, 0-0.2 |
| `time_cost` (7.2) | 0.06 | 0-0.4 |
| `interruption_cost` (7.2) | 0 | 0-2 |
| `wrongful_sanction_cost` | 0.5 | 0-2 |
| `learning_rate`, `imitation_rate`, `choice_temperature` | 0.1, 0.02, 0.25 | 0-0.5, 0-0.1, 0.05-1 |
| `evasion_rate`, `evasion_decay`, `evasion_cost` | 0.15, 0.01, 0.3 | 0-0.5, 0-0.1, 0-1 |
| `classifier_retraining`, `intensity_rate`, `deterrence` | 0.2, 0.05, 0.5 | 0-0.5, 0-0.1, 0-2 |
| `habit_gain`, `stop_bias` | 0.08, -2.5 | 0-0.1125, -5-0 |
| `churn_base`, `leave_threshold`, `harassment_exit` | 0.02, 0.15, 0.05 | 0-0.1, 0-0, 0-0.2 |
| `join_base`, `reputation_sensitivity`, `homophily` | 0.004, 2.0, 0.7 | 0-0.02, 0-5, 0.5-1 |
| `account_cost_deterrence`, `bot_farm_floor` | 0.5, 0.2 | 0-1, 0-1 |
| `bot_strategy` (3.5) | adaptive | shared-pool, own-side, adaptive |
| `bot_brigading` (3.5) | 1 | 0-5 |
| `polarisation_rate`, `contact_rate`, `spillover_rate` | 0.004, 0.01, 0.01 | 0-0.00075, 0-0.005, 0-0.05 |
| `polarisation_backfire` (8.1) | 0 | 0-1 |
| `credulity_scale` | 1.0 | 0-2 |
| `ad_rate`, `ad_annoyance` | 0.001, 0.02 | 0.0003333-0.003, 0.006667-0.06 |
| `subscription_price` (8.3) | 1.87 | 1-4 |
| `subscription_uptake`, `subscription_elasticity` (8.3) | 0.0585, 1 | 0.01-0.12, 0.5-2 |
| `subscription_hurdle` (8.3) | 0.5 | 0.1667-1.5 |
| `living_income` | 1.0 | 0.005-0.045 |
| `base_session_items` | 20 | 5-60 |
| `recheck_rate`, `notification_pull`, `extra_session_share` (6.1) | 0, 0, 0.1 | 0-40, 0-20, 0-1 |
| `quality_signal_noise`, `polish_gain`, `polish_cost` | 0.15, 0.3, 0.03 | 0-0.5, 0-1, 0-1 |
| `reflective_quality_weight`, `in_group_outrage_cost`, `clickbait_penalty` | 0.8, 0.8, 0.3 | 0-2, 0-1, 0-1 |
| `style_quality_gap` | 1.0 | 0-1.5 |
| `survey_impulsive_weight`, `survey_habit_weight` | 0.3, 0.1 | 0-0.5, 0-0.5 |

Ranges revised 2026-11-26 (lab #31), by a rule fixed before the robustness runs (`incentive robustness --ranges
revised`, the default). The Default column gives the specification's text values, and the documented ranges were
written for them; where 13.1 records a calibrated change, the range is scaled by the same factor as the default, code
over text. Old values: `classifier_retraining` 0-1 (0.2 to 0.1, factor 0.5), `intensity_rate` 0-0.5 (0.05 to 0.01,
factor 0.2), `habit_gain` 0-0.3 (0.08 to 0.03, factor 0.375), `leave_threshold` -0.5-0.5 (0.15 to 0, factor 0, so the
range is 0 alone and the assumption keeps its default), `polarisation_rate` 0-0.02 (0.004 to 0.00015, factor 0.0375)
and `contact_rate` 0-0.05 (0.01 to 0.001, factor 0.1). An assumption without a documented range gets default/3 to 3 x
default at the code's default, clamped to the catalogue: `ad_rate`, `ad_annoyance`, `subscription_hurdle` and
`living_income` (0.015; 0.005 is the catalogue's floor) had none ("-"). The subscription price and uptake ranges were
written for their current defaults (8.3) and stay. Two population settings vary too, from default/2 to 2 x default
in global sampling: the troll share (3.1), 0.015-0.06 around 0.03, and the bot share (3.5), 0.025-0.1 around 0.05;
the troll share is relative, normalised with the other archetypes' shares when the population is drawn. `--ranges
documented` samples the old values, clipped to the catalogue, with the catalogue range where none was documented.

Run settings (`run`): `seed`, `days` (default 365), `population_size` (default 2,000 natively, 1,000 in the browser),
`initial_user_share` (0.6), `post_lifetime_days` (3), `candidate_cap` (80), `offline_ties` (8), `initial_follows`
(30), `summary_window` (0.3), `tracked_agents` (default 10, two per archetype, for the inspector), `deactivation`
(default none; when set, `days` 35, `share` 0.1, `placebo` false and `ends_before_end` 0 unless given: a deactivation
experiment, 13.2), `notifications_off` (default none; when set, `days` 7, `share` 1 and `placebo` false unless given,
and the run needs the 7 baseline days before it: a notifications-off experiment, 13.2), `feed_experiment` (default
none; when set, `kind` `no_reshares`, `days` 90, `share` 0.2 and `placebo` false unless given; `kind` may also be
`{"hide_flagged": {"threshold": t}}` with `t` in 0-1 or `{"fewer_same_group": {"penalty": p}}` with `p` in 0-10: a
feed experiment, 13.2). At most one of the three experiments may be set.

## 12. Determinism

- A run is a pure function of its configuration: the same configuration gives a bit-identical history on the same
  platform and build. Native and browser builds may differ in the last bits of transcendental functions (`ln`, `exp`,
  `tanh`, and the samplers built on them), so their histories can drift apart over a long run; compare them by
  distribution, not bit for bit.
- Randomness comes from counter-based streams: each draw site seeds a `rand_pcg` generator from
  `(seed, day, entity, stream)` mixed with SplitMix64, where `stream` names the purpose (population, archetype race,
  posting, post draws, feed, engagement, moderation, economy, social, spillover, survey, bots, bot boosts,
  brigade, experiment, sessions). Two runs that differ in one lever therefore share every draw the lever does not touch,
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
  metric but the experiment's own. The notifications-off experiment (fact 7) works the same way: it draws only on its
  first day, on the same stream, its last day draws nothing, and its placebo matches a run without it. A break that
  ends before the run draws nothing when it ends: its group's openings and churn draw as anyone's from then on,
  each keyed on the person as before. The feed experiment (13.2) also draws only on its first day, on the same
  stream. Its filters and penalty only change which candidates a treated viewer's feed gathers and how it orders
  them, so only the treated viewers' own feed and engagement draws move, which are keyed on them. A filter that
  hides flagged posts needs every post's classifier score; with the classifier switched off it computes the score
  on the classifier's own draw, keyed on the post, which nothing else uses then. Counting its dose (9.2) reads the
  kept scores and draws nothing. Its placebo therefore matches a run without it in every metric but its own.
- The extra sessions of 6.1 are drawn on their own stream, keyed on the person and the day, right after the person
  decides to open the app; with both rates at 0 nothing is drawn, so such a run makes exactly the draws it made
  before extra sessions existed. The day's feed is built once and its draws do not depend on its length, and the
  extra sessions draw after the first session on its engagement stream, so with the rates above 0 a person's first
  session makes the same draws as without extra sessions; what the extra sessions do (likes, shares, follows) still
  reaches the feeds built after them.
- No iteration over hash maps influences results. Phases visit individuals in ascending index.

## 13. Calibration record

This section records the values the code uses where they differ from the text above, the choices the text leaves
open, the measured behaviour of the presets (section 14) at `N = 2,000` with 8 seeds (mean and spread), the facts, the
scenarios and the run time. Sections 13.3 to 13.5 are regenerated by `incentive report --seeds 1..8` (default size:
`N = 2,000`, 365 days, 8 paired seeds; output `exports/report/report.md`); every number quoted from them names its
comparison there. How well the model reproduces field findings never used in tuning, and how stable and sensitive its
results are under noise on the world assumptions, is assessed in [validation.md](validation.md) (lab #31).

Targets for calibration:

- No degenerate dynamics under the default design: after 365 days between 40% and 90% of individuals are users; the
  Outrage share of posts rises over the year but stays below 50%; the toxic impression share lies between 3% and 20%.
- The trade-off exists in both directions: some designs cut the harm index markedly at a moderate engagement cost, and
  over a full year the engagement ranking's lead in engagement shrinks or reverses as harassment, churn and
  reputation catch up with it, under at least some plausible world assumptions.
- The stylized facts of section 15 hold.

At the final defaults, with one session a day (13.2's final block), every target above holds: after 365 days 0.499
of society are users, the toxic impression share is 0.178, and the Outrage share of posts rises by 0.018 (0.008 to
0.028) to 0.254. No required check of section 15 fails; three are undecided (13.4). Two targets this round named for
the default design are missed there by the owner's decision: visits a day (T3) and the share a notification starts
(T1) hold only in the checking habits world (11), which fails fact 2 (13.2).

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
| 11 | `living_income` (hub #23) | 1.0 | 0.015 | As required: a 0.3 engagement fund pays 21% of Creators a living |
| 3.5 | bot posts per day | 3 | 1 | Bots wrote 28% of posts and doubled the Outrage share at day 0 |
| 3.5 | `bot_boosts` (v0.2) | 20 | 8 | See below: bots supplied a third of engagement and taught Clickbait |
| 7.5, 11 | `intensity_rate` (v0.2) | 0.05 | 0.01 | See below: intensity ran into the classifier within weeks |
| 6.7, 11 | `classifier_retraining` (v0.2) | 0.2 | 0.1 | See below: a month to catch up instead of two weeks |
| 6.1 | session length's notification factor (lab #30) | 0.5 | 0 | See below: no field figure supports it |
| 6.1 | opening chance's notification factor (hub #23) | 0.3 | 0.15 | See below: fact 7's sessions check failed |
| 8.3, 11 | `subscription_price` (hub #23) | 5.0 | 1.87 | R1: 3.8 times the yearly revenue per daily active (13.2) |
| 8.3, 11 | uptake's largest chance (hub #23) | 0.5 | 0.0585 | R2: 5% of daily actives subscribe (13.2) |

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

The lab #30 and hub #23 changes to notification intensity's terms are written into the text as well (6.1); the Text
column gives the v0.3 value. Fact 7, the notifications-off week (section 15), set both:

- Session length's notification factor 0, from `1 + 0.5 * notification_intensity * impulsivity`. No field figure
  supports a longer session when notifications are on. In the checking habits world (11), lab #30 removed it first,
  in an order set before that builder tuned (the session length's term first, then the opening chance's; the step of
  0.05 was its own choice), when fact 7's time and sessions checks failed there with v0.3's terms (time -0.543,
  sessions -0.428; 13.2's history). Its removal stays in the default world, where restoring it was not tested.
- Opening chance's notification factor `1 + 0.15 * notification_intensity`, from 0.3. Lab #30 shrank it in steps of
  0.05 to 0.2 until fact 7 held in the checking habits world (13.2's history). In the default world, with one session a
  day, fact 7's sessions check failed again at 0.2 (-0.415, -0.428 to -0.401, at `time_cost` 0.08 in the first round
  of the fixed point), so by the same rule the factor shrank to 0.15, where the check holds (-0.330, -0.344 to -0.316;
  -0.326 at the final defaults). The habit gain's notification term (7.3, 0.5) was not needed and stays.

The engagement benchmark of 9.3 is the text's 1.5 again. Lab #30 had set it to 6 typical sessions, because in the
checking world the most engaging design reached the cap on every seed at 1.5; with one session a day by default the
default design scores 0.153 and the most engaging design 0.258 at 1.5 (v0.3: 0.183 and 0.382), and the checking world's
values are in 9.3.

History of hub #23 (2026-11-26). Lab #30 turned the checking habits on by default, which took a daily active from
about 16 items a day to about 47. Three quantities sized for one session a day were then re-sized to v0.3's shares by
rules fixed before tuning: `candidate_cap` from 80 to 240, `bot_boosts` from 8 to 22 and the bots' brigade count from
3 to 10 a day, with `time_cost` refitted to 0.19 and `living_income` to 0.045. Fact 2 then held on the seed means, but
fact 3's polarisation check, the user share and the toxic share failed (13.2's history). The owner's decision turned
the checking habits off by default and reverted the re-size; the rows above are v0.3's again, and the three rounds of
that re-size are summarised in 13.2.

The lab #16 changes came with the ad-free tier of 8.3, which replaced the v0.3 business model (subscribers paid
`(1 - ads_share) * subscription_price / 30` a day, uptake was `0.5 * sig((mean E_ref - hurdle) / 0.1)` whatever the
price, and every impression earned ads). The price and the largest chance, now `world.subscription_uptake`, are
written into the text (8.3, 11); the Text column gives the v0.3 value. They were refitted for the default of one
session a day (13.2's final block, full size, 8 seeds):

- `subscription_price` 1.87 and `subscription_uptake` 0.0585, the fixed point of R1 and R2 (13.2). In the checking
  world lab #16 had fitted 5.95 and 0.0455: three times the items earn about three times the ad revenue.
- `living_income` 0.015 (v0.3: 0.025). The v0.1 calibration ("a 0.3 engagement fund pays 21% of Creators a living", the
  engagement-paid creators preset, 14.1) was made on the ad scale, and the tier's revenue now joins the fund. On the
  grid of the control (0.005), at the final defaults, the share is 0.391 at 0.01, **0.246** at 0.015 and 0.153 at 0.02;
  0.015 lies closest to 0.21 (by 0.036, against 0.057 at 0.02, about four standard errors of the 8-seed mean, 0.009).
  The share falls steeply across one step of the grid, a grid chosen at the landing; a finer value such as 0.017
  would come closer but lies off it. In the first round of the fixed
  point (`time_cost` 0.08) the same grid gave 0.385, 0.236, 0.146, 0.101 at 0.025, 0.076 at 0.03 and 0.052 at 0.035.
  Nothing in the default design or the facts' designs reads `living_income`. In the checking world the round had fitted
  0.08, and 0.045 after the re-size.

Additions the text does not have, each kept as small as possible:

- Informedness drifts toward its baseline 0.6 by 1% of the gap a day, for everyone (forgetting and news from outside
  the platform). Without it informedness runs to its bounds.
- A person follows at most 150 accounts; follows from liked recommendations otherwise grow without bound.
- `population.bot_posts_per_day` and `population.bot_clickbait_share` (default 0.5) are configuration fields, so the
  misinformation scenario can make bots Clickbait-heavy.
- The control ranges of `polarisation_rate` (0 to 0.002) and `contact_rate` (0 to 0.01) follow the new defaults;
  `living_income` ranges from 0.005 to 1.

### 13.2 Choices where the text is open

The round of hub #23 (labs #29, #30 and #16) ended with one session a day by default: the owner's decision of
2026-11-26 kept lab #30's extra sessions as a documented world setting, the checking habits of section 11, and refitted
the round's calibrated constants for the default world. After the general choices below come the final defaults (the
final block), the checking habits world at those defaults, and the round's history in brief: lab #30's calibration of
the checking world, lab #16's calibration of revenue, the refit of `time_cost` with checking on, and the re-size of the
candidate pool and the bots. The measurements in the general items below predate the round's calibration: they were
taken with one session a day, as now, but with v0.3's notification terms and business model and, unless they name
another value, at v0.3's `time_cost` of 0.115.

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
- Subscriptions: on day 0 there is no history, so the value `V` of 8.3 is taken to equal the hurdle. Decisions
  fall in the economy phase, after the day's sessions (section 5), so on a decision day the impressions already
  seen carried ads by the old status, while the day's tier revenue and fan funding count the new one.
- Subscriptions read the day's reflective utility before the time cost (`E_ref`, 7.3), while wellbeing and stay
  utility read it after. R2 calibrates uptake's scale on that value (below); reading the net value would put the
  time cost, which fact 6 calibrates, into who pays, and R2 cannot tell the two readings apart. Fan funding
  reads each post's `R`, also before the time cost. The interruption cost (7.2) is treated like the time cost:
  wellbeing and stay utility read it, subscriptions do not.
- The value behind uptake takes out the ad annoyance the person felt and adds the annoyance the tier would spare
  at the current ad load (8.3), so it is the feed's ad-free value plus the escape from ads as a separate motive.
  Taking the felt annoyance out first keeps a subscriber, who feels no ads, from valuing the same feed more than a
  non-subscriber does, which would let subscribing feed on itself. Against what a non-subscriber feels, the escape
  then counts twice, once in the ad-free value and once as the motive, which makes uptake lean harder on the ad load
  than a value of either the ad-free feed alone or the felt feed plus the escape would. R2 fixes the level at the
  default design only; how uptake differs between ad loads (the most engaging design, the front's designs) rests on
  this choice, and the two alternatives were not tested.
- Extra sessions (6.1), in a world that turns them on, come only to people who opened the app that day. The first
  session's opening chance already carries the notifications' pull on whether to open at all (the factor
  `1 + 0.15 * notification_intensity`), so a
  notification-driven extra session is a return on a day the person was coming anyway, and a day without the first
  session has no extra ones. The draws are independent Poisson counts, one for habit and one for notifications, so a
  day's habitual returns do not crowd out its notification-driven ones. Each extra session's cap is a share of the
  same day's first cap, and the feed is not rebuilt between sessions, because no posts are written within a day.
  The habitual sessions read before the notification-driven ones; since all extra sessions share one cap and one
  stop rule, the order changes only which of them a used-up feed leaves empty, and every drawn session counts and,
  if a notification started it, costs its interruption even then. A time-limited user's limit (6.10) counts the
  whole day, so a later session's cap is cut to what is left of it. In the metrics each active bot counts as one
  session, since the platform cannot tell bots from people.
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
- The field's design (`deactivation_effect`): the mean wellbeing change of the break group minus that of the stayers, in
  one run. Participants who left during the experiment are left out: it compares the participants still on the platform
  at the end, and only stayers leave. It is NaN when a group is empty, as at share 1. Under the integrity preset at
  v0.3's default `time_cost` (0.115, one session a day, 8 seeds, full size) the experiment has 5,641 participants, 705 a
  seed and 61% of the users on its first day, of whom 552 (9.8%) take the break; 264 of the 5,089 stayers (5.2%) leave
  before the end, and the standard deviation of the participants' starting wellbeing is 0.153 (0.145 to 0.160 across
  seeds; measured while former users' wellbeing froze, which returning users move a little), so the field design's
  effect there, 0.075, is about 0.011 of wellbeing. Under the Engagement ranking 4,992 take part (67% of the users), 319
  of the 4,486 stayers (7.1%) leave, and the standard deviation is 0.172. Letting former users recover (7.3) changes no
  draw, so these counts stand. With about 70 people a seed on the break, and people's distance from their wellbeing
  baseline varying widely, the per-seed effects scatter by 0.083 standard deviations (0.080 at 0.105 with frozen
  leavers), so the 95% interval at 8 seeds is about ± 0.069. A larger share inside one run would empty the stayers'
  feeds. The field compared six weeks off with one week off; the model compares five weeks off with staying on.
- The paired design (`break_change`, fact 6), which only a simulation can run: a run with the experiment at share 1,
  every regular user on the break, and its placebo on the same seed are identical up to the first day and assign the
  same people, so the difference of their `break_change` compares the same people with and without the break, and who
  happens to be in the break group no longer adds noise. `break_change` is the mean change of everyone assigned to the
  break; members who leave count too, which happens only in the placebo, and off the platform their wellbeing recovers
  toward its baseline by the rule of 7.3, as on the break, just as the field kept surveying control participants who
  quit and found their wellbeing still moving. Until lab #29 a leaver's wellbeing froze when they left. Letting it
  recover moved the paired effect at the default from 0.054 to 0.052 (8 seeds, full size): the real run is unchanged,
  since nobody on the break leaves, and the placebo's change rose from -0.037 to -0.035. The placebo's leavers were
  therefore worth about 0.002 standard deviations, far less than the 0.015 that the breakdown at the end of this item
  had put on them together with spillover. Under the integrity preset at v0.3's default `time_cost` (0.115, one session
  a day, 8 seeds, full size) the paired effect is 0.052 ± 0.048 (standard deviation of the per-seed effect), against
  0.075 ± 0.083 for the field's design on the same seeds; across v0.3's grid (measured with frozen leavers) the paired
  spread lies between 0.048 and 0.075 up to `time_cost` 0.225 (0.084 at 0.25, 0.130 at 0.3). The spread that remains
  comes from the real run, not the placebo: the break group's change is 0.017 ± 0.051 in the real run and -0.035 ± 0.009
  in the placebo. On the break, wellbeing closes 51% of its gap to the baseline in 35 days (7.3: one minus 0.98 to the
  35th power), so the real run's change measures how far the year on the platform moved the group from its baseline, and
  that differs between seeds, which draw different populations and platforms; on the platform, wellbeing stays near
  where the year left it. More seeds narrow it; no design within a run can. With frozen leavers the field's design gave
  larger effects than the paired one on the same seeds at every value of the grid: by 0.025 at `time_cost` 0, by 0.017
  to 0.041 at each step from 0.085 to 0.13, and by 0.043, 0.055 and 0.080 at 0.15, 0.2 and 0.3. At v0.3's default the
  gap was 0.023 (0.075 against 0.052) and splits three ways (a real run and its placebo at share 0.1, 8 seeds). About
  +0.016 comes from which tenth of the regular users is drawn: the paired effect is 0.068 at share 0.1 against 0.052 at
  share 1, and since the assignment draws are keyed on the person and the first day, nearly the same people are drawn at
  every `time_cost`, so the shift repeats along the grid. About -0.004 is imbalance between the groups (the placebo's
  `deactivation_effect`). About +0.011 comes from how the designs count leavers, which the field's design drops and the
  paired one counts with their recovering wellbeing, together with any spillover onto the stayers; these measures cannot
  separate the two. With frozen leavers the same split was +0.011, -0.005 and +0.015 of a gap of 0.020: recovery took
  about 0.004 from the leavers' part and, since the tenth's own placebo leavers recover too (its placebo change fell
  from -0.040 to -0.043), added about 0.005 to the tenth's. These shifts are of the size of the leavers' small numbers.
  The leavers' part should grow with churn, as the gap grows with `time_cost`. At the final defaults the paired spread
  is 0.067. In the checking habits world it was about 0.12 (0.116 at `time_cost` 0.13); it has the same source, the
  year's distance from baseline, which there varies more between seeds (13.2's history).
- The notifications-off experiment (`run.notifications_off`, fact 7) is a researcher's intervention like the
  deactivation experiment, kept on the truth side in the same way; at most one of the two runs in a run. It lasts
  the last `days` days of the run (default 7, the field's week). At the start of its first day the participants are
  found as for the deactivation experiment, the users who opened the app on at least 4 of the previous 7 days, and
  each joins the treated group with probability `share` (default 1, the paired design), drawn on the experiment
  stream (section 12). For the treated, notification intensity is 0 in every term (6.1, 7.3): no
  notification-driven sessions, no lift to the chance of opening the app, no lift to the habit gain. They still
  open the app on their own, check it out of habit, receive Attacks and may churn, as field participants who
  switched notifications off in their phone's settings could; Attacks reach their targets whatever the
  notification intensity (6.9). In a placebo run (`placebo` true) the same people are drawn and nothing changes.
  Each participant's baseline is their mean daily time on the platform (dwell, 0 on a day without a session) and
  their mean sessions a day (the first and the extra ones, 0 on a day without a session) over the 7 days before the
  first day; the experiment logs the same over its own days, at the end of each day. On the run's last day three
  measures read the treated group's mean change, each in standard deviations over all participants of the
  baseline it starts from: time, from the baseline week's mean to the mean over the experiment's days; sessions,
  the same; wellbeing, from the start of the first day to the end of the last, as fact 6's measure. Treated members
  who leave count, with no time and no sessions after leaving, as a phone's log would record them. The paired
  difference of a real run and its placebo on the same seed (share 1, under the integrity preset) is fact 7. Like
  Dekker et al. (2024), who compared the intervention week with the baseline week, the per-day measures compare
  weekly means, not one day's draws.
- A deactivation experiment's break may end before the run (`run.deactivation.ends_before_end`, default 0): it then
  covers the days `[end - ends_before_end - days, end - ends_before_end)`, so that use in the weeks after the break
  can be measured, as the field did. On the first day after the break its group is back under the usual rules: it
  may open the app, with the habit the break left, and may churn. The break's measures (9.2) are taken at the end of
  its last day and reported on the run's last day, and the break group's mean daily time on the platform from the
  break's end to the run's end is `post_break_dwell`, every member counted, with no time on a day without a session
  or after leaving. At the default the break ends with the run and every number of fact 6 is unchanged.
- The feed experiment (`run.feed_experiment`) is a researcher's intervention like the other two, kept on the truth
  side, though it acts through the feed, as the field studies' extensions and holdbacks did; at most one of the
  three experiments runs in a run. It lasts the last `days` days of the run (default 90). Its participants are found
  as for the deactivation experiment, and each joins the treated group with probability `share` (default 0.2),
  drawn on the experiment stream (section 12). For a treated viewer the feed changes by `kind`; for everyone else,
  and for everyone in a placebo run (`placebo` true, the same people drawn), nothing changes.

  - `hide_flagged` with a `threshold`: candidates whose toxicity classifier score (6.7) exceeds the threshold are
    dropped, as a field study hid posts above a classifier's score. The score is observable: the platform computes
    it when it screens a new post, and each post keeps it. With the classifier switched off the score is still
    computed for such an experiment, on the classifier's own draw, and nothing is removed.
  - `no_reshares`: reshares are dropped from the candidates; original posts stay, in network and recommended, as in
    the field's feeds without reshares.
  - `fewer_same_group` with a `penalty`: the penalty is subtracted from the ranking score (6.4) of posts by the
    viewer's own group, so fewer of them reach the top of the feed, as the field down-ranked like-minded sources. A
    chronological feed has no score, so it is unchanged.

  A filter acts while the candidates are gathered (6.2): a hidden post or a reshare is skipped in network and is
  not eligible for recommendation, so the in-network cap and the recommendation quota fill with other posts, as a
  real feed would backfill. Each participant's use and exposure are logged at the end of each of the experiment's
  days, and its measures (9.2) are read on the run's last day; the paired comparison of a real run and its placebo
  on the same seed is the experiment's effect on the treated. How large a dose each field study gave (the share of
  impressions hidden, the fall in same-group exposure) is matched by choosing `threshold` and `penalty`, which the
  measures report as manipulation checks.
- Wellbeing of non-users recovers toward the baseline at the rate of a user's day without a session (7.3): former
  users, banned people, and people who never joined, who sit at the baseline. Until lab #29 it froze when a person
  left. The change moves no draw, so it moves only what reads a non-user's wellbeing: the wellbeing of everyone who
  ever used the platform and the benefit index that contains it, returning users' wellbeing (so users' wellbeing,
  the archetype breakdown and survey answers, a little) and the experiment's measures. At full size (`N = 2,000`,
  365 days, seeds 1 to 8, paired) the wellbeing of everyone who ever used the platform rose by 0.011 (0.008 to 0.014)
  under the Engagement ranking, whose leavers sit below their baseline, and fell by 0.003 (0.002 to 0.004) under the
  integrity preset and by 0.008 (0.007 to 0.008) under Quality and bridging, whose leavers had been above it; the
  benefit index moved by +0.002, -0.001 and -0.002, users' wellbeing by less than 0.001, and the user share not at
  all.
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
  a day kept brigade reports between a quarter and a third of all reports and marks under the Engagement ranking
  (29%, 13.3) at one session a day. Judgement. At the final defaults the share is 0.327 under the Engagement ranking.
- `polarisation_backfire` 0: Bail et al. (2018) found backfire among Republicans, but Nyhan et al. (2023) found that
  reducing exposure to like-minded sources on Facebook for three months changed no measured attitude; the default
  leaves contact as the only effect of calm out-group posts. Not measured at other values.
- `time_cost` (7.2), calibrated by a rule: the default is the value at which fact 6's effect (section 15) at full size
  (`N = 2,000`, 365 days, seeds 1 to 8, `incentive facts --fact deactivation`) lies closest to 0.060 standard
  deviations, the Facebook estimate of Allcott, Gentzkow, Wittenbrink et al. (2025), on a grid of 0.005. The target, the
  integrity preset as the design that has to match it, "closest" and the grid's step were fixed before any value was
  tried; the search range is set from a coarse scan. The default, **0.06**, comes from the rule's run at the final
  defaults of hub #23, with one session a day (the final block below). In the checking habits world the rule gave 0.13,
  and 0.19 after the re-size (the history at the end of this section). In v0.3, with one session a
  day, the rule first ran on the field's design (above), which gave 0.105 (effect 0.071, against 0.040 at 0.095, 0.032
  at 0.1, 0.081 at 0.11 and 0.074 at 0.115) but could not pin the value: its interval at 8 seeds is about ± 0.067, wider
  than the whole span of its effects from 0.095 to 0.115. The paired design replaced it for calibration, and fact 6 uses
  it. On the paired design, over 0.06 to 0.14 (a range set after the field's design had covered 0, 0.05, each step from
  0.085 to 0.13, and 0.15, 0.2 and 0.3), the rule gave 0.115 (effect 0.054, 0.013 to 0.094), against 0.053 at 0.11 and
  0.076 at 0.12, a tie (0.0533 against 0.0536); the field's design gave 0.074 (0.006 to 0.142) at 0.115 on the same
  seeds. That run, and the grid below, were measured while former users' wellbeing froze when they left; with recovery
  (7.3, lab #29) the paired effect at 0.115 was 0.052 (0.012 to 0.092). v0.3's paired effects from 0.06 to 0.14 under
  the integrity preset, 8-seed mean ± the standard deviation of the per-seed effect:

| `time_cost` (v0.3) | 0.06 | 0.065 | 0.07 | 0.075 | 0.08 | 0.085 | 0.09 | 0.095 | 0.1 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Effect (SD) | -0.091 | -0.075 | -0.072 | -0.048 | -0.037 | -0.022 | -0.011 | 0.020 | 0.009 |
| ± | 0.072 | 0.062 | 0.069 | 0.059 | 0.065 | 0.075 | 0.056 | 0.057 | 0.061 |

| `time_cost` (v0.3) | 0.105 | 0.11 | **0.115** | 0.12 | 0.125 | 0.13 | 0.135 | 0.14 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Effect (SD) | 0.034 | 0.053 | 0.054 | 0.076 | 0.106 | 0.097 | 0.118 | 0.131 |
| ± | 0.055 | 0.058 | 0.048 | 0.065 | 0.058 | 0.058 | 0.073 | 0.065 |

In v0.3 the paired grid rose by about 0.028 standard deviations per 0.01 of `time_cost` and turned down only twice,
by 0.011 and 0.009: its values share seeds, so the seeds' noise moves them together. The 8-seed mean had a standard
error of about 0.02, so the rule fixed `time_cost` to within about 0.007 (one standard error). At 0 the effect was
-0.267 under the integrity preset and 0.048 under the Engagement ranking, and at 0.115 0.054 and 0.361 (8 seeds,
frozen leavers); the grid at the final defaults is in the final block.

**The final defaults (hub #23).** The owner's decision (2026-11-26, lab #30): the checking habits are off by default
(`recheck_rate` and `notification_pull` 0, one session a day), the re-size is reverted (`candidate_cap` 80, 8 boosts
and 3 brigade reports and marks a bot a day), the session length's notification term stays removed, the engagement
benchmark returns to 1.5 (13.1), and `time_cost`, the revenue targets and `living_income` are refitted for the default
world by their own rules, together with fact 7, until all hold at the same constants. Every number in this block was
measured at full size (`N = 2,000`, 365 days, seeds 1 to 8) under the default design unless named, as 8-seed means over
the summary window (the last 110 days) ± the standard deviation across seeds, with 95% t-intervals where they decide.

The rules, each fixed before tuning:

- `time_cost`: fact 6's paired effect under the integrity preset closest to 0.060 on a grid of 0.005, after a coarse
  scan (the item on `time_cost` above).
- R1, the price: the tier costs 3.8 times the yearly revenue per daily active user under the default design, converted
  to 30 days, with the tier's own revenue included. Snap 2025: Snapchat+ at about $3.99 a month, $47.9 a year, against
  $12.61 of revenue per daily active user (the sum of its quarterly ARPU for 2025). Meta Verified, at $11.99 a month on
  the web, is about 2.5 times Meta's 2025 revenue per daily active person ($57.03).
- R2, uptake: under the default design 5% of daily actives subscribe, band 1% to 10%. Snap's fourth quarter of 2025: 24
  million subscribers, 5.1% of 474 million daily actives. Telegram Premium had about 1.3% of monthly actives (12 of 950
  million, 2024); X Premium about 0.5% and YouTube Premium about 5% (both unverified). No measured price elasticity of
  uptake for a social platform's tier was found (the Pandora experiment of Goli et al. 2024 varied the ad load, not the
  price), so unit elasticity is a judgement, explored from 0.5 to 2.
- `living_income`: the engagement-paid creators preset (a 0.3 engagement fund) pays a living to the share of Creators
  closest to 0.21, on the control's grid of 0.005 (13.1).
- Fact 7 (section 15): if a check fails, the opening chance's notification factor shrinks in steps of 0.05 until it
  holds, and the loop runs again (the order set before lab #30's calibration builder tuned; the step was its own).

Both revenue targets divide by reported daily actives (9.1, bots included), as a platform reports them. The default
price is also uptake's reference price, so under the default design the price factor is 1 and uptake does not depend on
the price; and the default design pays no creators, so nothing else in it reads the price. R2 therefore fixes the uptake
scale alone, and R1 then fixes the price in one step: with ad revenue `a` and uptake `u` per daily active a day, `p =
3.8 * 30 * (a + u * p / 30)`, so `p = 114 a / (1 - 3.8 u)`. The hurdle (0.5) is kept.

The fixed point took two rounds and a check. It started from v0.3's `time_cost` (0.115) and lab #16's checking-world
revenue constants (price 5.95, uptake scale 0.0455), where uptake was 0.035 and R1 gave a price of 2.30:

| Round | Opening factor | `time_cost` | Fact 6 | Fact 7, sessions | Uptake scale (R2) | Price (R1) | `living_income` |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 0.2 | 0.08 | 0.068 (0.016 to 0.121) | -0.415 (-0.428 to -0.401), fail | 0.06: 0.0499 | 1.87 | 0.015: 0.236 |
| 2 | 0.15 | 0.06 | 0.057 (0.001 to 0.113) | -0.326 (-0.344 to -0.309), pass | 0.0585: 0.0499 | 1.87 | 0.015: 0.246 |
| check | 0.15 | 0.06 | 0.057 (0.001 to 0.113) | -0.326, pass | 0.0585: 0.0499 | 1.869 | 0.015 |

- Round 1. The uptake scale first, at `time_cost` 0.115: 0.065 gave 0.0494 and 0.07 0.0531, and R1 a price of 1.86; then
  the coarse scan of `time_cost` (fact 6: -0.128 at 0, -0.008 at 0.05, 0.110 at 0.1, 0.159 at 0.115, 0.187 at 0.13,
  0.228 at 0.15) and the fine grid, 0.032 at 0.065, 0.050 at 0.07, 0.048 at 0.075, **0.068 at 0.08**, 0.084 at 0.085 and
  0.098 at 0.09. At 0.08 the uptake scale was refitted (0.066: 0.0532, 0.062: 0.0514, **0.06: 0.0499**, 0.058: 0.0479),
  the price came to 1.87 and `living_income` to 0.015 (13.1). Fact 7's sessions check then failed below -0.4, so the
  opening factor shrank from 0.2 to 0.15: there the check gave -0.330 (-0.344 to -0.316) and fact 6 0.093.
- Round 2, at opening factor 0.15. Fact 6's grid (below) gave 0.06; the uptake scale moved to 0.0585 (0.06: 0.0514,
  0.059: 0.0502, **0.0585: 0.0499**); the price stayed 1.87 and `living_income` 0.015. Fact 7 holds.
- The check, at the final constants: fact 6 moved by less than 0.0001 with the uptake scale, so its grid stands, and
  R1, R2, `living_income` and fact 7 hold. No constant moved.

Fact 6's paired effect under the integrity preset at the final constants (the uptake scale 0.06 for the grid, 0.0585
for the default), mean ± the standard deviation of the per-seed effect:

| `time_cost` | Effect (SD) | 95% interval |
| --- | --- | --- |
| 0 | -0.101 ± 0.057 | -0.148 to -0.054 |
| 0.05 | 0.022 ± 0.073 | -0.039 to 0.083 |
| 0.055 | 0.033 ± 0.061 | -0.018 to 0.083 |
| **0.06** | 0.057 ± 0.067 | 0.001 to 0.113 |
| 0.065 | 0.054 ± 0.055 | 0.008 to 0.100 |
| 0.07 | 0.069 ± 0.073 | 0.009 to 0.130 |
| 0.075 | 0.080 ± 0.069 | 0.022 to 0.137 |
| 0.08 | 0.093 ± 0.079 | 0.027 to 0.159 |
| 0.085 | 0.098 ± 0.049 | 0.057 to 0.139 |
| 0.1 | 0.140 ± 0.048 | 0.099 to 0.180 |
| 0.115 | 0.175 ± 0.069 | 0.117 to 0.232 |
| 0.13 | 0.220 ± 0.069 | 0.163 to 0.277 |

0.06 lies closest to 0.060 (by 0.003, against 0.006 at 0.065 and 0.009 at 0.07). The effect rises by about 0.025 per
0.01 of `time_cost` (0.198 from 0.05 to 0.13), and the 8-seed mean has a standard error of about 0.024, so the rule
fixes `time_cost` to within about 0.01. Fact 6 is undecided at 0.06 (13.4); on 16 seeds, a supplementary line, its
effect is 0.044 ± 0.067 (0.009 to 0.080), still undecided. The value is about half v0.3's 0.115, where the effect is now
0.175 (v0.3 after lab #29: 0.052). Most of the shift comes from the smaller opening factor: with v0.3's 0.3 and every
other constant final the effect is 0.094 (0.042 to 0.146) at 0.115 and -0.037 (-0.097 to 0.023) at 0.06. The rest came
with the other two changes that reach the integrity preset at one session a day, the removed session length term and
lab #16's business model; how fewer notification-driven opens make a break worth more was not taken apart.

Fact 7 at the final constants: time -0.168 (-0.181 to -0.155), sessions -0.326 (-0.344 to -0.309), wellbeing -0.003
(-0.004 to -0.002), all inside their bands. With one session a day the sessions measure counts the days a person opens
the app in a week, so it moves only through the opening chance's factor and habit.

R1 and R2 at the final constants. The default design earns 11.07 a day, 8.96 from ads and 2.11 from the tier, over 675
reported daily actives: 0.01639 a day or 5.98 a year per daily active (standard error 0.6%), and 3.8 times that over
30 days is 1.869; the closed form, with `a` = 0.01328 and `u` = 0.0499, gives 1.868. Uptake is 0.0499 ± 0.0086 (0.0427
to 0.0571). The two targets together set the tier's share of the default design's revenue to `3.8 u` = 19%; Snap's
subscriptions ("Other Revenue", $232 million of $1,716.5 million) brought 13.5% of its revenue in the fourth quarter of
2025, less than 24 million subscribers at $3.99 would pay, so Snap's subscribers pay less than the list price on
average. Ranges (11): the price from 1 to 4, about half to twice the default; the uptake scale from 0.01 to 0.12, about
R2's band under the default design.

Uptake under every preset, subscribers per reported daily active: engagement ranking 0.050, chronological 0.089, Quality
and bridging 0.105, calm by design 0.087, subscription 0.107, engagement-paid creators 0.024, integrity 0.074, most
engaging 0.051. Quality and bridging and the subscription preset lie just above R2's band, which names only the default
design.

Targets at the final defaults (`time_cost` 0.06, opening factor 0.15, price 1.87, uptake scale 0.0585, `living_income`
0.015, one session a day), under the default design unless named:

| Target (section 13, 13.1, 13.2) | Measured | Band | Verdict |
| --- | --- | --- | --- |
| User share after 365 days | 0.499 ± 0.030 (0.474 to 0.524; lowest seed 0.464) | 0.4 to 0.9 | holds |
| Toxic impression share | 0.178 ± 0.027 (0.155 to 0.200) | 0.03 to 0.2 | holds |
| Outrage share of posts | 0.236 to 0.254: +0.018 (0.008 to 0.028) | rises, below 0.5 | holds |
| Harm cut at a moderate engagement cost | Q&B: harm -80%, engagement -17% | judgement | holds |
| Lead reverses (weak habit) | 1.06 in the first 30 days, 0.71 (0.67 to 0.75) in the last 95 | judgement | holds |
| R2, uptake per reported daily active | 0.0499 ± 0.0086 (0.0427 to 0.0571) | 0.01 to 0.1 (0.05) | fitted |
| R1, price from revenue per daily active | 1.869 against the price of 1.87 | point target | fitted |
| Creators living, engagement-paid creators | 0.246 ± 0.026 at 0.015 | closest to 0.21 | fitted |
| Fact 7 | time -0.168, sessions -0.326, wellbeing -0.003 | 13.4 | holds |
| T3, sessions per daily active | 1 (14.76 with checking habits) | 8 to 25 (15) | missed by owner decision |
| T1, notification share of sessions | 0 (0.114 with checking habits) | 0.05 to 0.25 (0.11) | missed by owner decision |

- The toxic share's interval touches the band's top (0.2003); the seed mean lies inside.
- The trade-off. Quality and bridging cuts the harm index by 80% (0.408 to 0.082) for 17% of the engagement index (0.153
  to 0.127), and ends with 0.125 more of society as users (0.10 to 0.15). In the default world the Engagement ranking's
  lead in engagement grows over the year, from 1.11 (1.10 to 1.12) in the first 30 days to 1.20 (1.17 to 1.24) in the
  last 95; in the weak-habit world (`habit_gain` 0.01, `reputation_sensitivity` 5, `harassment_exit` 0.15) it reverses,
  from 1.06 (1.05 to 1.06) to 0.71 (0.67 to 0.75), with 0.231 fewer of society as users (0.203 to 0.258).
- Against v0.3 (8 seeds, the v0.3 binary), under the default design: the user share over the summary window is 0.526
  (v0.3 0.479), the toxic share 0.178 (0.174), the Outrage share of impressions 0.468 (0.451), the engagement index
  0.153 (0.183), and a human daily active reads 15.2 items a day (17.9). The smaller opening factor means fewer opens
  and less habit, and the lower `time_cost` leaves each day on the platform worth more; the two were not taken apart.
- Run time. One run of 365 days at `N = 2,000` (`incentive run --seeds 1`, single-threaded in release, wall time
  including the CSV export) takes 2.8 seconds on the owner's desktop (three runs: 2.81 to 2.84); 13.6 has the run
  times at these defaults (v0.3: 2.9).

**The checking habits world at the final defaults.** The setting of section 11 (`recheck_rate` 27.5,
`notification_pull` 3, `extra_session_share` 0.1) with every other constant final, full size, 8 seeds, against the
default world (fact effects with their 95% intervals and verdicts):

| Measure | Default world | Checking habits |
| --- | --- | --- |
| Sessions per daily active (T3: 15, band 8 to 25; bots count one each, humans alone about 15.8) | 1 | 14.76 ± 0.25 |
| Notification share of sessions (T1: 0.11, band 0.05 to 0.25) | 0 | 0.114 ± 0.002 |
| Items a day per human daily active | 15.2 | 46.6 |
| Fact 1, outrage impressions | 0.404 (0.371 to 0.438), pass | 0.182 (0.164 to 0.200), pass |
| Fact 1, reflective value | 0.161 (0.137 to 0.186), pass | 0.121 (0.105 to 0.138), pass |
| Fact 2, rise | 0.018 (0.008 to 0.028), undecided | -0.027 (-0.035 to -0.020), fail |
| Fact 2, rise minus no learning | 0.014 (0.002 to 0.025), undecided | -0.013 (-0.017 to -0.009), fail |
| Fact 6 | 0.057 (0.001 to 0.113), undecided | -0.329 (-0.436 to -0.222), fail |
| Fact 7, sessions | -0.326 (-0.344 to -0.309), pass | -0.343 (-0.352 to -0.334), pass |
| User share after 365 days | 0.499 | 0.732 |
| Toxic impression share | 0.178 | 0.095 |
| Harm index | 0.408 | 0.267 |
| Engagement index | 0.153 | 0.749 |
| Regretted share (held out) | 0.0045 | 0.470 (0.461 to 0.479) |

The setting meets T1 and T3 at the final defaults, as lab #30 fitted it, and fact 7 holds there too (time -0.302,
wellbeing -0.001). It is not the default for three reasons:

- It triples a day's reading, while every per-impression rate of the model (the bots' 8 boosts a day, the cap of 80
  candidates, the learning and polarisation rates) was calibrated at about 16 items a day. With 46.6 items read out of
  80 candidates (a ratio of 0.58, against 0.19 in the default world) the ranking selects less: the Outrage share of
  impressions halves (0.238 against 0.468), the bots supply 0.095 of the engagement (0.231) and file 0.102 of the
  reports and marks (0.327), and the Outrage share of posts falls over the year, so fact 2 fails.
- Fact 6 fails at the default `time_cost`: in the checking world the rule gave 0.13 (the history below), because more
  reading and a farm diluted by it make the integrity preset's feed worth more than its time at 0.06.
- Re-sizing what was sized for one session a day did not restore the time scale (the history below): with a pool of
  240 candidates and the farms scaled to v0.3's shares, fact 2 held on the seed means, but fact 3's polarisation
  check, the user share and the toxic share failed, and the weak-habit world became degenerate. The owner's decision
  was to keep the setting as a world to explore, not the default.

In the checking world the most engaging design's engagement index sits at 1 on every day of every seed's summary window
(9.3), and the regretted share follows the judged length of a check: 0.197 (0.183 to 0.211) at `extra_session_share`
0.05, 0.470 at 0.1 and 0.646 (0.643 to 0.650) at 0.2, with the rates held (sessions 11.75 and 17.59, items 27.8
and 70.1). The integrity preset there gives 0.388 and Quality and bridging 0.242 (13.4).

**History of the round: the checking habits on by default.** Lab #30 turned the extra sessions on by default, and
lab #16, the refit of `time_cost` and the re-size were all calibrated in that world before the owner's decision turned
it off. Every number below was measured at full size (`N = 2,000`, 365 days, seeds 1 to 8) at constants that no longer
hold; the research notes of 2026-11-26 are in the lab's gitignored `exports/research-2026-11-26/`.

Lab #30, the extra sessions. The targets were named before any value was tried:

- T1, the share of sessions a notification starts, under the default design: 11%, band 5% to 25%. Heitmayer and
  Lahlou (2021) video-coded 1,130 smartphone interactions: "89% of smartphone interactions are initiated by users,
  not by notifications"; their participants mostly kept their phones silent or out of reach, hence the wide band.
- T3, sessions per daily active under the default design: 15, band 8 to 25. Derived: if checks follow screen time,
  a feed app's share of phone checks is its share of screen time, Facebook's 69 of 333 minutes a day in Allcott,
  Gentzkow and Song (2022), 21%, times 57 to 86 checks a day (Fitz et al. 2019; Dekker et al. 2024), which gives 12
  to 18. Snap reported in its S-1 (2017) that "users visit Snapchat more than 18 times per day", a messaging app.
- Fact 7 (section 15), the notifications-off week, under the integrity preset.
- Judgements, not fitted: an extra session, a check, shows about two items (`extra_session_share` 0.1); and
  `interruption_cost` 0, because the field finds no wellbeing gain from switching notifications off, the model has no
  fear of missing out, which offsets the relief in the field, and batching, which helped mood and stress in Fitz et
  al. (2019), cannot be represented. The interruption cost stays an explorable world assumption (section 11, 0 to 2).
- The regretted share of time stays the held-out target of 13.4: reported, not tuned.

`recheck_rate` and `notification_pull` were fitted to T3 and T1 (the smallest sum of squared relative errors, on a
coarse grid and then steps of 0.5 and 0.1). Both counts include sessions that found the day's feed used up (6.1);
their share was not measured, so T1 and T3 bound from above the sessions that show anything. Fact 7 then failed, so
notification intensity's terms were shrunk in an order set before that builder tuned, the session length's first, then
the opening chance's, in steps of 0.05 that it chose, with the rates refitted after each change (the terms are the
opening chance's and the session length's factors, v0.3 0.3 and 0.5):

| Terms | Rates | Time (SD) | Sessions (SD) | Wellbeing (SD) | Verdict |
| --- | --- | --- | --- | --- | --- |
| 0.3, 0.5 | 25.5, 3 | -0.543 (-0.554 to -0.533) | -0.428 (-0.436 to -0.420) | 0.039 (0.036 to 0.041) | fail |
| 0.3, 0 | 26.5, 2.9 | -0.414 (-0.423 to -0.404) | -0.443 (-0.453 to -0.433) | 0.010 (0.006 to 0.015) | fail |
| 0.25, 0 | 26.5, 2.9 | -0.373 (-0.384 to -0.362) | -0.408 (-0.416 to -0.400) | 0.006 (0.002 to 0.010) | fail |
| 0.2, 0 | 26.5, 2.9 | -0.339 (-0.345 to -0.333) | -0.375 (-0.380 to -0.370) | 0.007 (0.002 to 0.012) | pass |
| **0.2, 0** | **27.5, 3** | -0.339 (-0.345 to -0.333) | -0.375 (-0.381 to -0.368) | 0.010 (0.006 to 0.013) | pass |

The fit gave `recheck_rate` 27.5 and `notification_pull` 3: 14.98 ± 0.23 sessions per daily active and a notification
share of 0.111 ± 0.002, at `time_cost` 0.115. These are the values of the checking habits setting (11); at the final
defaults they give 14.76 and 0.114 (above). Fact 7's wellbeing check held across the interruption cost's range (0.093
at 2) and beyond it up to 6, where the effect flattens at about 0.157 and its interval still reaches into the band,
so it has no failing world. The judged length of
a check moved the most: at `extra_session_share` 0.05 and 0.2 the sessions were 11.86 and 17.73, the regretted share
0.197 and 0.646 (0.475 at 0.1), and fact 7's sessions check failed at 0.2 (-0.428) as did fact 6 (0.801).

Lab #16, the revenue. The ad-free tier of 8.3 replaced the v0.3 business model; R1 and R2 were named then (the final
block above) and fitted at `time_cost` 0.115 in the checking world: the uptake scale 0.0455 (uptake 0.0500) and the
price 5.95, 3.8 times a yearly revenue per daily active of 19.06, with `living_income` 0.085. The change moved what
does not depend on the world: the subscription preset lost 89% of its revenue and its creators term, so its benefit
index fell from 0.549 to 0.484, level with the integrity preset, and v0.3's Pareto front designs, which took most
of their revenue from subscriptions, lost about half of it (design 32's benefit lead over the engagement ranking fell
from 0.149 to 0.062). The paying designs learnt less harm, since `living_income` divides the money reward that teaches
styles (7.5): under engagement-paid creators the harm index fell from 0.372 to 0.306. Without payouts the change moved
the benefit index and wellbeing by at most 0.005.

The refit of `time_cost`. With the checking habits on, fact 6's paired effect at 0.115 was -0.037 ± 0.123, where lab #29
alone had left it at 0.052 ± 0.048. A diagnostic run found the cause: the bot farms boost a fixed number of posts a day,
so three times the human engagement halved their share of it (under the integrity preset 0.232 to 0.117), the top of the
feed carried less bot-boosted, low-value content, and first-session items were worth 0.340 on reflection against 0.130
at one session a day; without bots the two worlds' feeds were worth about the same (0.438 and 0.423). The spread grew
because the per-item margin between value and time was summed over 44 items a day instead of 15. The rule, run again,
gave 0.13 (0.051, -0.046 to 0.148), with `living_income` 0.08; T1, T3 and fact 7 held. But fact 2 failed (the Outrage
share of posts fell by 0.034, -0.039 to -0.028), because with about 44 items read out of at most 80 candidates the
ranking selected far less: the Outrage share of impressions was 0.22 (v0.3: 0.45). At one session a day, with the
round's other constants, the share rose by 0.028.

The re-size. The owner's first decision was to re-size what was sized for one session a day, by rules fixed before
tuning, to v0.3's shares measured with the v0.3 binary under the Engagement ranking: the candidate cap to the ratio of
items read per human daily active to candidates per feed (0.2241, on a grid of 10), the bots' boosts to their share of
reported engagement (0.2091, a grid of one), the brigade count per bot to their share of reports and see-less marks
(0.2904, a grid of one), then `time_cost` by its rule, round after round until no constant moved. The observable metric
`candidates_per_feed` (9.1) was added for the first rule; it draws nothing. Three rounds gave a cap of 240
(ratio 0.222), 22 boosts (0.212), 10 brigade reports and marks a day (0.291) and `time_cost` 0.19 (0.073), with
`living_income` 0.045. Fact 2 then held on the seed means (rise 0.011, -0.010 to 0.032, undecided) and fact 1 passed
(0.394), but fact 3's polarisation check failed (12.1% at day 90, against 4.0% with the cap of 80 and the other
constants of the re-size), and so did two targets at the head of section 13: 0.384 of society were users after the year
and the toxic share was 0.227. The Engagement ranking's harm index rose from 0.256 to 0.506, the weak-habit world became
degenerate (2.9% of society users), the bots filed 55% to 60% of the reports and marks under Quality and bridging and
calm by design, the Engagement ranking's paired break effect was 0.786 (13 times the field's), uptake fell to 2.9%, the
held-out regretted share was 0.514, and one full-size run took 9.2 seconds against 3.4 at a cap of 80. Checking habits
triple a day's reading, while every per-impression rate was calibrated at about 16 items a day, and re-sizing two
constants and a code constant did not restore that time scale. The owner's second decision turned the checking habits
off by default and reverted the re-size (the final block above).

### 13.3 Presets, comparisons and findings

The tables and findings of this section were measured at the final defaults of hub #23 (13.2's final block): one session
a day, notifications' opening factor 0.15 without a session length term (lab #30), `time_cost` 0.06, the ad-free tier at
a price of 1.87 (lab #16), and former users' wellbeing recovering (lab #29). A finding that moved is marked "moved",
with its v0.3 value (the v0.3 report is kept as the lab's gitignored `exports/report/report-v03.md`). The Max column
shows `max-engagement`, still v0.3's front design 61 (13.7 and 14.1): no design of the regenerated atlas is as engaging
at full size. While the preset held front design 60 for a time, nothing else in the report moved. 13.7 compares it with
the other designs.

`incentive report`: seeds 1 to 8, `N = 2,000`, mean ± standard deviation across seeds, summary over the last 30% of the
run (110 days of 365, 30 of 90). ER is the engagement ranking, Q&B quality and bridging, Sub the subscription, Paid the
engagement-paid creators, Integrity the engagement ranking with integrity, Max the most engaging design found (13.7).
Observable metrics are marked (O), the rest are truth; `~` marks a metric still moving in its window (section 9). Net
revenue is ad and tier revenue minus creator payouts (8.3). Comparisons are paired by seed; "row n" below is comparison
n of the report, with its 95% interval.

Over 365 days:

| Metric | ER | Chrono | Integrity | Q&B |
| --- | --- | --- | --- | --- |
| Engagement index (O) | 0.153±0.005 | 0.101±0.003~ | 0.149±0.005 | 0.127±0.003 |
| Harm index | 0.408±0.040 | 0.126±0.018 | 0.403±0.050 | 0.082±0.010~ |
| Harm index without polarisation | 0.428±0.036 | 0.157±0.022 | 0.474±0.054 | 0.103±0.012~ |
| Discourse quality | 0.658±0.031 | 0.814±0.009 | 0.784±0.025 | 0.878±0.005 |
| Benefit index | 0.407±0.007 | 0.425±0.004 | 0.426±0.008 | 0.435±0.001 |
| Benefit index without creators | 0.508±0.009 | 0.531±0.005 | 0.532±0.010 | 0.543±0.002 |
| Users in society | 0.526±0.026~ | 0.498±0.015~ | 0.589±0.024 | 0.633±0.009 |
| Human daily actives | 604±23 | 494±14~ | 634±20 | 620±11 |
| Impressions (O) | 9194±291 | 6082±171~ | 8917±271 | 7624±170 |
| Net revenue (O) | 11.07±0.54~ | 8.88±0.35~ | 11.83±0.51 | 11.70±0.22 |
| Toxic impressions | 0.178±0.027 | 0.020±0.010 | 0.098±0.028 | 0.004±0.001 |
| Outrage impressions | 0.468±0.045 | 0.057±0.014 | 0.319±0.040 | 0.015±0.002 |
| Misinformation impr. | 0.172±0.007~ | 0.077±0.012 | 0.130±0.009~ | 0.033±0.003 |
| Harassment per 1,000 | 2.56±0.65 | 3.20±0.59 | 7.99±1.52 | 3.68±0.92~ |
| Outrage posts | 0.256±0.021 | 0.166±0.013 | 0.226±0.015 | 0.166±0.011 |
| Wellbeing, ever-users | 0.577±0.010 | 0.614±0.004 | 0.610±0.010 | 0.646±0.003 |
| Survey (O) | 0.544±0.007 | 0.517±0.006 | 0.547±0.008 | 0.538±0.006 |
| Polarisation | 0.487±0.012 | 0.418±0.003 | 0.445±0.009 | 0.386±0.003 |
| Informed non-users | 0.583±0.003 | 0.598±0.001 | 0.605±0.004 | 0.626±0.002 |
| Creators living | 0 | 0 | 0 | 0 |
| Harmful payouts | - | - | - | - |
| Bot engagement | 0.231±0.010 | 0.510±0.005 | 0.263±0.012 | 0.452±0.006 |

The other presets, with ER again for reference:

| Metric | ER | Calm | Sub | Paid | Max |
| --- | --- | --- | --- | --- | --- |
| Engagement index (O) | 0.153±0.005 | 0.103±0.003 | 0.128±0.003 | 0.138±0.008~ | 0.258±0.005 |
| Harm index | 0.408±0.040 | 0.376±0.021 | 0.070±0.008~ | 0.485±0.043~ | 0.386±0.043 |
| Harm index without polarisation | 0.428±0.036 | 0.468±0.024 | 0.088±0.010~ | 0.474±0.028 | 0.381±0.035 |
| Discourse quality | 0.658±0.031 | 0.821±0.011 | 0.878±0.007 | 0.554±0.059~ | 0.690±0.035 |
| Benefit index | 0.407±0.007 | 0.448±0.003 | 0.460±0.003 | 0.432±0.016 | 0.406±0.008 |
| Benefit index without creators | 0.508±0.009 | 0.560±0.003 | 0.547±0.002 | 0.479±0.021 | 0.471±0.012 |
| Users in society | 0.526±0.026~ | 0.547±0.019~ | 0.657±0.011 | 0.422±0.047~ | 0.674±0.016 |
| Human daily actives | 604±23 | 506±16 | 635±12 | 514±41~ | 896±13 |
| Impressions (O) | 9194±291 | 6191±188 | 7654±167 | 8269±453~ | 15462±286 |
| Net revenue (O) | 11.07±0.54~ | 9.06±0.47~ | 4.01±0.17~ | 6.33±0.57~ | 9.02±0.27 |
| Toxic impressions | 0.178±0.027 | 0.040±0.010 | 0.003±0.000 | 0.261±0.063 | 0.174±0.019 |
| Outrage impressions | 0.468±0.045 | 0.130±0.016~ | 0.012±0.001 | 0.611±0.073 | 0.483±0.035 |
| Misinformation impr. | 0.172±0.007~ | 0.199±0.010 | 0.025±0.001 | 0.182±0.007~ | 0.125±0.007 |
| Harassment per 1,000 | 2.56±0.65 | 1.37±0.42 | 3.16±0.77~ | 1.95±0.61 | 3.70±1.03 |
| Outrage posts | 0.256±0.021 | 0.147±0.013 | 0.157±0.010 | 0.336±0.038 | 0.286±0.019 |
| Wellbeing, ever-users | 0.577±0.010 | 0.618±0.004 | 0.656±0.003 | 0.554±0.012 | 0.584±0.013 |
| Survey (O) | 0.544±0.007 | 0.521±0.006 | 0.539±0.005 | 0.542±0.009 | 0.532±0.014 |
| Polarisation | 0.487±0.012 | 0.422±0.005 | 0.384±0.003 | 0.527±0.023 | 0.502±0.017 |
| Informed non-users | 0.583±0.003 | 0.582±0.002 | 0.633±0.001 | 0.576±0.002 | 0.585±0.002 |
| Creators living | 0 | 0 | 0.110±0.013 | 0.245±0.025~ | 0.142±0.024~ |
| Harmful payouts | - | - | 0.000±0.000 | 0.836±0.046 | 0.520±0.040 |
| Bot engagement | 0.231±0.010 | 0.373±0.008 | 0.454±0.006 | 0.248±0.013 | 0.190±0.006 |

Over 90 days, the horizon of a field experiment:

| Metric | ER | Chrono | Integrity | Q&B |
| --- | --- | --- | --- | --- |
| Engagement index (O) | 0.153±0.004 | 0.121±0.002 | 0.137±0.003 | 0.121±0.002 |
| Harm index | 0.274±0.032~ | 0.125±0.019 | 0.294±0.023 | 0.095±0.007 |
| Harm index without polarisation | 0.331±0.037~ | 0.157±0.023 | 0.367±0.028 | 0.119±0.009 |
| Discourse quality | 0.780±0.016 | 0.815±0.004 | 0.843±0.005 | 0.874±0.002 |
| Benefit index | 0.443±0.006 | 0.427±0.002 | 0.449±0.003 | 0.429±0.001 |
| Benefit index without creators | 0.553±0.007 | 0.533±0.002 | 0.562±0.004 | 0.536±0.001 |
| Users in society | 0.607±0.007 | 0.598±0.008 | 0.618±0.006 | 0.622±0.006 |
| Human daily actives | 652±10 | 586±8 | 631±10 | 601±9 |
| Impressions (O) | 9195±224 | 7232±97 | 8212±182 | 7231±138 |
| Net revenue (O) | 12.26±0.37 | 10.60±0.23 | 11.90±0.35 | 11.20±0.37 |
| Toxic impressions | 0.120±0.021~ | 0.018±0.008 | 0.059±0.010 | 0.005±0.001 |
| Outrage impressions | 0.377±0.040~ | 0.058±0.012 | 0.218±0.019~ | 0.019±0.002 |
| Misinformation impr. | 0.129±0.008 | 0.062±0.011 | 0.097±0.005~ | 0.034±0.004 |
| Harassment per 1,000 | 3.45±1.11 | 4.49±0.91 | 7.64±1.36 | 4.80±0.90 |
| Outrage posts | 0.260±0.020 | 0.178±0.014 | 0.229±0.017 | 0.173±0.012 |
| Wellbeing, ever-users | 0.605±0.007 | 0.616±0.004 | 0.632±0.003 | 0.642±0.003 |
| Survey (O) | 0.542±0.014 | 0.517±0.013 | 0.539±0.013 | 0.527±0.014 |
| Polarisation | 0.430±0.004 | 0.420±0.002 | 0.421±0.003 | 0.412±0.002 |
| Informed non-users | 0.598±0.001 | 0.600±0.000 | 0.604±0.001 | 0.609±0.001 |
| Creators living | 0 | 0 | 0 | 0 |
| Harmful payouts | - | - | - | - |
| Bot engagement | 0.250±0.010 | 0.471±0.007 | 0.302±0.009 | 0.469±0.007 |

The other presets, with ER again for reference:

| Metric | ER | Calm | Sub | Paid | Max |
| --- | --- | --- | --- | --- | --- |
| Engagement index (O) | 0.153±0.004 | 0.112±0.002 | 0.121±0.002 | 0.165±0.006 | 0.216±0.008 |
| Harm index | 0.274±0.032~ | 0.334±0.011 | 0.088±0.009 | 0.338±0.041~ | 0.268±0.038 |
| Harm index without polarisation | 0.331±0.037~ | 0.418±0.014 | 0.110±0.011 | 0.400±0.047 | 0.317±0.041 |
| Discourse quality | 0.780±0.016 | 0.830±0.007 | 0.876±0.002 | 0.715±0.026 | 0.797±0.024 |
| Benefit index | 0.443±0.006 | 0.455±0.004 | 0.446±0.004 | 0.504±0.008 | 0.450±0.005 |
| Benefit index without creators | 0.553±0.007 | 0.569±0.004 | 0.538±0.001 | 0.534±0.012 | 0.514±0.008 |
| Users in society | 0.607±0.007 | 0.612±0.008 | 0.628±0.006 | 0.589±0.009 | 0.631±0.006 |
| Human daily actives | 652±10 | 557±9 | 605±9 | 660±12 | 795±12 |
| Impressions (O) | 9195±224 | 6710±129 | 7239±133 | 9884±387 | 12953±470 |
| Net revenue (O) | 12.26±0.37 | 10.29±0.44 | 3.76±0.34 | 8.11±0.28 | 8.26±0.45 |
| Toxic impressions | 0.120±0.021~ | 0.031±0.006 | 0.005±0.001 | 0.171±0.036~ | 0.120±0.021~ |
| Outrage impressions | 0.377±0.040~ | 0.112±0.012 | 0.017±0.002 | 0.550±0.066 | 0.434±0.046 |
| Misinformation impr. | 0.129±0.008 | 0.156±0.008 | 0.026±0.002 | 0.136±0.008 | 0.111±0.005 |
| Harassment per 1,000 | 3.45±1.11 | 2.02±0.53 | 4.69±0.96 | 3.30±0.97 | 4.62±1.09 |
| Outrage posts | 0.260±0.020 | 0.162±0.015 | 0.166±0.012 | 0.324±0.030 | 0.296±0.025 |
| Wellbeing, ever-users | 0.605±0.007 | 0.626±0.003 | 0.648±0.003 | 0.579±0.012 | 0.610±0.011 |
| Survey (O) | 0.542±0.014 | 0.517±0.014 | 0.530±0.015 | 0.548±0.013 | 0.508±0.024 |
| Polarisation | 0.430±0.004 | 0.420±0.002 | 0.412±0.002 | 0.439±0.005 | 0.435±0.006 |
| Informed non-users | 0.598±0.001 | 0.597±0.001 | 0.609±0.001 | 0.596±0.001 | 0.598±0.001 |
| Creators living | 0 | 0 | 0.076±0.018~ | 0.383±0.050~ | 0.194±0.053~ |
| Harmful payouts | - | - | 0.000±0.000 | 0.761±0.040 | 0.352±0.054 |
| Bot engagement | 0.250±0.010 | 0.367±0.006 | 0.469±0.006 | 0.229±0.012 | 0.227±0.012 |

**Targets.** Under the default design 0.526 ± 0.026 of individuals are users over the summary window and 0.499 on day
365, 0.464 on the lowest seed (target 0.40 to 0.90; v0.3: 0.479 and 0.444). The Outrage share of human posts rises from
0.210 on day 0 to 0.256 ± 0.021 in the summary window (below 0.50); it peaks near 0.28 around day 170 and then gives a
little ground. The toxic impression share is 0.178 ± 0.027 (target 0.03 to 0.20): the mean meets the target, but two of
the eight seeds lie above 0.20 (the highest 0.213). Society-wide polarisation starts at 0.42. All three targets hold on
the mean, and the user share lies further from its lower bound than in v0.3.

**The trade-off runs both ways.** Quality and bridging cuts the harm index by 80% (0.408 to 0.082, row 20, ratio of
0.202) for 17% of the engagement index (0.153 to 0.127; moved: v0.3 14%, v0.2 21%). The engagement trap's known solution
cuts the harm index by 38% (row 1: -0.153, -0.173 to -0.133; v0.3 41%) and raises the engagement index by 10% (+0.016,
+0.012 to +0.019; moved: v0.3 +14%). The engagement ranking's lead over Quality and bridging grows over the year in the
default world, more than in v0.3 (1.11 times in the first 30 days, row 12; 1.20 times in the last 95, row 13; v0.3: 1.13
and 1.15), because habit builds, although the engagement ranking ends with 0.109 fewer of society as users (row 13:
-0.129 to -0.090; v0.3 0.137): in the last 95 days 0.522 of society use it, against 0.632 for Quality and bridging. With
weak habit formation and a strong reputation effect (`habit_gain` 0.01, `reputation_sensitivity` 5, `harassment_exit`
0.15) the lead starts at 1.06 (row 14) and reverses to 0.71 in the last 95 days (row 15: -0.027, -0.031 to -0.023; v0.3:
0.60), while the engagement ranking ends with 0.203 fewer of society as users (-0.227 to -0.179; v0.3: 0.23). Both
targets on the trade-off hold.

**What this round changed.** Four changes of hub #23 reach the default world; the conclusions they moved are marked
"moved" below. In short:

- The notification terms (lab #30). Notifications now raise the chance of opening the app by half as much (factor 0.15,
  v0.3 0.3) and no longer lengthen sessions (v0.3 0.5), so that switching them off for a week cuts use as little as the
  field found (fact 7). At the default intensity of 0.6 people open the app less often and build less habit: a human
  daily active reads 15.2 items a day (v0.3 17.9), and the engagement index falls from 0.183 to 0.153 under the
  engagement ranking and from 0.158 to 0.127 under Quality and bridging. The same change means that notifications at
  full strength add less (below).
- The time cost (13.2), halved from 0.115 to 0.06 by its rule. Time on the platform costs half as much, so a day on a
  feed of mixed value is worth more and fewer people leave: under the engagement ranking 0.526 of society are users
  (v0.3 0.479) and ever-users' wellbeing is 0.577 (v0.3 0.542), while Quality and bridging, whose time is mostly worth
  it, barely moves (0.633 and 0.646; v0.3 0.613 and 0.645). Every comparison against `time_cost` 0 is about half its
  v0.3 size (below).
- The ad-free tier (lab #16). Every design now earns from ads and from the tier, whose uptake falls with the price and
  rises with the value people get: the engagement ranking earns 11.07 a day, 8.96 from ads and 2.11 from the tier.
  Quality and bridging and the integrity preset earn more (11.70 and 11.83), because more of their users subscribe
  (13.2). The subscription preset, which shows no ads, now earns only from a tier priced from the default design's
  revenue and taken by about a tenth of its daily actives (13.2): 4.01 a day against 87.91 in v0.3 (moved). It pays a
  living to 0.110 of Creators (v0.3 0.392) and its benefit index falls from 0.524 to 0.460, now close to calm by
  design's 0.448. With `living_income` refitted (13.1) the engagement-paid creators pay a living to 0.245 of Creators
  (v0.3 0.124).
- The recovery of former users (lab #29). People who leave now recover toward their baseline wellbeing, as people on a
  break do, so wellbeing of everyone who ever used the platform rises most where many leave: at v0.3's constants by
  0.011 under the engagement ranking and by less than 0.01 elsewhere (13.2).

The moved findings, with their causes where these runs show them:

- The survey now ranks the engagement ranking above Quality and bridging and the subscription over the year, as it did
  over 90 days (moved; v0.3: level with Quality and bridging, below the subscription). The engagement ranking's
  wellbeing rose the most with the halved time cost, and the survey reads wellbeing, coloured by recent pleasure and
  habit, which the notification terms change; the two were not taken apart.
- Notifications at full strength bring a fifth more engagement under the engagement ranking, not half (moved), since
  they no longer lengthen sessions and raise the chance of opening the app by half as much.
- Report trust cuts Quality and bridging's harassment by about half, not by more than two thirds (moved).
- The integrity preset's harm index is now level with the engagement ranking's (0.403 against 0.408; moved: v0.3 0.361
  against 0.411), its toxic share higher than in v0.3 (0.098; v0.3 0.075); these runs do not separate why.
- With everyone on the bridging alternative feed, the adaptive bot farms now raise misinformation impressions (moved:
  v0.3 lowered them), and at the strongest polish Quality and bridging shows more misinformation than the engagement
  ranking (moved: v0.3 as much).
- Calm by design's harm index now lies measurably below the engagement ranking's (moved: v0.3 undecided).

The comparisons of notifications, time and time limits, all against the defaults on the same seeds, with one session a
day:

- Notifications at full strength (`notification_intensity` 1 against the default 0.6) raise the engagement index by 22%
  under the engagement ranking (row 31: 0.153 to 0.188; moved: v0.3 50%) and by 15% under Quality and bridging (row 32;
  moved: v0.3 46%), and keep more people on the platform: 0.035 and 0.019 more of society as users (v0.3 0.078 and
  0.062). On the good feed they still make people a little happier: under Quality and bridging users' wellbeing rises by
  0.008 (+0.007 to +0.008; v0.3 0.019), ever-users' by 0.008 (+0.007 to +0.008) and the benefit index by 0.003, time not
  worth it falls by 0.006 (-0.011 to -0.002), and the harm index does not move (-0.002, -0.005 to +0.002; v0.3 -0.024).
  Under the engagement ranking they leave wellbeing unchanged (users -0.001, -0.004 to +0.002; ever-users -0.001, -0.004
  to +0.001), raise the benefit index by 0.008 (v0.3 0.016) and lower time not worth it by 0.017 (-0.026 to -0.007; v0.3
  0.053). With one session a day a notification only makes opening the app more likely and builds habit; a session costs
  its time but no interruption (section 16), so on a feed worth more than its time an extra session still pays.
- Without a time cost (`time_cost` 0) the engagement ranking has 13% more engagement (row 33: 0.153 to 0.173; v0.3 30%),
  0.078 more of society as users (0.526 to 0.605; v0.3 0.168), users' wellbeing 0.033 higher (+0.027 to +0.038; v0.3
  0.080) and a share of time not worth it 0.021 lower (0.593 to 0.572; v0.3 0.056). Under Quality and bridging the time
  cost matters less: without it there is 8.5% more engagement (row 34; v0.3 19%), 0.058 more users (v0.3 0.116) and
  wellbeing 0.021 higher (v0.3 0.051), and time not worth it falls from 0.137 to 0.050 (v0.3 0.276 to 0.043), since few
  of its items are worth less than nothing. The harm and benefit indices move by at most 0.006. Every effect is 0.38
  to 0.46 of its v0.3 size; the time cost and the opening factor both halved, and these runs do not separate them.
- A daily time limit for everyone (`time_limit_share` 1) cuts the engagement index by 1.4% (row 35: -0.0021, -0.0037 to
  -0.0005; v0.3 3%) and lowers users' wellbeing by 0.003 (-0.006 to -0.0001; v0.3 +0.002, -0.004 to +0.007); time not
  worth it moves by +0.008 (-0.000 to +0.017). The limit is each person's time budget (6.10), which a session of about
  15 items rarely exceeds (the held-out regretted share is 0.0045, 13.4), so it binds on few days; why it lowers
  wellbeing a little was not taken apart.

The comparisons added in hub #23:

- The checking habits world (section 11: `recheck_rate` 27.5, `notification_pull` 3, `extra_session_share` 0.1) against
  the default world. Under the engagement ranking (row 36) a daily active opens the app 14.76 times a day, 11% of the
  sessions started by notifications, and the engagement index rises 4.9-fold (0.153 to 0.749), time spent 4.7-fold.
  Reading more of a pool of at most 80 candidates, the ranking selects less: Outrage impressions halve (0.468 to 0.238),
  the toxic share falls from 0.178 to 0.095, the harm index from 0.408 to 0.267, and the bot farms' share of the
  engagement from 0.232 to 0.095, since their boosts are fixed. The benefit index rises by 0.044 (+0.027 to +0.061) and
  0.203 more of society are users, but users' wellbeing does not move (-0.002, -0.014 to +0.010), and the held-out
  regretted share rises from 0.0045 to 0.470. Under Quality and bridging (row 37) the setting gives 8.8 sessions a day,
  19% of them from notifications, 3.6 times the engagement, a harm index of 0.042 (0.082), 0.160 more users and users'
  wellbeing 0.087 higher (+0.086 to +0.089), with a regretted share of 0.242: on a feed mostly worth its time, more
  reading helps. 13.2 lists why the setting is not the default.
- The subscription price doubled (`subscription_price` 3.74 against 1.87) under the engagement ranking (row 38). Uptake
  halves (0.0499 to 0.0254 subscribers per daily active, unit elasticity, section 11), the tier's revenue stays where it
  was (+0.04, -0.10 to +0.18), and the people who no longer subscribe see ads again, so ad revenue rises by 1.2% and
  total revenue by 1.3% (+0.146, +0.009 to +0.283). Engagement, wellbeing and the user share do not move (each within
  ±0.0003). At unit elasticity the price moves only the split between ads and the tier.

**Findings of v0.1, as they now stand.**

- A stricter classifier works, which reverses v0.1. At a threshold of 0.3 the toxic share falls by 95% (row 9: 0.178 to
  0.009; v0.1: by 16%). Evasion barely dents the cut and retraining does not carry it: 98% at `evasion_rate` 0.05
  (row 10) and 95% with `classifier_retraining` 0 (row 11). The cost lies in wrongful sanctions, which the harm index
  counts: they rise from 3.3 to 45.4 a day (+42.1, +41.2 to +43.0; v0.3 46.6), fourteen times as many, enough to fill
  their component, so the harm index falls only 22% (0.408 to 0.318; v0.3 25%). A threshold of 0.55 alone cuts the toxic
  share to 0.49 of the baseline (row 3; v0.3 0.45) for six times the wrongful sanctions (19.9 a day).
- Down-ranking in place of removal (v0.1: strict enforcement with heavy down-ranking caught fewer trolls) was not
  re-measured as such. Its cousin shows in the integrity preset: a demoted Attack still reaches its target (6.9), so
  harassment is 7.99 per 1,000 users under Integrity against 2.56 under the Engagement ranking.
- Bridging gamed by bots reproduces again, for the adaptive farms only (moved). With every user on the bridging
  alternative feed, misinformation impressions fall under the shared pool (0.162 to 0.101, row 16) and own side (0.166
  to 0.038, row 17), but rise under the adaptive farms (0.172 to 0.203, row 18: +0.031, +0.000 to +0.063; v0.3: 0.166 to
  0.111), while bots supply 45% to 50% of the engagement. In v0.1 they rose from 0.11 to 0.20 under the shared pool.
  Which change of this round turned the adaptive case was not taken apart.
- The v0.1 trap solution (`w_reply` 0, `w_see_less` 5) fails: brigading bots file see-less marks against popular
  out-group posts (3.5), and the toxic share rises to 1.10 of the baseline (row 5: +0.018, -0.003 to +0.038; v0.3 0.92)
  while the harm index rises 0.220 (+0.176 to +0.264; v0.3 +0.101). The see-less penalty alone raises the harm index by
  0.251 (row 6: +0.212 to +0.289; v0.3 0.177); with `bot_brigading` 0 it lowers it by 0.074 (row 7: -0.088 to -0.060;
  v0.3 0.076). Stated preferences that bots can file backfire at high weight. The new solution is in 13.5. Without any
  cost of toxicity (`churn_base`, `harassment_exit`, `reputation_sensitivity` 0) it costs 6.9% of the engagement index
  (row 8: 0.232 to 0.216; v0.3 7.7%).
- The weak-habit lead reverses, more strongly than in v0.1 and v0.2 but less than in v0.3 (0.71 in the last 95 days,
  against 0.92, 0.82 and 0.60).

**Findings of v0.2, as they now stand.**

- Brigading and report trust. Brigading bots file 33% of all reports and see-less marks under the Engagement ranking and
  63% under Quality and bridging (rows 25, 26; v0.3: 29% and 62%), where the see-less weight invites marks. Weighting
  reports by their reporters' record (`report_trust` 1) does not stop the filing but takes its effect: under the
  Engagement ranking the toxic share falls by 0.030 (row 25: -0.049 to -0.011) and the harm index by 0.031 (-0.050 to
  -0.012); under Quality and bridging harassment falls from 3.68 to 1.73 per 1,000 users (row 26: -1.94, -2.55 to
  -1.34), by 53% (moved: v0.3 71%, from 4.45). It carries the trap's solution: with ten reviews per 1,000 users and
  `w_reply` 0 but without trust, the toxic share falls to 0.61 of the baseline (row 4; v0.3 0.57), short of half; with
  trust to 0.42 (row 1; v0.3 0.40).
- Polish against the quality ranking. Authors and adaptive bots polish where the ranking reads `Q_hat`: 23% of human
  posts under Quality and bridging against 11% under the Engagement ranking (row 20; v0.3 28% and 12%). Quality and
  bridging's harm index is 0.19 of the Engagement ranking's without polish (row 19), 0.20 at the default gain (row 20),
  0.28 at a gain of 0.6 (row 21) and 0.55 at the maximum of 1 (`incentive compare --a engagement-maximiser --b
  quality-bridging --set world.polish_gain=1`, 8 seeds: 0.225 against 0.408). Its harm cut so falls from 81% to 45%,
  0.56 of the cut at the default gain (v0.3: from 80% to 46%, 0.58), so at the maximum it saves less than half the harm.
  The misinformation cut goes faster: misinformation impressions are 0.15, 0.19, 0.40 and 1.21 of the Engagement
  ranking's at those gains, so at the maximum Quality and bridging shows more misinformation than the Engagement ranking
  (+0.036, +0.028 to +0.044; moved: v0.3 as much, +0.019, -0.009 to +0.047). Without its quality weight Quality and
  bridging has 3.2 times the harm (row 22) and 7.6 times the misinformation.
- The survey is gameable and misleads, now over the year as well (moved). Answers mix wellbeing with recent pleasure and
  habit. Over the year the survey reads the Engagement ranking (0.544 ± 0.007) above Quality and bridging (0.538 ±
  0.006; paired -0.006, -0.010 to -0.002, `incentive compare`, 8 seeds), above the subscription (0.539; -0.005, -0.009
  to -0.0005) and above the chronological feed (0.517) and calm by design (0.521), while wellbeing of ever-users ranks
  the Engagement ranking below all four (0.577 against 0.646, 0.656, 0.614 and 0.618; paired gap to Quality and bridging
  +0.069, +0.062 to +0.077). In v0.3 the survey read the Engagement ranking as level with Quality and bridging and below
  the subscription over the year; in v0.2 it ranked it first, as now. Over 90 days it ranks it first as before: 0.542
  against 0.530 for the subscription (paired -0.013, -0.017 to -0.008), 0.527 for Quality and bridging (-0.016, -0.019
  to -0.013), 0.517 for the chronological feed and 0.517 for calm by design, while ever-users' wellbeing ranks it below
  all four (0.605 against 0.648, 0.642, 0.616 and 0.626). Timing games the survey too, though not upward at the
  defaults: collecting every answer right after a session lowers it by 0.101 (row 27: -0.109 to -0.094; v0.3 0.077) at
  unchanged wellbeing, because recent pleasure (a smoothed `tanh(E_imp / 20)`) lies below wellbeing; a platform that
  maximised the survey would keep the lever at 0.
- Account cost's trade-off. Raising it from 0.2 to 1 cuts the bots' share of engagement from 0.232 to 0.086 (row 24),
  misinformation impressions by 0.074 and the harm index by 0.090 (-0.113 to -0.067), and the engagement index by 0.013
  (-0.017 to -0.009). It does not measurably keep people out: the user share changes by -0.004 (-0.024 to +0.016), as in
  v0.3. These runs do not separate why.
- Wrongful sanctions in the harm index. Calm by design's classifier threshold of 0.5 wrongly removes 22.9 posts a day
  against the Engagement ranking's 3.3 (row 23: +19.6, +19.2 to +20.1); without polarisation its harm index is 0.040
  above the Engagement ranking's (+0.020 to +0.060), and with it 0.032 below (0.376 against 0.408, -0.055 to -0.009;
  moved: v0.3 -0.021, -0.049 to +0.007). Its misinformation share is also higher (+0.027; v0.3 +0.046): hidden like
  counts, share friction and depth caps cut human sharing, so bot-boosted posts carry more of the trending signal (bots
  supply 37% of its engagement).
- The chronological feed against the realistic ranked feed. Over the first 90 days, the horizon of Guess et al. (2023),
  the chronological feed has 0.86 of the integrity preset's time spent (row 29), 0.38 of its toxic impressions,
  polarisation lower by 0.0005 (-0.0009 to -0.0001; v0.3 -0.000, -0.001 to +0.000), about a tenth of a percent, and 0.73
  of its misinformation impressions (v0.3 0.79), where the field found more; fact 3 records this as a known limit (13.4,
  section 16). Over the year the chronological feed's polarisation is 6.1% below the integrity preset's (row 30: -0.027,
  -0.033 to -0.022; v0.3 4.4%) and 14% below the Engagement ranking's (row 28: -0.068, -0.076 to -0.061; v0.3 15%). That
  year-long effect is a model prediction, flagged as such: field experiments lasted three months and found none.
- Integrity measures pay off in the model except for harassment and, now, the harm index: against the Engagement ranking
  the integrity preset has 55% of the toxic share (0.098 against 0.178; v0.3 43%), three quarters of the misinformation
  (0.130 against 0.172), the same engagement index within 3% (0.149 against 0.153), more users (0.589 of society against
  0.526), and a harm index of 0.403 against 0.408 (moved: v0.3 0.361 against 0.411): without polarisation its harm index
  is higher (0.474 against 0.428), with three times the harassment (7.99 per 1,000).

### 13.4 Stylized facts

`incentive facts`: 8 paired seeds, `N = 2,000`, 365 days; the same checks run in `crates/incentive-core/tests/facts.rs`
on 4 seeds at `N = 600` for 120 days (240 days for facts 5 and 7, and `N = 2,000` for 365 days for fact 6), where they
judge only the seed means, as regression checks of direction. Effects are paired mean differences, means or ratios of
means, ± the standard deviation of the per-seed effect. The interval is the 95% t-interval of the per-seed effect (of
the per-seed ratios for a ratio, of the signed relative differences for the polarisation check); the verdict is pass
when the interval lies inside the band, fail when it lies wholly outside, undecided otherwise. A failing world is a
world assumption (via `incentive facts --set`) in which the check fails at full size; section 15 explains each. Every
number in this section was measured at the final defaults of hub #23, with one session a day (13.2's final block).

| Fact | Check | Effect | Interval | Band | Verdict |
| --- | --- | --- | --- | --- | --- |
| 1 | Outrage impressions, ER minus chronological | 0.404 ± 0.040 | 0.371 to 0.438 | 0.15 to 0.6 | pass |
| 1 | Reflective value per impression, chronological minus ER | 0.161 ± 0.029 | 0.137 to 0.186 | 0.05 to 0.4 | pass |
| 2 | Rise of the Outrage share of posts under ER | 0.018 ± 0.012 | 0.008 to 0.028 | 0.01 to 0.15 | undecided |
| 2 | That rise minus the rise with `learning_rate` 0 | 0.014 ± 0.013 | 0.002 to 0.025 | 0.01 to 0.15 | undecided |
| 3 | Time spent, chronological over ER | 0.641 ± 0.025 | 0.621 to 0.663 | 0.4 to 0.9 | pass |
| 3 | Relative difference in polarisation, days 80 to 90 | 0.028 ± 0.005 | -0.032 to -0.024 | < 0.05 | pass |
| 3 | Toxic, days 30-90, chrono over Integrity | 0.336 ± 0.093 | 0.252 to 0.407 | 0.5 to 0.9 | fail (limit) |
| 3 | Misinformation, days 30-90, chrono over Integrity | 0.640 ± 0.095 | 0.560 to 0.719 | 1.05 to 3 | fail (limit) |
| 4 | Largest cascades with `reshare_depth_cap` 1 over without | 0.720 ± 0.052 | 0.680 to 0.767 | 0.3 to 0.85 | pass |
| 5 | Vulnerable users who left, Troll share 0.12 minus 0.03 | 0.074 ± 0.025 | 0.053 to 0.095 | 0.03 to 0.3 | pass |
| 6 | Break group's change, real minus placebo, Integrity | 0.057 ± 0.067 | 0.001 to 0.113 | 0.02 to 0.25 | undecided |
| 7 | Time a day, off minus placebo, Integrity | -0.168 ± 0.015 | -0.181 to -0.155 | -0.4 to 0.1 | pass |
| 7 | Sessions a day, off minus placebo, Integrity | -0.326 ± 0.021 | -0.344 to -0.309 | -0.4 to 0.1 | pass |
| 7 | Wellbeing, off minus placebo, Integrity | -0.003 ± 0.001 | -0.004 to -0.002 | -0.15 to 0.15 | pass |

Of the twelve required checks nine pass on their intervals and three are undecided, both of fact 2's and fact 6's; none
fails, and all twelve hold on the seed means. Fact 2's checks lie inside the band on the seed means with intervals that
reach just below its lower bound of 0.01. Fact 6 is undecided as in v0.3 (0.052 after lab #29), since `time_cost` puts
its effect at 0.060 by construction and the paired spread is about 0.07; on 16 seeds it is 0.044 ± 0.067 (0.009
to 0.080), a supplementary line, still undecided. Both known limits ("limit" in the table) fail: the chronological feed
shows a third of the integrity preset's toxic impressions (0.336; v0.3 0.312) and 0.640 of its misinformation
(v0.3 0.702). In v0.3 after lab #29 nine checks were required (fact 7 came with lab #30): eight passed and fact 6 was
undecided. Every fact is judged on 8 seeds. In the checking habits world (11) fact 2 fails both checks and fact 6 fails
(13.2).

The worlds in which each check fails at full size (8 seeds, `incentive facts --set ...`), with the 95% interval and
verdict there; "none" names the strongest world tried, where no world tried fails:

| Fact | Check | World | Effect there | Interval | Verdict |
| --- | --- | --- | --- | --- | --- |
| 1 | Outrage impressions | `outrage_appeal` 0, `arousal_appeal` 0 | 0.129 | 0.121 to 0.138 | fail |
| 1 | Outrage impressions (upper bound) | `outrage_appeal` 2.5 | 0.653 | 0.636 to 0.670 | fail |
| 1 | Reflective value | `outrage_appeal` 0, `arousal_appeal` 0, `time_cost` 0 | -0.017 | -0.027 to -0.007 | fail |
| 2 | Rise | `outrage_appeal` 0.6 | -0.015 | -0.021 to -0.008 | fail |
| 2 | Rise minus no learning | `outrage_appeal` 0.6 | -0.004 | -0.009 to 0.001 | fail |
| 3 | Time spent | `habit_gain` 0, `stop_bias` -5 | 0.967 | 0.947 to 0.989 | fail |
| 3 | Polarisation at day 90 | `polarisation_rate` 0.0005 | 0.090 | -0.098 to -0.082 | fail |
| 3 | Toxic impressions against Integrity | the defaults | 0.336 | 0.252 to 0.407 | fail |
| 3 | Misinformation against Integrity | the defaults | 0.640 | 0.560 to 0.719 | fail |
| 4 | Cascades | nothing impulsive behind sharing, `time_cost` 0 | 0.868 | 0.851 to 0.885 | fail |
| 5 | Vulnerable who left | none: harassment harmless, `time_cost` 0 | 0.024 | -0.001 to 0.050 | undecided |
| 6 | Break minus placebo | `time_cost` 0 | -0.101 | -0.148 to -0.054 | fail |
| 6 | Break minus placebo (upper bound) | `time_cost` 0.2 | 0.403 | 0.344 to 0.463 | fail |
| 7 | Time a day | none: v0.3's opening factor 0.3 (13.1) | -0.305 | -0.321 to -0.288 | pass |
| 7 | Sessions a day | lab #30's opening factor 0.2 (13.1) | -0.421 | -0.440 to -0.401 | fail |
| 7 | Sessions a day | v0.3's opening factor 0.3 | -0.609 | -0.631 to -0.587 | fail |
| 7 | Wellbeing | none: v0.3's opening factor 0.3 | -0.010 | -0.014 to -0.006 | pass |

Every world was rerun at the final defaults:

- Fact 1. Without the appeals the outrage difference fails (0.129) and the reflective value gap is undecided (0.043,
  0.033 to 0.052); with time free as well both fail (0.100 and -0.017), and without the bot farms too they are 0.006
  and -0.178. Its upper bound fails at `outrage_appeal` 2.5 (0.653), as in v0.3 (0.639), and at 3 (0.702).
- Fact 2 is undecided at the defaults. With `outrage_appeal` 0.6 both checks fail (rise -0.015, difference from no
  learning -0.004), and at 0.4 more clearly (-0.030 and -0.010). With `time_cost` 0 the rise passes (0.020, 0.010
  to 0.029) and the difference is undecided (0.017, 0.008 to 0.025). The checking habits world fails both (-0.027 and
  -0.013; 13.2).
- Fact 3. When feeds change neither session length nor habit the chronological feed takes 0.967 of the Engagement
  ranking's time, as in v0.3; with `polarisation_rate` 0.0005 polarisation differs by 9.0% at day 90, and with
  `outrage_appeal` 2.5 by 9.1%, with the toxic ratio there 0.167.
- Fact 4 has a failing world again, as in v0.3: without anything impulsive behind sharing and with time free the
  ratio is 0.868 (0.851 to 0.885); at the default time cost the same world gives 0.868 (0.850 to 0.887), undecided.
- Fact 5 has no failing world: when harassment neither hurts nor drives people out the difference is 0.030 (0.011
  to 0.049), with time free as well 0.024 (-0.001 to 0.050), and with `reputation_sensitivity` 0 instead 0.029 (0.008
  to 0.049), all undecided.
- Fact 6's upper bound fails at `time_cost` 0.2 (0.403) and 0.3 (0.722); at 0.15 the effect of 0.258 lies above the
  band on the seed mean but is undecided (0.213 to 0.303).
- Fact 7. With one session a day, switching notifications off cuts use only through the opening chance's factor
  (6.1) and habit, so only that code constant moves it: at lab #30's 0.2 the sessions check fails (-0.421), at v0.3's
  0.3 more clearly (-0.609), while the time check passes in both (-0.210 and -0.305). The time check therefore has no
  failing world in the default world; in the checking world lab #30 found it failing with v0.3's terms (-0.543,
  13.2's history). The interruption cost acts only on extra sessions, so in the default world it changes nothing (at
  2 every effect equals the default's), and with time free the checks give -0.163, -0.323 and -0.006. The wellbeing
  check fails in no world tried.

Fact 1 against the chronological feed is 8.4-fold in the model (0.460 against 0.055 outrage impressions over the second
half of the year; v0.3: ninefold, 0.451 against 0.049). Its field magnitude is checked in fact 3 against the integrity
preset, where the chronological feed shows 0.336 of the toxic impressions and 0.640 of the misinformation. At the
tests' sizes every required check holds on the seed means: at `N = 600`, seeds 1 to 4, fact 1 gives 0.338 and 0.092
after 120 days, fact 2 0.066 and 0.071, fact 3 0.713 and 0.029, fact 4 0.788, fact 5 0.063 after 240 days, fact 7
-0.158, -0.302 and -0.003 after 240 days, and fact 6 0.063 at `N = 2,000` after 365 days. The depth cap cuts the
largest cascades to 0.720 of their size, not more, because trending recommendations expose original posts at depth 0
and the cap only stops the second hop.

**Held-out target.** Named before tuning and not tuned against: under the default design, the regretted share of time
(items beyond a person's time budget, 9.2) should lie between 0.20 and 0.42, around the 31% of social-media use that
Allcott, Gentzkow and Song (2022) attribute to self-control problems. At the final defaults it is 0.0045 ± 0.0006
(0.0040 to 0.0050, 8 seeds, summary window), two orders of magnitude below the band, and below v0.3's 0.025: with one
session a day a daily active reads 15.2 items, a first session capped at 14 to 26 items by habit (6.1), against time
budgets of 15 to 40, and the smaller opening factor lowers habit and with it the cap. The integrity preset gives 0.0025
and Quality and bridging 0.0002. In the checking habits world (11) the share is 0.470 ± 0.010 (0.461 to 0.479), above
the band; the integrity preset gives 0.388 and Quality and bridging 0.242. There it follows the judged length of a
check, with the rates held: 0.197 (0.183 to 0.211) at `extra_session_share` 0.05, about one item a check, 0.470 at 0.1
and 0.646 (0.643 to 0.650) at 0.2. In plain terms: one session a day leaves the model with almost no compulsive use;
the checking world brings it into or above the field's range by an amount that a judgement sets, and no field figure for
the length of a check pins it. Nothing was tuned on it. The two measures also differ (a fixed daily budget against the
use people would cut if they could commit; section 16). The share of time on items worth less than their time
(`low_value_time`, 9.2: 0.593 under the default design) prices value rather than self-control and is not a test of the
target.

### 13.5 Scenarios

Thresholds as in 14.2. The misinformation surge asks for +0.03 (the best designs found reach about +0.04, because
non-users are only half pulled toward their user ties), and the public square for at least 10% less polarisation. The
engagement trap gained a third goal in v0.2, at most twice the baseline's wrongful removals and demotions: with evasion
fading and the classifier retraining, a stricter classifier alone solved the first two goals in v0.2 (a threshold of
0.55, row 3 of 13.3) at six times the wrongful sanctions; at the final defaults of hub #23 its toxicity goal is
undecided (below). The misinformation surge's Clickbait-heavy bots are `population.bot_clickbait_share` 0.9. Known
solutions, as changes to the scenario's starting design, at full size on 8 paired seeds (`incentive scenario <id>
--solution --seeds 1..8`, the same runs as the report's scenario table), with the goal margins' 95% intervals. At the
final defaults of hub #23 every known solution passes at full size, and all but the public square's passed at test size
as well; the public square's solution gained `w_share` 1 (below):

| Scenario | Known solution | Against the baseline (verdict: pass) |
| --- | --- | --- |
| Engagement trap | `w_reply` 0, `review_capacity` 10, `report_trust` 1 | engagement 1.10x, toxic 0.42x, FP 0.94x |
| Toxicity spiral | `review_capacity` 10, `account_cost` 1, `classifier_threshold` 0.3 | harassment 0.0005x, DAU 0.94x |
| A living for creators | `payout_scheme` FanFunded | living 0.47 against 0.25, harmful payouts 1.1% |
| Misinformation surge | Quality and bridging ranking with notes, `account_cost` 1 | non-users' informedness +0.038 |
| Public square | `w_reply` 0, `w_bridging` 0.6, `w_share` 1 | polarisation 0.85x, engagement 1.10x |

Margins: the trap's engagement +0.039 (+0.035 to +0.042; v0.3 +0.053), toxic share +0.013 (+0.005 to +0.022; v0.3
+0.018), wrongful sanctions +3.46 (+3.14 to +3.78); the spiral's harassment +4.21 (+3.86 to +4.56) and actives +91 (+81
to +100; v0.3 +119); creators +0.228 (+0.205 to +0.251) and payouts +0.189 (+0.187 to +0.192); the surge's +0.008
(+0.006 to +0.010; v0.3 +0.016) above its +0.03; the square's polarisation +0.025 (+0.019 to +0.030) and engagement
+0.046 (+0.043 to +0.049); FP in the table are the wrongful removals and demotions. The trap's toxicity margin is the
thinnest relative to its threshold, about 15% of it, and the surge's is about a quarter of its +0.03, half its v0.3
margin.

The public square's solution of v0.3 (`w_reply` 0, `w_bridging` 0.6) still passes at full size (polarisation +0.016,
+0.010 to +0.021), but at test size its polarisation margin is undecided (+0.0051, -0.0047 to +0.0149). The smallest
change found that passes at both sizes halves the share weight (`w_share` 2 to 1): at test size polarisation +0.0146
(+0.0067 to +0.0225) and engagement +0.047 (+0.041 to +0.054); at full size as in the table. Of the single additions
tried at test size, a larger bridging weight made the margin worse (0.7: +0.0028, 0.8: +0.0017, 1: -0.0032, all
undecided), as did a smaller one (0.5: +0.0019, 0.4: -0.0016), and diversity 0.3 stayed undecided (+0.0048);
`w_see_less` 1 passed narrowly (+0.0118, +0.0014 to +0.0223), `w_share` 1.5 too narrowly to keep (+0.0090, +0.0012 to
+0.0168), and `w_quality` 0.2 passed (+0.0183, +0.0083 to +0.0284) at a larger engagement cost (full size: engagement
0.161 against 0.169 with `w_share` 1). The scenario's goals did not change, since the baseline fails them by
construction.

Naive attempts (lab #12). Each scenario records a plausible single-lever attempt beside its known solution
(`Scenario::naive_attempt`), and the tests check that it fails at test size, with verdict fail rather than undecided. On
the same 8 seeds at full size, and at test size (`N = 600`, six seeds):

| Scenario | Naive attempt | Failing goal | Margin, full size | Margin, test size |
| --- | --- | --- | --- | --- |
| Engagement trap | chronological feed | engagement | -0.029 (-0.033 to -0.025) | -0.028 (-0.033 to -0.023) |
| Toxicity spiral | `review_capacity` 1 | harassment | -2.94 (-3.78 to -2.09) | -2.04 (-3.06 to -1.02) |
| Creators | QualityWeighted payouts | harmful payouts | -0.493 (-0.546 to -0.439) | -0.503 (-0.607 to -0.399) |
| Misinformation surge | community notes | informedness | -0.030 (-0.030 to -0.029) | -0.029 (-0.030 to -0.028) |
| Public square | `w_reply` 0 | polarisation | -0.031 (-0.032 to -0.029) | -0.033 (-0.041 to -0.024) |

The spiral's attempt is three times the reviewers (0.3 to 1, the default). Every other goal of a naive attempt passes.
The chronological feed meets the trap's toxicity and wrongful-sanction goals but keeps only 0.66 of the engagement index
(0.101 against a threshold of 0.130). Paying by the quality classifier gives more Creators a living (0.292 against
0.246) but sends 69% of payouts to Outrage, Clickbait and Attack posts, since it still pays by engagement, only scaled
by the classifier's score. Community notes alone leave non-users' informedness where it was (+0.0002 against the
baseline). Other attempts on the same seeds at full size: `w_reply` 0 alone keeps 0.75 of the trap's toxic share (row 2;
margin -0.044, -0.051 to -0.037); a threshold of 0.55 alone breaks the wrongful-sanction goal (19.9 against 6.5) and
leaves the toxicity goal undecided (0.49 of the baseline; margin +0.002, -0.012 to +0.017); per-view payouts fail the
creators' scenario as the quality-weighted ones do (68.5% harmful, margin -0.485); `w_bridging` 0.6 alone now fails the
public square (0.93x; margin -0.014, -0.018 to -0.011; v0.3 undecided). For the spiral, at test size,
`block_effectiveness` 1 fails (-4.14, -4.98 to -3.30) and ten times the reviewers (`review_capacity` 3, calm by design's
value) is undecided (-0.46, -2.37 to +1.44), as are `review_capacity` 10 (+0.45, -0.38 to +1.28) and `account_cost` 1
(-1.57, -3.31 to +0.16).

One single lever solves the toxicity spiral: a classifier threshold of 0.3 alone passes both goals at both sizes (full
size: harassment 3.30 against a threshold of 4.21, margin +0.91, +0.32 to +1.50, actives +123; test size +0.89, +0.38 to
+1.40). The spiral is therefore not hard for a player who tries the classifier first; its known solution adds reviewers
and an account cost, which take harassment to almost nothing. Its goals were not changed: the baseline fails them, and
14.2 recalibrates only a scenario whose baseline passes.

The tests in `crates/incentive-core/tests/scenarios.rs` check, for every scenario on the same baseline runs at `N = 600`
on six seeds (three left the trap's toxicity interval straddling the threshold), that the baseline does not pass, the
known solution passes every goal, and the naive attempt fails.

### 13.6 Run time

One run of 365 days at `N = 2,000` takes 2.8 seconds single-threaded in release on the owner's desktop (`incentive
run --seeds 1`, wall time including the CSV export, three runs at the final defaults of hub #23, 2.81 to 2.84; v0.3:
2.9, and 3.5 before the time cost, which leaves fewer users). `incentive report` at full size, now with 38 comparisons
and each scenario's naive attempt (976 runs, 568 distinct), takes about 3.3 minutes (196 seconds) on 8 CPUs. The core
and CLI test suites run in about 42 seconds under the dev profile on 8 threads, compile excluded: the fact tests take 24
seconds, almost all of it fact 6 (`N = 2,000`, 365 days), and the scenario tests, which now run each naive attempt as
well, take 17 seconds.

### 13.7 Exploration

`incentive explore` searched every design lever at once (37 genes: one per lever, two for the reshare depth cap) with
the world assumptions at the final defaults of hub #23 (13.2): one session a day, the time cost (7.2), the ad-free tier
(8.3) and the recovery of former users' wellbeing. Each candidate ran on seeds 1 and 2 at `N = 600` for 180 days, and
its metrics are the mean of the two summaries. After each search every design in its file was rerun at the same size on
the fresh seeds 9 to 16 (`--confirm-seeds`, the default), and the file keeps those confirmed metrics beside the
search's. The four searches behind the app's Atlas tab, with search seed 1 and a mutation rate of 0.1 per gene:

| Search | Library algorithm | Generations x batch | Designs simulated | Confirmed | Wall time |
| --- | --- | --- | --- | --- | --- |
| Goodhart, `engagement_index` | `SimpleGA`, elitism 2, from 40 random designs | 25 x 40 | 947 | 18 | 133 s |
| Goodhart, `survey_satisfaction` | the same | 25 x 40 | 949 | 14 | 120 s |
| Map | `MapElites`, 16 x 16 grid, 128 initial designs (the presets first) | 50 x 64 | 3,089 | 108 | 364 s |
| Pareto front | `Nsga2`, population 64 (the presets and random designs) | 40 x 64 | 2,622 | 64 | 347 s |

That is 7,607 designs and 15,214 search runs, plus 1,632 confirmation runs, in 16 minutes on 8 CPUs (v0.3: 20); the
confirmation takes 7 to 46 seconds of each search. The exact commands are in the README. The searches start from the
presets except `max-engagement` (14.1), which comes from an earlier search (v0.3's Pareto search, chosen by the rule
below): the initial designs leave it out by its id, so its levers never reach a search, and the commands regenerate
these files byte for byte whatever the preset holds (checked by running small map and Pareto searches before and after
the preset changed: the files were identical). The map's axes are the engagement index from 0.04 to 0.36 and the harm
index from 0 to 0.64, in cells of 0.02 by 0.04; values outside fall into the edge cells. The engagement axis was kept:
every design found lies inside it (0.055 to 0.232 on the map, up to 0.266 on the front, search values), though its upper
third stays empty; harm reaches 0.676, in the top row.

The reduced size changes the numbers. On seeds 9 to 16 the engagement ranking scores engagement 0.159, harm 0.341 and
benefit 0.485 at `N = 600` over 180 days, against 0.151, 0.408 and 0.411 at full size: a shorter run has less time for
harm and churn to build up. Most front designs move the other way on engagement: 47 of the 64 gain at full size
(by 0.010 on average), while their harm rises by 0.031 on average (front design 60: engagement 0.236 confirmed, 0.246 at
full size; harm 0.382 and 0.504). Every number below that is not marked as a search or confirmed value comes from
full-size runs (`N = 2,000`, 365 days) on seeds 9 to 16, which no search selected on:
`incentive config --from-atlas <file> --design <n>` turned each design into a configuration, and
`incentive run --population 2000 --days 365 --seeds 9..16` ran it; differences are paired by seed, with 95% intervals,
as `incentive compare` computes them.

**The most engaging design found** is v0.3's front design 61, run at the final defaults, which stays the preset
`max-engagement` (14.1): engagement 0.258 at full size, 1.71 times the engagement ranking's 0.151 (+0.107, +0.103
to +0.112). This round's searches did not reach it; it remains the most engaging design any search has found. The rule
was changed in this round, after v0.3's rule had picked front design 60 and the outgoing preset and front designs 57
and 58 were seen to beat that pick: `max-engagement` is the design with the highest full-size engagement
(`N = 2,000`, 365 days, seeds 9 to 16) among all 64 front designs of the Pareto atlas and the outgoing preset run at the
final defaults. v0.3's rule reran at full size only the six front designs with the highest confirmed engagement and took
the highest mean, so that the seeds that measure the winner would not also choose it among all 64. It missed the
full-size best twice. In v0.3, design 51 had a higher full-size mean than the rule's pick but lay outside the six. In
this round the rule picked design 60 (0.246) from designs 64, 62, 59, 63, 60 and 61, and three candidates beat it: front
designs 58 (0.253, +0.007, +0.001 to +0.013) and 57 (0.251, +0.006, +0.002 to +0.009), ninth and seventh on their
confirmed engagement, and the outgoing preset (+0.012, +0.006 to +0.018). The wider rule lets seeds 9 to 16 choose among
65 candidates as well as measure the winner, which may flatter the winner's level a little; its lead over the next
candidate is 0.005. At full size the top candidates rank: v0.3's design 61 0.258; front design 58 0.253 (-0.005, -0.009
to -0.001), 57 0.251 (-0.007, -0.011 to -0.002), 54 0.249 (-0.009, -0.013 to -0.004), 49 0.246 (-0.012, -0.016 to
-0.008), 56 0.246 (-0.012, -0.018 to -0.006), 60 0.246 (-0.012, -0.018 to -0.006) and 59 0.245 (-0.013, -0.018 to
-0.008). Every candidate can be told apart from the winner: each interval excludes 0, design 58's narrowly. The top of
the front is flat at full size: thirteen front designs lie between 0.240 and 0.253, and design 64, first on its
confirmed engagement, falls to 0.227. The final design of the engagement Goodhart search reaches 0.224, 0.034 less than
the winner (-0.038 to -0.030).

Design 61 weighs likes (3.0 against 1.0) and dwell (2.7 against 0.5) far above the engagement ranking, replies (1.0
against 1.5) a little below and shares (0.2 against 2.0) far below, adds a small bridging weight (0.1), a see-less
weight (1.6) and some diversity (0.15), shows half the recommended posts (20% against 40%) and pushes notifications at
full strength. Its classifier removes at 0.75, a little laxer than 0.7; nine reviews per 1,000 users a day, weighted by
reporters' records (`report_trust` 0.7), remove reported posts but carry no sanctions, and blocks do not work (0). It
pays 15% of revenue to creators per view to accounts with at least 42 followers, shows half the ad load, charges 0.05
for an account (0.2) and collects 90% of survey answers right after a session. Against the engagement ranking it has 6%
less harm (0.385 against 0.408, -0.023, -0.037 to -0.009), the same toxic share (0.169 against 0.171, -0.002, -0.012
to +0.009), a little more polarisation (0.501 against 0.486, +0.014, +0.008 to +0.020) and more harassment (4.2
against 3.3 per 1,000, +0.9, +0.4 to +1.3), users' wellbeing 0.015 higher (0.585 against 0.570, +0.009 to +0.020), a
share of time not worth it 0.046 lower (0.542 against 0.588, -0.066 to -0.026) and 0.152 more of society as users (0.673
against 0.521, +0.136 to +0.168). Its benefit index is the same (0.408 against 0.411, -0.003, -0.010 to +0.004): without
the creators term it is 0.041 lower (-0.049 to -0.032), and the creators term (0.146 of Creators earn a living) makes up
the difference. Its net revenue is 8.9 against 10.9 (-1.93, -2.16 to -1.69). Against design 60, the old rule's pick, it
has more engagement (+0.012, +0.006 to +0.018), less harm (-0.119, -0.132 to -0.106), less than half the toxic share
(0.169 against 0.408) and higher users' wellbeing (+0.072, +0.053 to +0.091). On seeds 1 to 8 the report gives it 0.258
± 0.005 engagement and 0.386 ± 0.043 harm (13.3, column Max).

At the search's size it scores 0.254 engagement, 0.347 harm and 0.434 benefit on seeds 1 and 2, below the most engaging
front designs on engagement (0.266, search values) but dominated by none of them: the search did not reach that part of
the front. On seeds 9 to 16 at that size it scores 0.238, below the confirmed engagement of front designs 64 (0.243)
and 62 (0.239) and level with design 59 (0.238). So at the search's size it would not lead even on the same seeds; it
leads only at full size.

**Lowest harm at a given share of that engagement.** Of the 64 front designs, rerun at full size:

| Share of its engagement | At least | Lowest harm on the front | Design | Its engagement | Confirmed values |
| --- | --- | --- | --- | --- | --- |
| 80% | 0.206 | 0.112 | front design 42 | 0.208 | 0.134 at 0.191 (design 33) |
| 90% | 0.232 | 0.259 | front design 47 | 0.232 | 0.248 at 0.218 (design 53) |
| 100% | 0.258 | none | none | at most 0.253 (design 58) | 0.359 at 0.238 (design 59) |

The last column applies the same shares to the confirmed values at the search's size, against the most engaging design's
own engagement at that size on the same seeds (0.238). No front design reaches the most engaging design's engagement at
full size. The nearest, design 58, has 98% of it (-0.005, -0.009 to -0.001) with 19% more harm (0.458, +0.073, +0.067
to +0.079) and twice the toxic share (0.330 against 0.169); design 54 has 97% (-0.009, -0.013 to -0.004) at the same
harm (+0.001, -0.009 to +0.011), and design 49 has 95% (-0.012, -0.016 to -0.008) at a harm that cannot be told apart
(-0.010, -0.028 to +0.008), though both have a higher toxic share (0.299 and 0.248). Giving up 10% of the engagement
removes a third of the harm (design 47: -0.026, -0.031 to -0.021; harm -0.126, -0.149 to -0.102), giving up 11%
removes 65% (design 43: -0.028, -0.032 to -0.024; harm -0.251, -0.268 to -0.234), giving up 19% removes 71%
(design 42: -0.050, -0.054 to -0.046; harm -0.273, -0.290 to -0.257), and giving up 22% removes 79% (design 40: -0.057
and -0.304). The trade-off steepens at the top again: from design 40 to design 43 harm rises by 0.053 for 0.029 of
engagement, from design 43 to design 49 by 0.241 for 0.016. The front holds harm under 0.10 up to an engagement of 0.201
(design 40), under 0.15 up to 0.230 (design 43) and under 0.30 up to 0.240 (design 53, harm 0.268). Of the 64 front
designs, 40 beat the engagement ranking on engagement, harm and benefit at once at full size (35 on their confirmed
values).

**High engagement with low harm, against the most engaging design.** Front design 40, the most engaging front design
with harm under 0.10, is the clearest case: at full size

| Design | Eng. | Harm | Benefit | No creators | Toxic | Wellbeing | Ever-users | Low value | Revenue | Wrongful |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Engagement ranking | 0.151 | 0.408 | 0.411 | 0.514 | 0.171 | 0.570 | 0.578 | 0.588 | 10.9 | 3.1 |
| Most engaging found (v0.3's 61) | 0.258 | 0.385 | 0.408 | 0.473 | 0.169 | 0.585 | 0.587 | 0.542 | 8.9 | 1.8 |
| Front design 58 | 0.253 | 0.458 | 0.456 | 0.451 | 0.330 | 0.555 | 0.564 | 0.623 | 8.5 | 0.0 |
| Front design 60 | 0.246 | 0.504 | 0.431 | 0.412 | 0.408 | 0.513 | 0.533 | 0.739 | 9.6 | 0.0 |
| Front design 49 | 0.246 | 0.375 | 0.496 | 0.480 | 0.248 | 0.605 | 0.604 | 0.532 | 10.6 | 0.5 |
| Front design 43 | 0.230 | 0.134 | 0.519 | 0.541 | 0.051 | 0.667 | 0.660 | 0.250 | 8.3 | 3.8 |
| Front design 40 | 0.201 | 0.081 | 0.541 | 0.561 | 0.035 | 0.706 | 0.696 | 0.126 | 9.6 | 0.3 |
| Goodhart, engagement, final | 0.224 | 0.551 | 0.367 | 0.402 | 0.393 | 0.494 | 0.522 | 0.773 | 4.6 | 0.0 |
| Goodhart, survey, final | 0.199 | 0.223 | 0.492 | 0.557 | 0.089 | 0.689 | 0.679 | 0.249 | 3.0 | 0.0 |

(Benefit without the creators term, wellbeing of users and of ever-users, the share of time not worth it
(`low_value_time`, 9.2), net revenue, wrongful removals and demotions a day.) Against the most engaging design,
design 40 gives up 22% of the engagement (-0.057, -0.061 to -0.054) for 79% less harm (-0.304, -0.323 to -0.286), a
fifth of the toxic share (0.035 against 0.169), 0.098 less polarisation, 0.121 more wellbeing of users (+0.113
to +0.130), 0.416 less time not worth it (-0.437 to -0.394) and 0.020 more of society as users (+0.012 to +0.027). Part
of its benefit lead (+0.133, +0.126 to +0.140) is the creators term: without it the lead is +0.088 (+0.081 to +0.094).
Its net revenue is 0.70 higher (+0.50 to +0.89). Design 43 gives up 11% (-0.028, -0.032 to -0.024) for 65% less harm
(-0.251, -0.268 to -0.234), with under a third of the toxic share (0.051), users' wellbeing 0.082 higher (+0.075
to +0.090), 0.058 more of society as users (+0.052 to +0.065) and a benefit lead of 0.111 (+0.103 to +0.119; without the
creators term +0.067, +0.061 to +0.073), but it wrongly removes or demotes 2.0 more posts a day (3.8 against 1.8, +1.73
to +2.24) and earns 0.61 less (-0.73 to -0.48). Against the engagement ranking design 40 has 33% more engagement
(+0.050, +0.046 to +0.053) and 80% less harm (-0.327, -0.351 to -0.304), and users' wellbeing 0.136 higher (+0.126
to +0.145); the creators term supplies most of its benefit lead there: +0.130 (+0.119 to +0.140) with creators, +0.047
(+0.037 to +0.057) without. Meaningful interactions nearly vanish (0.002 a user against 0.047), as on most of the
engaging front designs; the front's benefit comes from the other components. Net revenue is 1.2 below the engagement
ranking's (-1.23, -1.56 to -0.90) although design 40 pays 40% of its revenue to creators: its gross revenue is 16.1
against 10.9, 29% of it from the ad-free tier (the engagement ranking: 19%). It keeps the classifier (removing at 0.8,
laxer than 0.7), graduated sanctions and the full ad load, and wrongly removes or demotes 0.3 posts a day against 3.1.

**What front design 40 rests on.** `incentive factorial --a engagement-maximiser --b front-40.json --levers
notification_intensity,w_reply,w_bridging,review_capacity,payout_share,account_cost --population 2000 --days 365` (64
cells, seeds 9 to 16; ± is the 95% interval) sets every subset of six of its levers back to the engagement ranking's
values: notifications 1.0 (ranking 0.6), `w_reply` 0.3 (1.5), `w_bridging` 0.3 (0), `review_capacity` 5.1 (1), 40% of
revenue paid to creators (0) and `account_cost` 0.75 (0.2). The six were chosen by setting each of the 19 levers with an
effect in which design 40 differs from the engagement ranking back one at a time: they had the largest effects on
engagement, harm, benefit or users' wellbeing; the share of recommended posts (-0.037 harm alone) and the share prompt
(-0.027 harm, -0.008 engagement) came next. With all six back the design keeps engagement 0.181, harm 0.359,
benefit 0.416 and users' wellbeing 0.595: still 20% more engagement than the engagement ranking (+0.030, +0.027
to +0.034) with 12% less harm (-0.049, -0.059 to -0.040), the same benefit (+0.005, -0.001 to +0.010) and more wellbeing
(+0.025, +0.021 to +0.028). The six levers carry 0.020 of its engagement lead of 0.050, 0.278 of its harm cut
of 0.327, 0.125 of its benefit lead of 0.130 and 0.111 of its wellbeing lead of 0.136. Main effects (from the engagement
ranking's value to design 40's) and Shapley values:

- Engagement: notifications at full strength add +0.037 ± 0.001 (main effect; Shapley +0.036 ± 0.001), 186% of the six
  levers' net gain, and the low reply weight +0.008 ± 0.001, while the account cost (-0.016 ± 0.002) and the reviews
  (-0.006 ± 0.001) cost engagement. The largest interactions: reviews with the account cost -0.010 ± 0.002, payouts with
  the account cost +0.005 ± 0.002, notifications with the account cost -0.005 ± 0.001. So notifications supply about 73%
  of the design's engagement lead over the engagement ranking (0.036 of 0.050); the levers outside the six, among them
  the ranking weights, the few recommended posts and the share prompt, add 0.030 together, and the account cost and the
  reviews take back 0.023.
- Harm: bridging carries 32% of the six levers' cut (Shapley -0.088 ± 0.007), the account cost 30% (-0.084 ± 0.014), the
  reviews 30% (-0.083 ± 0.012) and the low reply weight 21% (-0.059 ± 0.008), notifications 0% (-0.001 ± 0.005), while
  payouts add harm (+0.037 ± 0.012, -13%). The largest interactions: reviews with the account cost -0.096 ± 0.014
  (together they cut harm by far more than the sum of the two alone), reviews with payouts -0.050 ± 0.012, bridging with
  payouts -0.040 ± 0.010 and the reply weight with payouts -0.023 ± 0.008 (payouts add harm under the engagement
  ranking's moderation and weights and lower it under design 40's), payouts with the account cost +0.038 ± 0.013,
  bridging with the reviews +0.023 ± 0.010 and with the account cost +0.022 ± 0.007 (each does part of the other's
  work).
- Benefit: payouts 59% (+0.074 ± 0.006), the account cost 20% (+0.025 ± 0.006), bridging 16% (+0.020 ± 0.003),
  notifications 6% (+0.008 ± 0.003), the reviews 6%, the reply weight -7%; the reviews with the account cost +0.017
  ± 0.005, the reply weight with payouts +0.014 ± 0.003 and bridging with payouts +0.013 ± 0.005.
- Users' wellbeing (decomposed from the same runs): bridging 31% (+0.034 ± 0.004), the account cost 27% (+0.030
  ± 0.005), the reply weight 24% (+0.027 ± 0.005), the reviews 18% (+0.020 ± 0.004), notifications 10% (+0.011 ± 0.003)
  and payouts -10% (-0.011 ± 0.006) alone, but payouts with bridging +0.022 ± 0.005 and with the reviews +0.021 ± 0.004,
  and the reviews with the account cost +0.023 ± 0.005. Notifications also lower the share of time not worth it (main
  effect -0.024 ± 0.010).

So design 40 rests on moderation and the price of an account for harm, which work together far more than alone, on its
ranking (a bridging weight and a low reply weight) for harm and wellbeing, on notifications for engagement, and on
payouts for benefit. Notifications help it a little on wellbeing and leave its harm unchanged, as under Quality and
bridging (13.3, row 32): its feed is worth its time. The other designs were not taken apart this way.

**What notifications do on the front.** Every one of the 64 front designs keeps notifications at 0.8 or more (61 at full
strength), as both final Goodhart designs do. Setting notifications back to 0.6 in four front designs and in the most
engaging design, with the rest as found, gives the effect of full strength (± is the 95% interval):

| Design | Engagement | Harm | Users' wellbeing | Time not worth it |
| --- | --- | --- | --- | --- |
| Most engaging found (v0.3's 61) | +0.053 ± 0.002 | -0.013 ± 0.012 | +0.011 ± 0.006 | -0.038 ± 0.016 |
| Front design 60 | +0.052 ± 0.004 | +0.007 ± 0.006 | -0.011 ± 0.006 | -0.012 ± 0.014 |
| Front design 49 | +0.045 ± 0.005 | +0.008 ± 0.010 | +0.002 ± 0.009 | -0.005 ± 0.025 |
| Front design 43 | +0.037 ± 0.001 | -0.013 ± 0.005 | +0.014 ± 0.003 | -0.029 ± 0.011 |
| Front design 40 | +0.030 ± 0.002 | -0.010 ± 0.007 | +0.018 ± 0.003 | -0.025 ± 0.017 |

The front and the most engaging design agree with the report's rows 31 and 32. Notifications raise engagement by 18%
to 27% (the most engaging design without them would have 0.205, still 1.36 times the engagement ranking's). Where the
feed is mostly worth its time (designs 40 and 43, time not worth it 0.13 and 0.25) they raise users' wellbeing by 0.014
to 0.018 and lower harm a little; on design 49 (0.53) they leave both unchanged; on the most engaging design (0.54, a
little below the engagement ranking's 0.59) they raise users' wellbeing by 0.011 and lower harm by 0.013; on design 60,
whose share of time not worth it (0.74) lies above the engagement ranking's, they lower wellbeing by 0.011 and raise
harm by 0.007. The model still charges nothing for the interruption itself at the defaults (`interruption_cost` 0,
section 16).

**The winner's curse.** Confirmed values (seeds 9 to 16) against search values (seeds 1 and 2), both at the search's
size, as mean differences. The seed sets differ too: the seven presets, which no search selected, lose 0.005
engagement, 0.017 harm, 0.006 benefit and 0.029 survey satisfaction from seeds 1 and 2 to seeds 9 to 16. Against that
shift:

- The 108 map elites lose 0.005 engagement (in 92% of them), 0.016 harm and 0.008 benefit, 0.002 more benefit than the
  shift: the map as a whole is inflated little. The 16 elites with the highest benefit lose 0.007 benefit (11
  of 16), 0.002 more than the shift; the 16 with the lowest harm lose only 0.002 harm (lower in 10 of 16), 0.015 worse
  than the shift, and 0.007 benefit (14 of 16), 0.002 more than the shift.
- The 64 front designs lose 0.013 engagement (63 of them), 0.009 beyond the shift, 0.032 harm (0.015 more than the
  shift) and gain 0.006 benefit (0.012 against the shift). The six most engaging front designs lose 0.023 engagement on
  average (all six), 0.018 more than the shift, about 9% of their search value; their harm falls by 0.028 and their
  benefit rises by 0.032, because selection for engagement also picked draws with high harm and low benefit.
- The final engagement Goodhart design falls from 0.253 to 0.226 (-0.027, 0.022 beyond the shift), the final survey
  design from 0.645 to 0.592 (-0.054, 0.025 beyond the shift).

A search value is thus optimistic by 0.01 to 0.03 in whatever the search pushed on hardest; ranks hold better than
levels, though not at the flat top of the front, where full size reorders designs that differ by less than 0.01:
design 64, first on its confirmed engagement, is 21st of the front at full size, and design 58, ninth, is first on the
front.

**Goodhart runs.** Maximising the engagement index alone raised the best design's engagement from 0.135 to 0.253 over 25
generations (search values). At full size, generation 0's best against the final design: engagement 0.135 to 0.224
(+0.089, +0.083 to +0.094), harm 0.107 to 0.551 (+0.444, +0.417 to +0.470), toxic share 0.016 to 0.393,
polarisation 0.397 to 0.557, users' wellbeing 0.676 to 0.494 (-0.182, -0.202 to -0.162), time not worth it 0.163
to 0.773, benefit 0.476 to 0.367, misinformation 0.06 to 0.16 and 0.140 less of society as users. The search started
from a random design without a classifier, switched it on in generation 1 only, cut human review from 6.9 per 1,000
users to 1.8, pushed notifications to 0.95 from generation 5 and to full strength from generation 15, showed no
recommended posts from generation 8, dropped the bridging weight and in the end the time limits, and weighted likes
(2.9), shares (3.0), dwell (2.2) and followed accounts (1.3). The survey rose with it, from 0.502 to 0.569
(+0.067, +0.056 to +0.078), while wellbeing fell nearly three times as far. The final design is worse than the
engagement ranking on harm (+0.142, +0.131 to +0.153), toxicity, benefit and wellbeing (-0.076, -0.089 to -0.063), and
below the most engaging design found on engagement (-0.034, -0.038 to -0.030): it keeps users' attention but not the
users (0.506 of society against its 0.673). Its notifications at full strength rather than 0.6 bring 0.052 of its
engagement (+0.046 to +0.059) and 0.037 of its survey (+0.030 to +0.044) and leave wellbeing unchanged (-0.006, -0.018
to +0.006).

Maximising survey satisfaction alone raised it from 0.531 to 0.645 (search values). At full size: survey 0.515 to 0.612
(+0.097, +0.090 to +0.104), engagement 0.086 to 0.199, benefit 0.420 to 0.492 and 0.270 more of society as users; this
time harm fell, from 0.307 to 0.223 (-0.085, -0.117 to -0.052), users' wellbeing rose from 0.607 to 0.689
(+0.082, +0.073 to +0.092) and misinformation fell from 0.23 to 0.05, but the toxic share doubled, from 0.043 to 0.089
(+0.046, +0.031 to +0.062). On the search's own seeds harm rose (0.275 to 0.321, search values), on the confirmation
seeds it fell (0.280 to 0.237): the search did not push harm either way. The search kept the classifier off in every
generation, kept graduated sanctions, cut review from 5.7 per 1,000 users to 0.8, kept ads at 0.05 in most generations,
the last included (89% of its revenue comes from the tier, and its net revenue is 3.0), and pushed notifications to full
strength in generation 14 and from generation 20 on. Against the engagement ranking the final design has more engagement
(0.199 against 0.151), less harm (-0.186, -0.204 to -0.167), a lower toxic share (0.089 against 0.171), more benefit
(+0.081) and more wellbeing (+0.119, +0.112 to +0.126). It does not exploit the timing: `survey_after_session` stayed
at 0 in all 25 generations' best designs, and setting it to 1 in the final design lowers the survey by 0.141 (-0.150
to -0.132) with every truth metric unchanged. Notifications at full strength instead of 0.6 raise its survey by 0.023
(+0.019 to +0.028) and users' wellbeing by 0.016 (+0.011 to +0.021); wellbeing, weighted 0.6 in an answer (9.1),
accounts for about 0.010 of the survey's gain, pleasure and habit for the rest. In this world the survey was a fair
proxy: maximising it raised wellbeing and cut harm, though it let toxicity double, where maximising engagement raised
harm fivefold.

**What changed from v0.3.** Four changes of the round reach the atlas: the time cost re-fit (0.115 to 0.06), the
notification recalibration (opening factor 0.3 to 0.15, session length term 0.5 to 0), the ad-free tier with its
calibrated price, uptake and `living_income` (0.025 to 0.015), and the recovery of former users' wellbeing. Their
separate effects were measured by setting `time_cost` back to 0.115 and `living_income` back to 0.025, one at a time, in
the engagement ranking, designs 60 and 40, and v0.3's designs 61 and 32 run at the final defaults; the notification
terms and the recovery are constants of the code, so no run here sets them back.

- The most engaging design found is still v0.3's design 61, with a smaller lead: at the final defaults it has 1.71 times
  the engagement ranking's engagement with 6% less harm (v0.3: 2.14 times with 14% less); the model cut its engagement
  from 0.382 to 0.258. This round's searches did not reach it. Their most engaging front design at full size, design 58,
  has 1.68 times the engagement ranking's engagement with 12% more harm and nearly twice the toxic share, and the old
  rule's pick, design 60, 1.63 times with 24% more harm, 2.4 times the toxic share and lower wellbeing.
- The time cost re-fit. At 0.115 instead of 0.06, the five designs have 0.007 to 0.026 less engagement, users'
  wellbeing 0.031 to 0.043 lower and 0.034 to 0.096 less of society as users; harm moves by at most 0.021 (the
  engagement ranking: +0.021, +0.006 to +0.036). So halving the time cost raised engagement on the two most engaging of
  the five (v0.3's design 61 and design 60) by about a tenth and wellbeing everywhere by 0.03 to 0.04; it did not cut
  v0.3's design 61's engagement.
- The notification recalibration. Notifications at full strength now raise engagement by 18% to 27% on the front
  (v0.3: 43% to 49%), as under the engagement ranking (row 31: 22%; v0.3 50%). On the same two v0.3 designs at the final
  defaults they add 0.053 (design 61; v0.3 +0.125) and 0.032 (design 32; v0.3 +0.099). The recalibrated terms are the
  change that acts on notifications directly, and the halved time cost would raise their effect rather than cut it, but
  these runs do not separate them from the tier and the recovery. Notifications still raise wellbeing on a feed worth
  its time and on the most engaging
  design; on design 60, whose time is mostly not worth it, they lower it.
- The ad-free tier. The v0.3 front lived on subscriptions: all 64 designs kept ads at 0.5 or less (median 0.33), and
  design 32 took 92% of its revenue from subscribers. Under the tier 60 of the 64 front designs keep ads at 0.7 or more
  (median 0.85), and the tier brings a median 30% of their revenue (8% to 78%). Revenue reaches the search's objectives
  only through the creators term of the benefit index, which payouts fund. v0.3's design 32 at the final defaults
  matches design 40's engagement (+0.001, -0.002 to +0.004) with a little less harm (0.068, -0.013, -0.021 to -0.004),
  but its net revenue is 4.4 against design 40's 9.6 (v0.3: 40.9), its creators living 0.269 against 0.461, and its
  benefit index 0.049 lower. The lower `living_income` that came with the tier raises the share of Creators living
  by 0.06 to 0.14 on the paying designs and their benefit index by 0.015 to 0.024; engagement, harm and users' wellbeing
  move by at most 0.013.
- The recovery of former users' wellbeing changes ever-users' wellbeing, which no search objective reads; no run here
  separates its effect on the atlas.
- The coexistence trade. v0.3's design 32 gave up 18% of the most engaging design's engagement for 86% of its harm; now
  design 40 gives up 22% for 79%, design 42 19% for 71%, design 43 11% for 65% and design 47 10% for a third. No front
  design matches the most engaging design's engagement (v0.3: design 51 matched it with 39% less harm); design 49,
  at 95% of it, has as much harm. The trade-off steepens at the top again, which in v0.3 it no longer did. The cut now
  rests on moderation and the account cost together with bridging, where v0.3's design 32 rested on bridging and a feed
  of followed accounts.
- The map filled 108 of 256 cells (42%, v0.3: 156), with engagement from 0.055 to 0.232 and harm from 0.03 to 0.68
  (search values); v0.3 reached 0.33. Its high-engagement edge stays high on the harm axis: its five elites above an
  engagement of 0.20 all have harm of 0.35 or more, and its lowest confirmed harm at a confirmed engagement of at
  least 0.18 is 0.336, where the front holds 0.112. The map's fitness is benefit, so it keeps designs that pay creators
  over designs that do little harm; benefit across its cells follows the share of Creators earning a living as closely
  as in v0.3 (correlation 0.84, v0.3: 0.85).
- The Goodhart runs. The engagement search ended lower (0.224 at full size against 0.330) but nearer the most engaging
  design found (-0.034 against -0.052), whose engagement fell further. The survey search, which in v0.3 raised harm
  (0.165 to 0.361) with wellbeing flat, now cuts harm (0.307 to 0.223) and raises wellbeing (+0.082), and part of its
  notifications' survey gain is now wellbeing, where in v0.3 none was.

**Limits of these results.** One world (the defaults), one search seed per search, two seeds per candidate and a reduced
size: the size reorders the flat top of the front, and the most engaging design known, v0.3's design 61, is not on this
front: this round's searches did not reach it, and at the search's size it would not lead. "Most engaging found" is the
best of the 65 candidates the rule above compares at full size, not a bound: a longer or larger search may find more,
and seeds 9 to 16 both choose the winner among the candidates and measure it. The factorial covers six levers of one
design; the levers outside it carry 0.030 of design 40's engagement lead together, untaken apart. The time cost is one
value of time for everyone, calibrated on the integrity preset (section 16); at the defaults a notification costs only
the time it brings and not the interruption (`interruption_cost` 0), which still flatters the lever that carries
design 40's engagement, and the checking habits world (section 11), where people open the app many times a day, was not
searched. Net revenue rests on the ad rate and the tier's price and uptake, calibrated to the default design only
(13.2); the creators term rests on `living_income`, which sets how many Creators a payout supports, and payouts cost
nothing in any score but net revenue. Levers without effect in a design (the alternative feed when nobody uses it, the
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
- Most engaging design found (`max-engagement`): v0.3's front design 61, the most engaging design any search has found,
  by the rule of 13.7 (the highest full-size engagement among the 64 front designs of the current Pareto atlas and this
  preset); this round's searches did not reach it. Every lever as found: `w_like` 3.0, `w_reply` 1.0, `w_share` 0.2,
  `w_dwell` 2.7, `w_bridging` 0.1, `w_see_less` 1.6, `w_recency` 0.1, `w_in_network` 0, `recommended_share` 0.2,
  `diversity` 0.15, `classifier_threshold` 0.75, `review_capacity` 9.0, `report_trust` 0.7, `sanctions` None,
  `payout_share` 0.15 with PerView payouts to accounts with at least 42 followers (`payout_threshold_followers`),
  `block_effectiveness` 0, `notification_intensity` 1.0, `account_cost` 0.05, `ads_share` 0.5 and
  `survey_after_session` 0.9, the other levers at the defaults; `demote_penalty` 2.1 and `noted_penalty` 2.4 have no
  effect in it (its classifier removes, and it has no community notes). No one proposes it: it shows what maximising
  engagement looks like in this model. It has about 70% more engagement than the Engagement ranking, with about 5% less
  harm, the same toxic share and slightly higher wellbeing (13.3, column Max); a front design with 89% of its engagement
  has a third of its harm (13.7). `incentive explore` leaves it out of its initial designs, so the searches regenerate
  the same atlas.

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

Each scenario also records a naive attempt (lab #12): a plausible change of one lever that fails it, kept beside the
known solution and checked in the tests to fail, with verdict fail at test size, so that a scenario is shown not to be
solved by the first obvious move. The attempts are the chronological feed for the engagement trap, three times the
reviewers (`review_capacity` 1) for the toxicity spiral, payouts weighted by the quality classifier for the creators,
community notes for the misinformation surge and `w_reply` 0 for the public square (13.5). A naive attempt is chosen,
not calibrated: a scenario's goals are never weakened to make one fail. The toxicity spiral can be solved with one
lever, a classifier threshold of 0.3 (13.5).

## 15. Stylized facts the model is checked against

Each fact becomes an automated test on small runs (`N` = 600, 120 days, 4 seeds, paired by seed; facts 5 and 7 run 240
days, fact 6 runs at `N` = 2,000 for 365 days) and a check at full size in the CLI's `facts` command, which prints
the effect sizes. Each check is a band: a lower bound with a margin that a world without the mechanism misses, and an
upper bound where an effect could be implausibly large. `incentive facts --set path=value` checks the facts in another
world; for each fact below a world is named in which it fails, so the check is not true by construction. At the final
defaults of hub #23, with one session a day, no required check fails; both checks of fact 2 and fact 6 are undecided
(13.4). Two qualifications apply to the failing worlds. First, fact 5 has no failing world among those tried (its
strongest worlds stay undecided). Second, fact 7's sessions check fails only in worlds of a code constant (the opening
chance's notification factor), and its time and wellbeing checks fail in no world tried at the defaults. In the
checking habits world (11) fact 2 and fact 6 fail (13.2). Effects at the defaults are in 13.4.

1. Engagement ranking amplifies divisive content that users do not value. Against a chronological feed, the Engagement
   ranking shows more Outrage impressions (paired difference of the share over the second half of the run between 0.15
   and 0.6), and the reflective value of what it shows is lower (between 0.05 and 0.4 per impression). Milli et al.
   (2025) found in an audit of Twitter's ranking that the engagement-based algorithm amplified emotionally charged,
   out-group hostile content that users did not prefer. Both checks pass at the defaults (difference 0.404, reflective
   value 0.161). Fails when people are not drawn by arousal or in-group hostility (`outrage_appeal` and `arousal_appeal`
   0: difference 0.129; the reflective value gap is undecided there, 0.043, and fails with time free as well, -0.017),
   and at the upper bound when in-group hostility pulls about twice as hard (`outrage_appeal` 2.5: 0.653), as in v0.3
   (0.639). In the checking habits world the difference falls to 0.182, near the lower bound, since with about 47 items
   read out of at most 80 candidates the ranking concentrates outrage less. The upper bound guards against a feed taken
   over by outrage; field audits compare a chronological feed with a real ranker, which carries integrity measures, so
   the field magnitude is checked against the integrity preset in fact 3.
2. Social reward teaches outrage. Under the Engagement ranking the Outrage share of posts rises from the first fifth of
   the run to the last (by 0.01 to 0.15), and by 0.01 to 0.15 more than with `learning_rate` 0. Brady et al. (2021).
   Both checks are undecided at the defaults, inside the band on the seed means: the share rises by 0.018 (0.008
   to 0.028) and by 0.014 (0.002 to 0.025) more than without learning (13.4). Fails when in-group hostility pulls half
   as hard (`outrage_appeal` 0.6: rise -0.015, and -0.004 against no learning), as in v0.3 (-0.020 and -0.008), and in
   the checking habits world (rise -0.027, and -0.013 against no learning; 13.2), where the share falls over the year.
3. A chronological feed cuts time spent, barely moves polarisation in three months, and shifts exposure from incivility
   toward untrustworthy content. Guess et al. (2023) found on Facebook and Instagram during the 2020 US election that
   the chronological feed cut time spent and left polarisation and other attitudes unchanged over three months; it also
   raised the share of content from untrustworthy sources (by about two thirds on Facebook) and lowered the share of
   uncivil content, by tens of percent rather than by a factor. The checks: time spent under chronological over the
   Engagement ranking between 0.4 and 0.9; society-wide polarisation at days 80 to 90 within 5% of the Engagement
   ranking's; and, against the integrity preset (14.1) as the realistic ranked feed, over days 30 to 90 (the field
   experiment lasted three months), toxic impressions chronological over integrity between 0.5 and 0.9 (a cut by tens of
   percent), and misinformation impressions chronological over integrity between 1.05 and 3. The last two are known
   limits (section 16) and the tests do not require them: the model fails both at the defaults (toxic 0.336, a threefold
   cut; misinformation 0.640). The first two pass (time 0.641, polarisation 2.8%). Failing worlds: when feeds change
   neither session length nor habit (`habit_gain` 0, `stop_bias` -5) time spent falls only to 0.967 of the Engagement
   ranking's, as in v0.3; with three times the polarisation per hostile impression (`polarisation_rate` 0.0005)
   polarisation differs by 9.0% at day 90; with `outrage_appeal` 2.5 the toxic ratio drops to 0.167.
4. Forwarding limits slow viral spread. With `reshare_depth_cap` 1 the largest share cascades shrink to between 0.3 and
   0.85 of their size without a cap. de Freitas Melo et al. (2019) modelled WhatsApp's limits. Fails when nothing
   impulsive drives sharing and time is free (`outrage_appeal`, `arousal_appeal`, `social_proof` and `time_cost` 0:
   ratio 0.868, 0.851 to 0.885), because cascades then stay short with or without the cap, as in v0.3 (0.860); with the
   default time cost that world is undecided (0.868, 0.850 to 0.887).
5. Harassment drives its targets away. Four times the Trolls raise the share of Vulnerable users who left by 0.03 to 0.3
   (Pew Research Center, 2017). It passes at the defaults (0.074, 0.053 to 0.095), and no world tried fails it: when
   harassment neither hurts nor drives people out the difference is 0.030 (0.011 to 0.049), 0.024 (-0.001 to 0.050)
   with time free as well, and 0.029 (0.008 to 0.049) with `reputation_sensitivity` 0 instead, all undecided, at or
   below the lower bound on the seed means. The remaining effect comes from toxic feeds and reputation alone.
6. Leaving the platform raises wellbeing. Under the integrity preset (14.1, the model's realistic ranked feed, the
   comparison of fact 3), a deactivation experiment (13.2) keeps every regular user off the platform for the last 35
   days of the run, and its placebo on the same seed draws the same people but keeps them on; the break group's
   wellbeing change minus its change in the placebo lies between 0.02 and 0.25 standard deviations of the participants'
   starting wellbeing: positive, and below about four times the field estimate. The paired design compares the same
   people with and without the break, which the field cannot do; the field's design, a tenth on the break against the
   stayers in one run, scatters more across seeds (13.2). Allcott, Gentzkow, Wittenbrink et al. (2025) found that people
   who deactivated Facebook for the six weeks before the 2020 US election reported a 0.060 standard deviation
   improvement in an index of happiness, depression and anxiety, relative to controls who deactivated for only the first
   of those weeks; for Instagram the figure was 0.041 (samples of 19,857 Facebook and 15,585 Instagram users). Allcott
   et al. (2020) found that four weeks of deactivation before the 2018 midterms increased subjective well-being.
   `time_cost` is calibrated on this fact (13.2), so passing it shows the calibration, not an independent prediction.
   Fails without a time cost (`time_cost` 0: effect -0.101), because every item of a good feed then adds reflective
   value and leaving costs people that value, and at the upper bound when time is worth about three times the default
   (`time_cost` 0.2: effect 0.403; at 0.15 the effect of 0.258 lies above the band on the seed mean, but its interval,
   0.213 to 0.303, reaches into it). At the default it is 0.057 (0.001 to 0.113), undecided; in the checking habits
   world, at the same `time_cost`, it is -0.329 and fails, because more reading dilutes the bot farms and makes the
   integrity preset's feed worth more than its time (13.2). The effect builds up over the year (section 16), so its
   test runs for 365 days, at `N` = 2,000.
7. A week without notifications barely changes use or wellbeing. Under the integrity preset, a notifications-off
   experiment (13.2) switches the platform's notifications off for every regular user for the last 7 days of the run,
   and its placebo on the same seed draws the same people and changes nothing. Real minus placebo, the treated group's
   change in daily time on the platform and its change in sessions a day, from the week before, each lie between -0.4
   and +0.1 standard deviations of the participants' baseline, and its change in wellbeing between -0.15 and +0.15.
   Dekker et al. (2024), a preregistered randomized trial with 205 Dutch young adults, had the treatment group disable
   all notifications of all apps for a week: notifications fell from 154.00 to 8.17 a day, but neither checks per day
   (t(205) = -0.81, p = .421) nor screen time (p = .094) changed significantly; as differences in differences over the
   baseline standard deviation, screen time -0.11 SD and checks -0.04 SD. Their switch-off was phone-wide, so switching
   off one platform's notifications may cut that platform's own use more, hence a lower bound well below the field's
   null. Kushlev et al. (2016) found no wellbeing effect of alerts: a week with alerts off had "no significant main
   effects" on productivity and psychological well-being ("t's < .89, p's > .37"). Fitz et al. (2019) found no mood gain
   from receiving no notifications (d = -0.07) and more anxiety (d = 0.56): "delivering no notifications at all either
   had no benefits or produced negative effects". Lab #30 and hub #23 shrank the opening chance's notification factor
   until the time and sessions checks held (13.1, 13.2), so passing them shows the calibration, not an independent
   prediction; the wellbeing check was not tuned. With one session a day, switching notifications off moves use only
   through that factor and habit. Fails, for sessions, with lab #30's factor 0.2 (-0.421) and v0.3's 0.3 (-0.609),
   constants in the code rather than world assumptions; the time check passes in both (-0.210 and -0.305), so it has no
   failing world at the defaults (in the checking habits world with v0.3's terms it failed, -0.543; 13.2). The upper
   bounds have no failing world: nothing in the model lets notifications lower use. The wellbeing check fails in no
   world tried, in either direction: the interruption cost acts only on extra sessions, so it changes nothing at the
   defaults; in the checking habits world at lab #30's constants it raised the effect to 0.093 at 2, the top of its
   range, and to about 0.157 at 6, where the seed means leave the band but the interval does not; with time free the
   effect is -0.006. A week moves wellbeing too little for this check to bite (section 16). The test runs at `N` = 600
   for 240 days, where the sessions check, the closest to its bound, holds with a margin
   (`crates/incentive-core/tests/facts.rs`).

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
    +0.008, +0.007 to +0.008, 13.3, row 32);
  - the reflective self never ends a session: sessions stop on impulse and habit (6.1), so within a session a feed
    worth less than its time holds people as long as one worth more, and only churn answers the difference;
  - one session a day by default, so there is no habit of checking many times.

  The checking habits world (11) adds that habit: habitual and notification-driven extra sessions fitted to field
  figures (13.2: about 15 sessions per daily active, 11% of them started by notifications, under the default design).
  It is not the default, because it triples a day's reading while every per-impression rate of the model was
  calibrated at about 16 items a day: there fact 2 fails, and fact 6 fails at the default `time_cost` (13.2). Its cost
  per notification-driven session (`interruption_cost`, 7.2) stays at 0 by judgement.

  The value of time is calibrated on one preset, the integrity preset taken as the realistic ranked feed (as fact 3
  takes it). On the Engagement ranking the same experiment's paired effect is 0.103 (0.061 to 0.145) at `time_cost` 0,
  near the field's 0.060, and 0.233 (0.183 to 0.283) at the default, about four times the field's figure (8 seeds
  each, at the final defaults; v0.3: 0.048 at 0 and 0.361 at 0.115). Calibrated on the Engagement ranking, the value
  that matches the field would lie near 0. Which preset the field's Facebook feed resembles more decides the value, and
  the model cannot tell. The value also rests on the time scale of reading: in the checking habits world, with about
  three times the items a day, the same rule gave 0.13, because more reading diluted the bot farms and made the
  integrity preset's feed worth more (13.2's history).

  The fact 6 effect also rests on the year's state of the platform and on the size of the society: on the paired
  design at the final defaults, at `N = 2,000` it is -0.122 standard deviations after 120 days, -0.008 after 240 and
  0.057 after 365 (8 seeds each); after 365 days the mean of seeds 1 to 4 is -0.059 at `N = 600`, -0.027 at `N =
  1,000`, 0.027 at `N = 1,500` and 0.063 at `N = 2,000`.
- Notifications are modelled thinly, and their calibration (13.2, fact 7) rests on field figures that fit the
  model loosely:

  - there is no fear of missing out: switching notifications off has no psychological cost, although the field
    found more anxiety and fear of missing out without them (Fitz et al. 2019: anxiety d = 0.56; Dekker et al. 2024:
    notification fear of missing out +1.18 SD); with `interruption_cost` 0 the model has neither that cost nor the
    relief it offsets, and with an interruption cost it has only the relief;
  - batching cannot be represented: notifications have one intensity, not a schedule, so the arm that helped mood,
    stress and attention in Fitz et al. (2019), notifications three times a day, has no counterpart;
  - the field figures are phone-wide, used for one platform: the checks behind T3 and the switch-off of Dekker et
    al. (2024) cover every app on the phone, so the target for one platform's sessions is derived (a share of checks
    in proportion to screen time) and fact 7's lower bounds are set below the field's null;
  - in the checking habits world, the length of a check is a judgement (`extra_session_share` 0.1, about two items),
    and its most consequential one: at 0.05 or 0.2 the regretted share runs from 0.20 to 0.65 (13.4), and in lab #30
    fact 7's sessions check failed at 0.2 (13.2);
  - a week moves wellbeing little (7.3), so fact 7's wellbeing check has no failing world in any world tried.

- Revenue is calibrated to one platform's figures (lab #16, refitted for one session a day in hub #23, 13.2), and much
  of it stays simple:

  - one ad-free tier, at one price for everyone, and no ad load a person can choose short of it;
  - the price elasticity of uptake is a judgement (1, explored from 0.5 to 2): no measured price elasticity of a
    social platform's tier was found;
  - the price is a world assumption, not a design lever, so designs are compared at one price and none can set its
    own; a world with another price moves uptake against the default price;
  - ad revenue per impression is fixed (`ad_rate`), with no advertiser demand: an ad earns as much on a toxic feed
    as on a calm one, and more impressions never lower its price;
  - Snap is the single reference for both targets, the price (R1) and the uptake (R2). Together they set the tier's
    share of the default design's revenue to 19%, where Snap's subscriptions brought 13.5% of its revenue in the
    fourth quarter of 2025;
  - only the default design's uptake is calibrated: across the presets it runs from 2.4% (engagement-paid creators) to
    10.7% (the subscription preset) of daily actives, and Quality and bridging and the subscription preset lie just
    above R2's band; the hurdle is kept from v0.3.

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
  recommendations, which a chronological feed never shows. A platform that is chronological from day 0 therefore never
  follows bots or the prolific sources that post misinformation, while in the ranked feeds misinformation rides on
  bot-inflated engagement. So the model fails the field direction of Guess et al. (2023): over days 30 to 90 the
  chronological feed shows 0.64 of the integrity preset's misinformation impressions where the field found about two
  thirds more (fact 3, a known limit), and it shows an eighth of the Engagement ranking's outrage impressions (0.055
  against 0.460 over the second half of the year; a ninth in v0.3) where field audits find differences of tens of
  percent. Against the integrity preset the cut in incivility is too deep: over the same days the chronological feed
  shows 0.336 of its toxic impressions (0.252 to 0.407, fail; v0.3: 0.312), where
  the field lowered the share of uncivil content by tens of percent (fact 3's toxic check, band 0.5 to 0.9, a second
  known limit). The field experiment moved people whose networks had formed under ranking; the model has neither a
  mid-run switch for part of the population nor untrustworthy pages that people follow.
- Compulsive use is priced crudely. With one session a day, items beyond a person's time budget are 0.45% of time under
  the default design at the final defaults (v0.3: 2.5%), against the 31% of social-media use that Allcott, Gentzkow and
  Song (2022) attribute to self-control problems (the held-out target of 13.4). The checking habits world (11) puts the
  share at 0.470, above the field's 0.20 to 0.42, and there it turns on the judged length of a check (0.197 to 0.646
  from one item a check to four, 13.4), so it closes the gap by an amount that a judgement sets. The time cost charges
  every minute at one price, whatever drew the person in, so it prices time, not the lack of self-control: regret,
  wellbeing and the benefit index still understate what compulsive use costs.
- The model's rates are sized for one session a day. Every per-impression rate (learning, polarisation, the bot farms'
  boosts and brigading, the candidate cap of 80) was calibrated at about 16 items a day, so a world that triples the
  reading, such as the checking habits world, changes the ranking's selection rather than only adding time: a daily
  active reads about three fifths of a feed's candidates instead of a fifth, the Outrage share of impressions halves,
  the farms' share of the signals halves, and the Outrage share of posts falls over the year (fact 2 fails, 13.2).
  Re-sizing the candidate cap and the farms to v0.3's shares for that world (hub #23) brought the rise back on the seed
  means only, apparently through the farms' larger boosts (a cap of 200 alone did not restore it), and broke fact 3's
  polarisation check (12.1% at day 90), the user
  share and the toxic share; it also tripled the ranking time (13.2's history). A model with several sessions a day
  needs its rates recalibrated per impression, not two constants re-sized.
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
- Allcott, Gentzkow, & Song (2022). Digital addiction. *American Economic Review*, 112(7). Read as NBER Working
  Paper 28936 (revised March 2022). <https://www.nber.org/system/files/working_papers/w28936/w28936.pdf>
- Bakshy, E., Messing, S., & Adamic, L. A. (2015). Exposure to ideologically diverse news and opinion on Facebook.
  *Science*, 348(6239), 1130-1132. Read in the authors' summary on the Facebook Research blog.
  <https://doi.org/10.1126/science.aaa1160>
- Bail, C. A., Argyle, L. P., Brown, T. W., Bumpus, J. P., Chen, H., Hunzaker, M. B. F., Lee, J., Mann, M.,
  Merhout, F., & Volfovsky, A. (2018). Exposure to opposing views on social media can increase political
  polarization. *PNAS*, 115(37), 9216-9221. <https://doi.org/10.1073/pnas.1804840115>
- Brady, W. J., Wills, J. A., Jost, J. T., Tucker, J. A., & Van Bavel, J. J. (2017). Emotion shapes the diffusion of
  moralized content in social networks. *PNAS*, 114(28), 7313-7318. <https://doi.org/10.1073/pnas.1618923114>
- Brady, W. J., McLoughlin, K., Doan, T. N., & Crockett, M. J. (2021). How social learning amplifies moral outrage
  expression in online social networks. *Science Advances*, 7(33). <https://doi.org/10.1126/sciadv.abe5641>
- Dekker, Baumgartner, Sumter, & Ohme (2024). Beyond the Buzz. *Media Psychology*, 28(1), 162-188 (2025); a
  preregistered randomized trial of switching all notifications off for a week. Read in the first author's thesis
  chapter. <https://doi.org/10.1080/15213269.2024.2334025>
- de Freitas Melo, P., Vieira, C. C., Garimella, K., Vaz de Melo, P. O. S., & Benevenuto, F. (2019). Can WhatsApp
  counter misinformation by limiting message forwarding? *Complex Networks and Their Applications VIII*.
  <https://doi.org/10.1007/978-3-030-36687-2_31>
- Fitz, Kushlev, Jagannathan, Lewis, Paliwal, & Ariely (2019). A randomized field experiment on batching
  smartphone notifications (description; the lab's notes do not record the title). *Computers in Human Behavior*,
  101, 84-94.
- Guess, A. M., Malhotra, N., Pan, J., Barberá, P., et al. (2023). How do social media feed algorithms affect
  attitudes and behavior in an election campaign? *Science*, 381(6656), 398-404 (corrected 2024 and 2026).
  <https://doi.org/10.1126/science.abp9364>
- Heitmayer & Lahlou (2021). Why are smartphones disruptive? *Computers in Human Behavior*, 116.
- Kleinberg, J., Mullainathan, S., & Raghavan, M. (2023). The challenge of understanding what users want: Inconsistent
  preferences and engagement optimization. *Management Science*. <https://doi.org/10.1287/mnsc.2022.03683>
- Kushlev, Proulx, & Dunn (2016). Silence Your Phones. *Proceedings of CHI 2016*, 1011-1020.
  <https://www.interruptions.net/literature/Kushlev-CHI16.pdf>
- Milli, S., Carroll, M., Wang, Y., Pandey, S., Zhao, S., & Dragan, A. D. (2025). Engagement, user satisfaction, and
  the amplification of divisive content on social media. *PNAS Nexus*. <https://doi.org/10.1093/pnasnexus/pgaf062>
- Pew Research Center (2017). Online Harassment 2017.
  <https://www.pewresearch.org/internet/2017/07/11/online-harassment-2017/>
- Rathje, S., Van Bavel, J. J., & van der Linden, S. (2021). Out-group animosity drives engagement on social media.
  *PNAS*, 118(26). <https://doi.org/10.1073/pnas.2024292118>
- Snap Inc. (2017). Form S-1 registration statement (figures for Q4 2016).
  <https://www.sec.gov/Archives/edgar/data/1564408/000119312517029199/d270216ds1.htm>
