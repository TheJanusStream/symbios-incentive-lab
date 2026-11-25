# The incentive lab model (v0.1)

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
- `evasion` in `[0, 1]`: skill at writing toxicity that classifiers miss, starting at 0.
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
`round(population.bot_share * N * (1 - design.account_cost)^2)` (default `bot_share` 0.05). Each bot belongs to a
group, posts Clickbait or Outrage (50/50) at 3 posts per day, and each day likes and shares `bot_boosts` (default 20)
random recent posts by bots or Clickbait authors. Bots have no experience, wellbeing or learning. Their engagement is
counted by the platform (it is observable and inflates ranking signals) but never in ground-truth metrics. A banned
bot is replaced after a delay with probability `1 - account_cost` per day.

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

### 4.2 Generating a post

A post of style `s` by author `i` on day `t`:

- `topic`: topic 0 with probability 0.7 for Outrage and Attack, otherwise drawn from the author's interests.
- `quality = clamp01(base_q + 0.4 * (skill - 0.5) + N(0, 0.08))` (Attack: no skill term).
- `arousal = clamp01(base_a + N(0, 0.08))`.
- `outrage = clamp01(base_o + 0.2 * temperament * hostile + N(0, 0.05))`, where `hostile` is 1 for Outrage and
  Attack, else 0.
- `toxicity = clamp01(base_x + 0.25 * temperament * hostile + N(0, 0.05))`.
- `truth = clamp01(base_t + 0.1 * (skill - 0.5) + N(0, 0.08))`.
- `detectability = clamp01(0.9 - 0.8 * evasion)` if `toxicity > 0.3`, else 0.9. Evasion is the model's "algospeak":
  coded toxicity that still hurts but that classifiers miss.
- `target` (Attack only): an out-group user, with probability 0.5 one of the out-group accounts in the top 5% by
  followers, otherwise a random out-group active user.
- `group`: the author's group. Counters: impressions, likes (split by viewer group), positive replies, angry replies,
  shares, reports, see-less marks. Flags: removed, demoted, noted. Shares carry a depth (6.6).

A post is a feed candidate for `run.post_lifetime_days` (default 3) days.

## 5. A simulated day

The phases run in this order, and within a phase in ascending individual index, so a run is a pure function of its
configuration (section 12):

1. Bots: replace banned bots (3.5).
2. Joining: non-users may join, churned users may return (7.4).
3. Posting: active users and bots write posts (7.5 chooses styles, 4.2 builds them).
4. Automated moderation of the new posts (6.7).
5. Harassment delivery: each surviving Attack reaches its target by notification unless the target blocked the
   author (6.9).
6. Sessions: each active user who opens the app gets a feed and consumes it (6.1 to 6.6, 7.1 to 7.3).
7. Bots boost posts (3.5).
8. Community notes (6.8).
9. Human review and sanctions (6.7).
10. Economy: revenue, creator payouts (section 8.3).
11. Learning: authors update style values, evasion and imitation (7.5).
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
- Out-of-network: `ceil(design.recommended_share * candidate_cap)` posts sampled from all recent posts with
  probability proportional to `1 + observed engagement` (likes + replies + shares, bot engagement included), the
  platform's "trending" signal. Default `recommended_share` 0.4, `candidate_cap` 80.
- If there are more in-network candidates than `candidate_cap * (1 - recommended_share)`, keep the most recent.
- A chronological feed (`design.chronological`) uses in-network candidates only, ordered newest first.

### 6.3 The platform's predictions (observable)

The platform predicts like, reply and share probabilities and dwell time for each candidate. With
`q = design.predictor_quality` (default 0.85) and `P` the true probability from 7.1:

`P_hat = q * P + (1 - q) * P_style + N(0, 0.02)`, clamped to `[0, 1]`,

where `P_style` is the platform's running average of that probability for the post's style, updated daily from
observed counts. Better prediction is a dial: a more capable predictor finds engaging content more reliably, whatever
that content does to people.

Other observable signals: `Q_hat = clamp01(quality + N(0, 0.15))`, a noisy quality classifier; the bridging score
`B = 4 * likes_A * likes_B / (likes_A + likes_B + 2)^2`, high when both groups like a post; and `SL(author)`, the
author's see-less marks per impression (exponential moving average).

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
(`w_bridging` 1.5, `w_quality` 0.5, `w_like` 0.3, `w_recency` 0.3). Users adopt it in order of low impulsivity,
reflecting that people who choose their own feeds tend to be the more reflective ones.

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
  hit passionate speech that is not toxic: such removals are false positives (truth: `toxicity < 0.5`). Automated
  removal gives no strike but counts as a sanction in learning (7.5).
- Human review: `design.review_capacity` reviews per 1,000 users per day (default 1.0). Reviewers take reported posts
  in order of report count and judge truthfully: `toxicity >= 0.5` removes the post and gives its author a strike.
- Sanctions (`design.sanctions`): None; Graduated (strike 1 a warning, 2 a 3-day suspension, 3 a 14-day suspension,
  4 a ban); Strict (ban at the first strike). A banned person leaves the platform. After 7 to 30 days it may come back
  with a new account with daily probability `reentry * (1 - account_cost)` (`reentry` 0.5 for Trolls, 0.05 for
  others): a new account starts with no followers and no strikes.

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

- `design.time_limit_share` (default 0): users who opt into a daily limit equal to their `time_budget`, adopted in
  order of low impulsivity.
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
  R = 0.8 * quality * min(m, 2) / 2 + 0.3 * [Personal and mutual follow] + 0.2 * (truth - 0.5)
    - 0.8 * toxicity * (1 - toxicity_tolerance) - 0.4 * outrage * (0.4 if same else 1.0) - 0.3 * [Clickbait]
  ```

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
  (default 0.01).

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

Habit is how engagement can keep people on a platform while their wellbeing falls.

### 7.4 Churn and joining

- A user leaves with daily probability `world.churn_base * sig(8 * (world.leave_threshold - stay_ema)) +
  world.harassment_exit * harassment events today * vulnerability` (defaults 0.02, 0.15, 0.05), or when banned.
  Leaving keeps the follow lists (dormant) and sets `left_day`.
- A non-user who has never used the platform joins with daily probability
  `world.join_base * (0.2 + share of offline ties who are users) * reputation` (default `join_base` 0.004), where
  `reputation = max(0.1, 1 - world.reputation_sensitivity * toxic_reputation)` and `toxic_reputation` is the
  30-day moving average of the toxic impression share (default `reputation_sensitivity` 2.0).
- A former user can return 30 days after leaving, with 0.3 times the joining probability.
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
  `r = intrinsic[s] + approval_sensitivity * S / 3 + money_weight * M - 1.0 * [removed or struck]`, where
  `S = ln(1 + likes + 2 * positive replies + 3 * shares) + temperament * ln(1 + angry replies) +
  0.5 * ln(1 + followers gained)` and `M` is the post's payout divided by `world.living_income / 30`. Authors cannot
  tell bot engagement from human engagement, so bot likes reward them too.
- Update: `style_value[s] += world.learning_rate * (r - style_value[s])` (default 0.1). Styles not written drift back
  toward intrinsic value by 1% a day.
- Imitation: with daily probability `world.imitation_rate` (default 0.02) a user picks a random followed account with
  a higher `reward_ema` and moves its style values 20% of the way toward that account's.
- Evasion: each automated or human removal of a post with `toxicity > 0.3` raises the author's evasion by
  `world.evasion_rate * (1 - evasion)` (default 0.15).

Grounding: Brady et al. (2021) found that people who received more likes and retweets for moral outrage expressed
more outrage later. This learning rule is the model's version of that.

### 7.6 Following

- Liking a post by an account `v` does not follow makes `v` follow it with probability `0.05 * (1 + quality)`.
- A post with `toxicity * (1 - toxicity_tolerance) > 0.5` makes `v` unfollow its author with probability 0.2.

## 8. Society and economy

### 8.1 Polarisation drift

Every day each individual's `affective_polarisation` drifts toward its baseline by 0.2% of the gap.

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

### 9.1 Observable: the platform's view

- Reported daily actives and accounts (humans plus bots), new accounts, departures.
- Human impressions, time spent (sum of dwell), likes, replies and shares (bots and angry replies included), and their
  rate per reported daily active.
- Reports, see-less marks, automated removals, human removals, notes.
- Revenue, payouts.
- 30-day retention: the share of users active 30 days ago who are active today.
- Survey satisfaction: once a week a 5% sample of current users is asked; each answers with probability
  `0.3 * (0.5 + wellbeing)` and reports `clamp01(wellbeing + 0.1 + N(0, 0.1))`. Happier users answer more often and
  everybody answers a little high, so the survey is biased, and it never reaches people who left.

### 9.2 Truth: the god's-eye view

- Engagement: human daily actives, human impressions, regretted share of time (items beyond `time_budget`).
- Harm: toxic impression share (toxicity above 0.5), outrage impression share, misinformation impression share,
  harassment events per 1,000 users per day, false-positive removals, the share of reported engagement that came from
  bots.
- Participants: mean wellbeing of current users; mean wellbeing of everyone who ever used the platform (current and
  former users, which removes survivorship bias); wellbeing by archetype; meaningful interactions per user (positive
  replies and likes between mutual follows on Personal posts); the share of Vulnerable individuals who left.
- Non-users: mean affective polarisation and informedness of non-users, and the same for users.
- Public discourse: toxic and truthful shares among the day's top 5% most-viewed posts; positive cross-group likes per
  impression; society-wide affective polarisation.
- Creators' livelihoods: Creators earning a living, median income, Gini coefficient of Creator income, payout share
  by style.
- Content mix: share of posts by style, by day.

### 9.3 Composite indices

Each index lies in `[0, 1]`; the formulas are deliberately simple so a player can trace them.

- `engagement_index = min(1, human impressions / (1.5 * N * base_session_items))`.
- `harm_index` = mean of `min(1, toxic share / 0.2)`, `min(1, harassment per 1,000 users / 20)`,
  `min(1, misinformation share / 0.3)` and `clamp01((society polarisation - day-0 value) / 0.2)`.
- `discourse_quality` = mean of (1 - toxic share among top posts), truthful share among top posts,
  `min(1, positive cross-group likes per impression / 0.1)` and `1 - clamp01((society polarisation - day-0 value) /
  0.2)`.
- `benefit_index` = mean of the wellbeing of everyone who ever used the platform, society-wide informedness,
  `min(1, meaningful interactions per user / 2)`, the share of Creators earning a living, and `discourse_quality`.

## 10. Design levers (the player's platform)

| Group | Lever | Default (Engagement maximiser) |
| --- | --- | --- |
| Ranking | `w_like`, `w_reply`, `w_share`, `w_dwell` | 1.0, 1.5, 2.0, 0.5 |
| Ranking | `w_quality`, `w_bridging`, `w_see_less` | 0, 0, 0 |
| Ranking | `w_recency`, `w_in_network` | 0.3, 0.2 |
| Ranking | `chronological`, `recommended_share`, `diversity` | false, 0.4, 0 |
| Ranking | `predictor_quality` | 0.85 |
| Ranking | `demote_penalty`, `noted_penalty` | 1.0, 0.5 |
| Visibility | `like_counts_visible` | true |
| Virality | `reshare_depth_cap`, `share_friction` | none, 0 |
| Moderation | `classifier_enabled`, `classifier_threshold`, `moderation_action` | true, 0.7, Remove |
| Moderation | `review_capacity` (per 1,000 users per day), `sanctions` | 1.0, Graduated |
| Context | `community_notes` | false |
| Creator money | `payout_share`, `payout_scheme`, `payout_threshold_followers` | 0, PerEngagement, 0 |
| User controls | `block_effectiveness`, `time_limit_share`, `notification_intensity` | 0.8, 0, 0.6 |
| User controls | `alt_feed_share`, `alt_feed` | 0, Chronological |
| Identity | `account_cost` | 0.2 |
| Business | `ads_share` | 1.0 |

Levers can change during a run (`Simulation::set_design`); a change applies from the next day.

## 11. World assumptions

These are not design choices. They are the model's beliefs about people, kept separate so a player can test whether
a conclusion survives a different world.

| Assumption | Default | Range to explore |
| --- | --- | --- |
| `outrage_appeal`, `arousal_appeal`, `social_proof` | 1.2, 1.0, 0.3 | 0-3, 0-2, 0-1 |
| `harassment_harm`, `comparison_cost`, `regret_cost` | 3.0, 0.1, 0.05 | 0-6, 0-0.5, 0-0.2 |
| `learning_rate`, `imitation_rate`, `choice_temperature` | 0.1, 0.02, 0.25 | 0-0.5, 0-0.1, 0.05-1 |
| `evasion_rate` | 0.15 | 0-0.5 |
| `habit_gain`, `stop_bias` | 0.08, -2.5 | 0-0.3, -5-0 |
| `churn_base`, `leave_threshold`, `harassment_exit` | 0.02, 0.15, 0.05 | 0-0.1, -0.5-0.5, 0-0.2 |
| `join_base`, `reputation_sensitivity`, `homophily` | 0.004, 2.0, 0.7 | 0-0.02, 0-5, 0.5-1 |
| `polarisation_rate`, `contact_rate`, `spillover_rate` | 0.004, 0.01, 0.01 | 0-0.02, 0-0.05, 0-0.05 |
| `credulity_scale` | 1.0 | 0-2 |
| `ad_rate`, `subscription_price`, `subscription_hurdle`, `ad_annoyance` | 0.001, 5.0, 0.5, 0.02 | - |
| `living_income` | 1.0 | - |
| `base_session_items` | 20 | 5-60 |

Run settings (`run`): `seed`, `days` (default 365), `population_size` (default 2,000 natively, 1,000 in the browser),
`initial_user_share` (0.6), `post_lifetime_days` (3), `candidate_cap` (80), `offline_ties` (8), `initial_follows`
(30), `summary_window` (0.3), `tracked_agents` (default 10, two per archetype, for the inspector).

## 12. Determinism

- A run is a pure function of its configuration: the same configuration gives a bit-identical history on the same
  platform and build. Native and browser builds may differ in the last bits of transcendental functions (`ln`, `exp`,
  `tanh`, and the samplers built on them), so their histories can drift apart over a long run; compare them by
  distribution, not bit for bit.
- Randomness comes from counter-based streams: each draw site seeds a `rand_pcg` generator from
  `(seed, day, entity, stream)` mixed with SplitMix64, where `stream` names the purpose (population, posting, feed,
  engagement, moderation, economy, social, spillover, survey, bots). Two runs that differ in one lever therefore share
  every draw the lever does not touch, which sharpens side-by-side comparisons (common random numbers).
- No iteration over hash maps influences results. Phases visit individuals in ascending index.

## 13. Calibration record

The builder of `incentive-core` fills this section: the values the code uses where they differ from the text above,
the reward timing chosen in 7.5, the measured behaviour of the presets (section 14) over 365 days at `N = 2,000` with
at least 8 seeds (mean and spread), and the run time per run.

Targets for calibration:

- No degenerate dynamics under the default design: after 365 days between 40% and 90% of individuals are users; the
  Outrage share of posts rises over the year but stays below 50%; the toxic impression share lies between 3% and 20%.
- The trade-off exists in both directions: some designs cut the harm index markedly at a moderate engagement cost, and
  over a full year the engagement maximiser's lead in engagement shrinks or reverses as harassment, churn and
  reputation catch up with it, under at least some plausible world assumptions.
- The stylized facts of section 15 hold.

### 13.1 Values that differ from the text

The structure of every formula is as written; these constants changed. Each was measured with the command-line runner
(`incentive run`, `compare`, `sweep`, `facts`) on seeded runs at `N = 2,000`.

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
  toward the observed rate per impression of human engagement. The platform's offline statistics, these rates and
  30-day retention, are taken as cleaned of bots; its live signals are not: bot boosts raise displayed counts and
  social proof, trending, bridging and engagement payouts. `Q_hat` is drawn once per post.
- Trending weights are a snapshot taken when the day's sessions start. A post that is both in-network and a reshare
  is a candidate once; out-of-network recommendations have depth 0.
- Diversity re-ranking lifts calm out-group posts once by `0.3 * diversity`; only the author and topic penalties
  accumulate with each pick.
- A delivered Attack counts as an experienced day for the target's wellbeing even without a session, and the target
  reports it with the report probability of 7.1, target bonus included. Without target reports, reviewers never saw
  Attacks, because viral outrage always had more reports.
- Bots: `round(bot_share * N)` slots, of which as many as the current account cost lets in (3.5) are active; an
  empty slot refills after at least 3 days with daily probability `1 - account_cost`. Bots have no evasion, can be
  followed, and can be paid. Fan funding pays any author whose post gave positive `R`.
- Subscriptions: on day 0 there is no history, so the mean daily `E_ref` is taken to equal the hurdle.
- Wellbeing of former users stays where it was when they left (7.3 updates users only).
- A banned person may return after a wait drawn uniformly from 7 to 30 days. Review capacity below one review is a
  chance of one review.
- Metrics: outrage impressions are impressions of posts with outrage above 0.5; false positives count removals and
  demotions of posts with toxicity below 0.5; Creator metrics cover Creators who ever used the platform; the largest
  cascade counts human shares of the posts whose life ends that day. Survey satisfaction exists on days 6, 13, 20 and
  so on, 30-day retention from day 30, and payout shares on days with payouts; on other days they are NaN
  (`DailyMetrics::get_opt` returns `None`), and summaries average only the days that have a value.

### 13.3 Presets over a year

365 days, `N = 2,000`, seeds 1 to 8, summary over the last 110 days, mean ± standard deviation across seeds
(`incentive run --preset <id> --seeds 1..8`). EM is the engagement maximiser, Q&B quality and bridging, Sub the
subscription, Paid the engagement-paid creators. Observable metrics are marked (O), the rest are truth.

| Metric | EM | Chrono | Q&B | Calm | Sub | Paid |
| --- | --- | --- | --- | --- | --- | --- |
| Engagement index (O) | 0.263±0.004 | 0.162±0.005 | 0.168±0.002 | 0.138±0.003 | 0.169±0.002 | 0.274±0.005 |
| Harm index | 0.499±0.038 | 0.190±0.020 | 0.093±0.015 | 0.277±0.032 | 0.070±0.013 | 0.508±0.045 |
| Discourse quality | 0.662±0.025 | 0.817±0.004 | 0.880±0.002 | 0.816±0.010 | 0.881±0.002 | 0.601±0.029 |
| Benefit index | 0.416±0.007 | 0.432±0.003 | 0.453±0.001 | 0.452±0.003 | 0.541±0.006 | 0.440±0.010 |
| Users in society | 0.656±0.017 | 0.615±0.015 | 0.721±0.011 | 0.644±0.015 | 0.739±0.007 | 0.615±0.025 |
| Human daily actives | 850±17 | 684±19 | 757±12 | 627±12 | 773±8 | 838±21 |
| Impressions (O) | 15750±262 | 9746±326 | 10064±133 | 8264±151 | 10146±87 | 16464±278 |
| Toxic impressions | 0.162±0.017 | 0.015±0.002 | 0.001±0.000 | 0.084±0.013 | 0.000±0.000 | 0.134±0.021 |
| Outrage impressions | 0.550±0.043 | 0.059±0.011 | 0.003±0.001 | 0.245±0.031 | 0.002±0.001 | 0.690±0.049 |
| Misinformation impr. | 0.109±0.006 | 0.098±0.007 | 0.005±0.001 | 0.109±0.009 | 0.002±0.001 | 0.102±0.005 |
| Harassment per 1,000 | 7.96±1.46 | 7.12±1.01 | 7.01±1.09 | 5.35±0.87 | 5.43±0.96 | 7.57±1.66 |
| Outrage posts | 0.320±0.014 | 0.204±0.007 | 0.188±0.005 | 0.255±0.014 | 0.154±0.004 | 0.366±0.016 |
| Wellbeing, ever-users | 0.594±0.012 | 0.645±0.003 | 0.699±0.002 | 0.645±0.005 | 0.711±0.001 | 0.555±0.016 |
| Survey (O) | 0.705±0.014 | 0.757±0.009 | 0.806±0.008 | 0.753±0.013 | 0.816±0.006 | 0.664±0.026 |
| Polarisation | 0.508±0.012 | 0.419±0.004 | 0.374±0.003 | 0.433±0.005 | 0.372±0.003 | 0.550±0.019 |
| Informed non-users | 0.595±0.002 | 0.594±0.003 | 0.648±0.001 | 0.596±0.001 | 0.652±0.001 | 0.589±0.003 |
| Creators living | 0 | 0 | 0 | 0 | 0.426±0.032 | 0.210±0.023 |
| Harmful payouts | - | - | - | - | 0.000±0.000 | 0.843±0.028 |
| Bot engagement | 0.295±0.007 | 0.596±0.007 | 0.594±0.003 | 0.489±0.010 | 0.593±0.002 | 0.280±0.007 |

Under the default design the share of individuals who are users rises from 0.60 to 0.66, the Outrage share of posts
rises from about 0.28 on day 0 to 0.32, and the toxic impression share is 16%: all three targets hold. Society-wide
polarisation starts at 0.42.

The trade-off runs both ways:

- Quality and bridging cuts the harm index by 81% for 36% of the engagement index. The engagement trap's known
  solution (the default design with `w_reply` 0 and `w_see_less` 5) cuts toxic impressions to 0.42 of the default's
  and the harm index by about half while keeping all of its engagement: the angry-reply weight is not even
  engagement-optimal over a year, because it costs users.
- Under the default world the engagement maximiser's lead over Quality and bridging grows over the year as habit
  builds (1.13 times in the first 30 days, 1.57 times in the last 95). With weak habit formation and a strong
  reputation effect (`habit_gain` 0.01, `reputation_sensitivity` 5, `harassment_exit` 0.15, 4 seeds) the lead peaks
  at 1.11 around mid-year and shrinks to 1.05 in the last 95 days, below its starting 1.08, while the engagement
  maximiser loses users and Quality and bridging gains them.

Three findings came out of calibration rather than into it. A stricter classifier barely lowers toxic impressions
over a year, because removed authors learn evasion. Ranking toxic posts down hides them from the people who report
them, so strict enforcement with heavy down-ranking catches fewer trolls than strict enforcement alone. And a
bridging-ranked feed is gamed by coordinated bots: bots of both groups boost the same posts, which then look liked
across the divide. With every user on the bridging alternative feed, misinformation impressions rise from 0.11 to 0.20
(4 seeds), with or without community notes, while toxic impressions fall from 0.16 to 0.02 and bots supply 63% of the
engagement. Quality and bridging resists it, because its quality weight offsets the inflated bridging score.

### 13.4 Stylized facts

`incentive facts`: 8 paired seeds, `N = 2,000`, 365 days; EM is the engagement maximiser. Effects are paired mean
differences, means or ratios of means, with their standard deviation across seeds where one exists. The tests in
`crates/incentive-core/tests/facts.rs` run the same checks on 4 seeds at `N = 600` for 120 days (240 days for the
harassment fact, whose departures accumulate slowly).

| Fact | Check | Effect | Threshold | Result |
| --- | --- | --- | --- | --- |
| 1 | Outrage impression share, EM minus chronological | 0.471 ± 0.043 | > 0.02 | pass |
| 1 | Reflective value per impression, chronological minus EM | 0.160 ± 0.023 | > 0.02 | pass |
| 2 | Rise of the Outrage share of posts under EM | 0.056 ± 0.015 | > 0.01 | pass |
| 2 | That rise minus the rise with `learning_rate` 0 | 0.058 ± 0.010 | > 0.01 | pass |
| 3 | Time spent, chronological over EM | 0.61 | < 0.9 | pass |
| 3 | Relative difference in society-wide polarisation, days 80 to 90 | 0.028 | < 0.05 | pass |
| 4 | Largest cascades with `reshare_depth_cap` 1 over without | 0.73 | < 0.85 | pass |
| 5 | Share of Vulnerable users who left, Troll share 0.12 minus 0.03 | 0.160 ± 0.046 | > 0.02 | pass |

The depth cap shrinks the largest cascades by about a quarter, not more, because trending recommendations expose
original posts at depth 0 and the cap only stops the second hop.

### 13.5 Scenarios

Thresholds as in 14.2 except where noted. Two changed so that the baseline fails and a recorded design passes with a
margin: the misinformation surge asks for +0.03 (the best designs found reach about +0.04, because non-users are only
half pulled toward their user ties), and the public square asks for at least 10% less polarisation (merely "below the
baseline" was met by a single weight). The misinformation surge's Clickbait-heavy bots are
`population.bot_clickbait_share` 0.9. Known solutions, as changes to the scenario's starting design, with full-size
results on 8 seeds (`incentive scenario <id> --solution --seeds 1..8`):

| Scenario | Known solution | Result against the baseline |
| --- | --- | --- |
| The engagement trap | `w_reply` 0, `w_see_less` 5 | engagement 1.00x, toxic share 0.42x |
| Toxicity spiral | `review_capacity` 10, `account_cost` 1, `classifier_threshold` 0.3 | harassment 0.35x, DAU 0.92x |
| A living for creators | `payout_scheme` FanFunded | living 0.37 against 0.21, harmful payouts 0.6% |
| Misinformation surge | Quality and bridging ranking with notes, `account_cost` 1 | non-users' informedness +0.037 |
| Public square | `w_reply` 0, `w_bridging` 0.6 | polarisation 0.84x, engagement 0.97x |

### 13.6 Run time

One run of 365 days at `N = 2,000` takes 3.3 seconds single-threaded in release on the owner's desktop (`incentive run
--seeds 1`, wall time including the CSV export). The core's whole test suite runs in about 10 seconds under the dev
profile on 8 threads.

### 13.7 Exploration

`incentive explore` searched every design lever at once (35 genes: one per lever, two for the reshare depth cap) with
the world assumptions at their defaults. Each candidate ran on seeds 1 and 2 at `N = 600` for 180 days, and its metrics
are the mean of the two summaries. The four searches behind the app's Atlas tab, with search seed 1:

| Search | Library algorithm | Generations x batch | Designs simulated | Wall time |
| --- | --- | --- | --- | --- |
| Goodhart, `engagement_index` | `SimpleGA`, elitism 2, from 40 random designs | 25 x 40 | 948 | 124 s |
| Goodhart, `survey_satisfaction` | the same | 25 x 40 | 948 | about 2 min |
| Map | `MapElites`, 16 x 16 grid, 128 initial designs (the presets first) | 50 x 64 | 3,084 | 282 s |
| Pareto front | `Nsga2`, population 64 (the presets and random designs) | 40 x 64 | 2,620 | 343 s |

That is 7,600 designs and 15,200 runs in about 15 minutes on 8 CPUs, with a mutation rate of 0.1 per gene. The exact
commands are in the README. The map's axes are the engagement index from 0.04 to 0.36 and the harm index from 0 to 0.64,
in cells of 0.02 by 0.04; values outside fall into the edge cells.

The reduced size changes the numbers. The engagement maximiser scores engagement 0.236, harm 0.388 and benefit 0.497 at
`N = 600` over 180 days (8 seeds), against 0.263, 0.499 and 0.416 at full size (13.3): a shorter run has less time for
harm and churn to build up. With two seeds the harm index of one design varies by about 0.07 between seeds, more than
the height of a map cell, so an elite is partly a lucky draw. The key designs below were therefore rerun at full size
(`incentive compare --a engagement-maximiser --b <design>`, `N = 2,000`, 365 days, seeds 1 to 8); every difference
quoted from those runs has a 95% interval that excludes zero.

**Which regions are reachable.** The map filled 148 of its 256 cells (58%): engagement from 0.07 to 0.30, harm from 0.07
to 0.63. Up to an engagement index of 0.20 some design keeps harm near 0.07; beyond that the lowest harm found rises
with engagement: 0.16 at 0.22, 0.21 at 0.24, 0.26 at 0.26 and 0.35 at 0.28. The empty corners are not hard limits. The
Pareto search, which pushes outwards on purpose, found designs at harm 0.02 and at engagement 0.41, beyond everything
the map reached, so the map shows where 3,084 designs landed rather than the edge of the possible; a longer map search
would fill more of it. Benefit on the map runs from 0.33 to 0.79, and across its cells it follows the share of Creators
earning a living (correlation 0.82): that share is 0 unless the platform pays creators, so paying them is the largest
single lever on the benefit index, and almost every high-benefit elite pays out 30% to 50% of revenue.

**High engagement with low harm exists in this model.** Of the 64 designs on the Pareto front, 41 beat the engagement
maximiser on all three objectives at once. At full size:

| Design | Engagement | Harm | Benefit | Toxic impressions | Wellbeing of ever-users | Users |
| --- | --- | --- | --- | --- | --- | --- |
| Engagement maximiser | 0.263 | 0.499 | 0.416 | 0.162 | 0.594 | 0.656 |
| Map cell (8, 2) | 0.245 | 0.123 | 0.634 | 0.007 | 0.713 | 0.732 |
| Front design 13 | 0.241 | 0.089 | 0.749 | 0.011 | 0.753 | 0.785 |
| Front design 33 | 0.356 | 0.142 | 0.562 | 0.036 | 0.770 | 0.812 |

Front design 33 holds 36% more engagement than the engagement maximiser with 72% less harm. It still ranks hard on
engagement (like 2.5, share 2.2, dwell 3.0, notifications at full strength), and what keeps harm down is a set of brakes
the default lacks: a see-less penalty of 3.5, a strict classifier that removes at a threshold of 0.2, community notes,
the highest account cost (1.0, which keeps bot farms out), the reply weight cut to 0.6, recommended posts from strangers
cut from 40% to 15%, and creators paid 30% of revenue per view. The low-harm designs of the map share the pattern:
compared with its high-harm cells they listen more to see-less (1.9 against 0.7), review more reports (4.8 against 2.7
per 1,000 users), recommend fewer strangers' posts (0.22 against 0.62), charge more for accounts (0.44 against 0.11),
lean less on ads (46% against 87%) and pay by quality rather than engagement. The map's designs with the highest
engagement push notifications close to the maximum (0.85 on average).

**What front design 33 rests on.** Setting one of its levers at a time back to the engagement maximiser's value, at full
size on 8 paired seeds (every difference from the engagement maximiser has a 95% interval that excludes zero):

| Front design 33 with | Engagement | Harm | Benefit | Wrongful removals and demotions a day |
| --- | --- | --- | --- | --- |
| nothing changed | +36% | -72% | +0.15 | 66 (engagement maximiser: 2) |
| notifications back to 0.6 | -4% | -74% | +0.13 | 57 |
| account cost back to 0.2 | +34% | -75% | +0.14 | 102 |
| predictor quality back to 0.85 | +34% | -71% | +0.14 | 64 |
| classifier threshold back to 0.7 | +69% | -40% | +0.10 | 1 |
| all four back | +22% | -33% | +0.07 | 2 |

Its engagement lead comes entirely from notifications at full strength, and most of its harm reduction from a
classifier that removes at a threshold of 0.2 and so takes down or demotes about 66 posts a day whose toxicity is below
0.5. The model charges nothing for either (section 16): the harm index does not count wrongful removals, and on a good
feed every extra session adds wellbeing. Without those levers the design still beats the engagement maximiser on all
three objectives (+22% engagement, -33% harm, +0.07 benefit), so coexistence survives, at roughly half the size. A
search that is free to push such levers will push them; the other designs in the table above were not taken apart this
way, so read them, and the Atlas, with this in mind.

**The shape of the front.** It runs from engagement 0.10 at harm 0.02 to engagement 0.41 at harm 0.47. Designs with harm
at or below 0.12 exist up to an engagement index of about 0.30; beyond that the lowest harm on the front climbs steeply,
to 0.15 between 0.30 and 0.35 and to 0.25 above 0.35. Benefit peaks in the middle (0.83 at engagement 0.22) and is lower
at both ends: the calmest designs reach fewer people, the most engaging ones cost wellbeing. The far end is a trade the
engagement maximiser does not offer either: front design 64, rerun at full size, has 83% more engagement than the
engagement maximiser (0.479) with 14% more harm (0.567) and less toxicity (0.095), but lower wellbeing (0.441 against
0.594) and survey satisfaction (0.544 against 0.705).

**Goodhart runs.** Maximising the engagement index alone raised the best design's engagement from 0.19 to 0.37 over 24
generations, while its harm index rose from 0.27 to 0.53, toxic impressions from 1% to 18%, polarisation from 0.41 to
0.50, and its benefit fell from 0.44 to 0.37 and the wellbeing of ever-users from 0.65 to 0.55. The search learned to
switch the classifier off, give up human review and notes, and push notifications and the like and dwell weights to the
top. At full size its final design scores engagement 0.435, harm 0.662 and wellbeing 0.507, worse than the engagement
maximiser on harm, toxicity, benefit and wellbeing (it does pay some creators: 18% earn a living). Maximising survey
satisfaction alone did the opposite: the survey rose from 0.79 to 0.86 and the truth improved with it: benefit from 0.45
to 0.63, wellbeing from 0.69 to 0.77, Creators earning a living from 3% to 67%, harm from 0.11 to 0.09, and even
engagement from 0.17 to 0.23. At full size the final design beats the engagement maximiser on engagement (0.275 against
0.263) and cuts harm to a quarter (0.121 against 0.499). In this model the biased survey (9.1) is a far better proxy
than engagement: its bias toward happy current users did not open a gap the search could exploit within 25 generations,
although a survey blind to those who left could in principle be gamed by driving unhappy users away. That it was not is
a finding, not a guarantee.

**Limits of these results.** One world (the defaults), one search seed per search, two seeds per candidate and a reduced
size. Levers that have no effect in a design (the alternative feed when nobody uses it, the classifier's settings when
it is off, the payout scheme when nothing is paid) drift freely during a search; the reports and the Atlas tab leave
them out when they list how a design differs. Rerunning with another `--config` tests whether a finding survives a
different world.

## 14. Presets and scenarios

### 14.1 Presets

- Engagement maximiser: the defaults of section 10.
- Chronological: `chronological` true.
- Quality and bridging: `w_like` 0.5, `w_reply` 0.3, `w_share` 0.5, `w_quality` 1.0, `w_bridging` 1.0,
  `w_see_less` 1.0, `diversity` 0.3, `community_notes` on.
- Calm by design: `share_friction` 0.5, `reshare_depth_cap` 2, `like_counts_visible` false, `notification_intensity`
  0.2, `time_limit_share` 0.3, `classifier_threshold` 0.5, `review_capacity` 3.0.
- Subscription: `ads_share` 0, the Quality and bridging ranking, `payout_share` 0.3 with FanFunded payouts.
- Engagement-paid creators: the Engagement maximiser with `payout_share` 0.3 and PerEngagement payouts.

### 14.2 Scenarios

A scenario is a starting configuration, a duration and goals. Goals compare the player's run with a baseline run (the
Engagement maximiser on the same seed and world), so a scenario is always a side-by-side comparison. The thresholds
below are starting values; *calibrate* each so the baseline fails and at least one recorded design passes, and keep
that design as the scenario's known solution in the tests.

1. The engagement trap (365 days): keep at least 85% of the baseline's engagement index and halve its toxic impression
   share.
2. Toxicity spiral (180 days, Troll share 0.08, `review_capacity` 0.3): bring harassment per 1,000 users below half the
   baseline's while losing at most 20% of its human daily actives.
3. A living for creators (365 days, `payout_share` 0.3): more Creators earning a living than the baseline, with less
   than 20% of payouts going to Outrage, Clickbait and Attack posts.
4. Misinformation surge (180 days, `credulity_scale` 1.6, Clickbait-heavy bots): non-users' informedness at least the
   baseline's plus 0.05.
5. Public square (365 days): society-wide affective polarisation below the baseline's, with at least 80% of its
   engagement index.

## 15. Stylized facts the model is checked against

Each fact becomes an automated test on small runs (for example `N` = 600, 120 days, 4 seeds, paired by seed) that
checks the direction with a margin. The CLI's `facts` command runs the same checks at full size and prints the effect
sizes.

1. Engagement ranking amplifies divisive content that users do not value. Against a chronological feed, the
   Engagement maximiser shows more Outrage impressions, and the reflective value of what it shows is lower. Milli et
   al. (2025) found in an audit of Twitter's ranking that the engagement-based algorithm amplified emotionally charged,
   out-group hostile content that users did not prefer.
2. Social reward teaches outrage. Under the Engagement maximiser the Outrage share of posts rises over the run; with
   `learning_rate` 0 it does not. Brady et al. (2021).
3. A chronological feed cuts time spent, and over three months barely moves polarisation. Guess et al. (2023) found
   both on Facebook and Instagram during the 2020 US election.
4. Forwarding limits slow viral spread. With `reshare_depth_cap` 1 the largest share cascades shrink markedly. de
   Freitas Melo et al. (2019) modelled WhatsApp's limits.
5. Harassment drives its targets away. More Trolls raise the departure rate of Vulnerable users. Pew Research Center
   (2017).

## 16. Known limits

- One platform, two opinion groups, six topics and six styles: real platforms have many communities and formats.
- Human review is perfectly accurate and the classifier sees only the post; real moderation errs in both directions
  and sees context.
- People do not change archetype, only their learned style values, habits and attitudes.
- Effect sizes are starting values grounded in a handful of studies; the world-assumption panel exists because they
  are uncertain.
- The model has no advertisers, no regulators and no competing platforms.
- Over-moderation is nearly free. The harm index counts toxic and misinformation impressions, harassment and
  polarisation, not wrongful removals; a strict classifier's false positives show only in their own metric, so
  searches favour strict classifiers (13.7).
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

## References

- Allcott, H., Braghieri, L., Eichmeyer, S., & Gentzkow, M. (2020). The welfare effects of social media. *American
  Economic Review*, 110(3), 629-676. <https://doi.org/10.1257/aer.20190658>
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
