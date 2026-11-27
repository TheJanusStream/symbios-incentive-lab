# symbios-incentive-lab

A simulation of incentive systems on a social platform, played like a game. Turn platform features on and off, turn
the dials, and watch what happens to engagement, to harm, and to the people on the platform and beyond it: non-users,
public discourse and creators' livelihoods.

The question it explores: can engagement maximisation coexist with incentives that keep toxic outcomes low and
positive side effects high, and where does that break?

**Status:** v0.1 in development. The model is specified in [docs/model.md](docs/model.md), whose section 13 records
how it was calibrated and what the explorations of the design space found. The simulation core, the command-line
runner with its explorations, and the game app with its atlas (desktop and browser) work.

## How it works

Simulated people read feeds, write posts, like, reply, share, report, get harassed, learn which kinds of posts pay,
and join or leave the platform. The platform ranks feeds, moderates and pays creators using only what it can observe;
the lab shows both what the platform sees and what is actually true, so the gap between the two (Goodhart's law at
work) is visible. Design levers (what the platform chooses) are kept apart from world assumptions (how people are
assumed to behave), so a conclusion can be tested against a different world.

It is a toy model for building intuition, not a forecast of any real platform. Claims come from many seeded runs with
their spread, never from a single run.

## Layout

- `crates/incentive-core`: the engine-free, deterministic simulation, the statistics over seeds that the runner
  and the app share, and the atlas file types (`atlas`) that the runner writes and the app reads.
- `crates/incentive-cli`: batch runs, side-by-side comparisons, sweeps, scenarios, factorials, the calibration
  report and explorations of the design space from the command line. Only the runner depends on `symbios-genetics`.
- `crates/incentive-app`: the game app, binary `symbios-incentive-lab` (Bevy and egui, desktop and browser). Its
  library holds the lab's state, logic and views without Bevy; the binary is a thin Bevy layer around it.
  `assets/atlas/` holds the precomputed explorations the Atlas tab embeds.
- `index.html` and `.github/workflows/deploy.yml`: the browser build, deployed to GitHub Pages.

## Game app

Run it natively from the repository root:

```text
cargo run --release -p symbios-incentive-lab
```

The browser build is what `.github/workflows/deploy.yml` publishes to GitHub Pages: it builds the app for
`wasm32-unknown-unknown` with the size-tuned `wasm-release` profile, runs `wasm-bindgen` 0.2.128 (the version the app
pins) and serves the result with `index.html`. To try it locally, run the same three steps and serve `dist/` with any
static file server:

```text
cargo build -p symbios-incentive-lab --profile wasm-release --target wasm32-unknown-unknown
wasm-bindgen --out-dir dist --target web --out-name symbios-incentive-lab --no-typescript \
    target/wasm32-unknown-unknown/wasm-release/symbios-incentive-lab.wasm
cp index.html dist/
```

The population starts at 2,000 people natively and 1,000 in the browser; both can be changed. The simulation advances
within a time budget of about 10 ms per frame, so the window stays responsive at any speed.

A tour of the views:

- **Top bar:** the tabs, play and pause, one day at a time, the speed (1, 5 or 20 days per second, or as fast as the
  frame budget allows), restart, the seed, the day counter, and "Smooth (7-day mean)", on by default, which draws
  every daily chart and sparkline as the mean of the last 7 days. Tables and verdicts always use the raw values.
- **Sandbox:** your platform. On the left a preset picker and every design lever, grouped; a change applies from the
  next day. Below them the world assumptions (beliefs about people, not choices, applied at a restart), population and
  run length, and copying or pasting the configuration as JSON for `incentive run --config`. In the centre the
  engagement, harm and benefit indices, then what the platform sees next to what is true, one small chart per chosen
  metric with its own scale and its latest value, grouped by family, and breakdowns by style and archetype. Every
  chart's time axis fits the days run so far (at least 30). A dashed line marks each design change; hover it to see
  what changed. A short "Start here" box suggests three experiments from the documented findings.
- **Inspector** (in the Sandbox): the tracked individuals, one individual's traits, state, learned style mix and
  history, and the feed they were last served. Each item shows its ranking score with its components, the platform's
  predicted engagement next to the viewer's true appeal and reflective value, the post's hidden quality, toxicity and
  truth, and what the viewer did. Items scored high that the viewer would rather not have seen are highlighted: the
  Goodhart gap in one line.
- **Compare:** two designs on the same seed and world in lockstep, overlaid charts, a table of the last 30 days, and
  "Check on 8 seeds", which runs both designs on seeds 9 to 16 (seeds the Atlas searches never selected on) and
  reports the means and the paired difference with a 95% confidence interval, computed as `incentive compare` does.
  "Show components" adds the benefit index's five parts, the two indices without polarisation and without creators,
  and net revenue. A `~` marks a mean that was still moving at the end of its runs (docs/model.md, section 9).
- **Scenarios:** the five challenges. Your design runs against the fixed baseline; at the end every goal is judged on
  that one run, "Confirm on 8 seeds" judges each goal on its paired per-seed margins with a 95% interval (pass, fail
  or undecided, as `Scenario::judge` does), and a known solution waits behind a spoiler question.
- **Atlas:** precomputed explorations of the design space, embedded in the app so they work in the browser too. A map
  of engagement (across) against harm (up), each square coloured by the benefit index of the best design found there;
  the Pareto front of engagement, harm and benefit as a scatter; and two Goodhart runs, where a search maximised the
  engagement index or the survey alone while the truth was recorded. Hover a square or point for its metrics (with the
  engagement, harm and benefit re-run on the fresh seeds 9 to 16, and the benefit index's five parts) and
  the levers that differ most from the engagement ranking; pick it to open it in the Sandbox at full size or to compare
  it with the engagement ranking. The settings the designs were found at are shown with each view.
- **About:** what the lab is, how to read observable against true and levers against assumptions, how much to trust
  it, the stylized facts with their sources, and findings to discover.

For a screenshot without playing, `symbios-incentive-lab --screenshot shot.png --tab compare --days 120` (native only)
runs 120 days on seed 1, shows the tab, saves one PNG of the window and exits. The tabs are `sandbox`, `inspector`,
`compare`, `scenarios`, `atlas` and `about`.

## Command-line runner

The runner is the `incentive` binary of `crates/incentive-cli`. Build it in release mode and call it from the
repository root; every command prints a table and writes its files to `exports/` (ignored by git) unless `--out`
names another folder. Build it on its own, with `-p incentive-cli`: a workspace-wide build unifies shared dependencies'
features with the app's and changes the last bits of some draws, so its runs drift from the calibration record
(section 12 of [docs/model.md](docs/model.md)).

```text
cargo build --release -p incentive-cli
target/release/incentive presets              # the preset designs, explained
target/release/incentive levers               # every design lever and world assumption with its range
target/release/incentive scenarios            # the scenarios and their goals
target/release/incentive config --preset calm-by-design > my-design.json
```

Run one design on several seeds (`--seeds 1..8` is the default; a list such as `1,5,9` works too). Any field can be
changed with `--set path=value`, and `--days` and `--population` shrink a run for quick experiments:

```text
target/release/incentive run --preset chronological --seeds 1..8
target/release/incentive run --config my-design.json --set world.outrage_appeal=2 --days 180
```

Compare two designs paired by seed, with a 95% confidence interval for each difference:

```text
$ target/release/incentive compare --a engagement-maximiser --b quality-bridging \
    --metrics engagement_index,harm_index,benefit_index,toxic_share,survey_satisfaction
A = engagement-maximiser, B = quality-bridging: 8 paired seeds, N = 2000, 365 days

metric                     view        A       B    B - A  95% low  95% high
-------------------  ----------  -------  ------  -------  -------  --------  -
Engagement index          index   0.1521  0.1267  -0.0254  -0.0286   -0.0222  *
Harm index                index  0.3534~  0.0614  -0.2920  -0.3227   -0.2613  *
Benefit index             index   0.4047  0.4360   0.0313   0.0209    0.0417  *
Toxic impressions         truth   0.2089  0.0039  -0.2050  -0.2446   -0.1653  *
Survey satisfaction  observable   0.5438  0.5369  -0.0069  -0.0124   -0.0014  *
```

A trailing `~` marks a metric that is still moving inside its summary window: its least-squares change across the
window, averaged over the seeds, exceeds 10% of its mean and two standard errors (across the seeds, or from the
regression for one seed; section 9 of [docs/model.md](docs/model.md)), so the window averages a series that has not
settled. `run`, `compare`, `sweep`, `scenario` and `factorial` mark such metrics, and every summary in the JSON files
carries each metric's `slope`, its standard error `slope_se` and the list of `moving` metrics.

`--set-a` and `--set-b` change one side only, so one lever can be tested against the same design:

```text
target/release/incentive compare --a engagement-maximiser --b engagement-maximiser --set-b design.w_reply=0
```

Sweep one lever or assumption over a list of values (one row per value, mean and spread, and a CSV):

```text
target/release/incentive sweep --preset engagement-maximiser --lever design.w_see_less --values 0,1,2,5
```

Play a scenario: your design (a preset, a configuration file or `--set` changes) runs against the baseline on the
same seeds, and every goal is judged on the paired seeds. `--solution` runs the recorded known solution:

```text
$ target/release/incentive scenario engagement-trap --set design.w_reply=0 --set design.w_see_less=5 \
    --seeds 1..4 --population 600 --days 120
goal                                                             you  baseline  threshold   margin  95% low  95% high
------------------------------------------------------------  ------  --------  ---------  -------  -------  --------  ---------
Keep at least 85% of the baseline's engagement index          0.1599    0.1643     0.1396  +0.0203  +0.0078   +0.0328       PASS
At most half the baseline's toxic impression share            0.1303    0.1913     0.0956  -0.0347  -0.0979   +0.0285  UNDECIDED
At most twice the baseline's wrongful removals and demotions   1.111     1.215      2.431  +1.3194  +1.1137   +1.5252       PASS

Scenario not passed: 0 goal(s) failed, 1 undecided (more seeds may decide them).
```

The margin is how far each seed's run lies on the passing side of the threshold set by its own baseline, with a 95%
interval over the seeds. A goal passes when the whole interval is on the passing side, fails when it is wholly on the
failing side, and is undecided otherwise; a scenario passes only when every goal passes (section 14.2 of
[docs/model.md](docs/model.md)). More seeds narrow the intervals.

Check the stylized facts of the model at full size, with effect sizes, the 95% interval of each per-seed effect and
a verdict on it. `--seeds` is the number of paired seeds, `--fact` checks single facts by id (here fact 6, the
deactivation experiment, on 16 seeds) and `--set path=value` checks them in another world:

```text
target/release/incentive facts --seeds 8
target/release/incentive facts --fact deactivation --seeds 16
```

Check the model out of sample with `validate`: ten literature findings, V1 to V10, that calibration never used, with
bands fixed before the model was run on them (lab #31; the battery is `crates/incentive-core/src/validation.rs`, kept
apart from the stylized facts). Every check runs under the integrity preset and is judged there, as the facts are;
the same check under the engagement ranking is reported beside it as a sensitivity, not judged. The experiments copy
the field designs: a random 0.2 of the regular users is treated (a feed experiment for V1, V3 and V10, a five-week
break for V2), and each real run is paired with its placebo on the same seed. V4, V5 and V9 pair the design with
their comparison design, and V6, V7 and V8 read one run. For each check the table gives the effect with its standard
deviation across seeds, the 95% interval, the band, the verdict (pass, fail or undecided, judged as the facts are),
the raw change (the unscaled quantity behind the effect, beside any change in standard deviations) and the
engagement ranking's effect; V1 and V3 also report their doses, manipulation checks fixed by calibration. `--seeds`
(default 8), `--check` (ids, comma-separated), `--set path=value` (a world in which to check the findings) and
`--out` (default `exports/validation/`) are as for `facts`; the folder holds `validation.md`, with each check's
source quoted, and `validation.json`:

```text
target/release/incentive validate
target/release/incentive validate --check V1,V3 --set world.homophily=0.8
```

Regenerate the calibration tables of section 13 of [docs/model.md](docs/model.md) as `report.md`, Markdown ready to
paste: every preset at 365 and at 90 days (mean ± standard deviation, moving metrics marked), a list of named paired
comparisons with their differences, 95% intervals and ratios (the data table `COMPARISONS` in
`crates/incentive-cli/src/report_cmd.rs`, easy to extend), the stylized facts, and each scenario's known solution and
naive attempt (one obvious lever that fails) with their verdicts. `--seeds` (default `1..8`), `--population`
(default 2,000) and `--days` (the long horizon, default 365; given, it also sets the scenarios' days) size it; the
file goes to `exports/report/` unless `--out` names another folder. Identical runs are shared between the tables,
and the whole report is deterministic:

```text
target/release/incentive report --seeds 1..8
```

Take a difference between two designs apart with a full factorial: every subset of up to six levers of design B set to
design A's values, all cells paired by seed (default seeds `9..16`). It prints each cell's engagement, harm, benefit,
toxic share and wrongful sanctions, then for engagement, harm and benefit each lever's main effect, the two-way
interactions and each lever's Shapley share of the difference, all in the direction from A to B with 95% intervals:

```text
target/release/incentive factorial --a engagement-maximiser --b front-32.json --days 365 --population 2000 \
    --levers notification_intensity,w_bridging,w_dwell,w_like,payout_share,predictor_quality
```

A main effect is the mean change when one lever goes from A's value to B's, over every setting of the others; an
interaction is the change with both at B's values minus the two changes alone, averaged over the others; the Shapley
values add up to B minus the cell with every listed lever at A's value, which is A itself when the levers are all the
ways the two designs differ. A and B must share run settings, population and world (`--days` and `--population`
give both the same size). `--a` and `--b` take preset ids or configuration files, as `compare` does.

A design found by `incentive explore` becomes such a file with `config --from-atlas`: a Pareto front design by its
number from 1 (as printed and in the app), a map cell by `x,y` (column and row from 0, as printed), a Goodhart design
by its generation number or `last`. The file keeps the search's base configuration, its reduced size included, so pass
`--days 365 --population 2000` to `compare` or `factorial` to run it at full size:

```text
target/release/incentive config --from-atlas crates/incentive-app/assets/atlas/pareto.json --design 32 > front-32.json
```

### Robustness across worlds

`robustness` asks whether the model's results survive other worlds. It runs a battery in many sampled worlds: the
stylized facts (`--battery facts`), the headline claims H1 to H8 (`--battery headline`, the exhibit claims of
section 13.3 of [docs/model.md](docs/model.md) written as band checks, in `crates/incentive-core/src/headline.rs`),
the out-of-sample checks V1 to V10 under the integrity preset (`--battery validation`, as `validate` judges them),
or all three (`all`, the default). Each world runs every variant of every fact or claim on seeds 1 to
`--seeds` (default 2), paired across variants and the same in every world, at each battery's full size
(`N = 2,000`, 365 days) unless `--population` and `--days` shrink it. A check holds in a world when its effect on
the seed means lies in its band; a fact or claim holds when every check that is not a known limit does.

```text
target/release/incentive robustness --mode local --worlds 100 --seeds 2
target/release/incentive robustness --mode global --worlds 200 --battery headline --sample-seed 3
target/release/incentive robustness --mode global --ranges plausible --worlds 200 --seeds 2 --seeds-noisy 4
```

Which world assumptions vary, and how:

- Varied, 54 assumptions: the 52 world assumptions of the parameter catalogue (`world.*`, listed by
  `incentive robustness --help` and in the output) except the held ones, and two population settings, the troll share
  (`archetype_shares.troll`, relative, normalised with the other archetypes' shares when the population is drawn) and
  the bot share (`bot_share`). Run and design settings and the rest of the population never vary.
- Held at their defaults in every mode: the checking habits (`recheck_rate`, `notification_pull`,
  `extra_session_share`) and `interruption_cost`, off by the owner's decision (sections 6.1 and 13.2), and the
  session length `base_session_items`, held by lab #35 for the same reason: all of them change the time scale at
  which every per-impression rate was calibrated.
- `--mode local`: each varied number with a nonzero default is multiplied by U(1 - jitter, 1 + jitter) (`--jitter`,
  default 0.25) and clamped to its catalogue range. Assumptions whose default is 0 (mechanisms off:
  `leave_threshold`, `polarisation_backfire`) and the categorical `bot_strategy` keep their defaults.
- `--mode global`: a Latin hypercube over each varied number's range, from one of three range sets, all listed in
  section 11 of [docs/model.md](docs/model.md):
  - `--ranges revised` (the default), the ranges to explore: section 11's documented ranges, revised by the rule of
    lab #31: a range written for a text default that calibration changed (13.1) is scaled by the same factor as the
    default (`classifier_retraining`, `intensity_rate`, `habit_gain`, `polarisation_rate`, `contact_rate`); an
    assumption without a documented range gets default/3 to 3 x default (`ad_rate`, `ad_annoyance`,
    `subscription_hurdle`, `living_income`); the troll share and the bot share get default/2 to 2 x default. Three
    ranges are set by judgement: `leave_threshold` -0.1 to 0.1 (its default 0 stays in local mode) and
    `outgroup_like_cut` 0 to 1 (lab #35), and `exposure_contagion` 0 to 0.56, from off to twice its default
    (lab #38). `bot_strategy` is drawn uniformly.
  - `--ranges plausible`, lab #35's plausible ranges around the code's defaults: default/1.5 to 1.5 x default for a
    positive scale (strengths, costs, rates, chances per day or per event, prices, noise, and `stop_bias` by its
    magnitude); default +- 0.15, clamped, for a share or weight in [0, 1] (`social_proof`, `evasion_cost`,
    `account_cost_deterrence`, `homophily`, `polarisation_backfire`, `bot_farm_floor`, `in_group_outrage_cost`,
    `outgroup_like_cut`, `survey_impulsive_weight`, `survey_habit_weight`); -0.05 to 0.05 for `leave_threshold`;
    default/1.5 to 1.5 x default for the troll share and the bot share. `bot_strategy` keeps its default. The rule of
    every varied number is listed in code (`PLAUSIBLE_RULES` in `robustness/sample.rs`, whose test checks that none
    is missing) and in the output's rule table ("Plausible class"). The folder ends in `-plausible`.
  - `--ranges documented`: section 11's ranges as documented before lab #31, clipped to the catalogue range, and the
    catalogue range where none was documented; the folder ends in `-documented`.

  All three clamp to the catalogue range.
- `--seeds-noisy N`: the noisy facts and claims run on seeds 1 to N, the rest and the baseline on 1 to `--seeds`.
  Noisy are those whose seed share exceeded 0.4 in lab #31's runs (docs/validation.md, section 8, recommendation 4):
  fact 2 (`outrage-learning`), fact 5 (`harassment-exit`), H5, V1, V3, V4, V5, V9 and V10, and in global mode also
  H7. A fact's variants run on the same seeds for all its checks, so a whole fact or claim is noisy, not one check.
  Without the flag every item runs on `--seeds`.
- World 0 is always the default world, the reference; the summaries count the K sampled worlds. `--worlds 0` runs the
  default world alone, so `--mode local --jitter 0 --worlds 0 --seeds 8` gives the batteries' default values.

The worlds come from `--sample-seed` (default 1) and every run is deterministic, so a command reproduces its files.
The runs go to the CPUs in parallel, a chunk of eight worlds at a time, and identical runs (the default design is an
arm of several facts and claims) run once. The folder (default `exports/robustness/<mode>-<sample-seed>/`) holds:

- `worlds.json` and `worlds.csv`: per world, the sampled values, each battery's baseline user share and whether the
  world is degenerate (the default design's user share on the last day, averaged over the seeds, below 0.05), and in
  the JSON per check the per-seed effects, the seed-mean effect and whether it lies in its band, per fact or claim
  whether it holds;
- `checks.csv`: one row per world and check, with the per-seed effects joined by `;`;
- `summary.md` and `summary.json`: per check, fact and claim, the share of sampled worlds where it holds, over all and
  over the non-degenerate worlds, the share whose effect lies on the default world's side of its neutral value (0, 1
  for a ratio or for a ratio or share read as a mean, as in V6, V7 and V8), and the median and 5% and 95% quantiles
  of the effect; the seeds per world; and the failure classes of section 5.2 of
  [docs/validation.md](docs/validation.md): of the non-degenerate worlds where a check fails, those whose effect
  lies within two standard errors of its band (noise-compatible: the interval of the effect +- 2 SE overlaps the
  band) and the clear failures. The SE is the seed SD over the square root of the seeds, the seed SD the SD of the
  per-seed effects within a world, pooled over the non-degenerate worlds with a value. A fact or claim fails within
  noise in a world when every failing check that is not a known limit does. With one seed there is no seed SD and a
  failure is unclassed. Each check also gets its seed share, the squared SE over the variance of the worlds'
  effects.

Global mode adds each check's sensitivity over the non-degenerate worlds: the Spearman rank correlation and the
partial rank correlation (PRCC: ranks, then the correlation of the residuals after regressing out the other
assumptions and the categorical choices) of every varied number with the effect, the top five by absolute value, and
the median effect per `bot_strategy`. A PRCC is marked significant (`*`, `significant` in the JSON) when its absolute
value exceeds two standard errors, the SE being 1/sqrt(n - k - 2) for n worlds and k regression terms (Fisher's
standard error for a partial correlation with k - 1 columns held fixed); each check also gets the R2 of the rank
regression of its effect on every varied number and the categorical dummies, plain and adjusted for the k terms.
PRCC and R2 need more usable worlds than the varied assumptions plus 10 (63); with fewer, the summary gives Spearman
alone and says so; an assumption held to one value is left out of both. At full size with two seeds and all three
batteries a world took about 24 seconds on 8 CPUs for its 56 distinct runs (28 distinct arms on two seeds, measured
on two worlds); with `--seeds-noisy 4` it takes about 45 seconds for 94 distinct runs (measured on two worlds of the
plausible ranges, 2026-11-27). With the facts and the claims alone it took about 17 seconds for 32 distinct runs (the
facts have 10 distinct arms and the claims 7), and the validation battery adds 12 arms the others do not share. The
default world runs in every command.

### Exploring the design space

`incentive explore` searches every design lever at once, with the world assumptions held at the defaults (or those of
`--config`). Each candidate design runs on `--seeds` seeds (default 2) at a reduced size (`--population 600`,
`--days 180`), and its metrics are the mean summary. A search is deterministic for a given `--seed`: the same command
writes the same file. Every mode prints a table and writes JSON to `exports/` unless `--out` names another folder.

```text
target/release/incentive explore map        # MAP-Elites: best benefit in each cell of engagement x harm
target/release/incentive explore pareto     # NSGA-II: the front of engagement (up), harm (down), benefit (up)
target/release/incentive explore goodhart --proxy survey_satisfaction   # one observable proxy, truth recorded
```

`--generations`, `--batch` and `--mutation-rate` tune a search; `map` also takes `--grid` (cells per axis,
default 16) and `--initial` (designs before the first generation, the presets first). `map` and `pareto` start from
every preset except `max-engagement`, which comes from v0.3's Pareto search. The findings are in section 13.7 of
[docs/model.md](docs/model.md).

A search selects designs on few seeds, so its winners are partly lucky draws. After the search every design that ends
up in the file (the map's elites, the front, every Goodhart generation's best) is re-evaluated at the same size on
the fresh seeds of `--confirm-seeds` (default `9..16`; `none` skips it). The file stores those metrics beside the
search's, as `confirmed` for each design and `confirm_seeds` in its settings (files without them still load), and the
printed tables show them side by side in the `c.` columns.

The Atlas tab embeds four files in `crates/incentive-app/assets/atlas/`. They were made with these commands, one at a
time, which regenerate them byte for byte: about 29 minutes on 8 CPUs in all with the desktop under other load (a load
average of about 13, 2026-11-27), the confirmation on seeds 9 to 16 included (about 4 minutes for each Goodhart search
and 10 to 11 each for the map and the Pareto front). Raise `--generations`, `--batch`, `--seeds`, `--population` or
`--days` to regenerate them at a larger scale (the app needs a rebuild to embed new files):

```text
taskset -c 0-7 nice -n 19 target/release/incentive explore goodhart --proxy engagement_index \
    --out crates/incentive-app/assets/atlas
taskset -c 0-7 nice -n 19 target/release/incentive explore goodhart --proxy survey_satisfaction \
    --out crates/incentive-app/assets/atlas
taskset -c 0-7 nice -n 19 target/release/incentive explore map --out crates/incentive-app/assets/atlas
taskset -c 0-7 nice -n 19 target/release/incentive explore pareto --out crates/incentive-app/assets/atlas
```

Runs are spread over the CPUs with rayon, one run per thread; to keep a desktop responsive, limit them, for example
with `taskset -c 0-7 nice -n 19 target/release/incentive ...`.

## Using the core

`incentive-core` is deterministic and engine-free: no threads, clock, file system or operating-system randomness, so
it builds for `wasm32-unknown-unknown` and a run is a pure function of its configuration.

```rust
use incentive_core::{preset, MetricId, Simulation};

let mut config = preset("quality-bridging")?.config();
config.set_path("world.outrage_appeal", 2.0)?;
let mut sim = Simulation::new(config)?;
while let Some(day) = sim.step() {
    println!("day {}: toxic share {:.3}", day.day, day.get(MetricId::ToxicShare));
}
let summary = sim.summary();
```

- Configuration: `Config` (serde) with `run`, `population`, `design` and `world`; `presets()`, `scenarios()`, and
  `params()` with the metadata user interfaces build their controls from. `Config::set_path` changes any field by
  dotted path.
- Running: `Simulation::step`, `run_to_end`, `history`, `summary` and `set_design`, which applies from the next day.
- Metrics: `MetricId` with `info()` (key, name, format, family, observable or truth) and `DailyMetrics::get`; values
  that do not exist on a day, such as the weekly survey, are NaN and `get_opt` returns `None`.
- Inspector: `tracked`, `track`, `agent_history`, `agent_view`, `served_feed` (the feed a tracked individual was
  last served: every ranking component of every item, what they truly felt about it and what they did with it),
  `explain_feed` (a preview of anyone's feed, on its own random stream) and `post`. Inspecting never changes the
  run.
- Statistics over seeds: `Stats` (mean, spread and range of a metric), `MeanInterval` (a mean with its 95% t-interval)
  and `PairedDifference` (two designs paired by seed, with the 95% confidence interval of the difference), shared by
  the runner and the app. `Summary::slope` and `Summary::is_moving` tell whether a metric still moved inside its
  summary window.
- Scenario verdicts: `Scenario::judge` takes the player's and the baseline's summaries paired by seed and returns a
  `ScenarioVerdict`, each goal with its margin, 95% interval and `Verdict` (pass, fail or undecided).
- Atlas files: `MapAtlas`, `ParetoAtlas` and `GoodhartAtlas` with their `ExploreSettings`, `AtlasDesign` and
  `AtlasMetrics`: plain serde types, written by `incentive explore` and read by the app. A design's optional
  `confirmed` metrics come from the fresh seeds of `ExploreSettings::confirm_seeds`. `AtlasMetrics` also keeps the
  benefit index's five parts (`AtlasMetrics::BENEFIT_COMPONENTS`); files written before three of them were kept load
  with those as NaN.

## License

Apache-2.0, see [LICENSE](LICENSE).
