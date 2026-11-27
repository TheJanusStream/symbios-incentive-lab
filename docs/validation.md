# Validation of the incentive lab model (lab #31, updated after milestone 2)

This document records how well the model of [model.md](model.md) reproduces empirical findings, and how stable and
sensitive its results are under noise on the world assumptions. Lab #31 wrote it on 2026-11-27 from runs of the build
of that day (the working tree on top of b288a86). Its section 8 recommended eight modifications; labs #32 to #39
(milestone 2, hub #25) implemented them, and this update, also of 2026-11-27, reports every result again from runs of
the release build after milestone 2 (the working tree on top of 7ddb60a; binary
`exports/robustness/bin/incentive-r5`, with its SHA-256 beside it, the same file as `target/release/incentive`).
Where it helps to judge what changed, lab #31's value stands beside the new one and is always marked "lab #31". Every
number names the file it comes from; the files sit under the gitignored `exports/` folder and the commands of 2.8
regenerate them.

## 1. Purpose

The lab is a toy model for intuition about incentive design on one platform (model.md, section 1). Its results are
only as strong as two things this document measures:

- **Fit and accuracy.** Fit is how well the model reproduces what it was tuned on (the stylized facts, the
  calibration targets and the headline claims). Accuracy is how well it reproduces field findings it was never tuned
  on, with bands fixed before the model was run on them.
- **Stability and sensitivity.** Stability is whether a result survives small noise (about +-25%) on every world
  assumption around the defaults. Sensitivity is how a result moves across a range of each assumption, and which
  assumptions drive it. Since milestone 2 there are two sets of ranges: plausible ranges, which stand for beliefs, and
  the revised ranges of model.md section 11, which explore.

The analysis ends in what milestone 2 achieved and new recommendations (section 8), and in the limits of this
validation (section 9).

## 2. Method

### 2.1 In-sample and out-of-sample

In-sample checks were used in tuning, so passing them shows the calibration: the seven stylized facts of sections 13.4
and 15 of model.md (14 required checks since milestone 2, when fact 3's two exposure checks became required, and no
known limit), the calibration targets of 13.2, and the headline claims H1 to H8
(`crates/incentive-core/src/headline.rs`), the exhibit claims of 13.3 written as band checks. The held-out regretted
share of 13.4 was named before tuning and never tuned on; it is reported with the in-sample results because it belongs
to the calibration record.

Out-of-sample checks are the battery V1 to V10 (`crates/incentive-core/src/validation.rs`, run by `incentive
validate`): ten field findings with 21 required checks and one known limit, none used in calibration. Sources and
quotes are in `exports/research-validation/notes.md` and `exports/research-2026-11-26/notes.md`.

### 2.2 Pre-registration

Lab issue #31 fixed the out-of-sample checks and their bands from the literature before any run (its comment of
2026-11-26, 22:30). The bands are relative changes unless the field gives standard deviations. An amendment at 22:35,
also before any V run, moved every check onto the integrity preset (2.3). The first wiring demonstrations ran at 22:56
and the battery at 23:27, so no band was set after a model result was seen. Milestone 2 kept every band.

Milestone 2 fixed each in-sample target before tuning, from sources other than the V checks'
(`exports/research-recs/notes.md`): the misinformation share under the integrity preset, 2.5% (band 1.5% to 3.5%,
lab #32, Guess et al. 2023); the note timing, a lognormal fitted to field quantiles (lab #34); the causal relative risk
of a hostile post after a fully hostile feed, 1.6 (band 1.4 to 1.8, lab #38, Cheng et al. 2017, amended before tuning
from an observational to a causal measure); and the ranges of lab #35 before the next robustness run. The V checks were
not looked at while tuning. Two design changes were made after lab #31 had seen the failures they address, each with
the field's design as its rule and fixed on the issue before any run: V5 and fact 3's exposure checks moved to a mid-run
switch on a shared follow graph (lab #33, 2.4).

Two checks need a dose, set as manipulation checks, not outcomes: V1's same-group down-ranking penalty (0.28) was
calibrated so that the treated's same-group impression share falls by a third, as in the field (53.7% to 36.2%), and
V3's hiding threshold (0.56) so that about 7% of the treated's impressions are hidden. The doses were not re-set after
milestone 2. They are now -0.355 (95% interval -0.442 to -0.266) and 0.0786 (0.0690 to 0.0882) under the integrity
preset, and -0.379 and 0.224 under the engagement ranking (`exports/validation-r5/validation.md`; lab #31: -0.331 and
0.0729, -0.389 and 0.212). V4's friction dose does not map onto the field's accuracy prompt, so V4 judges direction
only.

The range rule for lab #31's global runs was fixed after the B1 global pilot, in which 2 of its 4 worlds were
degenerate under section 11's ranges as written, and before the real runs (lab #31's comment of 2026-11-26, 22:34;
`exports/robustness/pilot-global.txt`).

### 2.3 The integrity preset as the realistic feed

The field studies ran on Facebook, Instagram and Twitter, whose ranked feeds carry integrity measures. The model's
stand-in is the integrity preset (model.md, 14.1): the engagement ranking with demotion instead of removal, a quality
signal, see-less feedback and demoted community notes. Facts 3, 6 and 7 already compare against it. Every V check is
judged under it; the same check under the plain engagement ranking is reported beside it as a sensitivity and not
judged.

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
checks). Lab #31's V5 and fact 3 compared two platforms that ran from day 0, each with its own follow graph; those
numbers are kept below for comparison only.

### 2.5 Verdict rules

At the defaults each check runs on 8 seeds at full size (`N = 2,000`, 365 days). The verdict judges the 95%
t-interval of the per-seed effect: PASS when it lies inside the band, FAIL when it lies wholly outside, UNDECIDED when
it crosses a bound. Seed means are reported beside the intervals. A known limit is reported and not required.

Ratio convention (lab #39, 2026-11-27), one for every check of the battery, the facts and the headline claims: a
ratio, relative change or elasticity effect is a ratio of seed means, the mean of one arm over the mean of the other
on paired seeds. Its 95% interval and its verdict are of the per-seed values (`a / b` on each seed), so the interval's
centre, the mean of the per-seed ratios, can differ from the effect; `validation.json` and `facts.json` keep both
(`effect` and `interval.mean`). `incentive validate` and `incentive facts` print this convention with their results.
Within one run, V6 and V8 pool their counts over the summary window since lab #39 (a ratio of sums, not a mean of
daily ratios), and the feed experiments' shares and reach measures pool over the treated group and the experiment's
days. V7 still averages its daily shares.

An effect in standard deviations divides by the "SD used", which `incentive validate` prints with it (lab #39): the
participants' spread at the experiment's start, the mean over the seeds of the SD in the effect's first arm. At the
defaults it is 0.2125 for polarisation (V1, V10), 0.2278 for affective polarisation (V2), and 0.0232 and 0.0221 for
informedness (V2, V10) (`exports/validation-r5/validation.md`). The raw change is printed beside it.

In the robustness runs (2.6) a check holds in a world when its effect on the seed means lies inside its band, and a
fact or claim holds when all its required checks hold. "Sign" is the share of worlds whose effect lies on the default
world's side of its neutral value: 0 for a difference or a relative change, 1 for a ratio and for the ratios and
shares that V6, V7 and V8 read. The runs of this update compute it with those neutrals; lab #31's `summary.md` files
predate the fix, and lab #31's signs of V6, V7 and V8 were recomputed then
(`exports/validation-analysis/sign_shares.txt`).
V7's default is exactly 1.000, at its neutral, so its sign is the share of worlds also at 1.000 (no share after any
note).

### 2.6 Noise designs, range rules and failure classes

All three designs of this update vary 54 settings: 51 world assumptions of the catalogue, which since milestone 2
include `exposure_contagion` (lab #38) and `outgroup_like_cut` (lab #36), plus the troll share, the bot share and
`bot_strategy`. Held at their defaults (lab #35): the checking habits (`recheck_rate`, `notification_pull`,
`extra_session_share`) and `interruption_cost`, off by the owner's decision (model.md, 6.1 and 13.2), and
`base_session_items`, which sets the time scale like them; lab #31 varied it (5 to 60 around 20). Design and run
settings never vary. Each world runs seeds 1 and 2 at full size for all three batteries, and seeds 1 to 4
(`--seeds-noisy 4`) for the items whose seed share exceeded 0.4 in lab #31's runs: facts 2 and 5, H5, V1, V3, V4, V5,
V9 and V10, and H7 in the two global designs.

- **Local (stability):** 100 worlds. Each assumption with a nonzero default is multiplied by U(0.75, 1.25) and
  clamped to its catalogue range; `leave_threshold` and `polarisation_backfire` (default 0) and `bot_strategy` keep
  their defaults, so 51 are jittered.
- **Plausible (belief):** 200 worlds on a Latin hypercube over the plausible ranges fixed on lab #35 before the run:
  default/1.5 to 1.5 x default for the 40 positive scales and the two population shares, default +- 0.15 (clamped to
  [0, 1]) for the 10 shares and weights, -0.05 to 0.05 for `leave_threshold`, and `bot_strategy` held at adaptive.
  `exposure_contagion` runs from 0.187 to 0.42 and `outgroup_like_cut` from 0.65 to 0.95.
- **Global (exploration):** 240 worlds on a Latin hypercube over the revised ranges of model.md section 11, with
  `bot_strategy` drawn uniformly. Lab #31's rule (a range written for a text default that calibration changed is scaled
  by the same factor; an assumption without a documented range gets default/3 to 3 x default; the troll and bot shares
  default/2 to 2 x default; all clamped to the catalogue) gives 39 documented, 5 scaled, 4 default/3 and 2 population
  ranges. Three judgement ranges were fixed on labs #35 and #38 before the run: `leave_threshold` -0.1 to 0.1 (lab #31
  held it at 0 for want of a range), `exposure_contagion` 0 to 0.56 (off to twice the default) and `outgroup_like_cut`
  0 to 1.

Sensitivity is the partial rank correlation (PRCC) of each assumption with each effect, with its Spearman correlation,
over the non-degenerate worlds. A PRCC is marked significant when it lies beyond two standard errors of 0, the SE
being 1/sqrt(n - k - 2) for n worlds and k regression terms. With 53 numeric assumptions, about 4.55% of them, 2.4,
lie beyond 2 SE by chance alone. R2 is the adjusted R2 of a rank regression of the effect on every varied number (and
the strategy's dummies in the global design): the share of the effect's rank variance that additive monotone effects
explain.

Failure classes (lab #35, the rule of lab #31's 5.2): a failing world is "noise-compatible" when its effect lies within
two standard errors of its band, the SE being the seed SD over sqrt(seeds), and "clear" otherwise. The seed SD is the
SD of the per-seed effects within a world, pooled over the non-degenerate worlds with a value
(`crates/incentive-cli/src/robustness/mod.rs`, `seed_noise` and `noise_compatible`). An item fails within noise when
every failing required check does. The "seed share" is the squared SE over the variance of the worlds' effects: the
part of their spread that seed noise alone would make. Section 5.3 tests the pooled SD.

### 2.7 Degenerate worlds

A world is degenerate when the default design's user share on its last day, averaged over seeds 1 and 2, lies below
0.05: the platform has emptied, and most effects become ratios of near-zero quantities. The summaries report hold
shares over all worlds and over the non-degenerate ones.

### 2.8 Files and commands

| Material | Files |
| --- | --- |
| Validation at the defaults, 8 seeds | `exports/validation-r5/validation.md`, `validation.json` |
| Facts, calibration report (model.md 13.3 to 13.6) | `exports/report/report.md`; facts also `exports/recs-fixpoint/` |
| Fact 2 with and without the exposure contagion | `exports/brady-check/k0/`, `k0.28/` (`facts.json`) |
| Local noise, 100 worlds | `exports/robustness/r5-local/` |
| Plausible ranges, 200 worlds | `exports/robustness/r5-plausible/` |
| Revised ranges, 240 worlds | `exports/robustness/r5-global/` |
| V7 diagnostics | `exports/analysis-r5/v7-integrity-penalty*/` |
| This update's analysis | `exports/analysis-r5/*.txt`, from the scripts of the same names |
| Lab #31's runs and analysis | `exports/validation/defaults/`, `exports/robustness/local-1/`, `global-1/`, |
| | `global-2-documented/`, `headline-defaults-8seeds/`, `exports/validation-analysis/` |

Each robustness folder holds `summary.md`, `summary.json`, `checks.csv` (with the per-seed effects), `worlds.csv` and
`worlds.json`. The analysis scripts of this update are `tables.py` (compact tables of all six robustness runs),
`pooled_sd.py` (5.3), `global_extra.py` (strategies and degenerate worlds) and `top3.py` (drivers) in
`exports/analysis-r5/`, run from the repository root with `python3 -I`; like every output cited here they live in the
gitignored `exports/` folder. Every CLI run went through `taskset -c 0-7 nice -n 19 ionice -c 3`
(`exports/robustness/r5-chain.sh` for the first four, with `BIN` the binary named above):

```text
$BIN validate --seeds 8 --out exports/validation-r5
$BIN robustness --mode local --worlds 100 --seeds 2 --seeds-noisy 4 --sample-seed 1 --battery all \
  --out exports/robustness/r5-local
$BIN robustness --mode global --ranges plausible --worlds 200 --seeds 2 --seeds-noisy 4 --sample-seed 1 \
  --battery all --out exports/robustness/r5-plausible
$BIN robustness --mode global --ranges revised --worlds 240 --seeds 2 --seeds-noisy 4 --sample-seed 1 \
  --battery all --out exports/robustness/r5-global
incentive report --seeds 1..8
incentive facts --seeds 8 --fact outrage-learning --set world.exposure_contagion=0 --out exports/brady-check/k0
incentive run --preset integrity-ranking --seeds 1..8 --set design.noted_penalty=0 \
  --metrics noted_shares_before_note,notes,noted_false_posts_ended,false_posts_ended \
  --out exports/analysis-r5/v7-integrity-penalty0
```

`incentive report` writes to its default folder, `exports/report/`. The brady-check's second run sets
`world.exposure_contagion=0.28` (the default); the V7 runs repeat the last command with `noted_penalty` 0.1, 0.25, 0.5
and 1.0. Lab #31's commands are in its version of this section (git history).

## 3. In-sample fit

### 3.1 Stylized facts

The facts at the fixed point after labs #32, #38 and #34 (model.md 13.4; `exports/report/report.md`, 8 paired seeds).
Lab #31's effects are from `exports/validation-analysis/facts-rerun/`.

| Fact | Check | Effect | 95% interval | Band | Verdict | Lab #31 |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | Outrage impressions, ER minus chronological | 0.469 | 0.425 to 0.513 | 0.15 to 0.6 | pass | 0.404, pass |
| 1 | Reflective value per impression, chrono minus ER | 0.193 | 0.164 to 0.222 | 0.05 to 0.4 | pass | 0.161, pass |
| 2 | Rise of the Outrage share of posts under ER | 0.062 | 0.043 to 0.080 | 0.01 to 0.15 | pass | 0.018, undecided |
| 2 | That rise minus the rise without learning | 0.023 | 0.008 to 0.037 | 0.01 to 0.15 | undecided | 0.014, undecided |
| 3 | Time spent, chronological over ER | 0.636 | 0.616 to 0.657 | 0.4 to 0.9 | pass | 0.641, pass |
| 3 | Polarisation difference at day 90 | 0.037 | -0.043 to -0.032 | below 0.05 | pass | 0.028, pass |
| 3 | Toxic share, switch, chrono over integrity | 0.757 | 0.715 to 0.807 | 0.5 to 0.9 | pass | 0.336, fail (limit) |
| 3 | Misinformation share, switch, same | 1.682 | 1.595 to 1.771 | 1.05 to 3 | pass | 0.640, fail (limit) |
| 4 | Largest cascades, depth cap 1 over none | 0.756 | 0.732 to 0.781 | 0.3 to 0.85 | pass | 0.720, pass |
| 5 | Vulnerable who left, troll share 0.12 minus 0.03 | 0.095 | 0.059 to 0.131 | 0.03 to 0.3 | pass | 0.074, pass |
| 6 | Break group's wellbeing, real minus placebo, SDs | 0.066 | 0.011 to 0.122 | 0.02 to 0.25 | undecided | 0.057, U |
| 7 | Time a day, notifications off minus placebo, SDs | -0.166 | -0.177 to -0.155 | -0.4 to 0.1 | pass | -0.168, pass |
| 7 | Sessions a day, off minus placebo, SDs | -0.322 | -0.339 to -0.305 | -0.4 to 0.1 | pass | -0.326, pass |
| 7 | Wellbeing, off minus placebo, SDs | -0.002 | -0.004 to 0.001 | -0.15 to 0.15 | pass | -0.003, pass |

All 14 required checks hold: 12 pass on their intervals and 2 are undecided (fact 2's learning check and fact 6), and
all 14 hold on the seed means (lab #31: 12 required, 9 passed, 3 undecided). Fact 3's two exposure checks were known
limits that failed at lab #31 on separate platforms; on the switch design (2.4) both lie inside the field's bands, and
milestone 2 made them required. This is a declared change to an in-sample fact (lab #33), not a recalibration. Fact 6
and fact 7's use checks were tuned on (`time_cost` and the opening chance's notification factor), so passing them
shows the calibration.

Fact 2 is now carried mostly by the exposure contagion of lab #38, not by learning from likes
(`exports/brady-check/`, 8 seeds; model.md 13.4). With the contagion at its default of 0.28 the Outrage share rises by
0.062, and by 0.039 in the arm without learning; with `exposure_contagion` 0 it rises by 0.019 (0.012 to 0.026,
pass), and by 0.004 without learning, so its learning check is 0.015 (0.009 to 0.022, undecided). The contagion raises
the Outrage share in both arms of the learning check, which confounds that check (model.md 15). Lab #31's 0.018 had no
contagion.

### 3.2 Calibration targets and the held-out target

At the fixed point (model.md 13.2, its last block; `exports/recs-fixpoint/`, 8 seeds, default design):

| Target | Now | Band | Verdict | Lab #31 |
| --- | --- | --- | --- | --- |
| User share after 365 days | 0.468 (0.438 to 0.499; lowest seed 0.422) | 0.4 to 0.9 | holds | 0.499 |
| Toxic impression share | 0.209 (0.169 to 0.249) | 0.03 to 0.2 | fails on the mean | 0.178, holds on mean |
| Outrage share of posts | rises by 0.062 (0.043 to 0.080), ends at 0.351 | rises, below 0.5 | holds | +0.018 |
| Harm cut at a moderate engagement cost | Q&B: harm -83%, engagement -17% | judgement | holds | -80%, -17% |
| Held-out regretted share | 0.0055 (0.0048 to 0.0061) | 0.20 to 0.42 | fails | 0.0045 |

The toxic share is missed. Its seed mean lies above the band's top and 3 of the 8 seeds lie above 0.2 (0.225, 0.269,
0.285). Lab #38's contagion raised it from 0.180, no in-sample rule names a constant that would bring it back, and
none was tuned to rescue it: it is recorded as missed (model.md 13.2 and 16). Under the integrity preset the toxic
share is 0.105 ± 0.019, three times the 3.2% of Facebook feeds that Guess et al. (2023) estimated as uncivil
(1.6% of Instagram's; recommendation 1 of 8.2). Two targets named for the default design, sessions per daily active
(T3) and the notification share of sessions (T1), are missed by the owner's decision and hold only in the checking
habits world (T3 15.15, T1 0.111 there).

The held-out regretted share still fails on both its interval and its mean, two orders of magnitude below its band:
with one session a day the model has almost no compulsive use. In the checking habits world it is 0.482, above the
band (model.md 13.4).

### 3.3 Headline claims

All 13 checks of H1 to H8 hold at the defaults on 8 seeds (`exports/report/report.md`, the rows of model.md 13.3
named; the intervals are of the paired difference behind each effect). Lab #31's effects are from
`exports/robustness/headline-defaults-8seeds/`, where every seed of every check lay in its band.

| Claim | Check | Band | Effect, report row (95% interval of the difference) | Lab #31 |
| --- | --- | --- | --- | --- |
| H1 | Harm index, Q&B against ER, relative | -1 to -0.5 | -0.826, row 20 (-0.323 to -0.261) | -0.798 |
| H1 | Engagement index, Q&B against ER, relative | -0.33 to 0.33 | -0.165, preset table | -0.171 |
| H2 | Survey satisfaction, ER minus Q&B | above 0 | +0.007 (+0.001 to +0.012) | +0.0061 |
| H2 | Wellbeing of ever-users, ER minus Q&B | below 0 | -0.078 (-0.087 to -0.069) | -0.069 |
| H3 | Toxic impressions, threshold 0.3 over default | 0 to 0.5 | 0.058, row 9 (-0.236 to -0.158) | 0.051 |
| H3 | False-positive actions, same | above 3 | 12.5, row 9 (+41.5 to +42.9) | 13.85 |
| H4 | Harm index, trap solution minus ER | below 0 | -0.166, row 1 (-0.184 to -0.147) | -0.153 |
| H4 | Engagement index, trap solution over ER | above 0.9 | 1.119, row 1 (+0.015 to +0.022) | 1.102 |
| H5 | Engagement index, intensity 1 over 0.6 | above 1 | 1.238, row 31 (+0.034 to +0.038) | 1.224 |
| H5 | Wellbeing of users, intensity 1 minus 0.6 | below 0.01 | -0.0073, row 31 (-0.0130 to -0.0017) | -0.0012 |
| H6 | Engagement index, last 95 days, ER over Q&B | above 1 | 1.195, row 13 (+0.022 to +0.028) | 1.202 |
| H7 | Harassment per 1,000, report trust 1 minus 0, Q&B | below 0 | -1.94, row 26 (-2.47 to -1.41) | -1.94 |
| H8 | Harm index, `w_see_less` 5 minus 0 | above 0 | +0.197, row 6 (+0.167 to +0.228) | +0.250 |

The narrowest margin is again H2's survey difference, whose interval starts at +0.001. H5's wellbeing difference moved
from about 0 to slightly negative (model.md 13.3: more reading feeds more hostility into what authors write, through the
contagion), which still satisfies "not wellbeing".

In sample the model fits what it was tuned on: every required fact holds on the seed means, every headline claim holds
at the defaults, and every target named for the default design holds except T1 and T3, by decision, and now the toxic
share, which lab #38's contagion pushed above its bound. The one prediction it was not tuned on, the regretted share,
still lies a factor of about 36 below its band.

## 4. Out-of-sample accuracy

### 4.1 The checks at the defaults

Integrity preset, 8 paired seeds, full size (`exports/validation-r5/validation.md` and `.json`; lab #31's column from
`exports/validation/defaults/`); "rel." marks a relative change, "U" undecided.

| Id | Check | Model (95% interval) | Band | Verdict | Lab #31 |
| --- | --- | --- | --- | --- | --- |
| V1 | Polarisation, SDs | -0.068 (-0.088 to -0.049) | -0.12 to 0.12 | pass | -0.062, pass |
| V1 | Toxic share, rel. | -0.015 (-0.051 to 0.024) | -0.35 to -0.02 | U | -0.004, U |
| V1 | Time per day, rel. | -0.065 (-0.081 to -0.049) | -0.10 to 0.05 | pass | -0.058, pass |
| V1 | Misinformation share, rel. | -0.140 (-0.179 to -0.103) | -0.5 to 0 | pass | -0.151, pass |
| V2 | Affective polarisation, SDs | -0.053 (-0.059 to -0.047) | -0.14 to 0.02 | pass | -0.049, pass |
| V2 | Informedness, SDs | -0.556 (-0.635 to -0.476) | -0.30 to -0.05 | FAIL | +0.096, FAIL |
| V2 | Later use, rel. | -0.203 (-0.215 to -0.190) | -0.35 to -0.08 | pass | -0.190, pass |
| V3 | Time per day, rel. | -0.009 (-0.015 to -0.002) | -0.25 to -0.02 | FAIL | -0.016, U |
| V3 | Engagement per day, rel. | -0.022 (-0.033 to -0.010) | -0.30 to 0 | pass | -0.034, pass |
| V3 | Own posts' toxicity, rel. (limit) | -0.004 (-0.013 to 0.007) | -0.45 to -0.10 | FAIL (limit) | +0.003, FAIL |
| V4 | Truth of shares, rel. | +0.010 (0.005 to 0.015) | 0 to 0.25 | pass | +0.013, pass |
| V5 | Partisan half's reach, ranked over chrono | 1.324 (1.216 to 1.463) | 1 to 5 | pass | 0.959, U |
| V5 | Extremes' amplification over the middle's | 1.803 (1.439 to 2.507) | below 1.1 | FAIL | 2.251, FAIL |
| V6 | Out-group impressions over candidates | 0.959 (0.908 to 1.009) | 0.85 to 1 | U | 0.982, U |
| V6 | Out-group likes over impressions | 0.648 (0.611 to 0.686) | 0.75 to 0.97 | FAIL | 0.664, FAIL |
| V7 | Share of a noted post's shares before its note | 1.000 (1.000 to 1.000) | 0.70 to 0.95 | FAIL | 1.000, FAIL |
| V8 | Shares per impression, Outrage over others | 2.715 (2.581 to 2.850) | 1.3 to 3 | pass | 2.813, pass |
| V9 | Elasticity of time to ad load | -0.018 (-0.053 to 0.014) | -0.15 to -0.01 | U | -0.051, pass |
| V10 | Misinformation share, rel. | -0.103 (-0.122 to -0.085) | -0.8 to -0.05 | pass | -0.064, U |
| V10 | Engagement per day, rel. | -0.035 (-0.048 to -0.023) | -0.5 to -0.01 | pass | -0.033, pass |
| V10 | Polarisation, SDs | -0.024 (-0.036 to -0.011) | -0.1 to 0.1 | pass | -0.023, pass |
| V10 | Informedness, SDs | +0.060 (0.021 to 0.099) | -0.5 to 0 | FAIL | +0.038, FAIL |

Lab #31's V5 is the separate-platform design (2.4); its V6 and V8 are means of daily ratios (lab #39 pooled them: 0.982,
0.663 and 2.784 at lab #31's defaults). The field values, the engagement ranking's effect with the verdict its interval
would get (not judged), and the kind of test (4.2), in the same order; the sources are in
`exports/validation-r5/validation.md`.

| Id | Check | Field | Engagement ranking | Kind |
| --- | --- | --- | --- | --- |
| V1 | Polarisation | null, within 0.12 SD | -0.076, pass | calibration |
| V1 | Toxic share | -11% | -0.027, undecided | test |
| V1 | Time per day | -0.02 SD, null | -0.101, undecided | test |
| V1 | Misinformation share | -28% | -0.108, pass | test |
| V2 | Affective polarisation | -0.06 SD, -0.031 SD | -0.075, pass | calibration |
| V2 | Informedness | -0.19 SD, -0.098 SD | -0.210, pass | test |
| V2 | Later use | -23%, -15% | -0.227, pass | test |
| V3 | Time per day | -10% | -0.038, pass | test |
| V3 | Engagement per day | -0.056 SD | -0.082, pass | test |
| V3 | Own posts' toxicity | -30% | -0.034, fail | test (was absent) |
| V4 | Truth of shares | +2.1% to +9% | +0.008, pass | assumed |
| V5 | Partisan half's reach | above 1 | 1.496, pass | test |
| V5 | Extremes' amplification | about 1 | 2.243, fail | test |
| V6 | Out-group impressions | 0.92 to 0.95 | 0.815, fail | test |
| V6 | Out-group likes | 0.83 to 0.94 | 0.521, fail | assumed |
| V7 | Shares before the note | about 0.85 | 0.9997, fail | test |
| V8 | Outrage shares | 1.35 to 2.8 | 2.585, pass | assumed |
| V9 | Ad load elasticity | -0.02 to -0.07 | -0.043, pass | assumed |
| V10 | Misinformation share | down | -0.093, pass | test |
| V10 | Engagement per day | down | -0.025, pass | test |
| V10 | Polarisation | null | -0.028, pass | calibration |
| V10 | Informedness | down | +0.076, fail | test |

Of the 21 required checks 12 pass, 3 are undecided and 6 fail on their intervals (lab #31: 11, 5 and 5); 14 hold on the
seed means (lab #31: 13), V6's ranking ratio and V9 lying inside their bands on the mean. Moved into the band: V5's
partisan half (the new design) and V10's misinformation share. Moved out: V9 (to undecided, through seed noise, 4.3)
and V3's time (from undecided to fail). V2's informedness changed sign and now fails on size. The known limit, V3's own
toxicity, still fails; since lab #38 the model has a channel from what people see to what they write, so it now counts
as a test of that channel.

### 4.2 Scorecard

**Directions against magnitudes.** 17 required checks have a field direction; the other 4 are nulls or an upper bound
(V1 polarisation and time, V10 polarisation, V5's extremes). Judged on the 95% intervals:

| | Checks | Direction right | Direction wrong | Not established | Magnitude in band | Lab #31 |
| --- | --- | --- | --- | --- | --- | --- |
| Directional | 17 | 12 | 2 (V7, V10 inform.) | 3 (V1 toxic, V6 ranking, V9) | 9 | 11, 3, 3, 8 |
| Nulls and bound | 4 | - | - | - | 3 (V5 extremes fails) | 3 |

The conventions are lab #31's: V7's 1.000 counts as a wrong direction though it lies at the boundary of no share after
a note, and V2's polarisation counts as directional. The direction is right for 12 of 17 (lab #31: 11) and the
magnitude for 9 of 17 (11 on the seed means; lab #31: 8 and 10). V2's informedness moved from the wrong direction to
the right one with too large an effect in SDs (4.3); V9 lost its established direction to seed noise. Where the
direction is right but the check fails, the model's effect is too small (V3 time, -0.9% against -10%) or too large
(V2 informedness in SDs; V6 likes, 0.65 against 0.83 to 0.94).

**Mechanism by assumption against genuine tests.** "Kind" in 4.1 sorts the checks by what a pass can show:

- *Calibration-implied* (3: the polarisation nulls of V1, V2 and V10). `polarisation_rate` was cut so that fact 3's
  polarisation barely moves in three months (model.md 13.1, 13.4). All 3 pass, as at lab #31.
- *Direction assumed* (4: V4, V6 likes, V8, V9). The direction is written into an equation (share friction scales
  impulsive shares, out-group likes are cut by `1 - outgroup_like_cut * outrage`, hostility raises shares, ads lower
  reflective utility). 2 pass, V6's likes fail and V9 is undecided (lab #31: 3 passed).
- *Genuine tests* (14): the effect emerges from interactions no parameter was set for. 7 pass (V1 time and
  misinformation, V2 later use, V3 engagement, V5 half, V10 misinformation and engagement), 2 are undecided (V1
  toxic, V6 ranking) and 5 fail (V2 and V10 informedness, V3 time, V5 extremes, V7). Lab #31: 5, 5 and 4.

5 of the 7 calibration-implied or assumed checks pass, and 7 of the 14 genuine tests (lab #31: 6 of 7 and 5 of 14).
The genuine tests that pass are still mostly about time and engagement, plus two exposure shifts of feed experiments
(V5's half, V10's misinformation).

### 4.3 Findings

**Leaving the platform lowers informedness now, by too much in SDs (V2); removing reshares still raises it (V10).**
Lab #32 calibrated the prevalence of misinformation in sample (misinformation 0.0247 of impressions under the integrity
preset, from 0.130; model.md 13.1 and 13.2). Users' informedness rose from 0.607 to 0.642, and the break now lowers it,
as in the field: raw change -0.0129 (sd 0.0025 across seeds; lab #31 +0.0078), 2% of the level. The check divides by
the treated's spread at the start, which is only 0.0232 (V10: 0.0221), so the change reads -0.556 SD against the
field's -0.19 and -0.098 SD. Lab #31's spread was about 0.08 (derived from its raw and scaled changes): belief in
misinformation, which credulity makes unequal, spread people apart, and with misinformation five times rarer it no
longer does. At lab #31's spread the same raw change would read about -0.16 SD, inside the band, so V2's informedness
now fails on a scale that depends on how alike people's informedness is, not on its sign. The field's SDs are of a
knowledge index across very different people; the model's informedness is a narrow latent quantity. Removing reshares
still raises informedness a little, raw +0.0014 (sd 0.0011; lab #31 +0.0029), +0.060 SD: the treated's misinformation
share falls by 10.3% (now a pass), and reshares carry almost no informative gain in the model (lab #31's diagnosis,
`exports/validation-analysis/diag/v2v10-credulity0/`), whereas in the field they carried political news. Across worlds
V2's sign is the field's in 100% of local and 92% of plausible worlds, and V10's is the wrong one in 92% and 87%
(5.1, 6.1). Under the engagement ranking V2's informedness passes (-0.210 SD), with a spread that is not printed.

**Hiding flagged posts barely moves time, and the treated's own toxicity hardly responds (V3).** Time falls by 0.86%
against the field's 10%, now a fail (lab #31: -1.6%, undecided); the dose is 0.0786 of the treated's impressions hidden.
Per unit of dose the response is -0.11 under the integrity preset against -0.17 under the engagement ranking, whose dose
is three times larger (0.224) and whose time effect passes (-3.8%). The direction is robust and the size too small
almost everywhere: the sign holds in all local worlds and 98% of plausible ones, while only 10% and 8% reach the band
(the 5% to 95% range of local effects is -2.1% to -0.7%). The treated's own toxicity, the known limit, now has a
channel: lab #38's exposure contagion lets the hostile share of yesterday's feed raise the appeal of hostile styles. It
gives the field's direction across worlds (negative in 93% of local and 96% of plausible worlds; PRCC of
`exposure_contagion` -0.46 across the revised ranges) but -0.35% at the defaults (interval -1.25% to +0.73%) against the
field's -30%, and -3.4% under the engagement ranking at three times the dose. The contagion was set to the causal
relative risk of 1.6 between a fully hostile and a hostile-free day (Cheng et al.). Hiding 8% of impressions changes the
hostile share of a feed by a few points at most, which at that strength moves the chance of a hostile post by a few
percent at most, and the measured change is smaller still. A field effect of -30% from hiding 7% of items implies
channels the model lacks, such as replies drawn by what one sees; no run here separates them.

**Fewer like-minded posts barely move the toxic share (V1).** -1.5% (-5.1% to +2.4%) against the field's -11%,
undecided (lab #31: -0.4%). The sign is the default's in 67% of local worlds (lab #31: 41%) and 52% of plausible ones,
with a seed share of 0.32: the contagion did not create a reliable link between exposure and what the treated see.

**The most partisan authors are still amplified more than the middle (V5).** On the holdback design the more partisan
half's amplification, ranked over chronological, is 1.324 (pass; lab #31 0.959 on separate platforms), and the most
partisan tenth's over the middle fifth's is 1.803 (1.439 to 2.507), against the field's no extra amplification. Without
the impressions of posts removed or demoted by the end of their life they are 1.350 and 1.918; the treated's
impressions of all posts are 1.088 times as many on the ranked feed. Lab #33 measured the extremes ratio at 1.468
(1.071 to 2.285, undecided) at hub #23's defaults when it built the design; at the new defaults it is 1.803, so the
switch design took away part of lab #31's 2.25 and labs #32, #34 and #38 moved it back up; these runs do not say which.
Lab #31 traced most of the extra amplification to trending recommendations, which the chronological arm lacks in both
designs; that diagnosis was not repeated. The failure is robust in direction (above 1 in all local worlds and 93% of
plausible ones; 2% and 12% hold), but noisy: its seed SD is 0.68 at 4 seeds and its seed share 0.82 locally, and 5.3
shows that the pooled SD overstates its noise-compatible failures. Across the revised ranges something else happens:
in 39 of the 65 non-degenerate worlds with adaptive bots the partisan half's ratio falls below 0.5 (median 0.059),
more so where bots are many (Spearman with `bot_share` -0.54 among those worlds), while with the other strategies it
never does (medians 1.64; `exports/analysis-r5/global_extra.txt` and `tables.txt`). These runs do not explain it.

**People like out-group posts too rarely (V6).** The out-group share of in-network likes is 0.648 of its share of
impressions (lab #31: 0.664), against 0.83 to 0.94 for clicks in Bakshy et al. Lab #36 made the out-group cut a world
assumption (`outgroup_like_cut`, default 0.8) and decomposed `P_like` (its comment of 2026-11-27, integrity preset):
for same-group against out-group viewers the in-group hostility term contributes 0.457 against 0 to the logit,
arousal 0.442 against 0.313, the reflective term 0.079 against 0.185, and the out-group multiplier is 0.814; the mean
`P_like` is 0.249 against 0.131. The in-group appeal of hostility is the largest gap and the cut the second. The
global runs agree: across the revised ranges `outrage_appeal` has PRCC -0.85 and `outgroup_like_cut` (0 to 1) -0.55;
across the plausible ranges `outrage_appeal` has -0.84 and the cut (0.65 to 0.95) -0.09, not significant. So within
plausible values the cut hardly matters and the in-group appeal of hostility decides V6. The failure is robust: 89% of
local worlds fail, 84 of them clearly, with a seed share of 0.03. V6's ranking ratio, 0.959, is undecided and lies above
1 in 14% of local worlds.

**No one shares a post after its note, because noted posts are hidden (V7).** A noted post's human shares all come
before its note (1.000 under the integrity preset, 0.9997 under the engagement ranking), against about 0.85 derived from
Slaughter et al. Lab #34 made notes arrive in time (a lognormal delay with a median of 16 hours; each session gets a
time of day), and V7 did not move. The code was traced for this update:

- A false post gets its note decided when it is written; its delay counts from hour 0 of its day
  (`moderation.rs`, `decide_notes`). Each viewer's session hour is drawn before the viewer's feed is built
  (`session.rs`, `run_sessions`, stream SessionClock).
- A candidate carries the note when the post has at least 50 impressions and its age at the viewer's hour has passed
  the delay (`Post::note_shown`, called from `ranking/candidates.rs` at feed build). Viewers are served in index order,
  so impressions accumulate over the day and later viewers can meet a note on the post's first day.
- A candidate that carries the note loses `noted_penalty` from its ranking score (`ranking.rs`, `score`), and its
  share chance is halved (`behaviour.rs`), the model's version of the field's -46% after a note. A share counts as
  before the note while the impression did not carry it (`session.rs`, `shares_at_note`), and V7 is read over the
  noted posts as they end (`learning.rs`, `count_ending_post`).

Nothing in this chain is a defect. The ranking scores of the integrity preset are sums of terms of a few tenths
(`w_like` 1 times a like chance, recency 0.3 halving each day, in-network 0.2), and people read about 16 items a day, so
a penalty of 1.0 puts a noted post below everything a viewer reads: once a post's note shows, nobody is shown it, and
nobody shares it. Diagnostic runs, integrity preset, 8 seeds, the same binary (`exports/analysis-r5/v7-*`):

| `noted_penalty` | Share of shares before the note (mean ± SD over seeds) |
| --- | --- |
| 1.0 (integrity preset) | 1.000 ± 0.000 |
| 0.5 (the engagement ranking's default) | 0.988 ± 0.008 |
| 0.25 | 0.951 ± 0.025 |
| 0.1 | 0.885 ± 0.029 |
| 0 | 0.842 ± 0.032 |

Without the penalty the model gives 0.842, at the field's 0.85; with the timely notes of lab #34 the timing is no
longer a reason. So V7 measures the integrity preset's design choice to hide noted posts (model.md 13.2 and 14.1, kept
by judgement on lab #34), not a mechanism of the model's people. These numbers are now known, so choosing the penalty
from them would make V7 in-sample (recommendation 2 of 8.2). The measure also rests on few posts: about 0.31 noted
false posts end per day, some 34 per seed over the summary window, and V7 still averages daily shares. Across worlds V7
sits at 1.000 in 84% of local worlds and below it, never inside the band, in the rest; more outrage appeal gives more
shares after notes (PRCC -0.62 plausible, -0.71 revised).

**Seed noise moved V9.** The ad-load elasticity is -0.018 (-0.053 to +0.014), undecided (lab #31: -0.051, pass). Its
raw change is 121 time units with an SD of 252 across seeds; locally its seed share is 0.59 at 4 seeds, and all 24
local failures are noise-compatible, with the sign kept in 95% of worlds. The model's response to ad load is small
and noisy; more seeds would decide the verdict.

**What passes.** Fewer like-minded posts cut misinformation exposure (-14%) and time (-6.5%) within the field's
bands; after a break, use stays lower (-20% against -23% and -15%); removing reshares cuts misinformation exposure
(-10%) and engagement (-3.5%); ranking amplifies the more partisan half (1.32); friction raises the truth of shares
(+1.0%); Outrage is shared 2.7 times as often per impression as the non-hostile styles. These are the model's time,
engagement and exposure mechanics and its built-in directions.

### 4.4 The engagement ranking as the feed

Judged on the engagement ranking instead, the battery gives 14 passes, 2 undecided and 5 fails, against 12, 3 and 6
under the integrity preset (`exports/validation-r5/validation.md`; lab #31: 10, 5 and 6 against 11, 5 and 5). It now
does better on V2's informedness (-0.210 SD, pass), V3's time (-3.8%, pass) and V9 (-0.043, pass), and worse on V1's
time (-10.1%, undecided), V6's ranking ratio (0.815, below the band), V5's extremes (2.24) and V6's likes (0.52). Two of
its gains are not like for like: V3's dose is three times larger (per unit of dose the responses are -0.17 and -0.11),
and V2's informedness divides by an SD that the output does not print for this column. The count no longer favours the
integrity preset; the checks where it fits better (V1's time, V6) are the ones about how a ranked feed with integrity
measures differs from one without.

## 5. Stability (local)

### 5.1 Hold shares

100 worlds, every assumption jittered about +-25%, no degenerate world (`exports/robustness/r5-local/summary.md`;
lab #31 from `exports/robustness/local-1/`). "Sign" is given for the check named.

| Item | Holds | Lab #31 | Sign (check) | Lab #31 sign |
| --- | --- | --- | --- | --- |
| Fact 1 amplification | 83% | 97% | 100% (both) | 100% |
| Fact 2 outrage learning | 88% | 82% | 100%, 97% (rise, learning) | 97% (both) |
| Fact 3 chronological feed (now 4 checks) | 60% | 94% (2 checks) | 100% (all) | 100% |
| Fact 4 forwarding limits | 100% | 100% | 100% | 100% |
| Fact 5 harassment exit | 100% | 94% | 100% | 99% |
| Fact 6 deactivation | 45% | 50% | 71% | 76% |
| Fact 7 notifications off | 95% | 97% | 60% (wellbeing) | 70% |
| H1 calmer feed | 97% | 89% | 76% (engagement) | 87% |
| H2 survey misranks | 65% | 65% | 65% (survey) | 65% |
| H3 strict classifier | 100% | 100% | 100% | 100% |
| H4 trap solution | 100% | 100% | 99% (engagement) | 100% |
| H5 notifications | 96% | 96% | 79% (wellbeing) | 69% |
| H6 lead lasts | 76% | 87% | 76% | 87% |
| H7 report trust | 98% | 100% | 98% | 100% |
| H8 see-less backfire | 100% | 100% | 100% | 100% |
| V1 | 40% | 23% | 67% (toxic) | 41% |
| V2 | 15% | 10% | 100% (informedness, now negative) | 83% (positive) |
| V3 | 10% | 23% | 100% (time) | 96% |
| V4 | 100% | 95% | 100% | 95% |
| V5 | 2% | 1% | 100% (extremes) | 100% |
| V6 | 10% | 8% | 86% (ranking), 100% (likes) | 81%, 100% |
| V7 | 0% | 0% | 84% (at 1.000) | 98% |
| V8 | 92% | 78% | 100% | 100% |
| V9 | 76% | 71% | 95% | 88% |
| V10 | 8% | 0% | 92% (informedness, positive) | 99% |

Fact 3 lost ground through its polarisation check, not through its new exposure checks (95% and 100%): at the new
defaults the polarisation difference at day 90 lies at 0.037 to 0.041 against a bound of 0.05 (lab #31: 0.028), and
36% of local worlds exceed it, with a seed share of 0.02, so the spread is the worlds'. Fact 1's failures lie above its
upper bound of 0.6 (the default world gives 0.53 on two seeds), 13 of 17 within noise. Signs below 75%: fact 7's
wellbeing effect and H5's wellbeing difference sit near zero, fact 6 is a knife edge (5.4), H2's survey follows the
survey's weights, and V1's toxic share has little effect beyond noise.

### 5.2 A world's failure against seed noise

The failure classes of 2.6, from the summary, at item level (lab #31's from `exports/validation-analysis/analysis.txt`,
on 2 seeds per world throughout). "Seed share" is for the check named.

| Item | Failing | Noise-compatible | Clear | Seed share (check) | Lab #31: failing, compatible, clear |
| --- | --- | --- | --- | --- | --- |
| Fact 1 amplification | 17 | 13 | 4 | 0.08 (outrage) | 3 to 6 failing, all within noise |
| Fact 2 outrage learning | 12 | 12 | 0 | 0.41 (learning) | 18, 18, 0 |
| Fact 3 chronological feed | 40 | 16 | 24 | 0.02 (polarisation) | 3 to 6 failing |
| Fact 6 deactivation | 55 | 27 | 28 | 0.06 | 50, 20, 30 |
| H2 survey misranks | 35 | 17 | 18 | 0.06 (survey) | 35, 19, 16 |
| H6 lead lasts | 24 | 12 | 12 | 0.03 | 13, 7, 6 |
| V1 | 60 | 41 | 19 | 0.32 (toxic) | 77 failing |
| V2 | 85 | 21 | 64 | 0.13 (informedness) | 90, 12, 78 |
| V3 | 90 | 53 | 37 | 0.84 (time) | 77 failing |
| V5 | 98 | 66 | 32 | 0.82 (extremes) | 99, 43, 56 |
| V6 | 90 | 5 | 85 | 0.03 (likes) | 92, 15, 77 |
| V7 | 100 | 0 | 100 | 0.94 | 100, 0, 100 |
| V8 | 8 | 4 | 4 | 0.10 | 22, 9, 13 |
| V9 | 24 | 24 | 0 | 0.59 | 29, 29, 0 |
| V10 | 92 | 36 | 56 | 0.38 (informedness) | 100, 19, 81 |

H1, H5, H7, fact 7 and V4 fail in 0 to 5 worlds each, all within noise. V7's seed share of 0.94 reflects that most
worlds sit exactly at 1.000 on both seeds; its failures are all clear.

### 5.3 The pooled seed SD

A concern from code review: the classes use one seed SD per check, pooled over all non-degenerate worlds, so a check
whose ratio has a denominator near 0 in some worlds may carry an inflated SD into every world. From the per-seed effects
in each run's `checks.csv` (`exports/analysis-r5/pooled_sd.py`, output `pooled_sd.txt`), each check's pooled SD,
recomputed as the code does, was compared with the median of the within-world SDs. One correction is needed first: the
SD of 2 normal values has a median of 0.674 sigma (of 4 values, 0.888 sigma), so with 2 seeds the pooled SD exceeds the
raw median by a factor of about 1.48 even when every world has the same noise. The table uses the median divided by
that factor ("robust SD"); `pooled_sd.txt` also gives the raw median and the classes it would give.

- **Local:** the pooled SD lies within 0.77 to 1.31 times the robust SD for every check but V6's ranking ratio (1.56).
  Recounted with the robust SD the classes barely move (V5 66 to 56 of 98 noise-compatible, V3 53 to 51, V6 5 to 6,
  fact 1 13 to 11). The local classes of 5.2 stand.
- **Plausible:** six checks lie above 1.5: H1's engagement (1.59), H3's toxic ratio (1.81, no failures), V5's
  extremes (2.14), V6's ranking (2.07), V8 (1.62) and V9 (1.75). Recounted: H1 21 to 15 of 43 noise-compatible, V5 150
  to 103 of 176, V6 28 to 22, V8 6 to 4, V9 49 to 46.
- **Revised ranges:** most checks lie above 1.5, and some far above: H6 12.8, V2's affective polarisation 9.2, V8 5.6,
  H3's toxic ratio 5.4, V2's later use 4.9, fact 3's toxic ratio 4.5, V5's half 4.3, H7 3.9, V3's time 3.3. These are
  ratios and relative changes whose denominators near 0 in some worlds (H6's 95% quantile is 6.3). Recounted at item
  level:

| Item (revised ranges) | Failing | Compatible, pooled SD | Compatible, robust SD |
| --- | --- | --- | --- |
| H6 lead lasts | 80 | 79 | 11 |
| V3 | 154 | 151 | 31 |
| V1 | 167 | 81 | 27 |
| V2 | 171 | 42 | 13 |
| V5 | 128 | 78 | 46 |
| V8 | 41 | 22 | 3 |
| Fact 2 outrage learning | 160 | 25 | 10 |
| H2 survey misranks | 84 | 31 | 18 |
| H7 report trust | 84 | 82 | 70 |
| V10 | 176 | 39 | 21 |

The pooled column applies the code's rule with non-finite per-seed values skipped, so it also classes the worlds that
the summary leaves unclassed. A second defect shows in the same place: `stats::pooled_sd` skips a world whose per-seed
values contain NaN but not one with an infinite value, which makes the pooled SD NaN and leaves every failing world of
that check unclassed: V8 in both global designs (14 infinite per-seed values across the revised ranges, 1 across the
plausible ones) and V2's later use across the revised ranges, so `summary.md` reports V8's 41 and 9 failures and V2's 61
with no class. After these runs `pooled_sd` and `variance` skip infinite values too (hub #25), and the summary names
the pooled SD's inflation; the robust SD itself is recommendation 3 of 8.2.

In short: the local classes are sound; across the plausible ranges the pooled SD overstates noise for V5's extremes
and V6's ranking ratio; across the revised ranges "noise-compatible" counts are inflated for most items, so section 6
quotes the robust counts where they differ, and the global failures of H6, V1, V2, V3 and V8 are mostly clear
failures, not noise. H7's global failures stay mostly within noise with either SD.

### 5.4 What is robust and what hinges on the defaults

- **Robust to +-25%:** facts 4, 5 and 7, H1, H3, H4, H5, H7 and H8 hold in 95% to 100% of worlds, their few failures
  within noise. Fact 1 keeps its direction everywhere and its size in 83%, failing above its upper bound.
- **Fact 2 hinges on seed noise, not on the world:** it holds in 88% of worlds and every failure is within noise of
  the learning check's lower bound, where the default is undecided.
- **Fact 3's polarisation null hinges on the defaults:** it fails in 36% of worlds with little seed noise, while the
  time and exposure checks hold in 95% to 100%.
- **Fact 6 hinges on the defaults:** 45% hold, 28 of 55 failures clear, the 5% to 95% range of world effects -0.14 to
  0.43 against a band of 0.02 to 0.25. The `time_cost` calibration pins one combination of several assumptions on a
  knife edge, as lab #31 found (50%).
- **H2 hinges on a measurement assumption:** the survey ranks the engagement ranking above Q&B in 65% of worlds; its
  drivers are the survey's weights (6.4). The wellbeing half holds in every world.
- **H6 hinges on habit:** 76% hold (lab #31: 87%), 12 of 24 failures clear; the plausible run names habit formation and
  social proof (6.4). H1's engagement check, which lab #31 found failing in 11 worlds, now fails in 3, within noise.
- **Out of sample:** V4 (100%), V8 (92%) and V9 (76%, its failures all noise) are stable. The failures of V2, V6, V7
  and V10 are robust (56 to 100 clear failures each); V3's and V5's failures are robust in direction and noisy in size.

## 6. Sensitivity (plausible and global)

### 6.1 Hold shares across the plausible ranges

200 worlds, 1 degenerate (`exports/robustness/r5-plausible/summary.md`). The plausible ranges are new with lab #35, so
there is no lab #31 value; they stand for beliefs about the assumptions (2.6), with the bot farms adaptive throughout.

| Item | Holds | Sign (check) | Item | Holds | Sign (check) |
| --- | --- | --- | --- | --- | --- |
| Fact 1 | 71% | 100%, 98% | H1 | 78% | 100% (harm), 82% (engagement) |
| Fact 2 | 48% | 98%, 69% (learning) | H2 | 63% | 63% (survey) |
| Fact 3 | 29% | 100% (time) | H3 | 100% | 100% |
| Fact 4 | 100% | 100% | H4 | 100% | 100% (harm), 95% (engagement) |
| Fact 5 | 98% | 100% | H5 | 93% | 100% (engagement), 65% (wellbeing) |
| Fact 6 | 28% | 69% | H6 | 80% | 80% |
| Fact 7 | 99% | 100% (time), 60% (wellbeing) | H7 | 99% | 99% |
| V4 | 94% | 94% | H8 | 96% | 96% |
| V8 | 95% | 100% | V9 | 75% | 93% |
| V1 | 35% | 52% (toxic) | V2 | 18% | 92% (informedness, negative) |
| V3 | 8% | 98% (time) | V5 | 12% | 93% (extremes) |
| V6 | 12% | 78% (ranking), 100% (likes) | V7, V10 | 0%, 11% | 55% (at 1.000), 87% (informedness) |

What holds across plausible worlds:

- **Holds almost everywhere (93% to 100%):** forwarding limits (fact 4), harassment exits (fact 5), notifications off
  (fact 7), the strict classifier's trade-off (H3), the trap solution (H4), notifications raising engagement (H5),
  report trust (H7) and the see-less backfire under brigading farms (H8); out of sample V4 and V8. Q&B cuts harm by
  more than half in every plausible world (H1's harm check, 100%).
- **Holds in most worlds:** fact 1 (71%; direction 100%, failing above 0.6), H1 (78%: its engagement cost leaves the
  band where habit forms fast), H6 (80%), V9 (75%).
- **Hinges on the assumptions:** H2's survey (63%), fact 2's learning check (52%, sign 69%), fact 3 (29%, through its
  polarisation check, 42%; the exposure checks hold in 79% and 98%) and fact 6 (28%).
- **Fails robustly:** V2, V3, V5, V6, V7 and V10 hold in 0% to 18% of worlds; V1 in 35%, through its toxic share.

### 6.2 Hold shares across the revised ranges

240 worlds, 62 of them degenerate (lab #31: 49), so 178 non-degenerate (`exports/robustness/r5-global/summary.md`;
lab #31 from `exports/robustness/global-1/`). Hold shares over all worlds and over the non-degenerate ones; "sign" for
the main check over all worlds (a world without a value counts as not sharing the sign).

| Item | All | Non-degen. | Lab #31 | Sign | Lab #31 sign |
| --- | --- | --- | --- | --- | --- |
| Fact 1 | 44% | 44% | 48%, 50% | 100%, 73% | 100%, 71% |
| Fact 2 | 8% | 10% | 9%, 9% | 45%, 24% | 34%, 29% |
| Fact 3 (now 4 checks) | 3% | 3% | 21%, 20% | 92% (time) | 91% |
| Fact 4 | 92% | 96% | 92%, 96% | 100% | 100% |
| Fact 5 | 36% | 48% | 32%, 40% | 81% | 82% |
| Fact 6 | 8% | 10% | 7%, 7% | 88% | 85% |
| Fact 7 | 85% | 94% | 87%, 92% | 94% (time) | 96% |
| H1 | 27% | 36% | 26%, 32% | 92% (harm) | 92% |
| H2 | 47% | 53% | 46%, 50% | 53% (survey) | 52% |
| H3 | 98% | 99% | 95%, 98% | 100% | 100% |
| H4 | 78% | 86% | 77%, 82% | 81% (harm) | 81% |
| H5 | 87% | 84% | 82%, 81% | 100% (engagement) | 98% |
| H6 | 53% | 55% | 56%, 57% | 53% | 56% |
| H7 | 53% | 53% | 57%, 59% | 53% | 57% |
| H8 | 42% | 48% | 31%, 27% | 42% | 31% |
| V4 | 66% | 74% | 61%, 68% | 66% | 61% |
| V8 | 63% | 77% | 56%, 68% | 82% | 72% |
| V9 | 52% | 54% | 48%, 52% | 75% | 75% |
| V1 | 6% | 6% | 6%, 7% | 30% (toxic) | - |
| V2 | 3% | 4% | 2%, 2% | 42% (informedness, negative) | 83% (positive) |
| V3 | 14% | 13% | 25%, 23% | 70% (time) | - |
| V5 | 22% | 28% | 4%, 5% | 46% (extremes) | 82% |
| V6 | 4% | 4% | 5%, 6% | 65% (ranking), 98% (likes) | 59%, 100% |
| V7 | 0% | 0% | 2%, 3% | 41% (at 1.000) | 39% |
| V10 | 2% | 1% | 0%, 0% | 67% (informedness, positive) | 77% |

The revised ranges are ranges to explore, not beliefs: they include worlds where a mechanism is off or doubled, and
model.md section 15 names failing worlds inside them. Low hold shares there map where a mechanism operates; they do not
by themselves refute it. The bot strategy now matters more than any single number for four claims
(`exports/analysis-r5/global_extra.txt`, non-degenerate worlds):

| Claim | Holds, adaptive bots | Own-side | Shared pool | Median effect, adaptive against the others |
| --- | --- | --- | --- | --- |
| H8 see-less backfire | 91% (59 of 65) | 25% | 22% | +0.042 against -0.004 and -0.007 |
| H7 report trust | 80% | 27% | 45% | -0.58 against +0.04 and +0.02 |
| H6 lead lasts | 68% | 48% | 48% | 1.40 against 0.99 and 0.98 |
| H1 calmer feed | 25% | 46% | 40% | engagement -0.29 against +0.01 and +0.02 |

Only adaptive farms brigade (model.md 3.5), so H8 and H7, which are about brigading, cannot hold in two thirds of these
worlds by construction. Lab #31 already had H8 at 64% with adaptive bots against 9% and 6% (`global-1`), which its
reading in terms of the Clickbait penalty missed.

- **Robust across the ranges:** forwarding limits (96% non-degenerate), notifications off (94%), the strict
  classifier's trade-off (H3, 99%), the trap solution (H4, 86%), notifications raising engagement (H5's engagement
  check, 100%). Q&B cuts harm in 92% of worlds (95% of the non-degenerate), by "far more" (at least half) in 66% of the
  non-degenerate ones; the engagement ranking shows more outrage than the chronological feed in every world.
- **Hinges on the ranges:** fact 2's rise has the default's sign in 45% of worlds and its learning check in 24%;
  fact 3's polarisation check holds in 24% of non-degenerate worlds; fact 6 lies above its band in most (median 0.80 SD
  against a band of 0.02 to 0.25); H2, H6, H7 and H8 split about evenly, H7 and H8 by the bot strategy.
- **Out of sample:** no V finding that fails at the defaults passes in more than 28% of non-degenerate worlds. V2's
  informedness sign is negative in 42% of all worlds (55% of the non-degenerate), and with adaptive bots its median is
  positive (+0.53 SD against -0.23 and -0.16 with the other strategies). V7 stays at 1.000 in 41% of worlds and below
  it, never in the band, in the rest.

### 6.3 Degenerate worlds

62 of the 240 revised worlds (26%; lab #31 49, 20%) and 1 of the 200 plausible worlds are degenerate; no local world is
(`exports/analysis-r5/global_extra.txt`). The platform empties when habit forms slowly and churn is high, as at lab #31:
31 of the 61 worlds in the lowest quartile of `habit_gain` (below 0.028) are degenerate, against 6 of 59 in the highest,
and 26 of 59 in the top quartile of `churn_base` (above 0.075) against 3 of 61 in the bottom (Spearman with the
degenerate flag -0.36 and +0.30). New since lab #31: low `outrage_appeal` (-0.28; 29 of the 61 worlds in its lowest
quartile, below 0.75, are degenerate) and `leave_threshold` (+0.23), which now varies from -0.1 to 0.1. Both runs use
sample seed 1, but the new assumptions change the hypercube's columns, so their worlds are not the same; the higher
share may come from the newly varied assumptions or from `base_session_items` being held, which no run here separates.

### 6.4 Drivers of the key results

PRCC over the non-degenerate worlds, the strongest two or three by absolute value, all beyond 2 SE (plausible: 199
worlds, SE 0.083; revised: 178 worlds, SE 0.091); R2 is the adjusted R2 of the rank regression (2.6). Lab #31's top
drivers, from 191 revised worlds with `base_session_items` varied, are in its version of this section.

Across the plausible ranges:

| Result | Top drivers (PRCC) | R2 |
| --- | --- | --- |
| Fact 1, outrage | `outrage_appeal` +0.90, `style_quality_gap` -0.59 | 0.88 |
| Fact 2, rise | `reflective_quality_weight` -0.52, `exposure_contagion` +0.52 | 0.67 |
| Fact 2, learning | `learning_rate` -0.58, `bot_share` -0.43 | 0.58 |
| Fact 3, polarisation | `outrage_appeal` +0.95, `polarisation_rate` +0.92 | 0.96 |
| Fact 6 | `reflective_quality_weight` -0.86, `bot_share` +0.82 | 0.91 |
| H1, engagement | `habit_gain` -0.87, `outrage_appeal` -0.69 | 0.86 |
| H2, survey | `survey_impulsive_weight` +0.88, `survey_habit_weight` +0.79 | 0.84 |
| H6 | `habit_gain` +0.87, `outrage_appeal` +0.68, `social_proof` -0.57 | 0.86 |
| H7 | `archetype_shares.troll` -0.52, `bot_brigading` +0.44 | 0.62 |
| H8 | `bot_share` +0.64, `outrage_appeal` -0.63 | 0.74 |
| V2, informedness | `style_quality_gap` -0.80, `bot_share` +0.77 | 0.85 |
| V5, extremes | `bot_share` +0.55, `habit_gain` -0.52 | 0.57 |
| V6, likes | `outrage_appeal` -0.84, `bot_share` -0.74 | 0.89 |
| V7 | `outrage_appeal` -0.62, `arousal_appeal` -0.35 | 0.47 |

Across the revised ranges:

| Result | Top drivers (PRCC) | R2 |
| --- | --- | --- |
| Fact 1, outrage | `outrage_appeal` +0.86, `choice_temperature` -0.56 | 0.76 |
| Fact 2, rise | `learning_rate` -0.41, `arousal_appeal` -0.35, `exposure_contagion` +0.33 | 0.45 |
| Fact 2, learning | `learning_rate` -0.54, `outrage_appeal` -0.52 | 0.63 |
| Fact 3, polarisation | `outrage_appeal` +0.74, `polarisation_rate` +0.72 | 0.72 |
| Fact 6 | `time_cost` +0.57, `outrage_appeal` +0.56 | 0.70 |
| H1, engagement | `social_proof` +0.51, `stop_bias` -0.42 | 0.44 |
| H2, survey | `survey_impulsive_weight` +0.47, `survey_habit_weight` +0.45 | 0.49 |
| H6 | `social_proof` -0.51, `stop_bias` +0.42 | 0.44 |
| H7 | `bot_farm_floor` +0.31, `style_quality_gap` -0.29 | 0.31 |
| H8 | `outrage_appeal` +0.41, `clickbait_penalty` -0.29 | 0.62 |
| V2, informedness | `style_quality_gap` -0.61, `outrage_appeal` +0.43 | 0.71 |
| V5, extremes | `habit_gain` -0.34, `evasion_cost` -0.26 | 0.30 |
| V6, likes | `outrage_appeal` -0.85, `outgroup_like_cut` -0.55 | 0.82 |
| V7 | `outrage_appeal` -0.71, `habit_gain` -0.45 | 0.61 |

H8's Clickbait penalty, lab #31's top driver (-0.56), is no longer one: -0.06 across the plausible ranges, -0.29 across
the revised ones, behind the bot strategy (6.2). H2, H6 and fact 6 keep lab #31's drivers. Fact 2's learning check has
a negative PRCC with `learning_rate` in both designs (lab #31 had -0.58 too): faster learning gives a smaller rise over
the arm without learning, which these runs do not explain.

Over the 48 required checks with a PRCC, six assumptions take 82 of the 144 top-three places across the revised ranges:
`outrage_appeal` (27), `arousal_appeal` (13), `stop_bias` (12), `habit_gain` (11), `style_quality_gap` (10) and
`reflective_quality_weight` (9); across the plausible ranges six take 98: `outrage_appeal` (23), `habit_gain` (19),
`bot_share` (15), `style_quality_gap` (14), `stop_bias` (14) and `reflective_quality_weight` (13)
(`exports/analysis-r5/top3.txt`; lab #31: 84 of 138, with `base_session_items` fourth at 13). The model's results rest
mostly on how strongly hostility and arousal pull, how habit forms and how quality is valued.

A typical check has 13 of the 53 assumptions beyond 2 SE across the plausible ranges (4 to 22) and 9 across the revised
ones (2 to 18), against 2.4 expected by chance alone (4.55% of 53). Most checks therefore respond to several
assumptions at once, and a few beyond 2 SE in any one check could be chance.

### 6.5 Weak correlations

Required checks whose rank regression explains little across the revised ranges (adjusted R2 at or below 0.3): V1's
toxic share (0.09), V2's later use (0.12), forwarding limits (0.24), notifications off's sessions (0.25), H4's
engagement (0.28), V3's time (0.28), V10's informedness (0.22) and polarisation (0.29), and V5's extremes (0.30); across
the plausible ranges only V4 (0.03), H3's toxic ratio (0.20) and V3's time (0.25), with the known limit, V3's own
toxicity (0.10). Seed noise is a large part of their spread where their seed shares are high (V3's time 0.89, V4 0.44,
V1's toxic share 0.43 across the revised ranges). Where PRCC and Spearman disagree, the assumption matters only once the
others are held fixed (for example `time_cost` for fact 6 across the plausible ranges, PRCC +0.40 against Spearman
+0.04).

### 6.6 The documented ranges (lab #31, not rerun)

Lab #31 ran 64 worlds on section 11's ranges as written before the revision (`exports/robustness/global-2-documented/`).
Its degenerate share was the same as the revised run's (20%), caused mostly by `leave_threshold` (-0.5 to 0.5; Spearman
+0.52 with the degenerate flag), and the polarisation checks failed more often, because those ranges reach 13 times the
calibrated `polarisation_rate`. Milestone 2 replaced that contrast by the plausible ranges and gave `leave_threshold` a
judgement range of -0.1 to 0.1 instead. Its numbers are in lab #31's version of this section.

## 7. Overall assessment

The lab fits what it was tuned on, except the toxic share, which the new exposure contagion pushed above its bound and
which was left there. Out of sample it gets 12 of 21 checks and the direction of 12 of 17 directional findings, a
little better than at lab #31 (11, and 11 of 17), and half of its genuine tests pass (7 of 14, against 5). Milestone 2
fixed what it set out to fix where the fix was a design or a prevalence (V5's partisan half, fact 3's exposure checks,
V10's misinformation, V2's sign), and not where it was a new mechanism at in-sample strength (V3's own toxicity) or a
design choice kept by judgement (V7).

**It can be trusted for** the direction of the first-order trade-offs it was built around, which now survive small noise
and nearly all plausible worlds tried: an engagement ranking amplifies outrage and costs reflective value (fact 1: more
outrage in every world of all three designs, lower reflective value in 77% to 100%), a calmer feed cuts harm by more
than half (H1's harm check, 100% local and plausible, 95% of non-degenerate revised worlds in direction), a strict
classifier trades toxicity for wrongful sanctions (H3, 99% to 100%), the trap solution cuts harm and keeps engagement
(H4, 86% to 100%), forwarding limits shrink cascades (fact 4, 96% to 100%), notifications raise engagement without
raising wellbeing (H5, 84% to 96%), and switching people from a ranked to a chronological feed cuts their time and
shifts their exposure from incivility toward misinformation, in the field's sizes (fact 3's time and exposure checks,
79% to 100% local and plausible; V1's and V10's exposure checks). The see-less backfire (H8) and report trust (H7) are
robust wherever bot farms brigade, and absent where they do not.

**It is uncertain about:** the engagement cost of a calmer feed and whether the engagement ranking's lead lasts, which
turn on habit formation and social proof (H1's engagement check, H6); the survey's misranking, which turns on the
survey's assumed weights (H2); the share of outrage's rise that learning from likes carries, now that the contagion
carries most of it (fact 2); whether three months of a different feed leave polarisation unchanged, which now sits close
to its bound (fact 3); and the size of the break's wellbeing gain, a calibration identity on a knife edge (fact 6).

**It cannot be trusted for:**

- informedness: a break lowers it, as in the field, but by an amount whose size in SDs depends on how alike people's
  informedness is (V2), and removing reshares raises it, against the field, in 87% to 92% of local and plausible
  worlds (V10);
- how reach is distributed across partisanship: the most partisan authors are amplified 1.8 times as much as the middle
  on the field's own design (V5), and across the revised ranges the partisan half's amplification collapses with many
  adaptive bots, unexplained;
- community notes: the realistic feed hides noted posts by design, so no share follows a note (V7); without the
  penalty the model gives the field's 0.85, but that comparison has now been seen;
- cross-group liking (V6) and how hiding toxic posts changes what people write or how long they stay (V3, V1's toxic
  share): the directions now show, the sizes are a tenth to a hundredth of the field's;
- toxicity prevalence (0.209 under the default design, 0.105 under the integrity preset against the field's 3.2%) and
  the regretted share, which lies a factor of about 36 below its held-out band.

## 8. Recommendations

### 8.1 Lab #31's eight recommendations and what they achieved

1. **Rebalance informedness (lab #32).** Done: misinformation prevalence calibrated in sample to Guess et al. (2023),
   2.5% of impressions under the integrity preset (now 0.0247, from 0.130), by moving Clickbait's truth from 0.35 to
   0.60 and Outrage's from 0.60 to 0.66; the belief constants kept. Achieved: V2's informedness took the field's sign
   (raw -0.0129, from +0.0078) but fails on its SD scale (4.3); V10's kept the wrong sign (+0.0014 raw); V1's and V10's
   misinformation checks pass. Side effects: the misinformation surge scenario needed `bot_truth_shift`; the most
   engaging design lost its harm lead (model.md 13.3).
2. **A mid-run feed switch on a shared follow graph (lab #33).** Done: a `chronological` feed-experiment kind; V5 and
   fact 3's two exposure checks moved onto it, fixed before any run. Achieved: V5's partisan half passes (1.324, from
   0.959); its extremes fall from 2.251 to 1.803 but still fail; fact 3's exposure checks pass (0.757, 1.682, from 0.336
   and 0.640) and became required. A measurement-only split of reach into in-network and recommended impressions is not
   among the measures `validate` prints.
3. **Notes that can arrive in time (lab #34).** Done: a lognormal note delay with a median of 16 hours and a time of
   day per session; the penalty kept and documented as a design choice. Achieved: V7 unchanged (1.000). This update
   shows why: the penalty, not the timing (4.3; 0.842 without it).
4. **Ranges and noise design (lab #35).** Done: `base_session_items` held, `leave_threshold` given a judgement range,
   plausible ranges, 4 seeds for noisy items, failure classes, a PRCC significance mark and R2. Achieved: a plausible
   picture (6.1); `base_session_items` no longer dominates the drivers. Found: the pooled SD inflates the global
   noise classes and an infinite value disables them (5.3); the degenerate share rose to 26%.
5. **Out-group liking (lab #36).** Done: `outgroup_like_cut` is a world assumption, sampled, and `P_like` is decomposed.
   Achieved: the diagnosis (4.3: the in-group appeal of hostility is the largest gap, the cut second and negligible
   within its plausible range); no model change was intended, and V6's likes still fail (0.648).
6. **State what the exhibits rest on (lab #37).** Done in this round, from the runs of this update: model.md 13.3 and
   13.4 and the app's texts (about.rs, lab/mod.rs, atlas.rs).
7. **Exposure shapes posting (lab #38).** Done: `exposure_contagion` 0.28, set to Cheng et al.'s causal relative risk
   of 1.6. Achieved: V3's own toxicity takes the field's direction across worlds but about a hundredth of its size.
   Side effects: the toxic share rose to 0.209, above its target; fact 2's rise is now mostly contagion and its learning
   check is confounded.
8. **Raw scales and ratio conventions (lab #39).** Done: the SD used and raw change printed, V6 and V8 pooled, one ratio
   convention, V5's reach with and without removed posts. Achieved: the Jensen bias proved small (V8 2.813 to 2.784);
   the printed SD exposed informedness's small spread (0.023), which explains V2's new failure.

### 8.2 New recommendations

In order of priority. "Model" changes the dynamics, "tooling" the CLI or measurement, "documentation" model.md or the
exhibits. A change tuned on a V check makes that check in-sample; each item says how to avoid that.

1. **Calibrate toxicity prevalence (model; recalibration).** What: an in-sample target fixed before tuning, the 3.2% of
   Facebook feeds that Guess et al. (2023) estimated as uncivil (1.6% of Instagram's), under the integrity preset, with
   a rule naming the constant (the hostile styles' toxicity profiles, model.md 4.1, as lab #32 used truth profiles) and
   how the model's toxicity above 0.5 maps onto a classifier's label of incivility. Why: the default design's toxic
   share, 0.209, misses its target and the integrity preset's 0.105 is three times the field's (3.2). Expected effect:
   the target back in its band, as a consequence to check, not the target. Side effects: facts 1 to 3, H3 and H8, the
   scenarios and the contagion's strength need rechecking, and V3's hiding threshold must be re-set as a manipulation
   check (7% hidden) before V3 is rerun.
2. **Resolve what V7 tests (documentation, then tooling).** What: record that V7 at the integrity preset measures the
   design choice to hide noted posts (`noted_penalty` 1.0), and decide from documentation of how the field's platform
   treats noted posts in ranking, not from 4.3's numbers, whether V7 is judged under a notes design without the
   penalty, declared as a change made after the result was seen, or reported as not testable under the integrity
   preset. Pool V7 over the summary window like V6 and V8, since it rests on some 34 noted posts per seed. Why: the
   penalty alone makes V7 1.000; without it the model gives 0.842 (4.3). No recalibration.
3. **Robust seed noise in the failure classes (tooling).** What: in `seed_noise`, replace the pooled SD by a robust one
   (the median of the within-world SDs divided by its normal consistency factor, 0.674 for 2 seeds and 0.888 for 4, or a
   pooled SD over trimmed worlds); use the same SD for the seed share. Skipping non-finite per-seed values, which one
   infinite value needed to keep V8 and V2's later use classed, is done (hub #25, after these runs). Why: across the
   revised ranges the pooled SD makes most failures of H6, V1, V2, V3 and V8 look like noise (5.3). Expected effect: the
   classes of 5.3's robust column. No recalibration; the classes need only the per-seed effects that `checks.csv`
   already holds.
4. **Informedness on a field scale (tooling and documentation; model for V10).** What: fix before any rerun how V2 and
   V10 compare informedness with a knowledge index measured in SDs of a heterogeneous population (for example judge the
   raw change against a field raw change where one exists, or record the SD check as a loose mapping judged in
   direction only); then examine why reshares carry no informative gain (V10), as a model question. Why: V2 fails only
   because informedness's spread fell to 0.023 (4.3), and V10 keeps the wrong sign in 87% to 92% of local and plausible
   worlds. Side effects: none for the dynamics if only the scale changes.
5. **Diagnose V5 (tooling).** What: the in-network against recommended split of the treated's impressions that lab #31
   recommended, and a diagnostic of the collapse of the partisan half's amplification with many adaptive bots across
   the revised ranges (4.3, 6.2). Why: V5's extremes fail robustly in direction (1.80) and the collapse is unexplained.
   Measurement only.
6. **Separate learning from contagion in fact 2 (tooling and documentation).** What: report fact 2's learning check
   also with the contagion held off in both arms, and find why faster learning gives a smaller learning effect (PRCC of
   `learning_rate` -0.58 and -0.54, 6.4). Why: the contagion carries most of the rise and confounds the learning check,
   whose sign holds in 69% of plausible worlds. A change of the check's design is a declared change to an in-sample
   fact.
7. **Fact 3's polarisation margin (documentation; recalibration check).** What: at the next fixed point, check
   `polarisation_rate`'s rule against fact 3's polarisation check with its new margin (0.037 against 0.05). Why: the
   contagion raised the Outrage share and with it the day-90 polarisation; the check fails in 36% of local and 58% of
   plausible worlds with little seed noise (5.1, 6.1).
8. **The size of exposure effects on writing (model; out-of-sample kept).** What: examine which channels between what
   people see and what they write the model lacks (replies drawn by hostile items, a dose that builds over days), with
   any new strength set on in-sample sources, not on V3. Why: the contagion at Cheng et al.'s strength gives V3's
   direction at a hundredth of its size (4.3).
9. **More seeds where noise decides (tooling).** What: 8 seeds per world for V3's time, V5's extremes and V9, whose
   local seed shares remain 0.84, 0.82 and 0.59 at 4 seeds, and 8 seeds at the defaults for V9's verdict. Why: their
   verdicts and drivers cannot be read from 4 seeds (5.2).
10. **Degenerate worlds in the revised ranges (documentation).** What: report the revised-range hold shares beside the
    plausible ones in every summary of results, and note which revised ranges (`habit_gain` from 0, `churn_base` up to
    0.1) mostly explore emptied platforms. Why: 26% of the revised worlds are degenerate (6.3).

## 9. Limits of this validation

- **Loose mappings.** Several checks map a field measure onto a model analogue that is only roughly the same: clicks
  on hard news onto likes of in-network posts (V6), news knowledge onto informedness, whose spread is far smaller (V2,
  V10), untrustworthy-source exposure onto posts with `truth` below 0.5 (V1, V10, fact 3), an accuracy prompt onto
  share friction (V4), politicians' and news sources' amplification onto authors ranked by a partisanship trait (V5),
  and notes on one platform onto the integrity preset's demoted notes (V7).
- **Field results from other settings.** Some bands come from studies of a whole phone or another product: Dekker et
  al.'s notifications switch-off was phone-wide (fact 7), Pandora's ad load elasticity comes from music streaming
  (V9). They are used here for one platform.
- **Dose mapping.** V1's and V3's doses were matched as manipulation checks at lab #31's defaults and not re-set after
  milestone 2 (V3 now hides 7.9% against a target of 7%); under the engagement ranking the same settings give other
  doses (V3: 0.224 hidden), so the engagement column is not at the field's dose.
- **Spillover.** The feed experiments treat 20% of the regular users (V1, V3, V5, V10, fact 3), against a negligible
  share of a platform's users in the field studies. In the model the treated make up a fifth of the audience, so what
  they stop seeing, liking and sharing changes what everyone else sees and does, which feeds back to the treated; their
  change includes these knock-on effects, which the placebo run, with nobody treated, does not remove. V2's break also
  takes 20% of the regular users off the platform, and fact 6's break all of them.
- **SD scaling.** Effects in SDs depend on how heterogeneous the model's people are: informedness's SD is 0.023 at the
  defaults, so V2's verdict is one of scale (4.3).
- **Unverified sources.** V10 rests on the abstract of Guess et al. (2023b) only, and V3 on a working paper
  (Beknazar-Yuzbashev et al., CAGE WP 741/2025, May 2026 draft); both judge directions with wide bands. V6's figures
  come from the authors' blog summary of Bakshy et al. (2015), not the paper (`exports/research-validation/notes.md`).
- **Design changes after a failure was seen.** Lab #33 moved V5 and fact 3's exposure checks onto the field's design
  after lab #31 had seen them fail, with the rule fixed before any run; their passes are less independent than a
  first-time pass. 4.3's V7 runs are now known, so any change to V7's design is post hoc.
- **Seeds per world.** Per-world verdicts carry seed noise (2 seeds, 4 for the noisy items); the failure classes use a
  pooled SD that is sound locally and inflated across the revised ranges (5.3).
- **Coverage.** The checking habits, the interruption cost and `base_session_items` are never varied, and the bot
  strategy is held at adaptive across the plausible ranges, though four claims depend on it (6.2). Design and run
  settings (population size, run length, post lifetime) are not varied, and results depend on the run length
  (model.md 16). The documented ranges were not rerun.
- **The diagnostics.** 4.3's V7 runs change one design lever on 8 seeds; lab #31's diagnostics of V2, V5 and V6
  (`exports/validation-analysis/diag/`) were not repeated at the new defaults and are cited only as lab #31's.
