# Validation of the incentive lab model (lab #31, updated after milestone 3)

This document records how well the model of [model.md](model.md) reproduces empirical findings, and how stable and
sensitive its results are under noise on the world assumptions. Lab #31 wrote it on 2026-11-27; milestone 2 (labs #32
to #39, hub #25) and milestone 3 (labs #40 to #45, hub #26) implemented the recommendations of its section 8 in turn.
This update, of 2026-11-28, reports every result again from runs of the release build after milestone 3 (the working
tree on top of 48f0c17; binary `exports/robustness/bin/incentive-r6`, with its SHA-256 beside it). Where it helps to
judge what changed, milestone 2's value stands beside the new one, marked "M2" (from `exports/validation-r5/` and
`exports/robustness/r5-*`); lab #31's values are in earlier versions of this file (git history) and in one line of 4.2.
Every number names the file it comes from; the files sit under the gitignored `exports/` folder and the commands of 2.8
regenerate them.

## 1. Purpose

The lab is a toy model for intuition about incentive design on one platform (model.md, section 1). Its results are only
as strong as two things this document measures:

- **Fit and accuracy.** Fit is how well the model reproduces what it was tuned on (the stylized facts, the calibration
  targets and the headline claims). Accuracy is how well it reproduces field findings it was never tuned on, with bands
  fixed before the model was run on them.
- **Stability and sensitivity.** Stability is whether a result survives small noise (about +-25%) on every world
  assumption around the defaults. Sensitivity is how a result moves across a range of each assumption, and which
  assumptions drive it. There are two sets of ranges: plausible ranges, which stand for beliefs, and the revised ranges
  of model.md section 11, which explore. Every summary gives the plausible hold share and, beside it, the revised one
  over the non-degenerate worlds (lab #45).

The analysis ends in what milestone 3 achieved and new recommendations (section 8), and in the limits of this validation
(section 9).

## 2. Method

### 2.1 In-sample and out-of-sample

In-sample checks were used in tuning, so passing them shows the calibration: the seven stylized facts of sections 13.4
and 15 of model.md (14 required checks, no known limit), the calibration targets of 13.2, among them since lab #40 the
toxic share under the integrity preset, and the headline claims H1 to H8 (`crates/incentive-core/src/headline.rs`), the
exhibit claims of 13.3 written as band checks. The held-out regretted share of 13.4 was named before tuning and never
tuned on; it is reported with the in-sample results because it belongs to the calibration record.

Out-of-sample checks are the battery V1 to V10 (`crates/incentive-core/src/validation.rs`, run by `incentive validate`):
ten field findings with 21 required checks and one known limit, none used in calibration. Sources and quotes are in
`exports/research-validation/notes.md` and `exports/research-2026-11-26/notes.md`.

### 2.2 Pre-registration

Lab issue #31 fixed the out-of-sample checks and their bands from the literature before any run (its comment
of 2026-11-26, 22:30). The bands are relative changes unless the field gives standard deviations. An amendment at 22:35,
also before any V run, moved every check onto the integrity preset (2.3). No band was set after a model result was seen,
and milestones 2 and 3 kept every band.

Each milestone fixed its in-sample targets before tuning, from sources other than the V checks'
(`exports/research-recs/notes.md`). Milestone 2: the misinformation share under the integrity preset, 2.5% (lab #32),
the note timing (lab #34), the causal relative risk of a hostile post after a fully hostile feed, 1.6 (lab #38), and the
plausible ranges (lab #35). Milestone 3 (lab #40, its comment of 2026-11-27, 20:50, before any tuning run): the toxic
share of impressions under the integrity preset at 3.2%, band 2.2% to 4.2%, the share of Facebook feeds that Guess et
al. (2023) estimated as uncivil, the in-sample study behind fact 3 (Nyhan et al.'s 3.15%, V1's source, was not used).
The mapping was fixed with it: a post whose toxicity exceeds 0.5 counts as uncivil, as a classifier's label would mark
it, one to one, without rescaling (a loose mapping, section 9). The rule changed only the base toxicity of the two
hostile styles (model.md 4.1) and chose the smallest shift that reached the band; it moved Outrage from 0.35 to 0.30.
The refits that followed ran in an order fixed on the issue: the contagion, the doses, the fixed point, then lab #44's
check of `polarisation_rate`. The V checks were not looked at while tuning; the doses were read from their dose lines
alone.

Two checks need a dose, set as manipulation checks, not outcomes. V1's same-group down-ranking penalty is calibrated so
that the treated's same-group impression share falls by a third, as in the field (53.7% to 36.2%), and V3's hiding
threshold so that about 7% of the treated's impressions are hidden. Lab #40 re-set both at the new constants on
lab #31's grids (model.md 13.2's last block): V1's penalty moved from 0.28 to 0.26 (-0.327 on 8 seeds), and V3's
threshold stayed at 0.56 (0.0727). On the 16 seeds of this update they give -0.314 (95% interval -0.360 to -0.268)
and 0.0689 (0.0641 to 0.0737) under the integrity preset, and -0.333 and 0.197 under the engagement ranking
(`exports/validation-r6/validation.md`; M2, not re-set: -0.355 and 0.0786). V4's friction dose does not map onto the
field's accuracy prompt, so V4 judges direction only.

Design changes made after a failure had been seen, each with the field's design as its rule and fixed on the issue
before any rerun: V5 and fact 3's exposure checks moved to a mid-run switch on a shared follow graph (lab #33, 2.4), and
V7 moved to the field's notes design (lab #41, 4.3). V7's change was decided after milestone 2 had measured the result
it gives, so V7 no longer counts as a genuine test (4.2).

### 2.3 The integrity preset as the realistic feed

The field studies ran on Facebook, Instagram and Twitter, whose ranked feeds carry integrity measures. The model's
stand-in is the integrity preset (model.md, 14.1): the engagement ranking with demotion instead of removal, a quality
signal, see-less feedback and demoted community notes. Facts 3, 6 and 7 already compare against it. Every V check is
judged under it; the same check under the plain engagement ranking is reported beside it as a sensitivity and not
judged. V7 is the one exception to the preset as it stands: it runs the preset with notes on and no ranking penalty for
noted posts, the field's notes design (lab #41), and its engagement-ranking twin likewise.

### 2.4 Paired designs and the switch design

Every comparison is paired by seed: both arms run on the same seeds, so they share the population and its draws. The
experiments (facts 3, 6 and 7, and V1, V2, V3, V5, V10) go further: the real run and its placebo draw the same treated
people, and the effect is the treated's change in the real run against their change in the placebo. The field compares
randomised groups instead; the paired design removes most of the between-person noise and cannot be run in the field.
The feed experiments treat a random 0.2 of the regular users over the last 90 days (V1, V5, V10, fact 3) or 42 days
(V3); the break of V2 lasts 35 days.

The chronological switch (lab #33, model.md 13.2) is a feed experiment of kind `chronological`: under the integrity
preset the treated read the chronological feed of their in-network posts for the last 90 days, real against a placebo
that keeps them on the ranked feed, on the follow graph the ranked feed built over the earlier days. It copies Twitter's
holdback (V5, Huszar et al. 2022) and Guess et al. (2023)'s three-month switch (fact 3's toxic and misinformation
checks).

### 2.5 Verdict rules

At the defaults each V check runs on 16 seeds at full size (`N = 2,000`, 365 days; lab #43, 8 seeds before), the facts
and the headline claims on 8. The verdict judges the 95% t-interval of the per-seed effect: PASS when it lies inside the
band, FAIL when it lies wholly outside, UNDECIDED when it crosses a bound. Seed means are reported beside the intervals.
A known limit is reported and not required.

Ratio convention (lab #39), one for every check of the battery, the facts and the headline claims: a ratio, relative
change or elasticity effect is a ratio of seed means, the mean of one arm over the mean of the other on paired seeds.
Its 95% interval and its verdict are of the per-seed values (`a / b` on each seed), so the interval's centre, the mean
of the per-seed ratios, can differ from the effect; `validation.json` and `facts.json` keep both (`effect` and
`interval.mean`). Within one run, V6, V7 and V8 pool their counts over the summary window (a ratio of sums, not a mean
of daily ratios; V7 since lab #41), and the feed experiments' shares and reach measures pool over the treated group and
the experiment's days. The old means of daily ratios are printed as other measures, not judged.

An effect in standard deviations divides by the "SD used", which `incentive validate` prints with it: the participants'
spread at the experiment's start, the mean over the seeds of the SD in the effect's first arm. At the defaults it
is 0.2218 for polarisation (V1, V10), 0.2350 for affective polarisation (V2), and 0.0240 and 0.0231 for informedness
(V2, V10) (`exports/validation-r6/validation.md`). The raw change is printed beside it.

In the robustness runs (2.6) a check holds in a world when its effect on the seed means lies inside its band, and a fact
or claim holds when all its required checks hold. "Sign" is the share of worlds whose effect lies on the default world's
side of its neutral value: 0 for a difference or a relative change, 1 for a ratio and for the ratios and shares that V6,
V7 and V8 read. V7's default now lies below 1 (0.757 on the robustness runs' seeds), so its sign is the share of worlds
with some shares after a note; at M2 it was the share of worlds at exactly 1.000.

### 2.6 Noise designs, range rules and failure classes

All three designs vary 54 settings: 51 world assumptions of the catalogue plus the troll share, the bot share and
`bot_strategy`. Held at their defaults (lab #35): the checking habits (`recheck_rate`, `notification_pull`,
`extra_session_share`) and `interruption_cost`, off by the owner's decision (model.md, 6.1 and 13.2), and
`base_session_items`, which sets the time scale like them. Design and run settings never vary. Seeds come in three
tiers, the same seeds in every world: seeds 1 and 2 at full size for all three batteries; seeds 1 to 4
(`--seeds-noisy 4`) for facts 2 and 5, H5, V1, V4 and V10, and H7 in the two global designs; and seeds 1 to 8
(`--seeds-noisiest 8`, lab #43) for V3, V5 and V9, whose local seed shares had stayed at 0.59 to 0.84 on 4 seeds.

- **Local (stability):** 100 worlds. Each assumption with a nonzero default is multiplied by U(0.75, 1.25) and clamped
  to its catalogue range; `leave_threshold` and `polarisation_backfire` (default 0) and `bot_strategy` keep their
  defaults, so 51 are jittered.
- **Plausible (belief):** 200 worlds on a Latin hypercube over lab #35's plausible ranges: default/1.5 to 1.5 x default
  for the 40 positive scales and the two population shares, default +- 0.15 (clamped to [0, 1]) for the 10 shares and
  weights, -0.05 to 0.05 for `leave_threshold`, and `bot_strategy` held at adaptive. The rules are M2's; the ranges
  follow the defaults that lab #40's fixed point moved (`time_cost` 0.035, the price 1.99, the uptake scale 0.097,
  `living_income` 0.025).
- **Global (exploration):** 240 worlds on a Latin hypercube over the revised ranges of model.md section 11, with
  `bot_strategy` drawn uniformly: 39 documented, 5 scaled, 4 default/3 and 2 population ranges by lab #31's rule, and
  three judgement ranges (`leave_threshold` -0.1 to 0.1, `exposure_contagion` 0 to 0.56, `outgroup_like_cut` 0 to 1).
  Only `living_income`'s range moved with its default (0.0083 to 0.075, default/3 to 3 x default); with the same sample
  seed the 240 worlds are M2's, so the two rounds compare like for like.

Sensitivity is the partial rank correlation (PRCC) of each assumption with each effect, with its Spearman correlation,
over the non-degenerate worlds. A PRCC is marked significant when it lies beyond two standard errors of 0, the SE
being 1/sqrt(n - k - 2) for n worlds and k regression terms. With 53 numeric assumptions, about 4.55% of them, 2.4, lie
beyond 2 SE by chance alone. R2 is the adjusted R2 of a rank regression of the effect on every varied number (and the
strategy's dummies in the global design).

Failure classes (lab #35; robust SD since lab #42): a failing world is "noise-compatible" when its effect lies within
two standard errors of its band, and "clear" otherwise. The SE is the seed SD over sqrt(seeds), and the seed SD is now a
robust one: the median over the non-degenerate worlds of each world's SD of its per-seed effects, divided by the normal
consistency factor for its number of seeds (0.674 for 2, 0.888 for 4, 0.952 for 8), skipping non-finite values
(`crates/incentive-cli/src/robustness/stats.rs`, `robust_sd`; `mod.rs`, `seed_noise`). A guard: when that median is 0
while some world spreads, the seed SD is NaN and the failures stay unclassed. `summary.json` keeps the pooled SD that M2
used (`seed_sd_pooled`) and counts the worlds whose seeds agree exactly (`zero_spread_worlds`). One SE serves every
world of a check, so "clear" means beyond the typical world's seed noise, not beyond that world's own. An item fails
within noise when every failing required check does. The "seed share" is the squared SE over the variance of the worlds'
effects: the part of their spread that seed noise alone would make. It now rests on the robust SE, so it is not directly
comparable with M2's pooled shares; a pooled share is the robust one times (pooled SD / robust SD) squared, both in
`summary.json`. Section 5.3 compares the two SDs.

### 2.7 Degenerate worlds

A world is degenerate when the default design's user share on its last day, averaged over seeds 1 and 2, lies
below 0.05: the platform has emptied, and most effects become ratios of near-zero quantities. The summaries report hold
shares over all worlds and over the non-degenerate ones.

### 2.8 Files and commands

| Material | Files |
| --- | --- |
| Validation at the defaults, 16 seeds | `exports/validation-r6/validation.md`, `validation.json`; doses in |
| | `exports/validation-r6.log` |
| Facts, H1 to H8 at the defaults, 8 seeds | `exports/recs3-fixpoint/final/facts/`, `final/headline/` (lab #40) |
| Calibration report (model.md 13.3 to 13.6) | `exports/report-r6/report.md` |
| Local noise, 100 worlds | `exports/robustness/r6-local/` |
| Plausible ranges, 200 worlds | `exports/robustness/r6-plausible/` |
| Revised ranges, 240 worlds | `exports/robustness/r6-global/` |
| V7 on the robustness seeds (added, 5.3) | `exports/analysis-r6/validate-v7-seeds2/` |
| This update's analysis | `exports/analysis-r6/*.txt`, from the scripts of the same names |
| M2's runs and analysis | `exports/validation-r5/`, `exports/robustness/r5-*/`, `exports/analysis-r5/` |

Each robustness folder holds `summary.md`, `summary.json`, `checks.csv` (with the per-seed effects), `worlds.csv` and
`worlds.json`. The analysis scripts in `exports/analysis-r6/` are `compare.py` (M2 against this round, every check, with
drivers), `tables.py` (compact tables), `classes.py` (robust against pooled classes, 5.3), `world0.py` (the default
world on the robustness seeds against many seeds, 5.3), `degenerate.py` (degenerate worlds and bot strategies, 6.2
and 6.3), `v5_bots.py` (effects by bot strategy) and `top3.py` (drivers); run them from the repository root with
`python3 -I`. Every CLI run went through `taskset -c 0-7 nice -n 19 ionice -c 3` (`exports/robustness/r6-chain.sh` for
the first four, with `BIN` the binary named above):

```text
$BIN validate --seeds 16 --out exports/validation-r6
$BIN robustness --mode local --worlds 100 --seeds 2 --seeds-noisy 4 --seeds-noisiest 8 --sample-seed 1 \
  --battery all --out exports/robustness/r6-local
$BIN robustness --mode global --ranges plausible --worlds 200 --seeds 2 --seeds-noisy 4 --seeds-noisiest 8 \
  --sample-seed 1 --battery all --out exports/robustness/r6-plausible
$BIN robustness --mode global --ranges revised --worlds 240 --seeds 2 --seeds-noisy 4 --seeds-noisiest 8 \
  --sample-seed 1 --battery all --out exports/robustness/r6-global
$BIN validate --seeds 2 --check V7 --out exports/analysis-r6/validate-v7-seeds2
```

The last run was added by this update. The facts and the default world's headline claims are lab #40's runs (`incentive
facts --seeds 8`; `incentive robustness --battery headline --worlds 0 --seeds 8`), and the report is `incentive report
--seeds 1..8` (model.md 13.3). M2's commands are in its version of this section (git history).

## 3. In-sample fit

### 3.1 Stylized facts

The facts at the fixed point after lab #40 (model.md 13.4; `exports/recs3-fixpoint/final/facts/`, 8 paired seeds).

| Fact | Check | Effect | 95% interval | Band | Verdict | M2 |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | Outrage impressions, ER minus chronological | 0.540 | 0.489 to 0.591 | 0.15 to 0.6 | pass | 0.469, pass |
| 1 | Reflective value per impression, chrono minus ER | 0.222 | 0.191 to 0.253 | 0.05 to 0.4 | pass | 0.193, pass |
| 2 | Rise of the Outrage share of posts under ER | 0.091 | 0.072 to 0.110 | 0.01 to 0.15 | pass | 0.062, pass |
| 2 | That rise minus the rise without learning | 0.051 | 0.035 to 0.067 | 0.01 to 0.15 | pass | 0.023, undecided |
| 3 | Time spent, chronological over ER | 0.598 | 0.581 to 0.617 | 0.4 to 0.9 | pass | 0.636, pass |
| 3 | Polarisation difference at day 90 | 0.044 | -0.051 to -0.037 | below 0.05 | undecided | 0.037, pass |
| 3 | Toxic share, switch, chrono over integrity | 0.685 | 0.548 to 0.897 | 0.5 to 0.9 | pass | 0.757, pass |
| 3 | Misinformation share, switch, same | 1.797 | 1.682 to 1.920 | 1.05 to 3 | pass | 1.682, pass |
| 4 | Largest cascades, depth cap 1 over none | 0.754 | 0.715 to 0.797 | 0.3 to 0.85 | pass | 0.756, pass |
| 5 | Vulnerable who left, troll share 0.12 minus 0.03 | 0.094 | 0.075 to 0.113 | 0.03 to 0.3 | pass | 0.095, pass |
| 6 | Break group's wellbeing, real minus placebo, SDs | 0.059 | 0.016 to 0.103 | 0.02 to 0.25 | undecided | 0.066, U |
| 7 | Time a day, notifications off minus placebo, SDs | -0.162 | -0.179 to -0.146 | -0.4 to 0.1 | pass | -0.166, pass |
| 7 | Sessions a day, off minus placebo, SDs | -0.319 | -0.345 to -0.292 | -0.4 to 0.1 | pass | -0.322, pass |
| 7 | Wellbeing, off minus placebo, SDs | -0.002 | -0.005 to 0.001 | -0.15 to 0.15 | pass | -0.002, pass |

All 14 required checks hold on the seed means; 12 pass on their intervals and 2 are undecided (M2: 12 and 2). The
undecided pair changed: fact 2's learning check now passes and fact 3's polarisation check became undecided. Lab #40's
milder Outrage posts are removed and demoted less, so feeds hold more Outrage: fact 1's difference grows to 0.540,
fact 2's rise to 0.091 and its learning check to 0.051, and polarisation under the engagement ranking grows faster, so
fact 3's polarisation check reaches 0.044, 0.006 from its bound (lab #44; model.md 13.2's last block). The rule that set
`polarisation_rate` (no saturation within months) still holds, so the constant stayed at 0.00015. Fact 3's toxic switch
check scatters more across seeds (0.685 ± 0.209, against 0.757 ± 0.055), since the toxic shares it divides are less than
half as large. Fact 6 and fact 7's use checks were tuned on (`time_cost` and the opening chance's notification factor),
so passing them shows the calibration; fact 6 is undecided by construction, its effect fixed at 0.060 with a paired
spread of about 0.05.

Fact 2 is carried mostly by the exposure contagion of lab #38, not by learning from likes. M2 measured it
(`exports/brady-check/`, 8 seeds; model.md 13.4): with the contagion off the rise was 0.019 and the learning
check 0.015, against 0.062 and 0.023 with it. That check was not rerun after lab #40. The contagion raises the Outrage
share in both arms of the learning check, which confounds it (model.md 15), so the learning check's new pass does not by
itself show learning from likes (recommendation 8 of 8.2).

### 3.2 Calibration targets and the held-out target

At the fixed point after lab #40 (model.md 13.2, its last block; `exports/recs3-fixpoint/`, 8 seeds, default design
unless named):

| Target | Now | Band | Verdict | M2 |
| --- | --- | --- | --- | --- |
| Toxic share, integrity (lab #40) | 0.0433 (0.0322 to 0.0544) | 0.022 to 0.042 | above on mean | 0.105 |
| User share after 365 days | 0.511 (0.476 to 0.546; lowest seed 0.456) | 0.4 to 0.9 | holds | 0.468 |
| Toxic impression share | 0.131 (0.094 to 0.169) | 0.03 to 0.2 | holds | 0.209, fails on mean |
| Outrage share of posts | rises by 0.091 (0.072 to 0.110), ends at 0.395 | rises, below 0.5 | holds | +0.062, 0.351 |
| Harm cut at a moderate engagement cost | Q&B: harm -83%, engagement -24% | judgement | holds | -83%, -17% |
| Causal relative risk of hostile posts (lab #38) | 1.591 at 0.28 | 1.4 to 1.8 | fitted | 1.601 |
| Held-out regretted share | 0.0076 (0.0067 to 0.0086) | 0.20 to 0.42 | fails | 0.0055 |

The toxic share under the integrity preset misses its new target narrowly. Lab #40's rule chose Outrage 0.30
at 0.0418, 0.0002 inside the band's top; the refits that followed, which act on everything a person sees, moved it
to 0.0433, 0.0013 above it, with 3.2% inside the interval and one seed of eight (0.074) carrying most of the excess. The
pre-registered order does not repeat the profile's grid after the refits, so nothing was retuned; a supplementary run at
Outrage 0.29 gave 0.0427, also above, since the seeds' spread is wider than the band's last hundredth (model.md 13.2
and 16). The default design's toxic share, 0.131, lies inside section 13's band again; it was checked, not tuned. Two
targets named for the default design, sessions per daily active (T3) and the notification share of sessions (T1), are
missed by the owner's decision and hold only in the checking habits world, which was not rerun at these constants.

The held-out regretted share still fails on both its interval and its mean, two orders of magnitude below its band: with
one session a day the model has almost no compulsive use (model.md 13.4).

### 3.3 Headline claims

All 13 checks of H1 to H8 hold at the defaults on 8 seeds (`exports/recs3-fixpoint/final/headline/`; the report rows of
model.md 13.3 named). Effects follow the ratio convention; the intervals are of the per-seed effects.

| Claim | Check | Band | Effect (95% interval) | M2 |
| --- | --- | --- | --- | --- |
| H1 | Harm index, Q&B against ER, relative (row 20) | -1 to -0.5 | -0.827 (-0.855 to -0.788) | -0.826 |
| H1 | Engagement index, Q&B against ER, relative | -0.33 to 0.33 | -0.237 (-0.256 to -0.217) | -0.165 |
| H2 | Survey satisfaction, ER minus Q&B | above 0 | +0.013 (+0.008 to +0.017) | +0.007 |
| H2 | Wellbeing of ever-users, ER minus Q&B | below 0 | -0.092 (-0.103 to -0.081) | -0.078 |
| H3 | Toxic impressions, threshold 0.3 over default (row 9) | 0 to 0.5 | 0.069 (0.045 to 0.104) | 0.058 |
| H3 | False-positive actions, same | above 3 | 12.9 (12.5 to 13.4) | 12.5 |
| H4 | Harm index, trap solution minus ER (row 1) | below 0 | -0.164 (-0.205 to -0.122) | -0.166 |
| H4 | Engagement index, trap solution over ER | above 0.9 | 1.119 (1.086 to 1.153) | 1.119 |
| H5 | Engagement index, intensity 1 over 0.6 (row 31) | above 1 | 1.253 (1.239 to 1.268) | 1.238 |
| H5 | Wellbeing of users, intensity 1 minus 0.6 | below 0.01 | -0.006 (-0.012 to -0.0003) | -0.0073 |
| H6 | Engagement index, last 95 days, ER over Q&B (row 13) | above 1 | 1.306 (1.272 to 1.339) | 1.195 |
| H7 | Harassment per 1,000, report trust 1 minus 0, Q&B (row 26) | below 0 | -2.45 (-3.19 to -1.72) | -1.94 |
| H8 | Harm index, `w_see_less` 5 minus 0 (row 6) | above 0 | +0.075 (+0.018 to +0.132) | +0.197 |

The narrowest margin is now H8's: its effect fell to about a third of M2's, and one seed of eight lies below 0. The harm
index counts the toxic share, which lab #40 made a third to a half rarer; these runs do not take the smaller backfire
apart (recommendation 2 of 8.2), and section 6 shows that it also made H8 hinge on the assumptions. H1's engagement cost
grew from 17% to 24% at the new constants, still inside its band (model.md 13.2 does not separate the causes), and H2's
survey difference, the narrowest at M2, now has an interval starting at +0.008.

In sample the model fits what it was tuned on: every required fact holds on the seed means, every headline claim holds
at the defaults, and every target named for the default design holds except T1 and T3, by decision. The toxic share
under the integrity preset lies 0.0013 above its new band. The one prediction it was not tuned on, the regretted share,
still lies a factor of about 26 below its band.

## 4. Out-of-sample accuracy

### 4.1 The checks at the defaults

Integrity preset, 16 paired seeds, full size (`exports/validation-r6/validation.md` and `.json`; M2 from
`exports/validation-r5/`, 8 seeds); "rel." marks a relative change, "U" undecided.

| Id | Check | Model (95% interval) | Band | Verdict | M2 |
| --- | --- | --- | --- | --- | --- |
| V1 | Polarisation, SDs | -0.064 (-0.075 to -0.053) | -0.12 to 0.12 | pass | -0.068, pass |
| V1 | Toxic share, rel. | -0.016 (-0.043 to 0.013) | -0.35 to -0.02 | U | -0.015, U |
| V1 | Time per day, rel. | -0.060 (-0.066 to -0.053) | -0.10 to 0.05 | pass | -0.065, pass |
| V1 | Misinformation share, rel. | -0.132 (-0.151 to -0.113) | -0.5 to 0 | pass | -0.140, pass |
| V2 | Affective polarisation, SDs | -0.054 (-0.061 to -0.048) | -0.14 to 0.02 | pass | -0.053, pass |
| V2 | Informedness, SDs | -0.531 (-0.570 to -0.491) | -0.30 to -0.05 | FAIL | -0.556, FAIL |
| V2 | Later use, rel. | -0.215 (-0.227 to -0.201) | -0.35 to -0.08 | pass | -0.203, pass |
| V3 | Time per day, rel. | -0.0085 (-0.0130 to -0.0038) | -0.25 to -0.02 | FAIL | -0.009, FAIL |
| V3 | Engagement per day, rel. | -0.020 (-0.025 to -0.014) | -0.30 to 0 | pass | -0.022, pass |
| V3 | Own posts' toxicity, rel. (limit) | -0.002 (-0.007 to 0.003) | -0.45 to -0.10 | FAIL (limit) | -0.004, FAIL |
| V4 | Truth of shares, rel. | +0.010 (0.005 to 0.016) | 0 to 0.25 | pass | +0.010, pass |
| V5 | Partisan half's reach, ranked over chrono | 1.342 (1.275 to 1.433) | 1 to 5 | pass | 1.324, pass |
| V5 | Extremes' amplification over the middle's | 1.852 (1.646 to 2.222) | below 1.1 | FAIL | 1.803, FAIL |
| V6 | Out-group impressions over candidates | 0.943 (0.912 to 0.974) | 0.85 to 1 | pass | 0.959, U |
| V6 | Out-group likes over impressions | 0.605 (0.585 to 0.625) | 0.75 to 0.97 | FAIL | 0.648, FAIL |
| V7 | Shares before the note | 0.812 (0.792 to 0.833) | 0.70 to 0.95 | pass (post hoc) | 1.000, FAIL |
| V8 | Shares per impression, Outrage over others | 2.716 (2.650 to 2.782) | 1.3 to 3 | pass | 2.715, pass |
| V9 | Elasticity of time to ad load | -0.009 (-0.022 to 0.004) | -0.15 to -0.01 | U | -0.018, U |
| V10 | Misinformation share, rel. | -0.080 (-0.104 to -0.054) | -0.8 to -0.05 | pass | -0.103, pass |
| V10 | Engagement per day, rel. | -0.035 (-0.046 to -0.023) | -0.5 to -0.01 | pass | -0.035, pass |
| V10 | Polarisation, SDs | -0.030 (-0.037 to -0.024) | -0.1 to 0.1 | pass | -0.024, pass |
| V10 | Informedness, SDs | +0.067 (0.040 to 0.093) | -0.5 to 0 | FAIL | +0.060, FAIL |

M2's V7 was the integrity preset with its penalty for noted posts, as a mean of daily shares. The field values, the
engagement ranking's effect with the verdict its interval would get (not judged; 16 seeds), and the kind of test (4.2),
in the same order; the sources are in `exports/validation-r6/validation.md`.

| Id | Check | Field | Engagement ranking | Kind |
| --- | --- | --- | --- | --- |
| V1 | Polarisation | null, within 0.12 SD | -0.077, pass | calibration |
| V1 | Toxic share | -11% | -0.044, undecided | test |
| V1 | Time per day | -0.02 SD, null | -0.100, undecided | test |
| V1 | Misinformation share | -28% | -0.064, pass | test |
| V2 | Affective polarisation | -0.06 SD, -0.031 SD | -0.090, pass | calibration |
| V2 | Informedness | -0.19 SD, -0.098 SD | -0.220, pass | test |
| V2 | Later use | -23%, -15% | -0.261, pass | test |
| V3 | Time per day | -10% | -0.027, pass | test |
| V3 | Engagement per day | -0.056 SD | -0.050, pass | test |
| V3 | Own posts' toxicity | -30% | -0.024, fail | test (limit) |
| V4 | Truth of shares | +2.1% to +9% | +0.008, pass | assumed |
| V5 | Partisan half's reach | above 1 | 1.432, pass | test |
| V5 | Extremes' amplification | about 1 | 1.723, fail | test |
| V6 | Out-group impressions | 0.92 to 0.95 | 0.796, fail | test |
| V6 | Out-group likes | 0.83 to 0.94 | 0.443, fail | assumed |
| V7 | Shares before the note | about 0.85 | 0.894, pass | post hoc (was test) |
| V8 | Outrage shares | 1.35 to 2.8 | 2.424, pass | assumed |
| V9 | Ad load elasticity | -0.02 to -0.07 | -0.033, pass | assumed |
| V10 | Misinformation share | down | -0.084, pass | test |
| V10 | Engagement per day | down | -0.021, pass | test |
| V10 | Polarisation | null | -0.026, pass | calibration |
| V10 | Informedness | down | +0.084, fail | test |

Of the 21 required checks 14 pass, 2 are undecided and 5 fail on their intervals (M2: 12, 3 and 6); 14 hold on the seed
means, since both undecided checks lie just outside their bands on the mean (V1's toxic share at -1.6% against -2%, V9
at -0.009 against -0.01). Moved into the band: V6's ranking ratio (0.943, an interval now wholly inside) and V7 (a new
design, post hoc). Nothing moved out. Sixteen seeds narrow each interval by about a third; the one verdict they helped
change is V6's ranking ratio, whose mean also fell. The known limit, V3's own toxicity, still fails.

### 4.2 Scorecard

**Directions against magnitudes.** 17 required checks have a field direction; the other 4 are nulls or an upper bound
(V1 polarisation and time, V10 polarisation, V5's extremes). Judged on the 95% intervals:

| | Checks | Direction right | Direction wrong | Not established | Magnitude in band | M2 |
| --- | --- | --- | --- | --- | --- | --- |
| Directional | 17 | 14 | 1 (V10 inform.) | 2 (V1 toxic, V9) | 11 | 12, 2, 3, 9 |
| Nulls and bound | 4 | - | - | - | 3 (V5 extremes fails) | 3 |

The direction is right for 14 of 17 (M2: 12) and the magnitude for 11 of 17 (M2: 9; both 11 on the seed means). Two
checks gained: V6's ranking ratio established its direction and size, and V7 moved from a wrong direction (1.000, at the
boundary of no share after a note) to the field's; without V7 the counts are 13 of 16 and 10 of 16. Where the direction
is right but the check fails, the model's effect is too small (V3 time, -0.85% against -10%) or too large (V2
informedness in SDs; V6 likes, 0.61 against 0.83 to 0.94). History: lab #31 had 11 passes, 5 undecided and 5 fails, the
direction right for 11 of 17 and the magnitude for 8, and 5 of 14 genuine tests passing.

**Mechanism by assumption against genuine tests.** "Kind" in 4.1 sorts the checks by what a pass can show:

- *Calibration-implied* (3: the polarisation nulls of V1, V2 and V10). `polarisation_rate` was cut so that fact 3's
  polarisation barely moves in three months (model.md 13.1, 13.4). All 3 pass, as at M2.
- *Direction assumed* (4: V4, V6 likes, V8, V9). The direction is written into an equation (share friction scales
  impulsive shares, out-group likes are cut by `1 - outgroup_like_cut * outrage`, hostility raises shares, ads lower
  reflective utility). 2 pass, V6's likes fail and V9 is undecided, as at M2.
- *Genuine tests* (13, V7 no longer among them): the effect emerges from interactions no parameter was set for. 8 pass
  (V1 time and misinformation, V2 later use, V3 engagement, V5 half, V6 ranking, V10 misinformation and engagement), 1
  is undecided (V1 toxic) and 4 fail (V2 and V10 informedness, V3 time, V5 extremes). M2 on the same 13: 7, 2 and 4
  (with V7: 7 of 14).
- *Post hoc* (1: V7). It passes under the field's notes design, which lab #41 chose after M2 had measured what that
  design gives (4.3). It is reported and not counted.

5 of the 7 calibration-implied or assumed checks pass, and 8 of the 13 genuine tests (M2: 5 of 7 and 7 of 13). The
genuine tests that pass are still about time and engagement and the exposure shifts of feed experiments (V5's half, V6's
ranking ratio, V1's and V10's misinformation); none about what people write, how they like across groups or what they
know.

### 4.3 Findings

**Leaving the platform lowers informedness by too much in SDs (V2); removing reshares still raises it (V10).** The break
lowers users' informedness as in the field, raw -0.0127 (sd 0.0019 across seeds; M2 -0.0129), 2% of its level. The check
divides by the treated's spread at the start, 0.0240 (V10: 0.0231), so the change reads -0.531 SD against the field's
-0.19 and -0.098 SD. At lab #31's spread of about 0.08 the same raw change would read about -0.16 SD, inside the band:
V2 fails on a scale that depends on how alike people's informedness is, not on its sign. The field's SDs are of a
knowledge index across very different people; the model's informedness is a narrow latent quantity. Removing reshares
still raises informedness a little, raw +0.0016 (sd 0.0012; M2 +0.0014), +0.067 SD, while the treated's misinformation
share falls by 8.0% (a pass); reshares carry almost no informative gain in the model (lab #31's diagnosis), whereas in
the field they carried political news. Across worlds V2's sign is the field's in 100% of local and 92% of plausible
worlds, and V10's is the wrong one in 98% and 93% (5.1, 6.1). Under the engagement ranking V2's informedness passes
(-0.220 SD).

**Hiding flagged posts barely moves time, and the treated's own toxicity hardly responds (V3).** Time falls by 0.85%
against the field's 10% (fail), at a dose of 0.0689 of the treated's impressions hidden. Per unit of dose the response
is -0.12 under the integrity preset and -0.14 under the engagement ranking, whose dose is 2.9 times larger (0.197) and
whose time effect passes (-2.7%). On 8 seeds per world (lab #43) the failure is now clear: the size is too small in
every local and plausible world (0% hold), 89 of 100 local and 190 of 199 plausible failures lie beyond seed noise, and
the sign holds in 100% and 99%; the 5% to 95% range of local effects is -1.6% to -0.5%. The treated's own toxicity, the
known limit, falls by 0.20% (-0.69% to +0.25%) against the field's -30%, and by 2.4% under the engagement ranking at its
larger dose; it takes the field's direction in 90% of local and 95% of plausible worlds, more strongly where the
exposure contagion is stronger (PRCC -0.38 across the revised ranges). Hiding 7% of impressions changes the hostile
share of a feed by a few points at most, which at the contagion's in-sample strength moves the chance of a hostile post
by a few percent at most. A field effect of -30% implies channels the model lacks, such as replies drawn by what one
sees; no run here separates them.

**Fewer like-minded posts barely move the toxic share (V1).** -1.6% (-4.3% to +1.3%) against the field's -11%, undecided
as at M2; the engagement ranking gives -4.4%, also undecided. The sign is the default's in 77% of local and 63% of
plausible worlds, with a seed share of 0.64 locally: with toxic posts less than half as common, the relative change is
noisier than before (pooled seed SD 0.062 against 0.038). On the robustness runs' seeds 1 to 4 the default world reads
-7.5%, inside the band, which helps explain why V1 holds in 54% of local worlds while 16 seeds leave it undecided (5.3).

**The most partisan authors are still amplified more than the middle (V5).** On the holdback design the more partisan
half's amplification, ranked over chronological, is 1.342 (pass), and the most partisan tenth's over the middle fifth's
is 1.852 (1.646 to 2.222), against the field's no extra amplification. Without the impressions of posts removed or
demoted by the end of their life they are 1.364 and 1.927; the treated's impressions of all posts are 1.085 times as
many on the ranked feed. On 8 seeds per world the failure is robust and mostly clear: 0% of local worlds hold (83 of 100
failures clear, against 32 of 98 at M2 on 4 seeds), 5% of plausible ones, and the ratio lies above 1 in 98% of plausible
worlds. Lab #31 traced most of the extra amplification to trending recommendations, which the chronological arm lacks;
that diagnosis was not repeated. Across the revised ranges the partisan half's ratio still collapses with adaptive bots:
it lies below 0.5 in 38 of the 67 non-degenerate worlds with adaptive farms (median 0.11; M2 39 of 65), more so where
bots are many (Spearman with `bot_share` -0.55 among them), and never with the other strategies (medians 1.52 and 1.51;
`exports/analysis-r6/v5_bots.txt`). These runs do not explain it.

**People like out-group posts too rarely (V6); the ranking now trims out-group exposure as in the field.** The out-group
share of in-network likes is 0.605 of its share of impressions (M2 0.648), against 0.83 to 0.94 for clicks in Bakshy et
al. It fell with lab #40, which leaves more Outrage in feeds; the in-group appeal of hostility is the largest gap in
lab #36's decomposition of `P_like`, though no run here takes the change apart. The global runs agree: `outrage_appeal`
has PRCC -0.84 across the plausible and -0.86 across the revised ranges, `outgroup_like_cut` -0.57 across the revised
ranges and is not among the top three within its plausible range. The failure is robust: 98% of local worlds fail, 88 of
them clearly, with a seed share of 0.06. The ranking ratio, 0.943 (0.912 to 0.974), now passes; it holds in 79% of local
and 64% of plausible worlds.

**Under the field's notes design most shares come before the note (V7), but that design was chosen after the result.**
M2 traced V7's 1.000 to the integrity preset's `noted_penalty` of 1.0, which drops a noted post below everything a
viewer reads, and measured 0.842 without it (`exports/analysis-r5/v7-*`). Lab #41 then decided, from documentation of
the field platform rather than from those numbers, that V7 is judged under the field's notes design: the integrity
preset with community notes on and `noted_penalty` 0, and the engagement-ranking twin likewise. X does not demote noted
posts. The University of Washington's press release on Slaughter et al. (2025), of 2025-09-18, quotes Saveski: "From the
public release of the algorithm, we know that X does not explicitly deemphasize posts with notes attached, but that
could change in the future." Meta, by contrast, has demoted posts its fact-checkers rate false (read through search
summaries only, wording unverified), so the integrity preset's penalty stands for a fact-check demotion, a design choice
(model.md 14.1), not for V7's platform. V7 now pools over the summary window like V6 and V8, since it rests on some 34
noted posts per seed. Under that design it is 0.812 (0.792 to 0.833), inside the band of 0.70 to 0.95; the old mean of
daily shares gives 0.845 and the engagement ranking 0.894. Because the decision followed a measured 0.842, V7 is
reported and not counted among the genuine tests. It holds in 99% of local, 96% of plausible and 72% of non-degenerate
revised worlds, and more outrage appeal gives more shares after notes (PRCC -0.60 plausible, -0.50 revised).

**Sixteen seeds did not decide V9.** The ad-load elasticity is -0.009 (-0.022 to +0.004), undecided, with its mean now
just outside the band (M2: -0.018 on 8 seeds). Its raw change is 64 time units with an SD of 177 across seeds: the
model's response to ad load is small and noisy, and its mean lies near the band's edge. Locally it holds in 89% of
worlds, its failures mostly within noise (9 of 11), with the sign kept in 97%; across the plausible ranges in 67%,
all 66 failures within noise.

**What passes.** Fewer like-minded posts cut misinformation exposure (-13%) and time (-6.0%) within the field's bands;
after a break, use stays lower (-22% against -23% and -15%); removing reshares cuts misinformation exposure (-8.0%) and
engagement (-3.5%); ranking amplifies the more partisan half (1.34) and trims out-group exposure a little (0.94);
friction raises the truth of shares (+1.0%); Outrage is shared 2.7 times as often per impression as the non-hostile
styles. These are the model's time, engagement and exposure mechanics and its built-in directions.

### 4.4 The engagement ranking as the feed

Judged on the engagement ranking instead, the battery gives 15 passes, 2 undecided and 4 fails, against 14, 2 and 5
under the integrity preset (`exports/validation-r6/validation.md`; M2: 14, 2 and 5 against 12, 3 and 6). It does better
on V2's informedness (-0.220 SD, pass), V3's time (-2.7%, pass) and V9 (-0.033, pass), and worse on V1's time (-10.0%,
undecided), V6's ranking ratio (0.796, below the band), and V6's likes (0.44); V5's extremes fail under both (1.72). Two
of its gains are not like for like: V3's dose is 2.9 times larger (per unit of dose -0.14 against -0.12), and V2's
informedness divides by an SD that the output does not print for this column. The checks where the integrity preset fits
better (V1's time, V6) are the ones about how a ranked feed with integrity measures differs from one without.

## 5. Stability (local)

### 5.1 Hold shares

100 worlds, every assumption jittered about +-25%, no degenerate world (`exports/robustness/r6-local/summary.md`; M2
from `r5-local`). "Sign" is given for the check named.

| Item | Holds | M2 | Sign (check) | M2 sign |
| --- | --- | --- | --- | --- |
| Fact 1 amplification | 63% | 83% | 100% (both) | 100% |
| Fact 2 outrage learning | 97% | 88% | 100%, 100% (rise, learning) | 100%, 97% |
| Fact 3 chronological feed | 50% | 60% | 99% to 100% (all) | 100% |
| Fact 4 forwarding limits | 100% | 100% | 100% | 100% |
| Fact 5 harassment exit | 99% | 100% | 100% | 100% |
| Fact 6 deactivation | 44% | 45% | 70% | 71% |
| Fact 7 notifications off | 99% | 95% | 68% (wellbeing) | 60% |
| H1 calmer feed | 80% | 97% | 94% (engagement) | 76% |
| H2 survey misranks | 73% | 65% | 73% (survey) | 65% |
| H3 strict classifier | 100% | 100% | 100% | 100% |
| H4 trap solution | 100% | 100% | 99% (engagement) | 99% |
| H5 notifications | 97% | 96% | 81% (wellbeing) | 79% |
| H6 lead lasts | 93% | 76% | 93% | 76% |
| H7 report trust | 100% | 98% | 100% | 98% |
| H8 see-less backfire | 75% | 100% | 75% | 100% |
| V1 | 54% | 40% | 77% (toxic) | 67% |
| V2 | 16% | 15% | 100% (informedness, negative) | 100% |
| V3 | 0% | 10% | 100% (time) | 100% |
| V4 | 99% | 100% | 99% | 100% |
| V5 | 0% | 2% | 100% (extremes) | 100% |
| V6 | 1% | 10% | 90% (ranking), 100% (likes) | 86%, 100% |
| V7 | 99% | 0% | 100% (below 1) | 84% (at 1.000) |
| V8 | 92% | 92% | 100% | 100% |
| V9 | 89% | 76% | 97% | 95% |
| V10 | 2% | 8% | 98% (informedness, positive) | 92% |

Four in-sample items lost ground, each through a check whose default now sits near a bound. Fact 1's failures lie above
its upper bound of 0.6: the default world gives 0.540 on 8 seeds but 0.596 on the robustness runs' seeds 1 and 2, and 21
of its 37 failures are within noise. Fact 3 loses through its polarisation check (0.044 at the defaults
against 0.05; 0.0505 on seeds 1 and 2), which fails in 45% of local worlds with a seed share of 0.04, so the spread is
the worlds'. H8 fails in 25% of worlds (20 of them within noise), its effect at the defaults down from +0.197 to +0.075
and read at +0.033 on seeds 1 and 2. H1 loses through its engagement cost, which at -24% lies nearer its bound of -33%
(13 of 20 failures clear). H6 and H2 gained. Signs below 75%: fact 7's wellbeing effect and fact 6 sit near zero or on a
knife edge (5.4), H2's survey follows the survey's weights, and H8 now sits near zero.

### 5.2 A world's failure against seed noise

The failure classes of 2.6, from the summary, at item level, with the robust seed SD (M2's classes used the pooled
SD; 5.3 compares the two). "Seed share" is for the check named: robust, with the pooled equivalent after it.

| Item | Failing | Noise-compatible | Clear | Seed share (check) | M2: failing, compatible, clear |
| --- | --- | --- | --- | --- | --- |
| Fact 1 amplification | 37 | 21 | 16 | 0.10; 0.10 (outrage) | 17, 13, 4 |
| Fact 2 outrage learning | 3 | 3 | 0 | 0.27; 0.24 (rise) | 12, 12, 0 |
| Fact 3 chronological feed | 50 | 16 | 34 | 0.04; 0.03 (polarisation) | 40, 16, 24 |
| Fact 6 deactivation | 56 | 39 | 17 | 0.13; 0.08 | 55, 27, 28 |
| H1 calmer feed | 20 | 7 | 13 | 0.02; 0.03 (engagement) | 3, 3, 0 |
| H2 survey misranks | 27 | 6 | 21 | 0.03; 0.04 (survey) | 35, 17, 18 |
| H6 lead lasts | 7 | 4 | 3 | 0.02; 0.02 | 24, 12, 12 |
| H8 see-less backfire | 25 | 20 | 5 | 0.23; 0.20 | 0 |
| V1 | 46 | 43 | 3 | 0.64; 0.58 (toxic) | 60, 41, 19 |
| V2 | 84 | 24 | 60 | 0.17; 0.15 (informedness) | 85, 21, 64 |
| V3 | 100 | 11 | 89 | 0.65; 0.73 (time) | 90, 53, 37 |
| V5 | 100 | 17 | 83 | 0.64; 0.81 (extremes) | 98, 66, 32 |
| V6 | 99 | 11 | 88 | 0.06; 0.05 (likes) | 90, 5, 85 |
| V7 | 1 | 1 | 0 | 0.62; 0.45 | 100, 0, 100 |
| V8 | 8 | 4 | 4 | 0.08; 0.10 | 8, 4, 4 |
| V9 | 11 | 9 | 2 | 0.40; 0.50 | 24, 24, 0 |
| V10 | 98 | 28 | 70 | 0.53; 0.55 (informedness) | 92, 36, 56 |

Facts 4, 5 and 7, H3, H4, H5, H7 and V4 fail in 0 to 3 worlds each, all within noise. Eight seeds per world (lab #43)
made V3's and V5's failures clear, but not their local spread readable: like for like, with the robust SE in both
rounds, the local seed shares went from 0.71 to 0.65 (V3's time), 0.62 to 0.64 (V5's extremes) and 0.46 to 0.40 (V9)
(`exports/analysis-r5/pooled_sd.txt`, `exports/analysis-r6/classes.txt`). The +-25% worlds differ in these checks by
little more than seed noise even at 8 seeds, so their local drivers cannot be read; across the plausible ranges V5's and
V9's can, V3's only partly (6.4).

### 5.3 The robust seed SD

Lab #42 replaced the pooled seed SD by the robust one of 2.6, as M2's analysis recommended: a check whose ratio has a
denominator near 0 in a few worlds carried an inflated pooled SD into every world. The pooled SD stays in
`summary.json`. Recounted from `checks.csv` (`exports/analysis-r6/classes.py`), the robust SD reproduces the summary's
classes, and the pooled SD gives:

- **Local:** the pooled SD lies within 0.77 to 1.31 times the robust SD for every check but V6's ranking ratio (1.62).
  The classes barely move: V5 17 against 25 noise-compatible of 100, fact 6 39 against 33, H1 7 against 9, H2 6
  against 9.
- **Plausible:** four checks lie above 1.5: H3's toxic ratio (1.93, no failures), V5's extremes (1.67), V6's ranking
  ratio (1.99) and V8 (1.85). V5's failures are 83 noise-compatible of 191 with the robust SD against 131 with the
  pooled; fact 6's 50 against 39 (its pooled SD is the smaller), H8's 38 against 44.
- **Revised ranges:** most checks lie above 1.3, H6 at 13.7, V2's affective polarisation 21, V2's later use 11, H3's
  toxic ratio 11, V5's half 10 and V8 5.6. At item level:

| Item (revised ranges) | Failing | Compatible, robust SD | Compatible, pooled SD | M2: failing, robust recount |
| --- | --- | --- | --- | --- |
| H6 lead lasts | 68 | 15 | 68 | 80, 11 |
| V3 | 170 | 17 | 141 | 154, 31 |
| V1 | 172 | 60 | 108 | 167, 27 |
| V2 | 183 | 16 | 54 | 171, 13 |
| V5 | 137 | 33 | 104 | 128, 46 |
| V8 | 47 | 10 | 24 | 41, 3 |
| Fact 2 outrage learning | 161 | 11 | 29 | 160, 10 |
| H2 survey misranks | 80 | 16 | 23 | 84, 18 |
| H7 report trust | 74 | 65 | 71 | 84, 70 |
| H8 see-less backfire | 105 | 56 | 69 | 93, 48 |
| V10 | 187 | 20 | 35 | 176, 21 |

So across the revised ranges the failures of H6, V1, V2, V3, V5 and V8 are mostly clear, as M2's recount predicted (its
expectation for H6, 11 of 80, came out at 15 of 68); H7's and H8's stay mostly within noise with either SD. The guard
never fired: no check had a median within-world SD of 0. Worlds whose seeds agree exactly occur only across the revised
ranges, in H3's toxic ratio (6 worlds, no toxic impression at the strict threshold on either seed) and in V7 (6 worlds).
Unclassed failures come only from worlds without a value: V5's extremes in 22 revised worlds, V7 in 10 and V8 in 2.
Because one SE serves every world, a world whose own seeds are far noisier than the typical world's can be called clear
when its own noise would cover the gap; with a median as the basis, that is the price of not letting such worlds inflate
everyone's SE.

The robustness runs use the same seeds in every world, which shows in the default world (world 0) itself. On the seeds
of the tiers it reads fact 1 at 0.596 (0.540 on 8 seeds), H8 at +0.033 (+0.075), V1's toxic share at -7.5% (-1.6% on 16
seeds), V7 at 0.757 (0.812), V9 at -0.015 (-0.009) and V10's informedness at +0.096 (+0.067)
(`exports/analysis-r6/world0.txt`; V7's value reproduced by the added run `exports/analysis-r6/validate-v7-seeds2/`).
Most such shifts lie within 1.5 standard errors of the tier's mean, V1's toxic share (2.2) and V7 (2.0) beyond. The
local medians sit near world 0's values (fact 1 0.579, V7 0.760, H8 +0.039), so a seed set's luck probably moves many
worlds alike, and hold shares near a band's edge carry it: fact 1's 63% and H8's 75% read low, V1's 54% and V9's 89%
read high, against what the defaults on many seeds give (recommendation 1 of 8.2).

### 5.4 What is robust and what hinges on the defaults

- **Robust to +-25%:** facts 4, 5 and 7, H3, H4, H5 and H7 hold in 97% to 100% of worlds, their few failures within
  noise; H6 now in 93%. Fact 1 keeps its direction everywhere and its size in 63%, failing above its upper bound, which
  the default world nearly touches on the robustness seeds. H1's harm cut holds in every world.
- **Fact 2 now holds in 97%:** its learning check passes at the defaults and every failure is within noise.
- **Fact 3's polarisation null hinges on the defaults:** it fails in 45% of worlds with little seed noise (16 of 50
  failures within noise), while the time and exposure checks hold in 93% to 100%.
- **Fact 6 hinges on the defaults:** 44% hold, the 5% to 95% range of world effects -0.16 to 0.35 against a band of 0.02
  to 0.25. The `time_cost` calibration pins one combination of several assumptions on a knife edge.
- **H2 hinges on a measurement assumption:** the survey ranks the engagement ranking above Q&B in 73% of worlds; its
  drivers are the survey's weights (6.4). The wellbeing half holds in every world.
- **H1's engagement cost and H8 moved toward their bounds:** H1 holds in 80% (13 of 20 failures clear), H8 in 75% (20
  of 25 failures within noise).
- **Out of sample:** V4 (99%), V7 under its new design (99%), V8 (92%) and V9 (89%) are stable; V1 holds in 54%, mostly
  within noise. The failures of V2, V3, V5, V6 and V10 are robust (60 to 89 clear failures each).

## 6. Sensitivity (plausible and global)

### 6.1 Hold shares across the plausible ranges

200 worlds, none degenerate (M2: 1) (`exports/robustness/r6-plausible/summary.md`). The plausible ranges stand for
beliefs about the assumptions (2.6), with the bot farms adaptive throughout. Beside each plausible share stand M2's and
the share over the 188 non-degenerate worlds of the revised ranges (6.2).

| Item | Plausible | M2 | Revised, non-degenerate | Sign (check), plausible |
| --- | --- | --- | --- | --- |
| Fact 1 | 63% | 71% | 43% | 100%, 99% |
| Fact 2 | 78% | 48% | 14% | 98%, 96% (learning) |
| Fact 3 | 22% | 29% | 2% | 100% (time) |
| Fact 4 | 100% | 100% | 93% | 100% |
| Fact 5 | 98% | 98% | 43% | 100% |
| Fact 6 | 30% | 28% | 10% | 67% |
| Fact 7 | 97% | 99% | 92% | 100% (time), 60% (wellbeing) |
| H1 | 68% | 78% | 32% | 100% (harm), 91% (engagement) |
| H2 | 66% | 63% | 57% | 66% (survey) |
| H3 | 100% | 100% | 97% | 100% |
| H4 | 100% | 100% | 91% | 100% (harm), 91% (engagement) |
| H5 | 95% | 93% | 87% | 100% (engagement), 74% (wellbeing) |
| H6 | 90% | 80% | 64% | 90% |
| H7 | 100% | 99% | 61% | 100% |
| H8 | 60% | 96% | 44% | 60% |
| V1 | 48% | 35% | 9% | 63% (toxic) |
| V2 | 21% | 18% | 3% | 92% (informedness, negative) |
| V3 | 0% | 8% | 10% | 99% (time) |
| V4 | 94% | 94% | 75% | 94% |
| V5 | 4% | 12% | 27% | 98% (extremes) |
| V6 | 7% | 12% | 3% | 86% (ranking), 100% (likes) |
| V7 | 96% | 0% | 72% | 100% (below 1) |
| V8 | 94% | 95% | 75% | 100% |
| V9 | 67% | 75% | 59% | 90% |
| V10 | 7% | 11% | 1% | 93% (informedness, positive) |

What holds across plausible worlds:

- **Holds almost everywhere (94% to 100%):** forwarding limits (fact 4, 199 of 200; revised 93%), harassment exits
  (fact 5; revised 43%), notifications off (fact 7; 92%), the strict classifier's trade-off (H3; 97%), the trap solution
  (H4; 91%), notifications raising engagement (H5; 87%), report trust (H7; 61%); out of sample V4 (75%), V7 under its
  new design (72%) and V8 (75%). Q&B cuts harm by more than half in every plausible world (H1's harm check; 57% of the
  non-degenerate revised worlds, 93% in direction).
- **Holds in most worlds:** H6 (90%; revised 64%), fact 2 (78%; 14%), fact 1 (63%, direction 100%, failing
  above 0.6; 43%), H1 (68%: its engagement cost leaves the band where habit forms fast; 32%), V9 (67%; 59%).
- **Hinges on the assumptions:** H2's survey (66%; 57%), H8 (60%, down from 96%; 44%), fact 3 (22%; 2%), through its
  polarisation check (38%; the exposure checks hold in 76% and 100%), and fact 6 (30%; 10%).
- **Fails robustly:** V2, V3, V5, V6 and V10 hold in 0% to 21% of worlds (0% to 27% revised); V1 in 48% (9%), through
  its toxic share.

### 6.2 Hold shares across the revised ranges

240 worlds, 52 of them degenerate (M2: 62, on the same worlds), so 188 non-degenerate
(`exports/robustness/r6-global/summary.md`). Hold shares over all worlds and over the non-degenerate ones; "sign" for
the main check over all worlds (a world without a value counts as not sharing the sign).

| Item | All | Non-degen. | M2 | Sign | M2 sign |
| --- | --- | --- | --- | --- | --- |
| Fact 1 | 42% | 43% | 44%, 44% | 100%, 75% | 100%, 73% |
| Fact 2 | 13% | 14% | 8%, 10% | 50%, 30% | 45%, 24% |
| Fact 3 | 2% | 2% | 3%, 3% | 98% (time) | 92% |
| Fact 4 | 90% | 93% | 92%, 96% | 100% | 100% |
| Fact 5 | 34% | 43% | 36%, 48% | 80% | 81% |
| Fact 6 | 8% | 10% | 8%, 10% | 89% | 88% |
| Fact 7 | 88% | 92% | 85%, 94% | 99% (time) | 94% |
| H1 | 25% | 32% | 27%, 36% | 89% (harm) | 92% |
| H2 | 54% | 57% | 47%, 53% | 60% (survey) | 53% |
| H3 | 95% | 97% | 98%, 99% | 100% | 100% |
| H4 | 82% | 91% | 78%, 86% | 84% (harm) | 81% |
| H5 | 88% | 87% | 87%, 84% | 100% (engagement) | 100% |
| H6 | 63% | 64% | 53%, 55% | 63% | 53% |
| H7 | 57% | 61% | 53%, 53% | 57% | 53% |
| H8 | 43% | 44% | 42%, 48% | 43% | 42% |
| V4 | 66% | 75% | 66%, 74% | 66% | 66% |
| V8 | 62% | 75% | 63%, 77% | 87% | 82% |
| V9 | 57% | 59% | 52%, 54% | 82% | 75% |
| V1 | 10% | 9% | 6%, 6% | 35% (toxic) | 30% |
| V2 | 2% | 3% | 3%, 4% | 42% (informedness, negative) | 42% |
| V3 | 11% | 10% | 14%, 13% | 72% (time) | 70% |
| V5 | 22% | 27% | 22%, 28% | 49% (extremes) | 46% |
| V6 | 3% | 3% | 4%, 4% | 70% (ranking), 98% (likes) | 65%, 98% |
| V7 | 57% | 72% | 0%, 0% | 72% (below 1) | 41% (at 1.000) |
| V10 | 1% | 1% | 2%, 1% | 74% (informedness, positive) | 67% |

The revised ranges are ranges to explore, not beliefs: they include worlds where a mechanism is off or doubled, and
model.md section 15 names failing worlds inside them. Low hold shares there map where a mechanism operates; they do not
by themselves refute it, and a fifth of the worlds are emptied platforms (6.3). The bot strategy still splits three
claims (`exports/analysis-r6/degenerate.txt`, `v5_bots.txt`, non-degenerate worlds), and no longer H8:

| Claim | Holds, adaptive bots | Own-side | Shared pool | Median effect, adaptive against the others | M2, adaptive |
| --- | --- | --- | --- | --- | --- |
| H7 report trust | 87% (58 of 67) | 45% | 47% | -1.18 against +0.02 and +0.01 | 80% |
| H6 lead lasts | 70% | 57% | 63% | 1.52 against 1.07 and 1.14 | 68% |
| H1 calmer feed | 24% | 39% | 36% | engagement -0.35 against -0.07 and -0.13 | 25% |
| H8 see-less backfire | 49% | 41% | 41% | 0.000 against -0.004 and -0.003 | 91% |

Only adaptive farms brigade (model.md 3.5), so H7, which is about the reports they file, cannot hold in two thirds of
these worlds by construction. H8 lost its dependence on brigading: at M2 it held in 91% of the worlds with adaptive
farms against about a quarter with the others; now its median effect is about 0 under every strategy, and its rank
regression explains little (adjusted R2 0.12).

- **Robust across the ranges:** forwarding limits (93% non-degenerate), notifications off (92%), the strict classifier's
  trade-off (H3, 97%), the trap solution (H4, 91%), notifications raising engagement (H5's engagement check, 100%). Q&B
  cuts harm in 89% of worlds (93% of the non-degenerate), by "far more" (at least half) in 57% of the non-degenerate
  ones; the engagement ranking shows more outrage than the chronological feed in every world.
- **Hinges on the ranges:** fact 2's rise has the default's sign in 50% of worlds and its learning check in 30%;
  fact 3's polarisation check holds in 23% of non-degenerate worlds; fact 6 lies above its band in most (median 0.74 SD
  against a band of 0.02 to 0.25); H2, H6, H7 and H8 split about evenly, H7 by the bot strategy.
- **Out of sample:** no V finding that fails at the defaults passes in more than 27% of non-degenerate worlds. V2's
  informedness is negative in 42% of all worlds, and with adaptive bots its median is positive (+0.50 SD against -0.19
  and -0.15). V7 under its new design holds in 72% of the non-degenerate worlds.

### 6.3 Degenerate worlds

52 of the 240 revised worlds (22%; M2 62, 26%) are degenerate, none of the plausible (M2: 1) and none of the local ones.
The worlds are M2's, and all 52 were degenerate at M2 too; ten recovered, since lab #40's fixed point keeps more users.
From `worlds.csv` (`exports/analysis-r6/degenerate.py`), the degenerate share by fifth of each assumption's revised
range and its Spearman correlation with the degenerate flag (beyond 2 SE at 0.13 for 240 worlds), for the assumptions
beyond it:

| Assumption | Revised range | Spearman | Degenerate share by fifth, lowest to highest |
| --- | --- | --- | --- |
| `habit_gain` | 0 to 0.1125 | -0.31 | 50%, 23%, 15%, 8%, 12% |
| `churn_base` | 0 to 0.1 | +0.30 | 2%, 15%, 27%, 21%, 44% |
| `outrage_appeal` | 0 to 3 | -0.29 | 44%, 23%, 19%, 12%, 10% |
| `leave_threshold` | -0.1 to 0.1 | +0.25 | 6%, 10%, 27%, 35%, 29% |
| `arousal_appeal` | 0 to 2 | -0.20 | 38%, 25%, 17%, 17%, 12% |
| `time_cost` | 0 to 0.4 | +0.16 | 10%, 19%, 21%, 29%, 29% |
| `reflective_quality_weight` | 0 to 2 | -0.15 | 31%, 23%, 27%, 12%, 15% |
| `classifier_retraining` | 0 to 0.5 | +0.15 | 17%, 12%, 19%, 27%, 33% |
| `wrongful_sanction_cost` | 0 to 2 | +0.14 | 17%, 17%, 23%, 19%, 33% |

The ranges that mostly explore emptied platforms are the lowest fifth of `habit_gain` (0 to 0.0225, half of its 48
worlds degenerate) and, jointly, slow habit with high churn: of the 56 worlds with `habit_gain` in its lowest two fifths
and `churn_base` above 0.04, 30 are degenerate (54%), against 3 of the 56 with the opposite. The top fifth of
`churn_base` (0.08 to 0.1, 44%) and the lowest fifths of `outrage_appeal` (0 to 0.6, 44%) and `arousal_appeal` (0
to 0.4, 38%) come close: a platform that pulls little and holds little empties. No world with `habit_gain` and
`outrage_appeal` both in their upper three fifths is degenerate (0 of 83). `leave_threshold` above about -0.02 adds to
it. M2's suspicion (`habit_gain` from 0, `churn_base` up to 0.1) is confirmed, with the two appeals beside them. The bot
strategy does not matter (17 of 84 adaptive, 18 of 69 own-side, 17 of 87 shared-pool).

### 6.4 Drivers of the key results

PRCC over the non-degenerate worlds, the strongest three by absolute value, all beyond 2 SE (plausible: 200 worlds,
SE 0.083; revised: 188 worlds, SE 0.087); R2 is the adjusted R2 of the rank regression (2.6). M2's drivers are in its
version of this section; where they changed, the text below says so.

Across the plausible ranges:

| Result | Top drivers (PRCC) | R2 |
| --- | --- | --- |
| Fact 1, outrage | `outrage_appeal` +0.86, `choice_temperature` -0.55, `style_quality_gap` -0.50 | 0.84 |
| Fact 2, rise | `exposure_contagion` +0.59, `choice_temperature` -0.57, `reflective_quality_weight` -0.52 | 0.73 |
| Fact 2, learning | `learning_rate` -0.60, `choice_temperature` -0.43, `evasion_cost` -0.40 | 0.64 |
| Fact 3, polarisation | `outrage_appeal` +0.96, `polarisation_rate` +0.92, `habit_gain` +0.87 | 0.96 |
| Fact 6 | `reflective_quality_weight` -0.86, `bot_share` +0.81, `style_quality_gap` -0.80 | 0.91 |
| H1, engagement | `habit_gain` -0.89, `outrage_appeal` -0.77, `stop_bias` -0.69 | 0.89 |
| H2, survey | `survey_impulsive_weight` +0.90, `survey_habit_weight` +0.82, `social_proof` -0.43 | 0.87 |
| H6 | `habit_gain` +0.89, `outrage_appeal` +0.77, `stop_bias` +0.68 | 0.89 |
| H7 | `archetype_shares.troll` -0.63, `choice_temperature` -0.59, `evasion_decay` +0.51 | 0.73 |
| H8 | `outrage_appeal` -0.73, `bot_share` +0.63, `habit_gain` -0.63 | 0.82 |
| V2, informedness | `style_quality_gap` -0.80, `bot_share` +0.75, `outrage_appeal` +0.62 | 0.85 |
| V3, time | `stop_bias` -0.53, `habit_gain` -0.33, `harassment_exit` +0.32 | 0.41 |
| V5, extremes | `habit_gain` -0.44, `bot_share` +0.43, `stop_bias` +0.38 | 0.46 |
| V6, likes | `outrage_appeal` -0.84, `style_quality_gap` +0.73, `bot_share` -0.69 | 0.88 |
| V7 | `outrage_appeal` -0.60, `bot_share` +0.39, `choice_temperature` -0.26 | 0.50 |
| V9 | `reflective_quality_weight` +0.61, `habit_gain` +0.50, `style_quality_gap` +0.50 | 0.68 |

Across the revised ranges:

| Result | Top drivers (PRCC) | R2 |
| --- | --- | --- |
| Fact 1, outrage | `outrage_appeal` +0.84, `choice_temperature` -0.60, `arousal_appeal` +0.35 | 0.75 |
| Fact 2, rise | `learning_rate` -0.39, `arousal_appeal` -0.34, `exposure_contagion` +0.30 | 0.42 |
| Fact 2, learning | `learning_rate` -0.50, `outrage_appeal` -0.47, `bot_share` -0.36 | 0.59 |
| Fact 3, polarisation | `outrage_appeal` +0.74, `polarisation_rate` +0.72, `homophily` +0.32 | 0.71 |
| Fact 6 | `time_cost` +0.60, `reflective_quality_weight` -0.57, `outrage_appeal` +0.54 | 0.71 |
| H1, engagement | `social_proof` +0.49, `stop_bias` -0.42, `arousal_appeal` +0.37 | 0.44 |
| H2, survey | `survey_impulsive_weight` +0.49, `survey_habit_weight` +0.43, `social_proof` -0.34 | 0.46 |
| H6 | `social_proof` -0.49, `stop_bias` +0.42, `arousal_appeal` -0.37 | 0.43 |
| H7 | `intensity_rate` +0.29, `homophily` +0.22, `bot_brigading` -0.19 | 0.38 |
| H8 | `choice_temperature` +0.32, `subscription_uptake` +0.27, `reflective_quality_weight` +0.20 | 0.12 |
| V2, informedness | `style_quality_gap` -0.63, `habit_gain` -0.40, `credulity_scale` +0.40 | 0.72 |
| V3, time | `stop_bias` -0.43, `arousal_appeal` +0.30, `reputation_sensitivity` -0.18 | 0.24 |
| V5, extremes | `stop_bias` +0.26, `outrage_appeal` +0.26, `habit_gain` -0.25 | 0.21 |
| V6, likes | `outrage_appeal` -0.86, `outgroup_like_cut` -0.57, `style_quality_gap` +0.55 | 0.84 |
| V7 | `outrage_appeal` -0.50, `habit_gain` -0.32, `choice_temperature` -0.32 | 0.47 |
| V9 | `ad_annoyance` -0.48, `churn_base` -0.45, `habit_gain` +0.35 | 0.39 |

H8's plausible drivers are M2's (`bot_share` +0.64, `outrage_appeal` -0.63): what changed is its size at the defaults,
which puts many plausible worlds below 0. Across the revised ranges nothing in the table explains it. On 8 seeds the
rank regression explains more of V3's time and V9 across the plausible ranges (R2 0.41 and 0.68, against 0.25 and 0.51
at M2), though V3's seed share there is still 0.61; V5's extremes did not gain (0.46, M2 0.57). H2, H6 and fact 6 keep
their drivers. Fact 2's learning check keeps its negative PRCC with `learning_rate` in both designs (-0.60 and -0.50):
faster learning gives a smaller rise over the arm without learning, which these runs do not explain.

Over the 48 required checks with a PRCC, six assumptions take 86 of the 144 top-three places across the revised ranges:
`outrage_appeal` (19), `habit_gain` (18), `arousal_appeal` (17), `stop_bias` (13), `reflective_quality_weight` (10) and
`choice_temperature` (9); across the plausible ranges six take 100: `outrage_appeal` (23), `habit_gain` (21),
`choice_temperature`, `style_quality_gap` and `bot_share` (15 each) and `reflective_quality_weight` or `stop_bias` (11)
(`exports/analysis-r6/top3.txt`; M2: 82 and 98). The model's results rest mostly on how strongly hostility and arousal
pull, how habit forms and how sharply people choose.

A typical check has 13 of the 53 assumptions beyond 2 SE across the plausible ranges (4 to 19) and 7 across the revised
ones (3 to 17), against 2.4 expected by chance alone. Most checks respond to several assumptions at once, and a few
beyond 2 SE in any one check could be chance.

### 6.5 Weak correlations

Required checks whose rank regression explains little across the revised ranges (adjusted R2 at or below 0.3): H8
(0.12), V2's later use (0.13), V1's toxic share (0.14), forwarding limits (0.19), V5's extremes (0.21), V10's engagement
(0.21) and informedness (0.24), V3's time (0.24) and engagement (0.30), notifications off's sessions (0.30) and V10's
polarisation (0.30), with the known limit, V3's own toxicity (0.23); across the plausible ranges H3's toxic ratio (0.15)
and V4 (0.27), with V3's own toxicity (0.08). Seed noise is a large part of their spread where their seed shares are
high (V4 0.50 and V3's own toxicity 0.83 across the plausible ranges; V1's toxic share 0.22 across the revised ones);
elsewhere the effects respond to the bot strategy or to interactions an additive rank model misses. Where PRCC and
Spearman disagree, the assumption matters only once the others are held fixed (for example `stop_bias` for H2's
wellbeing check across the plausible ranges, PRCC +0.62 against Spearman +0.06).

### 6.6 The documented ranges (lab #31, not rerun)

Lab #31 ran 64 worlds on section 11's ranges as written before the revision (`exports/robustness/global-2-documented/`).
Its degenerate share was the revised run's of that time (20%), caused mostly by `leave_threshold` (-0.5 to 0.5), and the
polarisation checks failed more often, because those ranges reach 13 times the calibrated `polarisation_rate`.
Milestone 2 replaced that contrast by the plausible ranges and gave `leave_threshold` a judgement range of -0.1 to 0.1.

## 7. Overall assessment

The lab fits what it was tuned on: every required fact holds on the seed means and every headline claim at the defaults,
the default design's toxic share is back inside its band, and the new prevalence target under the integrity preset is
met to within 0.0013. Out of sample it gets 14 of 21 checks and the direction of 14 of 17 directional findings (M2: 12,
and 12 of 17), and 8 of its 13 genuine tests pass (M2: 7 of 13). Milestone 3 achieved what it set out to do where the
aim was a prevalence, a measurement or a clearer verdict (toxicity prevalence, the robust classes, the seed tiers), and
left V7 passing only under a design chosen after its result. Its price was in sample: lab #40 and its refits left fact 1
near its upper bound, fact 3's polarisation check at its bound, and H8's backfire at a third of its size.

**It can be trusted for** the direction of the first-order trade-offs it was built around, which survive small noise and
nearly all plausible worlds tried: an engagement ranking amplifies outrage and costs reflective value (fact 1: more
outrage in every world of all three designs, lower reflective value in 77% to 100%), a calmer feed cuts harm by more
than half (H1's harm check, 100% local and plausible, 93% of non-degenerate revised worlds in direction), a strict
classifier trades toxicity for wrongful sanctions (H3; 100% plausible, 97% revised), the trap solution cuts harm and
keeps engagement (H4; 100%, 91%), forwarding limits shrink cascades (fact 4; 100%, 93%), notifications raise engagement
without raising wellbeing (H5; 95%, 87%), and switching people from a ranked to a chronological feed cuts their time and
shifts their exposure from incivility toward misinformation, in the field's sizes (fact 3's time and exposure
checks, 76% to 100% local and plausible; V1's and V10's misinformation checks). Report trust (H7; 100%, 61%) is robust
wherever bot farms file reports, and the engagement ranking's lead lasts in most plausible worlds (H6; 90%, 64%).

**It is uncertain about:** the engagement cost of a calmer feed, which turns on habit formation (H1's engagement
check; 68% plausible, 32% revised); the see-less backfire, which now holds in 60% of plausible and 44% of revised worlds
(H8); the survey's misranking, which turns on the survey's assumed weights (H2; 66%, 57%); the share of outrage's rise
that learning from likes carries, now that the contagion carries most of it (fact 2; 78%, 14%); whether three months of
a different feed leave polarisation unchanged, which sits at its bound (fact 3; 22%, 2%); and the size of the break's
wellbeing gain, a calibration identity on a knife edge (fact 6; 30%, 10%).

**It cannot be trusted for:**

- informedness: a break lowers it, as in the field, but by an amount whose size in SDs depends on how alike people's
  informedness is (V2), and removing reshares raises it, against the field, in 93% to 98% of local and plausible worlds
  (V10; it holds in 7% of plausible and 1% of revised worlds);
- how reach is distributed across partisanship: the most partisan authors are amplified 1.85 times as much as the middle
  on the field's own design, in every local world (V5; 4% plausible, 27% revised), and across the revised ranges the
  partisan half's amplification collapses with many adaptive bots, unexplained;
- cross-group liking (V6; 7%, 3%) and how hiding toxic posts changes what people write or how long they stay
  (V3; 0%, 10%; V1's toxic share): the directions show, the sizes are a tenth to a hundredth of the field's;
- community notes, as a test: V7 passes under the field's notes design (96%, 72%), but that design was chosen after its
  result had been seen;
- the regretted share, which lies a factor of about 26 below its held-out band.

## 8. Recommendations

### 8.1 Milestone 3's six items and what they achieved

1. **Toxicity prevalence (lab #40).** Done: an in-sample target fixed before tuning (3.2%, band 2.2% to 4.2%, under the
   integrity preset), with its mapping; Outrage's base toxicity 0.35 to 0.30, then the refits in the fixed order (the
   contagion stays 0.28, V1's dose 0.26, V3's 0.56, `time_cost` 0.035, price 1.99, uptake scale 0.097,
   `living_income` 0.025). Achieved: 0.0418 when chosen, 0.0433 after the refits, 0.0013 above the band, recorded and
   not retuned; the default design's toxic share back inside its band (0.131). Side effects: fact 1 near its upper bound
   (63% local), fact 3's polarisation undecided, H8's effect a third of its size (60% plausible), H1's engagement
   cost 24%; V1's toxic share and V3's own toxicity did not move.
2. **What V7 tests (lab #41).** Done: decided from X's documentation that V7 runs under the field's notes design, and
   pooled over the window. Achieved: 0.812, a pass (96% plausible), declared post hoc and no longer counted.
3. **Robust seed SD (lab #42).** Done, with the guard and the pooled SD and zero-spread counts kept. Achieved: local
   classes as before; across the revised ranges H6's failures are 15 of 68 within noise instead of 68, V3's 17 of 170
   instead of 141 (5.3). The guard never fired.
4. **More seeds where noise decides (lab #43).** Done: V3, V5 and V9 on 8 seeds per world, `validate` on 16. Achieved:
   V3's and V5's failures are clear (89 and 83 of 100 local); their local seed shares barely fell (5.2), so their local
   drivers stay unreadable; V9 stays undecided at 16 seeds (-0.009); V6's ranking ratio became a pass.
5. **Fact 3's polarisation margin (lab #44).** Done: 0.044 against 0.05 at the new fixed point, undecided; the rule of
   `polarisation_rate` holds, no refit. Across worlds the check fails in 45% of local and 62% of plausible worlds.
6. **Revised-range reporting (lab #45).** Done in this update: the revised non-degenerate share beside every plausible
   one, and the ranges behind the degenerate worlds measured (6.3): the lowest fifth of `habit_gain`, slow habit with
   high churn, and the lowest fifths of the two appeals.

### 8.2 New recommendations

In order of priority. "Model" changes the dynamics, "tooling" the CLI or measurement, "documentation" model.md or the
exhibits. A change tuned on a V check makes that check in-sample; each item says how to avoid that.

1. **Seeds that do not shift every world alike (tooling).** What: let the robustness runs draw each world's seeds from a
   block of its own (for example seeds offset by the world's index), or report world 0's effect on the tier's seeds
   beside the defaults' many-seed effect in `summary.md` and flag checks where they differ by more than 2 SE. Why: the
   default world on seeds 1 and 2 reads fact 1 at 0.596 against 0.540, V7 at 0.757 against 0.812, V1's toxic share at
   -7.5% against -1.6% (5.3), and hold shares near a band's edge carry that luck. Expected effect: hold shares of
   fact 1, H8, V1 and V9 that no longer depend on one seed set; the median world's effect near the defaults' many-seed
   effect. Measurement only; no band or constant moves.
2. **Why the see-less backfire weakened (tooling and documentation).** What: decompose H8's harm difference into the
   harm index's components (toxic share, harassment, wrongful sanctions, polarisation; model.md 9.3) at the defaults and
   across the plausible worlds where it fails, and restate H8 in model.md 13.3 and the app from the result. Why: H8 fell
   from +0.197 to +0.075 at the defaults and from 96% to 60% of plausible worlds, and across the revised ranges it no
   longer depends on brigading (6.2); the harm index's cap on the toxic share changed another comparison this round
   (model.md 13.7). Expected effect: a claim whose mechanism is stated. In sample only; no V check is involved.
3. **Informedness on a field scale (tooling and documentation; model for V10).** What: fix before any rerun how V2 and
   V10 compare informedness with a knowledge index measured in SDs of a heterogeneous population (judge the raw change
   against a field raw change where one exists, or record the SD check as a loose mapping judged in direction only);
   then examine why reshares carry no informative gain (V10), as a model question with any new strength set on in-sample
   sources. Why: V2 fails only because informedness's spread is 0.024 (4.3), and V10 keeps the wrong sign in 93% to 98%
   of local and plausible worlds. Expected effect: V2 judged on its sign and a defensible size; V10's sign explained.
   Fixing the scale before seeing its verdict keeps V2 out of sample.
4. **Diagnose V5 (tooling).** What: the in-network against recommended split of the treated's impressions that lab #31
   recommended, and a diagnostic of the collapse of the partisan half's amplification with many adaptive bots across the
   revised ranges (4.3). Why: V5's extremes fail in every local world (1.85) and the collapse (38 of 67 worlds) is
   unexplained. Expected effect: a cause for both. Measurement only.
5. **The size of exposure effects on writing (model; out-of-sample kept).** What: examine which channels between what
   people see and what they write the model lacks (replies drawn by hostile items, a dose that builds over days), with
   any new strength set on in-sample sources, not on V3 or V1. Why: the contagion at Cheng et al.'s strength gives V3's
   own toxicity the field's direction at a hundredth of its size, and V1's toxic share stays undecided (4.3). Expected
   effect: a channel whose strength is fixed elsewhere and then tested by V3; V3 stays out of sample if its numbers are
   not consulted.
6. **Settle the toxic-share target (documentation; recalibration check).** What: measure the integrity preset's toxic
   share at the final constants on 16 seeds, under a rule fixed before the run: if the 16-seed mean stays above 4.2%,
   repeat lab #40's profile step once on its grid and run the refits again; otherwise record the target as met. Why: it
   lies 0.0013 above the band on 8 seeds, one seed carrying most of the excess (3.2). Expected effect: the target
   decided on enough seeds. In sample; V1's and V3's doses are re-set as manipulation checks if anything moves, and V
   results are not looked at.
7. **A genuine test of notes (documentation, then tooling).** What: find a field finding on community notes that the
   model has not been run against (another measure or platform), fix its band and the notes design from documentation
   before any run, and add it as a check. Why: V7 passes only under a design chosen after its result (4.3), so the
   battery has no genuine test of notes. Expected effect: an out-of-sample notes check again. Pre-registration keeps it
   out of sample.
8. **Separate learning from contagion in fact 2 (tooling and documentation).** What: report fact 2's learning check also
   with the contagion held off in both arms, and find why faster learning gives a smaller learning effect (PRCC of
   `learning_rate` -0.60 and -0.50, 6.4). Why: the contagion carries most of the rise and confounds the learning check,
   whose new pass (0.051) does not by itself show learning from likes (3.1). A change of the check's design is a
   declared change to an in-sample fact.
9. **Explore living platforms (tooling).** What: offer a revised design conditioned on a platform that does not empty,
   for example resampling degenerate worlds or reporting hold shares over the worlds with `habit_gain` above its lowest
   fifth, fixed before the next run. Why: half the worlds in the lowest fifth of `habit_gain` and 54% of those with slow
   habit and high churn are degenerate (6.3), so the revised hold shares mix two questions. Expected effect: revised
   shares over worlds where every mechanism operates. Ranges fixed before the run tune nothing.
10. **Fact 3's polarisation bound (documentation).** What: record whether the bound of 0.05, a relative difference,
    matches what Guess et al. (2023) could rule out, from the study's reported interval and before any rerun. Why: the
    check sits at its bound at the defaults (0.044) and fails in 45% of local and 62% of plausible worlds with little
    seed noise, driven by `outrage_appeal` and `polarisation_rate` (6.4). Expected effect: a bound with a stated source.
    A change is a declared change to an in-sample fact; no constant is refitted.

## 9. Limits of this validation

- **Loose mappings.** Several checks map a field measure onto a model analogue that is only roughly the same: clicks on
  hard news onto likes of in-network posts (V6), news knowledge onto informedness, whose spread is far smaller (V2,
  V10), untrustworthy-source exposure onto posts with `truth` below 0.5 (V1, V10, fact 3), an accuracy prompt onto share
  friction (V4), politicians' and news sources' amplification onto authors ranked by a partisanship trait (V5), and
  reposts on X onto shares under the integrity preset with notes on and no penalty (V7). Since lab #40 the prevalence
  target reads a post whose toxicity exceeds 0.5 as one a classifier would label uncivil, one to one: the model's
  toxicity is harm to readers and targets, the field's label a classifier's incivility, and the same reading underlies
  V1's toxic share and fact 3's toxic switch check.
- **Field results from other settings.** Some bands come from studies of a whole phone or another product: Dekker et
  al.'s notifications switch-off was phone-wide (fact 7), Pandora's ad load elasticity comes from music streaming (V9).
  They are used here for one platform.
- **Dose mapping.** V1's and V3's doses were re-set at the new constants on 8 seeds (-0.327 and 0.0727); on the 16 seeds
  of this update they read -0.314 and 0.0689, within their targets' noise. Under the engagement ranking the same
  settings give other doses (V3: 0.197 hidden), so the engagement column is not at the field's dose.
- **Spillover.** The feed experiments treat 20% of the regular users (V1, V3, V5, V10, fact 3), against a negligible
  share of a platform's users in the field studies. In the model the treated make up a fifth of the audience, so what
  they stop seeing, liking and sharing changes what everyone else sees and does, which feeds back to the treated; their
  change includes these knock-on effects, which the placebo run, with nobody treated, does not remove. V2's break also
  takes 20% of the regular users off the platform, and fact 6's break all of them.
- **SD scaling.** Effects in SDs depend on how heterogeneous the model's people are: informedness's SD is 0.024 at the
  defaults, so V2's verdict is one of scale (4.3).
- **Unverified sources.** V10 rests on the abstract of Guess et al. (2023b) only, and V3 on a working paper
  (Beknazar-Yuzbashev et al., CAGE WP 741/2025, May 2026 draft); both judge directions with wide bands. V6's figures
  come from the authors' blog summary of Bakshy et al. (2015), not the paper (`exports/research-validation/notes.md`).
  Lab #41's account of Meta's demotion was read through search summaries only.
- **Design changes after a failure was seen.** Lab #33 moved V5 and fact 3's exposure checks onto the field's design
  after lab #31 had seen them fail, with the rule fixed before any run; their passes are less independent than a
  first-time pass. Lab #41 moved V7 onto the field's notes design after M2 had measured 0.842 without the penalty; V7 is
  reported and excluded from the genuine tests.
- **Seeds per world.** Per-world verdicts carry seed noise (2 seeds, 4 or 8 for the noisy items), and every world uses
  the same seeds, so a seed set's luck can move many worlds alike (5.3). The failure classes use one robust SE per
  check: "clear" means beyond the typical world's seed noise, and a world with unusually noisy seeds can be called
  clear. Seed shares rest on the robust SE and compare with M2's only after conversion (2.6).
- **Coverage.** The checking habits, the interruption cost and `base_session_items` are never varied, and the bot
  strategy is held at adaptive across the plausible ranges, though three claims depend on it (6.2). Design and run
  settings (population size, run length, post lifetime) are not varied, and results depend on the run length
  (model.md 16). The documented ranges were not rerun, nor the checking habits world at the new constants.
- **The diagnostics.** M2's V7 runs (`exports/analysis-r5/v7-*`) were made at M2's constants; this update added one run,
  V7 on seeds 1 and 2 at the defaults (2.8). Lab #31's diagnostics of V2, V5 and V6
  (`exports/validation-analysis/diag/`) and M2's fact 2 runs without the contagion were not repeated at the new defaults
  and are cited only as theirs.
