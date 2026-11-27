# Validation of the incentive lab model (lab #31)

This document records how well the model of [model.md](model.md) reproduces empirical findings, and how stable and
sensitive its results are under noise on the world assumptions. It was written on 2026-11-27 from runs of the release
build of that day (the working tree on top of b288a86, with lab #31's measurement additions and no change to the
dynamics). Every number names the file it comes from; the files sit under the gitignored `exports/` folder and the
commands of 2.8 regenerate them.

## 1. Purpose

The lab is a toy model for intuition about incentive design on one platform (model.md, section 1). Its results are
only as strong as two things this document measures:

- **Fit and accuracy.** Fit is how well the model reproduces what it was tuned on (the stylized facts, the
  calibration targets and the headline claims). Accuracy is how well it reproduces field findings it was never tuned
  on, with bands fixed before the model was run on them.
- **Stability and sensitivity.** Stability is whether a result survives small noise (about +-25%) on every world
  assumption around the defaults. Sensitivity is how a result moves across each assumption's whole range, and which
  assumptions drive it.

The analysis ends in recommended modifications (section 8) and in the limits of this validation (section 9).

## 2. Method

### 2.1 In-sample and out-of-sample

In-sample checks were used in tuning, so passing them shows the calibration: the seven stylized facts of sections 13.4
and 15 of model.md (12 required checks and 2 known limits), the calibration targets of 13.2, and the headline claims H1
to H8 (`crates/incentive-core/src/headline.rs`), the exhibit claims of 13.3 written as band checks. The held-out
regretted share of 13.4 was named before tuning and never tuned on; it is reported with the in-sample results because
it belongs to the calibration record.

Out-of-sample checks are the battery V1 to V10 (`crates/incentive-core/src/validation.rs`, run by `incentive
validate`): ten field findings with 21 required checks and one known limit, none used in calibration. Sources and
quotes are in `exports/research-validation/notes.md` and `exports/research-2026-11-26/notes.md`.

### 2.2 Pre-registration

Lab issue #31 fixed the out-of-sample checks and their bands from the literature before any run (its comment of
2026-11-26, 22:30). The bands are relative changes unless the field gives standard deviations. An amendment at 22:35,
also before any V run, moved every check onto the integrity preset (2.3). The first wiring demonstrations ran at 22:56
and the battery at 23:27, so no band was set after a model result was seen.

Two checks need a dose. They were set as manipulation checks, not outcomes: V1's same-group down-ranking penalty
(0.28) was calibrated so that the treated's same-group impression share falls by a third, as in the field (where
it went from 53.7% to 36.2%), and V3's hiding threshold (0.56) so that about 7% of the treated's impressions are
hidden. At the defaults the doses are -0.331 (95% interval -0.422 to -0.238) and 0.0729 (0.0656 to 0.0801) under the
integrity preset, and -0.389 and 0.212 under the engagement ranking (`exports/validation/defaults/validation.md`).
V4's friction dose does not map onto the field's accuracy prompt, so V4 judges direction only.

The range rule for the global sensitivity runs (2.6) was fixed after the B1 global pilot, in which 2 of its 4 worlds
were degenerate under section 11's ranges as written, and before the real runs (lab #31's comment of 2026-11-26,
22:34; `exports/robustness/pilot-global.txt`).

### 2.3 The integrity preset as the realistic feed

The field studies ran on Facebook, Instagram and Twitter, whose ranked feeds carry integrity measures. The model's
stand-in is the integrity preset (model.md, 14.1): the engagement ranking with demotion instead of removal, a quality
signal, see-less feedback and demoted community notes. Facts 3, 6 and 7 already compare against it. Every V check is
judged under it; the same check under the plain engagement ranking is reported beside it as a sensitivity and not
judged. V5 compares the integrity preset with the chronological feed.

### 2.4 Paired designs

Every comparison is paired by seed: both arms run on the same seeds, so they share the population and its draws. The
experiments (facts 6 and 7, and V1, V2, V3, V10) go further: the real run and its placebo draw the same treated
people, and the effect is the treated's change in the real run minus their change in the placebo. The field compares
randomised groups instead; the paired design removes most of the between-person noise and cannot be run in the
field. The feed experiments treat a random 0.2 of the regular users over the last 90 days (V1, V10) or 42 days (V3);
the break of V2 lasts 35 days.

### 2.5 Verdict rules

At the defaults each check runs on 8 seeds at full size (`N = 2,000`, 365 days). The verdict judges the 95%
t-interval of the per-seed effect: PASS when it lies inside the band, FAIL when it lies wholly outside, UNDECIDED when
it crosses a bound. Seed means are reported beside the intervals. A known limit is reported and not required.

In the robustness runs (2.6) a check holds in a world when its effect on the seed means lies inside its band, and a
fact or claim holds when all its required checks hold. "Sign" is the share of worlds whose effect lies on the default
world's side of its neutral value: 0 for a difference or a relative change, 1 for a ratio, and 1 for the ratios and
shares that V6, V7 and V8 read as means. The robustness runs predate the fix that gave V6, V7 and V8 their neutral of
1 (before it their neutral was 0, so their sign was 100% whatever the worlds did), and their `summary.md` files still
show the old values. The sign shares of V6, V7 and V8 in this document were recomputed from each run's `checks.csv`
with the corrected rule (`exports/validation-analysis/sign_shares.py`, output in `sign_shares.txt`). V7's default is
exactly 1.000, at the neutral, so its sign is the share of worlds also at 1.000 (no share after any note); the other
worlds with a value lie below 1, on the field's side.

### 2.6 Noise designs and range rules

Both designs vary 53 settings: the 51 world assumptions of the catalogue except the checking habits
(`recheck_rate`, `notification_pull`, `extra_session_share`) and `interruption_cost`, which are off by the owner's
decision (model.md, 6.1 and 13.2), plus the troll share and the bot share. Design and run settings never vary. Each
world runs 2 seeds (seeds 1 and 2) at full size for all three batteries.

- **Local (stability):** 100 worlds. Each assumption with a nonzero default is multiplied by U(0.75, 1.25) and
  clamped to its catalogue range. The three settings whose default is 0 or categorical (`leave_threshold`,
  `polarisation_backfire`, `bot_strategy`) keep their defaults, so 50 assumptions are actually jittered.
- **Global (sensitivity):** 240 worlds on a Latin hypercube over the revised ranges of model.md section 11, with
  `bot_strategy` drawn uniformly. The rule, fixed on lab #31 before the runs: a range written for a text default that
  calibration changed (13.1) is scaled by the same factor as the default; an assumption without a documented range
  gets default/3 to 3 x default; the troll share and the bot share get default/2 to 2 x default; all are clamped to
  the catalogue. Sensitivity is the Spearman rank correlation and the partial rank correlation (PRCC) of each
  assumption with each effect, over the non-degenerate worlds.
- **Documented ranges (contrast):** 64 worlds on section 11's ranges as written before the revision. With 51
  non-degenerate worlds there are too few for PRCC, so only Spearman is available there.

`leave_threshold` stays fixed. Its calibrated default is 0 and the rule scales its documented range by a factor of 0,
which leaves a range of zero width. Lab #31 decided against setting an ad hoc range after the fact, so the revised
global runs say nothing about it; this is a coverage limit (section 9). The documented-range run does vary it, from
-0.5 to 0.5.

### 2.7 Degenerate worlds

A world is degenerate when the default design's user share on its last day, averaged over the seeds, lies below 0.05:
the platform has emptied, and most effects become ratios of near-zero quantities. The robustness summaries report hold
shares over all worlds and over the non-degenerate ones.

### 2.8 Files and commands

| Material | Files |
| --- | --- |
| Facts at the defaults, rerun for this document | `exports/validation-analysis/facts-rerun/stdout.txt`, `facts.json` |
| Headline claims at the defaults, 8 seeds | `exports/robustness/headline-defaults-8seeds/` |
| Validation at the defaults, 8 seeds | `exports/validation/defaults/validation.md`, `validation.json` |
| Local noise, 100 worlds | `exports/robustness/local-1/summary.md`, `checks.csv`, `worlds.csv` |
| Global, revised ranges, 240 worlds | `exports/robustness/global-1/summary.md`, `summary.json`, `checks.csv` |
| Global, documented ranges, 64 worlds | `exports/robustness/global-2-documented/` |
| This document's analysis | `exports/validation-analysis/analysis.txt` (from `analyse.py`), `band_sides.txt` |
| Diagnostics of section 4.3 | `exports/validation-analysis/diag/` |

The analysis scripts are `analyse.py`, `band_sides.py`, `informedness_series.py`, `v5_ratio.py`, `sign_shares.py`
and `recheck.py` in `exports/validation-analysis/`, run from the repository root with `python3 -I`. Like every output
cited here, they live in the gitignored `exports/` folder and are not tracked. Every CLI run went through
`taskset -c 0-7 nice -n 19 ionice -c 3`. The commands below name their output folders; the "Generated by" line of
each `summary.md` and `validation.md` written from 2026-11-27 on includes `--out`, and the older files' lines lack it:

```text
target/release/incentive facts --seeds 8 --out exports/validation-analysis/facts-rerun
target/release/incentive validate --seeds 8 --out exports/validation/defaults
target/release/incentive robustness --mode local --worlds 0 --seeds 8 --jitter 0 --battery headline \
  --out exports/robustness/headline-defaults-8seeds
target/release/incentive robustness --mode local --worlds 100 --seeds 2 --sample-seed 1 --battery all --jitter 0.25 \
  --out exports/robustness/local-1
target/release/incentive robustness --mode global --worlds 240 --seeds 2 --sample-seed 1 --battery all \
  --ranges revised --out exports/robustness/global-1
target/release/incentive robustness --mode global --worlds 64 --seeds 2 --sample-seed 2 --battery all \
  --ranges documented --out exports/robustness/global-2-documented
```

## 3. In-sample fit

### 3.1 Stylized facts

The facts, rerun at the current build on 8 paired seeds (`exports/validation-analysis/facts-rerun/stdout.txt`),
reproduce every effect and interval of model.md 13.4 to the printed digits.

| Fact | Check | Effect | 95% interval | Band | Verdict |
| --- | --- | --- | --- | --- | --- |
| 1 | Outrage impressions, ER minus chronological | 0.404 | 0.371 to 0.438 | 0.15 to 0.6 | pass |
| 1 | Reflective value per impression, chronological minus ER | 0.161 | 0.137 to 0.186 | 0.05 to 0.4 | pass |
| 2 | Rise of the Outrage share of posts under ER | 0.018 | 0.008 to 0.028 | 0.01 to 0.15 | undecided |
| 2 | That rise minus the rise without learning | 0.014 | 0.002 to 0.025 | 0.01 to 0.15 | undecided |
| 3 | Time spent, chronological over ER | 0.641 | 0.621 to 0.663 | 0.4 to 0.9 | pass |
| 3 | Polarisation difference at day 90 | 0.028 | -0.032 to -0.024 | below 0.05 | pass |
| 3 | Toxic, days 30-90, chronological over integrity | 0.336 | 0.252 to 0.407 | 0.5 to 0.9 | fail (limit) |
| 3 | Misinformation, days 30-90, chronological over integrity | 0.640 | 0.560 to 0.719 | 1.05 to 3 | fail (limit) |
| 4 | Largest cascades, depth cap 1 over none | 0.720 | 0.680 to 0.767 | 0.3 to 0.85 | pass |
| 5 | Vulnerable who left, troll share 0.12 minus 0.03 | 0.074 | 0.053 to 0.095 | 0.03 to 0.3 | pass |
| 6 | Break group's wellbeing, real minus placebo, SDs | 0.057 | 0.001 to 0.113 | 0.02 to 0.25 | undecided |
| 7 | Time a day, notifications off minus placebo, SDs | -0.168 | -0.181 to -0.155 | -0.4 to 0.1 | pass |
| 7 | Sessions a day, off minus placebo, SDs | -0.326 | -0.344 to -0.309 | -0.4 to 0.1 | pass |
| 7 | Wellbeing, off minus placebo, SDs | -0.003 | -0.004 to -0.002 | -0.15 to 0.15 | pass |

Of the 12 required checks 9 pass on their intervals and 3 are undecided (both of fact 2's and fact 6's); all 12 hold on
the seed means. Both known limits of fact 3 fail. Fact 6 and fact 7's use checks were tuned on (`time_cost` and the
opening chance's notification factor), so passing them shows the calibration.

### 3.2 Calibration targets and the held-out target

Model.md 13.2 records the targets at the final defaults; a rerun of the default design on 8 seeds
(`exports/validation-analysis/diag/default-8/`) gives the same values. The user share after 365 days is 0.499 (lowest
seed 0.464; target 0.4 to 0.9) and holds on its interval (0.474 to 0.524, model.md 13.2). The toxic impression share
is 0.178 (target 0.03 to 0.2): it holds on the mean, while its interval touches the bound (0.155 to 0.2003) and 2 of
8 seeds lie above 0.2. The Outrage share of posts rises by 0.018 (0.008 to 0.028) and stays below 0.5. The trade-off
targets hold by judgement (model.md 13.2). Two targets named for the default design, sessions per daily active (T3)
and the notification share of sessions (T1), are missed by the owner's decision and hold only in the checking habits
world.

The held-out regretted share of time fails on both its interval and its mean: 0.0045 (0.0040 to 0.0050, in
model.md 13.4; 0.0045 again in `exports/validation-analysis/diag/default-8/`) against a band of 0.20 to 0.42, two
orders of magnitude below. With one session a day the model has almost no compulsive use.

### 3.3 Headline claims

All 13 checks of H1 to H8 pass on their 95% intervals at the defaults, and every seed of every check lies inside its
band (8 seeds; intervals computed from the per-seed effects in `exports/robustness/headline-defaults-8seeds/checks.csv`,
`exports/validation-analysis/analysis.txt`). The narrowest margins are H2's survey difference, 0.0061 (with an
interval of 0.0024 to 0.0098, band above 0), and H5's wellbeing difference, -0.0012 (interval -0.0042 to 0.0019, band
below 0.01).

| Claim | Check | Effect | 95% interval | Band |
| --- | --- | --- | --- | --- |
| H1 | Harm index, Q&B against ER, relative | -0.798 | -0.815 to -0.780 | -1 to -0.5 |
| H1 | Engagement index, Q&B against ER, relative | -0.171 | -0.196 to -0.145 | -0.33 to 0.33 |
| H2 | Survey satisfaction, ER minus Q&B | 0.0061 | 0.0024 to 0.0098 | above 0 |
| H2 | Wellbeing of ever-users, ER minus Q&B | -0.069 | -0.077 to -0.062 | below 0 |
| H3 | Toxic impressions, threshold 0.3 over default | 0.051 | 0.033 to 0.069 | 0 to 0.5 |
| H3 | False-positive actions, threshold 0.3 over default | 13.85 | 13.23 to 14.56 | above 3 |
| H4 | Harm index, trap solution minus ER | -0.153 | -0.173 to -0.133 | below 0 |
| H4 | Engagement index, trap solution over ER | 1.102 | 1.076 to 1.128 | above 0.9 |
| H5 | Engagement index, intensity 1 over 0.6 | 1.224 | 1.211 to 1.237 | above 1 |
| H5 | Wellbeing of users, intensity 1 minus 0.6 | -0.0012 | -0.0042 to 0.0019 | below 0.01 |
| H6 | Engagement index, last 95 days, ER over Q&B | 1.202 | 1.166 to 1.239 | above 1 |
| H7 | Harassment per 1,000 users, report trust 1 minus 0 | -1.94 | -2.55 to -1.34 | below 0 |
| H8 | Harm index, `w_see_less` 5 minus 0 | 0.250 | 0.212 to 0.289 | above 0 |

In sample the model fits: every required fact holds on the seed means, every target named for the default design but
T1 and T3 holds, and every headline claim holds on every seed. The one prediction it was not tuned on, the regretted
share, misses by a factor of about 50.

## 4. Out-of-sample accuracy

### 4.1 The checks at the defaults

Integrity preset, 8 paired seeds, full size (`exports/validation/defaults/validation.md` and `.json`); "rel." marks
a relative change.

| Id | Check | Model (95% interval) | Band | Verdict |
| --- | --- | --- | --- | --- |
| V1 | Polarisation, SDs | -0.062 (-0.079 to -0.046) | -0.12 to 0.12 | pass |
| V1 | Toxic share, rel. | -0.004 (-0.029 to 0.016) | -0.35 to -0.02 | undecided |
| V1 | Time per day, rel. | -0.058 (-0.069 to -0.046) | -0.10 to 0.05 | pass |
| V1 | Misinformation share, rel. | -0.151 (-0.176 to -0.124) | -0.5 to 0 | pass |
| V2 | Affective polarisation, SDs | -0.049 (-0.054 to -0.043) | -0.14 to 0.02 | pass |
| V2 | Informedness, SDs | +0.096 (0.045 to 0.146) | -0.30 to -0.05 | FAIL |
| V2 | Later use, rel. | -0.190 (-0.206 to -0.174) | -0.35 to -0.08 | pass |
| V3 | Time per day, rel. | -0.016 (-0.020 to -0.013) | -0.25 to -0.02 | undecided |
| V3 | Engagement per day, rel. | -0.034 (-0.044 to -0.023) | -0.30 to 0 | pass |
| V3 | Own posts' toxicity, rel. (known limit) | +0.003 (-0.004 to 0.009) | -0.45 to -0.10 | FAIL (limit) |
| V4 | Truth of shares, rel. | +0.013 (0.005 to 0.021) | 0 to 0.25 | pass |
| V5 | Partisan half's reach, ranked over chronological | 0.959 (0.848 to 1.108) | 1 to 5 | undecided |
| V5 | Extremes' amplification over the middle's | 2.251 (1.793 to 2.907) | below 1.1 | FAIL |
| V6 | Out-group impressions over candidates | 0.982 (0.930 to 1.033) | 0.85 to 1 | undecided |
| V6 | Out-group likes over impressions | 0.664 (0.625 to 0.702) | 0.75 to 0.97 | FAIL |
| V7 | Share of a noted post's shares before its note | 1.000 (1.000 to 1.000) | 0.70 to 0.95 | FAIL |
| V8 | Shares per impression, Outrage over others | 2.813 (2.663 to 2.962) | 1.3 to 3 | pass |
| V9 | Elasticity of time to ad load | -0.051 (-0.074 to -0.028) | -0.15 to -0.01 | pass |
| V10 | Misinformation share, rel. | -0.064 (-0.093 to -0.037) | -0.8 to -0.05 | undecided |
| V10 | Engagement per day, rel. | -0.033 (-0.043 to -0.023) | -0.5 to -0.01 | pass |
| V10 | Polarisation, SDs | -0.023 (-0.030 to -0.015) | -0.1 to 0.1 | pass |
| V10 | Informedness, SDs | +0.038 (0.011 to 0.066) | -0.5 to 0 | FAIL |

The field values, the engagement ranking's effect with the verdict its interval would get (not judged), and the kind
of test (4.2), in the same order. Field values are from the sources and designs in
`exports/validation/defaults/validation.md`; V1's misinformation figure is the field's 0.76% to 0.55% of exposures
from repeat offenders, and V8's range spans Rathje's odds ratios for news media and congressional accounts.

| Id | Check | Field | Engagement ranking | Kind |
| --- | --- | --- | --- | --- |
| V1 | Polarisation | null, within 0.12 SD | -0.074, pass | calibration |
| V1 | Toxic share | -11% | -0.014, undecided | test |
| V1 | Time per day | -0.02 SD, null | -0.098, undecided | test |
| V1 | Misinformation share | -28% | -0.129, pass | test |
| V2 | Affective polarisation | -0.06 SD, -0.031 SD | -0.068, pass | calibration |
| V2 | Informedness | -0.19 SD, -0.098 SD | +0.281, fail | test |
| V2 | Later use | -23%, -15% | -0.230, pass | test |
| V3 | Time per day | -10% | -0.040, pass | test |
| V3 | Engagement per day | -0.056 SD | -0.098, pass | test |
| V3 | Own posts' toxicity | -30% | -0.004, fail | absent |
| V4 | Truth of shares | +2.1% to +9% | +0.009, pass | assumed |
| V5 | Partisan half's reach | above 1 | 0.950, undecided | test |
| V5 | Extremes' amplification | about 1 | 2.869, fail | test |
| V6 | Out-group impressions | 0.92 to 0.95 | 0.829, fail | test |
| V6 | Out-group likes | 0.83 to 0.94 | 0.569, fail | assumed |
| V7 | Shares before the note | about 0.85 | 1.000, fail | test |
| V8 | Outrage shares | 1.35 to 2.8 | 2.747, pass | assumed |
| V9 | Ad load elasticity | -0.02 to -0.07 | -0.022, undecided | assumed |
| V10 | Misinformation share | down | -0.060, undecided | test |
| V10 | Engagement per day | down | -0.024, pass | test |
| V10 | Polarisation | null | -0.028, pass | calibration |
| V10 | Informedness | down | +0.059, fail | test |

Of the 21 required checks 11 pass, 5 are undecided and 5 fail on their intervals; 13 hold on the seed means (V6's
ranking ratio and V10's misinformation share lie inside their bands on the mean). The known limit, V3's own toxicity,
fails as expected: the model has no channel from what people see to what they write.

### 4.2 Scorecard

**Directions against magnitudes.** 17 required checks have a field direction; the other 4 are nulls or an upper bound
(V1 polarisation and time, V10 polarisation, V5's extremes). Judged on the 95% intervals:

| | Checks | Direction right | Direction wrong | Direction not established | Magnitude in band |
| --- | --- | --- | --- | --- | --- |
| Directional | 17 | 11 | 3 (V2, V10 informedness; V7) | 3 (V1 toxic, V5 half, V6 ranking) | 8 |
| Nulls and bound | 4 | - | - | - | 3 (V5 extremes fails) |

Two conventions of this count: V7's 1.000 is counted as a wrong direction, but it lies at the boundary of no share
after a note, not on the side opposite the field's 0.85; and V2's polarisation counts as directional (its band, -0.14
to 0.02, leans negative after the field's -0.06 and -0.031 SD), while the polarisation checks of V1 and V10, with bands
symmetric around 0, count as nulls.

The direction is right for 11 of 17 and the magnitude for 8 of 17 (10 of 17 on the seed means). Where the direction is
right but the check is undecided or fails, the model's effect is too small (V3 time, -1.6% against -10%; V10
misinformation, -6.4%; V1 toxic share, -0.4% against -11%) or too strong (V6 likes, 0.66 against 0.83 to 0.94).

**Mechanism by assumption against genuine tests.** "Kind" in 4.1 sorts the checks by what a pass can show:

- *Calibration-implied* ("calibration", 3 checks: the polarisation nulls of V1, V2 and V10). `polarisation_rate` was cut
  to 0.00015 so that fact 3's polarisation barely moves in three months (model.md 13.1, 13.4); small polarisation
  changes in the field experiments follow from that calibration. All 3 pass.
- *Direction assumed* ("assumed", 4 checks: V4, V6 likes, V8, V9). The direction is written into an equation: share
  friction scales impulsive shares (7.1), out-group likes are cut by `1 - 0.8 * outrage` and in-group hostility
  raises likes (7.1), hostility raises shares (7.1), ads lower reflective utility (7.2). Only the magnitude is tested.
  3 pass; V6's likes fail on magnitude.
- *Genuine tests* ("test", 14 checks): the effect emerges from interactions no parameter was set for. 5 pass (V1 time
  and misinformation, V2 later use, V3 engagement, V10 engagement), 5 are undecided (V1 toxic, V3 time, V5 half, V6
  ranking, V10 misinformation) and 4 fail (V2 and V10 informedness, V5 extremes, V7).
- *Absent* (the known limit, V3's own toxicity): the model has no mechanism for it, and the check fails as expected.

The passes concentrate where the answer was built in: 6 of the 7 calibration-implied or assumed checks pass, against 5
of the 14 genuine tests. The genuine tests that pass are mostly about time and engagement, the quantities the model was
calibrated on most heavily (facts 3, 6 and 7, the time cost, the session length).

### 4.3 Findings

**Leaving the platform or losing reshares raises informedness (V2, V10).** The field found lower news knowledge after a
break (-0.19 and -0.098 SD) and after reshares were removed. The model gives +0.096 SD (raw +0.0078) after a break and
+0.038 SD (raw +0.0029) without reshares (`exports/validation/defaults/validation.md`). Diagnostics on 8 seeds under
the integrity preset (`exports/validation-analysis/diag/`):

- Users' mean informedness sits just above the 0.6 baseline: 0.607 over the summary window (`integrity-8/`),
  non-users 0.605. It reaches 0.614 between days 90 and 180 and falls to 0.603 by day 364, while the misinformation
  share of impressions rises from 0.112 on day 180 to 0.146 (`informedness-series.txt`).
- Without belief in misinformation (`world.credulity_scale` 0, which changes nothing else), users sit at 0.644 and
  non-users at 0.623 (`integrity-credulity0/`): the informative gain alone lifts users by 0.044 above the baseline, and
  the misinformation loss pulls 0.037 of it back.
- In that world the break lowers informedness, -0.687 SD (-0.782 to -0.593), raw -0.0141 (`v2v10-credulity0/`);
  with the loss in place the raw change is +0.0078. Subtracting the two suggests that over the break's 35 days the
  loss contributes about +0.022 against the gain's -0.014, about 1.5 times the gain. The split is indicative only:
  the two worlds sit at different informedness levels (users 0.644 against 0.607), and the drift toward 0.6, which
  turns the platform's net flow into a change once people leave, is proportional to the gap, so the two break effects
  are not additive parts of one effect.
- The drift toward 0.6 (1% of the gap a day, model.md 13.1) sets the speed, not the sign: it closes 30% of the gap
  in 35 days and turns whatever net flow the platform gave into a change once people leave. The sign follows from the
  platform's net flow for regular users being negative at the end of the year.
- Without reshares the misinformation share of the treated's impressions falls by 6.4% and informedness rises; with
  no belief in misinformation the same experiment gives +0.0006 raw (+0.029 SD, -0.041 to 0.098), so reshares carry
  almost no informative gain in the model. In the field they carried political news.

A hypothesis, not tested here and the basis of recommendation 1: the misinformation loss outweighs the informative gain
because the two constants were cut at different rates in calibration (0.05 to 0.008 and 0.01 to 0.0004, model.md 13.1)
and because the model's misinformation, any post with `truth` below 0.5, is common: 0.130 of impressions over the
summary window under the integrity preset (`integrity-8/`), against 0.76% of exposures from misinformation repeat
offenders in Nyhan et al.'s sample (`exports/validation/defaults/validation.md`). The failure is robust: the break's
informedness check fails in 90 of 100 local worlds and has the default's (positive) sign in 83 of the 100 worlds, and
V10's fails in 99 (5.2; `exports/robustness/local-1/summary.md`). Globally, more credulity makes the rise larger (PRCC
+0.47, `exports/robustness/global-1/summary.md`), as the diagnosis predicts.

**The most partisan authors are amplified 2.25 times as much as the middle (V5).** Huszar et al. found no extra
amplification of the far parties over the moderates. The diagnostics read the reach of the most partisan tenth of
authors over that of the middle fifth (`extreme_reach_ratio`, seed means over 8 seeds,
`exports/validation-analysis/diag/v5-*/`; per-seed ratios in `v5-ratios.txt`):

| Feed | Most partisan tenth's reach over the middle fifth's | Over chronological |
| --- | --- | --- |
| Chronological (the base design) | 0.079 | 1 |
| Integrity preset | 0.177 | 2.24 |
| Integrity preset with the chronological feed (`design.chronological` true) | 0.082 | 1.04 |
| Integrity preset without recommendations (`recommended_share` 0) | 0.096 | 1.22 |
| Chronological, `outrage_appeal` 0 | 0.070 | 1 |
| Integrity preset, `outrage_appeal` 0 | 0.121 | 1.73 |

V5's two arms differ in more than ranking. The chronological arm is the base design: it removes posts above a classifier
score of 0.7 and has no community notes, quality signal or see-less feedback. The integrity arm demotes posts above 0.6
and has all three. The third row keeps the integrity preset's settings and switches the ranking off; it was run for this
document (`diag/v5-integrity-ranking-design_chronological_true/`). Ranking alone, the integrity preset over the same
preset on the chronological feed, gives an extremes ratio of 2.17 on the seed means (mean of the per-seed ratios 2.25,
95% interval 1.69 to 2.81) and a partisan-half reach ratio of 0.934 (per-seed mean 0.945, 0.84 to 1.05). The moderation
measures alone, both arms chronological, move the extremes ratio by a factor of 1.04 and the partisan-half ratio by
1.03. So the ranking, not the moderation, makes the extra amplification; the verdicts at the defaults would be the same.

Under the chronological feed the most partisan tenth reach 8% of what the middle fifth reach; small networks may be one
reason, which no run here separates. The ranked feed's trending recommendations, drawn in proportion to
`1 + engagement` (6.2), give every post a floor of out-of-network reach and engaging posts more, which multiplies a
small reach most. Without recommendations the extra amplification falls from 2.24 to 1.22 (per-seed mean
1.27 ± 0.36). The coupling of hostility and partisanship in 7.1 (a post's charge is `outrage * (0.5 + partisanship)`
of the viewer, and the most partisan authors, mostly Partisans and Trolls, post the most hostile content) contributes
but is not the main mechanism: without the appeal of in-group hostility to likes the ratio falls to 1.73, not to 1 (the
hostility term in `P_share` is not scaled by `outrage_appeal` and stays). The follow graph may also contribute: both
arms run from day 0, so the chronological arm never grows its follow graph through liked recommendations (model.md
16), while Huszar's holdback group shared the ranked group's follow graph; no run here tests this. The same limit
may make fact 3's two known limits fail.

Two measurement points qualify the numbers. Reach per post counts every impression of a post over its life, including
those of posts later removed or demoted (`count_ending_post` in `crates/incentive-core/src/learning.rs`), so in the
arm that removes posts their early reach still counts. And V5, V6 and V8 average daily ratios over the summary window:
the mean of a ratio is biased upward when its denominator varies (Jensen's inequality), and `extreme_reach_ratio` rests
on the few posts the most partisan tenth end each day. Recommendation 8 proposes both corrections. The failure is
robust: 99 of 100 local worlds fail (`exports/robustness/local-1/summary.md`), with a large seed spread (seed SD 1.08
per world, `exports/validation-analysis/analysis.txt`).

**People like out-group posts too rarely (V6).** The out-group share of in-network likes is 0.664 of its share of
impressions, against 0.83 to 0.94 in Bakshy et al. (where the measure is clicks). Diagnostics on 8 seeds
(`exports/validation-analysis/diag/v6-*/validation.md`):

| World | Out-group likes over impressions (95% interval) |
| --- | --- |
| Defaults | 0.664 (0.625 to 0.702) |
| `in_group_outrage_cost` 1 (no reflective asymmetry) | 0.689 (0.648 to 0.729) |
| `arousal_appeal` 0 | 0.769 (0.742 to 0.797) |
| `outrage_appeal` 0 (no in-group appeal of hostility) | 0.920 (0.901 to 0.940) |
| `outrage_appeal` 0 and `in_group_outrage_cost` 1 | 0.940 (0.923 to 0.958) |

The main candidate is the in-group appeal of hostility, `outrage_appeal * H` in `A`, which raises same-group likes only.
The reflective asymmetry (`c_in`) moves the ratio by 0.025. Setting `outrage_appeal` to 0 is a world change, not a pure
switch: it also cuts the Outrage share of impressions under the integrity preset from 0.319 to 0.093
(`exports/validation-analysis/diag/v5-integrity-ranking*/`), so it removes part of the hostile content on which the
out-group cut `1 - 0.8 * outrage` acts, and the two channels, the in-group appeal and the out-group cut acting on less
hostile content, are not separated. The same switch pushes V6's other check out of its band, the ranking ratio to 1.154
(1.074 to 1.234; `diag/v6-outrage_appeal_0/validation.md`), and V8's Outrage share-rate ratio down to 1.98
(`diag/v5-integrity-ranking-world_outrage_appeal_0/summary.json`). The out-group cut is a code constant and could not be
switched off without a code change. The failure is robust: 85 of 100 local worlds lie below the band
(`exports/validation-analysis/band_sides.txt`), and `outrage_appeal` is the strongest global driver (PRCC -0.76,
`exports/robustness/global-1/summary.md`).

**No one shares a post after its note (V7).** A noted post's human shares all come before its note (1.000), against
about 0.85 derived from Slaughter et al. Diagnostics, integrity preset, 8 seeds
(`exports/validation-analysis/diag/v7-*/summary.json`):

| `noted_penalty` (integrity preset: 1.0) | Share of shares before the note |
| --- | --- |
| 1.0 | 1.000 |
| 0.5 | 0.998 ± 0.002 |
| 0 | 0.944 ± 0.009 |
| 1.0, post lifetime 7 days | 1.000 |

The ranking penalty pushes noted posts off every feed: at 0.5 almost no share follows a note, and only without a penalty
do 5.6% of shares come after it, still a third of the field's 15%: removing the penalty closes about a third of the gap
to 0.85. Late notes may explain the rest, which no run here tests: notes are written once a day after the sessions
(phase 8 of model.md 5), only on posts with 50 impressions, and a post earns most of its impressions early. A longer
post lifetime changes nothing, but it does not change when notes arrive either. The failure holds in all 100 local
worlds; in the 2% of global worlds where it passes, more items per session take feeds deep enough to reach noted posts
(`base_session_items` PRCC -0.79, `exports/robustness/global-1/summary.md`).

**Exposure and incivility are unlinked (V1 toxic share, V3).** Fewer like-minded posts barely move the toxic share of
the treated's impressions (-0.4%, against -11% in the field); the sign is the default's in only 41 of 100 local worlds
(`exports/robustness/local-1/summary.md`), so the model has no effect here, only seed noise (seed share 0.52,
`exports/validation-analysis/analysis.txt`).
Hiding flagged posts cuts time by 1.6% against the field's 10%, and in 77% of local worlds the effect is too small
(`exports/validation-analysis/band_sides.txt`): hidden posts are replaced by the next candidates, and toxic posts hold
attention less in the model than the field suggests. The treated's own toxicity does not respond (the known limit).

**What passes.** Fewer like-minded posts cut misinformation exposure (-15%) and time (-5.8%) within the field's
bands; after a break, use stays lower (-19% against -23% and -15%); removing reshares cuts engagement (-3.3%); friction
raises the truth of shares (+1.3%); Outrage is shared 2.8 times as often per impression as the non-hostile styles,
near the band's top of 3; time responds little to ad load (elasticity -0.051). These are the model's time and
engagement mechanics and its built-in directions.

### 4.4 The engagement ranking as the feed

Judged on the engagement ranking instead, the battery gives 10 passes, 5 undecided and 6 fails, against 11, 5 and 5
under the integrity preset (`exports/validation/defaults/validation.json`). It does better on V3's time (-4.0%,
pass), but its dose is three times larger there (0.212 of impressions hidden against 0.073): per unit of dose both
feeds give about the same response (-0.19 under the engagement ranking, -0.22 under the integrity preset), so the pass
is a dose artifact. It does worse on V1's time
(-9.8%, undecided), V6's ranking ratio (0.829, below the band), V9 (-0.022, undecided), and further from the band on V2
informedness (+0.281), V5 extremes (2.87) and V6 likes (0.569). The integrity preset is the better stand-in for the
field's feeds on these checks, as the amendment assumed.

## 5. Stability (local)

### 5.1 Hold shares

100 worlds, every assumption jittered about +-25%, 2 seeds each, no degenerate world
(`exports/robustness/local-1/summary.md`). "Sign" is given for the check named; for V6, V7 and V8 it was recomputed
with the corrected neutral of 1 (2.5; `exports/validation-analysis/sign_shares.txt`).

| Item | Holds | Sign (check) | Item | Holds | Sign (check) |
| --- | --- | --- | --- | --- | --- |
| Fact 1 amplification | 97% | 100% (both) | H1 calmer feed | 89% | 87% (engagement) |
| Fact 2 outrage learning | 82% | 97% (both) | H2 survey misranks | 65% | 65% (survey) |
| Fact 3 chronological feed | 94% | 100% (all) | H3 strict classifier | 100% | 100% |
| Fact 4 forwarding limits | 100% | 100% | H4 trap solution | 100% | 100% |
| Fact 5 harassment exit | 94% | 99% | H5 notifications | 96% | 69% (wellbeing) |
| Fact 6 deactivation | 50% | 76% | H6 lead lasts | 87% | 87% |
| Fact 7 notifications off | 97% | 70% (wellbeing) | H7 report trust | 100% | 100% |
| V4 friction | 95% | 95% | H8 see-less backfire | 100% | 100% |
| V8 Outrage shares | 78% | 100% | V9 ad load | 71% | 88% |
| V1 | 23% | 41% (toxic) | V2 | 10% | 83% (informedness) |
| V3 | 23% | 96% (time) | V5 | 1% | 100% (extremes) |
| V6 | 8% | 81% (ranking), 100% (likes) | V7, V10 | 0% | 98% (at 1), 99% (informedness) |

Four signs lie below 75%. Fact 7's wellbeing effect and H5's wellbeing difference are both near zero at the defaults
(-0.003 and -0.001), so their sign flips with noise while the checks hold (100% and 96%). H2's survey check (65%)
follows the survey's weight on impulse (5.3), and V1's toxic share (41%) has no effect beyond seed noise (4.3). V6's
ranking ratio lies above 1 in 19 of the 100 worlds, where ranking raises cross-cutting exposure instead of trimming
it. V7 sits at 1.000 in 98 worlds; in the other 2 some shares follow a note.

### 5.2 A world's failure against seed noise

With 2 seeds per world a check can fail in a world through seed noise alone. From the per-seed values in `checks.csv`
(`exports/validation-analysis/analysis.txt`): the seed SD is estimated from the two seeds of each world, and a failing
world is "noise-compatible" when its mean lies within two standard errors of a 2-seed mean (2 x seed SD / sqrt 2) of
the band. The "seed share" is the seed variance of a 2-seed mean over the variance of the world means: the part of the
spread across worlds that seed noise alone would produce; at 1 or above the jitter has no detectable effect.

The table is at item level: a failing world counts as noise-compatible only when every failing check of the item lies
within 2 SE of its band. Counted per check the shares can differ: for V5, 43 failing worlds are noise-compatible at
item level against 63 for its extremes check alone (`analysis.txt`).

| Item | Failing worlds | Noise-compatible | Clear failures | Seed share (check) |
| --- | --- | --- | --- | --- |
| Fact 2 outrage learning | 18 | 18 | 0 | 0.41 (rise against no learning) |
| Fact 6 deactivation | 50 | 20 | 30 | 0.05 |
| H1 calmer feed | 11 | 7 | 4 | 0.03 (engagement) |
| H2 survey misranks | 35 | 19 | 16 | 0.07 (survey) |
| H6 lead lasts | 13 | 7 | 6 | 0.02 |
| V2 | 90 | 12 | 78 | 0.08 (informedness) |
| V5 | 99 | 43 | 56 | 0.47 (extremes) |
| V6 | 92 | 15 | 77 | 0.03 (likes) |
| V7 | 100 | 0 | 100 | spread near 0 |
| V8 | 22 | 9 | 13 | 0.09 |
| V9 | 29 | 29 | 0 | 0.91 |
| V10 | 100 | 19 | 81 | 0.55 (informedness) |

Facts 1, 3, 5 and 7 and H5 fail in 3 to 6 worlds each, all or all but one noise-compatible; V4's 5 failures are all
noise-compatible (seed share 1.36).

### 5.3 What is robust and what hinges on the defaults

- **Robust to +-25%:** facts 1, 3, 4, 5 and 7, H3, H4, H5, H7 and H8 hold in 94% to 100% of worlds, and their few
  failures are seed noise near a bound.
- **Fact 2 hinges on seed noise, not on the world:** it holds in 82% of worlds, and every failure lies within noise
  of the lower bound 0.01, as at the defaults, where both checks are undecided. More seeds would decide it; the world
  does not.
- **Fact 6 hinges on the defaults:** it holds in only 50% of worlds, with 26% below the band and 24% above
  (`exports/validation-analysis/band_sides.txt`), and the spread is real (seed share 0.05). The SD of its world means
  at +-25%, 0.197, is close to the band's width of 0.23. The strongest local drivers are `reflective_quality_weight`
  (Spearman -0.54) and `outrage_appeal` (+0.48) (`analysis.txt`). The `time_cost` calibration on fact 6 pins one
  combination of several assumptions on a knife edge.
- **H2 hinges on a measurement assumption:** the survey ranks the engagement ranking above Q&B in 65% of worlds, and
  the result follows the survey's weight on impulse (`survey_impulsive_weight`, Spearman +0.72, `analysis.txt`). The
  wellbeing half of H2 holds in every world.
- **H1 and H6 hinge on habit:** they fail in 11 and 13 worlds. H1's engagement check fails where habit forms faster
  and Q&B loses more than a third of the engagement (`habit_gain`, Spearman -0.63), H6 where habit forms slower and
  the engagement ranking's lead disappears (+0.63).
- **Out of sample:** V4 (95%), V8 (78%) and V9 (71%) are stable in sign (95%, 100%, 88%), V9's failures being seed
  noise (seed share 0.91). The failures of V2, V5, V6, V7 and V10 are robust: 56 to 100 clear failures each.

## 6. Sensitivity (global)

### 6.1 Hold shares across the revised ranges

240 worlds, 49 of them degenerate (`exports/robustness/global-1/summary.md`). Hold shares over all worlds and over the
191 non-degenerate ones; "sign" for the main check, over all worlds (a world without a value counts as not sharing
the sign). The signs of V6, V7 and V8 were recomputed with the corrected neutral of 1 (2.5;
`exports/validation-analysis/sign_shares.txt`).

| Item | All | Non-degenerate | Sign | Item | All | Non-degenerate | Sign |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Fact 1 | 48% | 50% | 100%, 71% | H1 | 26% | 32% | 92% (harm) |
| Fact 2 | 9% | 9% | 34%, 29% | H2 | 46% | 50% | 52% (survey) |
| Fact 3 | 21% | 20% | 91% (time) | H3 | 95% | 98% | 100% |
| Fact 4 | 92% | 96% | 100% | H4 | 77% | 82% | 81% (harm) |
| Fact 5 | 32% | 40% | 82% | H5 | 82% | 81% | 98% (engagement) |
| Fact 6 | 7% | 7% | 85% | H6 | 56% | 57% | 56% |
| Fact 7 | 87% | 92% | 96% (time) | H7 | 57% | 59% | 57% |
| V4 | 61% | 68% | 61% | H8 | 31% | 27% | 31% |
| V8 | 56% | 68% | 72% | V9 | 48% | 52% | 75% |
| V1 | 6% | 7% | - | V2 | 2% | 2% | 83% (informedness) |
| V3 | 25% | 23% | - | V5 | 4% | 5% | 82% (extremes) |
| V6 | 5% | 6% | 59% (ranking), 100% (likes) | V7, V10 | 2%, 0% | 3%, 0% | 39% (at 1), 77% (informedness) |

The global ranges are ranges to explore, not beliefs: they include worlds where a fact's mechanism is off or doubled,
and model.md section 15 names failing worlds inside them (`outrage_appeal` 0.6 fails fact 2; `time_cost` 0.2 fails
fact 6's upper bound). Low global hold shares therefore map where a mechanism operates; they do not
by themselves refute it. The direction is the better robustness measure here:

- **Robust across the ranges:** forwarding limits (96% non-degenerate), notifications off (92%), the strict
  classifier's trade-off (H3, 98%), the trap solution (H4, 82%), notifications raising engagement (H5's engagement
  check, 100% non-degenerate). Q&B cuts harm in 92% of worlds (H1's sign), though by the claimed "far more" (at least
  half) in only 52% of the non-degenerate ones; the engagement ranking shows more outrage than the chronological feed
  in every world.
- **Hinges on the ranges:** fact 2's rise of outrage has the default's sign in only a third of the worlds; fact 6 lies
  above its band in 76% of non-degenerate worlds (`band_sides.txt`; their median effect is 0.889 SD, n 191, against
0.738 SD over all worlds with a value, `recheck.txt`); H8's backfire is reversed in 73%
  of non-degenerate worlds; H2's survey and H6's lasting lead split about evenly.
- **Out of sample:** no V finding with a failure at the defaults passes in more than 25% of worlds, and the wrong signs
  of V2 and V10 informedness persist (83% and 77% of worlds share the default's positive sign). V6's ranking ratio
  lies above 1 in 99 of the 240 worlds (ranking raises cross-cutting exposure there, against the field's mild trim),
  V7 stays at 1.000 in 93 and lies below it in 96 (51 have no noted post with shares), and V8's Outrage share rate
  exceeds the other styles' in 172 worlds and falls below it in 17.

### 6.2 Degenerate worlds

49 of the 240 revised worlds (20%) and 13 of the 64 documented worlds (20%) are degenerate; none of the 100 local
worlds is (`exports/validation-analysis/analysis.txt`). Under the revised ranges the platform empties when habit forms
slowly and churn is high: half the worlds in the lowest quartile of `habit_gain` (below 0.028) are degenerate, against
8% to 13% in the others, and a third of those in the upper half of `churn_base` (above 0.05), against 2% in its lowest
quartile (Spearman with the degenerate flag -0.33 and +0.33). Under the documented ranges the strongest driver is
`leave_threshold` (+0.52): 56% of the worlds in its top quartile (above 0.25) are degenerate, none in its bottom
quartile.

### 6.3 Drivers of the key results

Non-degenerate worlds of `exports/robustness/global-1/` (191; PRCC from `summary.md`, where the Spearman correlation
of every driver listed has the same sign). R2 is the adjusted R2 of a rank regression of the effect on the 51 varied
numbers (`exports/validation-analysis/analysis.txt`): the share of the effect's rank variance that additive monotone
effects explain. With 191 worlds and 52 other terms partialled out (50 assumptions and two dummies for
`bot_strategy`), the standard error of a PRCC is about 1/sqrt(136) = 0.086, so values below about 0.17 are not
distinguishable from 0.

| Result | Holds | Top drivers by PRCC | R2 |
| --- | --- | --- | --- |
| Fact 1, outrage impressions | 75% | `outrage_appeal` +0.76, `base_session_items` -0.71, `stop_bias` +0.41 | 0.74 |
| Fact 2, rise over no learning | 15% | `outrage_appeal` -0.60, `learning_rate` -0.58, `imitation_rate` +0.43 | 0.64 |
| Fact 3, time spent | 46% | `stop_bias` -0.65, `arousal_appeal` +0.63, `base_session_items` +0.43 | 0.61 |
| Fact 6, deactivation | 7% | `reflective_quality_weight` -0.56, `time_cost` +0.51, `outrage_appeal` +0.50 | 0.63 |
| H1, harm | 52% | `style_quality_gap` -0.41, `stop_bias` +0.33, `choice_temperature` +0.33 | 0.42 |
| H1, engagement | 52% | `social_proof` +0.49, `stop_bias` -0.40, `outrage_appeal` -0.37 | 0.49 |
| H2, survey | 54% | `survey_impulsive_weight` +0.37, `social_proof` -0.36, `survey_habit_weight` +0.33 | 0.48 |
| H6, lead lasts | 57% | `social_proof` -0.49, `stop_bias` +0.40, `outrage_appeal` +0.36 | 0.48 |
| H7, report trust (weak) | 59% | `outrage_appeal` -0.18, `style_quality_gap` +0.17, `ad_rate` -0.16 | 0.02 |
| H8, see-less backfire | 27% | `clickbait_penalty` -0.56, `outrage_appeal` +0.28, `choice_temperature` +0.25 | 0.27 |
| V2, informedness | 6% | `style_quality_gap` -0.55, `credulity_scale` +0.47, `reflective_quality_weight` -0.37 | 0.50 |
| V5, extremes (weak) | 15% | `bot_share` -0.26, `evasion_cost` -0.26, `style_quality_gap` -0.25 | 0.15 |
| V6, likes | 22% | `outrage_appeal` -0.76, `reflective_quality_weight` +0.50, `base_session_items` +0.44 | 0.74 |
| V7, before the note | 3% | `base_session_items` -0.79, `outrage_appeal` -0.69, `habit_gain` -0.28 | 0.69 |
| V8, Outrage shares | 68% | `arousal_appeal` -0.42, `style_quality_gap` -0.27, `stop_bias` +0.27 | 0.25 |
| V10, informedness | 15% | `base_session_items` +0.32, `habit_gain` +0.28, `wrongful_sanction_cost` +0.21 | 0.12 |

Six assumptions take 84 of the 138 top-three places over the 46 required checks: `outrage_appeal` (21),
`arousal_appeal` (15), `stop_bias` (14), `base_session_items` (13), `reflective_quality_weight` (11) and
`habit_gain` (10) (`exports/robustness/global-1/summary.json`, counted in this analysis). The model's results rest
mostly on how strongly hostility and arousal pull, how long sessions run and how habit forms.

`bot_strategy` matters for some results (medians per strategy, `summary.md`): V5's partisan-half reach ratio is 0.54
with adaptive bots (the default) against 1.14 and 1.09 with own-side and shared-pool bots, and H1's harm cut is -0.31
with adaptive bots against -0.58 and -0.60.

### 6.4 Weak correlations

Six required checks and one known limit have no assumption with |PRCC| above 0.3, and their rank regressions explain
little: H7 (adjusted R2 0.02), V1's toxic share (0.04), V2's later use (-0.03), V5's extremes (0.15), V10's
misinformation share (0.20) and engagement (0.13), and the known limit V3's own toxicity (0.03)
(`exports/validation-analysis/analysis.txt`). For these, seed noise is a large part of the spread across worlds (seed
share 0.42 for H7, 0.40 for V1's toxic share, 0.57 for V10's engagement), so their drivers cannot be read from 2
seeds. Where PRCC and Spearman disagree (for example `time_cost` for fact 3's time spent, PRCC -0.29 against Spearman
-0.01), the assumption matters only once the others are held fixed.

### 6.5 The documented ranges

The 64 worlds on section 11's ranges as written before the revision (`exports/robustness/global-2-documented/`) differ
from the revised ones where calibration moved a default far: `polarisation_rate` up to 0.002 (13 times the default,
against 5 times in the revised ranges), `habit_gain` up to 0.3, `intensity_rate` up to 0.5, `classifier_retraining`
up to 1, `contact_rate` up to 0.01, and `leave_threshold` from -0.5 to 0.5. The degenerate share is the same (20%),
but its cause moves to `leave_threshold` (6.2). The polarisation checks fail more often: fact 3's polarisation check
holds in 27% of the worlds against 41% in the revised run, V2's in 27% against 42%, V10's in 77% against 92%
(`exports/validation-analysis/analysis.txt`). The other item hold shares differ from the revised run's by up to 13
points over all worlds (documented minus revised, `exports/validation-analysis/recheck.txt`):

| Item | Documented | Revised | Difference (points) | Standard errors |
| --- | --- | --- | --- | --- |
| V8 | 69% | 56% | +13 | 1.9 |
| V9 | 36% | 48% | -12 | 1.8 |
| H5 | 92% | 82% | +11 | 2.5 |
| V3 | 14% | 25% | -11 | 2.0 |
| Fact 3 | 12% | 21% | -8 | 1.7 |
| H8 | 39% | 31% | +8 | 1.2 |
| Fact 2 | 2% | 9% | -7 | 3.0 |

The standard error of a difference of two proportions from 64 and 240 worlds is about 7 points at mid-range shares
and smaller near 0% or 100%. Fact 3's difference follows from its polarisation check (above). The others lie within
about 2 standard errors, except fact 2 (3.0) and H5 (2.5); with 25 items compared, about one difference beyond 2
standard errors is expected by chance, so the documented ranges may also lower fact 2's and raise H5's hold shares,
which this run cannot settle. The run shows that the documented ranges were written
for the text defaults and reach far past the calibrated ones on polarisation, and that `leave_threshold`, fixed in the
revised runs, is the single strongest cause of collapse when it varies.

## 7. Overall assessment

The lab fits what it was tuned on and gets the direction of most out-of-sample findings, but its accuracy comes mostly
from built-in or calibrated directions, and its genuine tests pass 5 of 14 (4.2).

**It can be trusted for** the direction of the first-order trade-offs it was built around: an engagement ranking
amplifies outrage and costs reflective value (fact 1; sign in every global world), a calmer feed cuts harm (H1's sign
in 92% of global worlds), a strict classifier trades toxicity for wrongful sanctions (H3), forwarding limits shrink
cascades (fact 4), notifications raise engagement without wellbeing gains (H5), and a feed change moves time and
engagement in the field's direction (V2 later use, V3 and V10 engagement, fact 3's time), with V3's time effect far too
small (-1.6% against -10%, undecided at the defaults and too small in 77% of local worlds). V1's time effect lies
inside a band around the field's null, and the engagement bands (-0.30 to 0 and -0.5 to -0.01) test little beyond
direction. Each of the checks named, except V3's time, holds in 96% to 100% of the local worlds
(`exports/robustness/local-1/summary.md`). Across the global ranges
(`exports/robustness/global-1/summary.md`) the directions mostly survive: fact 1's outrage and fact 3's time keep the
default's sign in 100% and 91% of worlds and H1's harm cut in 92%, and H3, fact 4 and H5's engagement check hold in 96%
to 100% of the non-degenerate worlds; the in-band magnitudes of the time and engagement effects hold in only 46%
(fact 3's time) to 88% (V1's time) of them.

**Its robust headline claims** are H3, H4, H5 and H7 and H8 locally (100% of local worlds each, H5 96%), and H3, H4 and
H5 across the global ranges (81% to 98% of non-degenerate worlds). H1 is robust in direction only. H2 depends on the
survey's assumed weight on impulse, and H6 on habit formation and social proof. Seed noise makes up a good part of
H7's global spread (seed share 0.42), and its R2 of 0.02 may also reflect interactions that a rank regression misses.
H8 reverses across the global ranges with the Clickbait penalty.

**It cannot be trusted for:**

- informedness and news knowledge: leaving the platform or removing reshares raises informedness, against the field,
  in 83% to 99% of worlds (V2, V10);
- how reach is distributed across partisanship: the most partisan authors are amplified 2.25 times as much as the
  middle (V5), by the ranking rather than the moderation measures (2.17 with the measures held), largely through
  trending recommendations; a chronological arm with a different follow graph may contribute, untested;
- community notes: no shares follow a note (V7);
- the magnitude of cross-group liking (V6), and any link between exposure and incivility (V1 toxic share, V3);
- fact 6's magnitude and the regretted share: fact 6 holds in half the local worlds, and the regretted share misses
  its held-out band by a factor of about 50.

## 8. Recommended modifications and additions

In order of priority. "Model" changes the dynamics, "tooling" the CLI or measurement, "documentation" model.md or the
exhibits. A change that tunes the model on a V check makes that check in-sample; each item says how to avoid that.

1. **Rebalance informedness (model; recalibration needed).** What: calibrate the prevalence of misinformation and the
   two belief constants (loss 0.008, gain 0.0004, model.md 13.1) to in-sample targets, for example the field's share of
   exposures from untrustworthy sources (about 1%, against the model's 13%) and a per-exposure learning rate, then rerun
   V2 and V10 as out-of-sample checks. Why: both informedness checks fail with the wrong sign, robustly (sections 4.3
   and 5.2). The hypothesis behind this item, not yet tested, is that the misinformation loss outweighs the gain for
   regular users because calibration cut the two constants at different rates (4.3). Expected effect, if it holds: the
   sign flips; with the gain alone the break gives -0.0141 raw (-0.69 SD), so the gain channel is large enough. Side
   effects: the benefit index, the misinformation surge scenario and V1's and V10's misinformation checks need
   re-checking.
2. **A mid-run feed switch on a shared follow graph (model and tooling).** What: a feed experiment kind that moves a
   random share of users to the chronological feed (and one that holds back ranking) for the last days of a run, so both
   arms share the follow graph the ranked feed built. Why: V5 and fact 3's two known limits compare feeds that ran from
   day 0 on different follow graphs (4.3; model.md 16), which may contribute to V5's failure; trending recommendations
   carry most of V5's extra amplification (2.24 to 1.22), and the ranking, not the integrity measures, makes it (2.17
   with the measures held in both arms). Expected effect: V5 and fact 3's toxic and misinformation checks can copy the
   field designs; their verdicts may change. Recalibration: none for the dynamics; fact 3's design change must be
   declared as a change to an in-sample fact. A measurement-only split of reach into in-network and recommended
   impressions (tooling) would show how much of V5 survives on in-network reach.
3. **Notes that can arrive in time (model; recalibration of the notes scenario).** What: let notes attach within a
   post's first day (between sessions, or after a delay drawn from field note timing), and record the integrity preset's
   `noted_penalty` of 1.0 as a design choice that hides noted posts; first test the timing hypothesis with a run in
   which notes arrive earlier. Why: no share follows a note in any local world. Of the gap of 0.15 to the field's 0.85,
   removing the penalty closes 0.056 (0.944 at 0, 1.000 at 1.0), about a third; the rest is unexplained. Late notes are
   a hypothesis to test: the post-lifetime run (4.3) does not change when notes arrive. Expected effect: V7 moves toward
   0.85 if the timing hypothesis holds. Recalibration: no target reads notes; the misinformation scenario's verdicts
   need re-checking. Choosing the penalty to pass V7 would make V7 in-sample, so the penalty should be set by judgement
   first.
4. **Ranges and noise design (tooling and documentation).** What: (a) hold `base_session_items` fixed or narrow it
   (now 5 to 60 around 20): it changes the time scale, the reason the checking habits are held fixed, and it is a
   top-three driver of 13 of 46 checks (6.3); (b) give `leave_threshold` a judgement range around 0, fixed before the
   next run, since it is the strongest cause of collapse when it varies (6.2, 6.5); (c) add a set of plausible ranges
   beside the ranges to explore, and report hold shares on both (6.1); (d) run 4 seeds per world for checks whose
   seed share exceeds 0.4 (in the local run fact 2's second check, fact 5, H5's wellbeing, V1's toxic share, V3's time
   and engagement, V4, V5's extremes, V9 and V10; in the global run also H7), and let the robustness summary report
   noise-compatible against clear failures, a PRCC significance mark (|PRCC| above 2 SE) and the rank-regression R2.
   Why: sections 5.2, 6.1 and 6.4. Expected effect: hold shares that separate seed noise from world dependence, and
   global results that reflect beliefs as well as exploration. No recalibration.
5. **Out-group liking (model and tooling; recalibration of facts 1 and 2 if changed).** What: make the out-group
   hostility cut (0.8 in `LIKE_OUTGROUP_HOSTILITY_CUT`) a world assumption, add a measurement-only decomposition of
   `P_like` by term, and examine whether in-group hostility should lift same-group likes as strongly as it does now.
   Why: V6's likes fail robustly (85% of local worlds below the band), and the in-group appeal of hostility is the
   dominant term (0.66 to 0.92 without it; 4.3). Expected effect: a ratio inside 0.75 to 0.97. Facts 1 and 2 rest on
   the same term (`outrage_appeal` is their top driver), so any change must be re-checked against them.
6. **State what the exhibits rest on (documentation).** What: in model.md 13.3, 13.4 and the exhibits, state that
   fact 6 is a calibration identity on a knife edge (it holds in 50% of local worlds, and the SD of its world means,
   a value of 0.197, is nearly the band's width of 0.23), that H2 depends on the survey's weight on impulse, H8 on the
   Clickbait penalty, H6 on habit and social proof, and that H1 is robust in direction, not in magnitude. Why:
   sections 5.3 and 6.1. No recalibration.
7. **Exposure shapes posting (model; recalibration).** What: a term by which the hostile share of what an author sees
   moves their hostile intensity or style values. Why: V3's own toxicity (the known limit) and V1's toxic share show no
   link between exposure and incivility (4.3). Expected effect: V3's own toxicity falls under hiding; fact 2 and the
   toxic share target must be re-checked, since the term feeds back into the outrage dynamics.
8. **Report raw scales and ratio conventions (tooling and documentation).** What: print each SD-scaled effect with the
   SD used and, where the field gives one, the field's raw change; say whether a ratio effect is a ratio of seed means
   or a mean of per-seed ratios (V5: 2.25 as a ratio of means, 2.35 as a mean of ratios on the same seeds,
   `exports/validation-analysis/diag/v5-ratios.txt`); compute the ratio diagnostics of V5, V6 and V8 from sums
   pooled over the summary window instead of a mean of daily ratios, which carries Jensen bias and, for
   `extreme_reach_ratio`, rests on few posts a day; and report reach per post (V5) both with and without the
   impressions of posts later removed or demoted, which it now includes (4.3). Why: informedness's SD depends on how
   unequal credulity makes people (about 0.08 at the defaults, 0.02 without belief in misinformation, derived from
   4.1's raw and scaled changes and `diag/v2v10-credulity0/validation.md`); the ratio and reach conventions change the
   numbers V5, V6 and V8 are judged on, and V5's two arms moderate differently (removal against demotion). No
   recalibration; a pooled ratio is a measurement change, so V5, V6 and V8 must be rerun and their bands kept.

## 9. Limits of this validation

- **Loose mappings.** Several checks map a field measure onto a model analogue that is only roughly the same: clicks
  on hard news onto likes of in-network posts (V6), news knowledge onto informedness (V2, V10), untrustworthy-source
  exposure onto posts with `truth` below 0.5 (V1, V10), an accuracy prompt onto share friction (V4), politicians'
  and news sources' amplification onto authors ranked by a partisanship trait (V5).
- **Field results from other settings.** Some bands come from studies of a whole phone or another product: Dekker et
  al.'s notifications switch-off was phone-wide (fact 7), Pandora's ad load elasticity comes from music streaming
  (V9). They are used here for one platform.
- **Dose mapping.** V1's and V3's doses are matched as manipulation checks under the integrity preset; under the
  engagement ranking the same settings give other doses (V3: 0.212 against 0.073 hidden), so the engagement column is
  not at the field's dose.
- **Spillover.** The feed experiments treat 20% of the regular users (V1, V3, V10), against a negligible share of a
  platform's users in the field studies. In the model the treated make up a fifth of the audience, so what they stop
  seeing, liking and sharing changes what everyone else sees and does, which feeds back to the treated; their change
  includes these knock-on effects, which the placebo run, with nobody treated, does not remove. V2's break also takes
  20% of the regular users off the platform, and fact 6's break all of them (an in-sample design).
- **SD scaling.** The model's informedness SD is small (about 0.08 at the defaults on a 0 to 1 scale, 0.02 without
  belief in misinformation), so effects in SDs are sensitive to how heterogeneous people are; the informedness
  failures are failures of sign, which the scaling does not affect.
- **Unverified sources.** V10 rests on the abstract of Guess et al. (2023b) only, and V3 on a working paper
  (Beknazar-Yuzbashev et al., CAGE WP 741/2025, May 2026 draft); both judge directions with wide bands. V6's figures
  come from the authors' blog summary of Bakshy et al. (2015), not the paper (`exports/research-validation/notes.md`).
- **2 seeds per world.** Per-world verdicts in the robustness runs carry seed noise; section 5.2 separates it where
  the per-seed values allow, and the checks named in 8.4 (d) need more seeds to read their drivers.
- **Coverage.** `leave_threshold` is fixed in the revised global runs, and the checking habits and the interruption
  cost are never varied; the documented-range run has too few worlds for PRCC. Design and run settings (population
  size, run length, post lifetime) are not varied, and results depend on the run length (model.md 16).
- **The diagnostics** of 4.3 use 8 seeds per setting and switch world assumptions, not code terms: `outrage_appeal` 0
  removes a term and changes the content mix, and the out-group cut and the belief constants could not be switched
  without a code change.
