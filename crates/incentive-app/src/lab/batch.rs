//! Time-sliced batch runs: whole runs on several seeds, advanced a day at a
//! time across frames so the window stays responsive without threads.

use incentive_core::{
    Config, MetricId, PairedDifference, Scenario, ScenarioVerdict, Simulation, Summary,
};

/// A queue of whole runs, worked through one simulated day at a time.
#[derive(Clone, Debug)]
pub struct BatchRun {
    jobs: Vec<Config>,
    current: Option<Simulation>,
    next: usize,
    results: Vec<Summary>,
    days_total: u64,
    days_done: u64,
    error: Option<String>,
}

impl BatchRun {
    /// Queues the runs. The inspector is switched off in each: nobody looks
    /// at a batch run's individuals.
    pub fn new(jobs: Vec<Config>) -> Self {
        let jobs: Vec<Config> = jobs
            .into_iter()
            .map(|mut c| {
                c.run.tracked_agents = 0;
                c
            })
            .collect();
        let days_total = jobs.iter().map(|c| u64::from(c.run.days)).sum();
        Self {
            jobs,
            current: None,
            next: 0,
            results: Vec::new(),
            days_total,
            days_done: 0,
            error: None,
        }
    }

    /// Simulates at least one day, then more while `keep_going` allows.
    /// Returns true once every run has finished.
    pub fn work(&mut self, keep_going: &mut dyn FnMut() -> bool) -> bool {
        loop {
            if self.is_finished() {
                return true;
            }
            if self.current.is_none() {
                match Simulation::new(self.jobs[self.next].clone()) {
                    Ok(sim) => self.current = Some(sim),
                    Err(e) => {
                        self.error = Some(e.to_string());
                        return true;
                    }
                }
            }
            if let Some(sim) = self.current.as_mut() {
                sim.step();
                self.days_done += 1;
                if sim.is_finished() {
                    self.results.push(sim.summary());
                    self.current = None;
                    self.next += 1;
                }
            }
            if !keep_going() {
                return self.is_finished();
            }
        }
    }

    /// Whether every run has finished (or one failed to start).
    pub fn is_finished(&self) -> bool {
        self.error.is_some() || self.next >= self.jobs.len()
    }

    /// Share of all simulated days done, in `[0, 1]`.
    pub fn progress(&self) -> f32 {
        if self.days_total == 0 {
            1.0
        } else {
            (self.days_done as f64 / self.days_total as f64) as f32
        }
    }

    /// Runs finished so far and runs queued.
    pub fn counts(&self) -> (usize, usize) {
        (self.results.len(), self.jobs.len())
    }

    /// The summaries of the finished runs, in queue order.
    pub fn results(&self) -> &[Summary] {
        &self.results
    }

    /// Why the batch stopped early, if it did.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

/// The configurations of a paired check: A and B on each seed, interleaved
/// (A on seed 1, B on seed 1, A on seed 2, ...), as the command-line runner
/// pairs them.
pub fn paired_jobs(a: &Config, b: &Config, seeds: &[u64]) -> Vec<Config> {
    seeds
        .iter()
        .flat_map(|&seed| {
            [a, b].map(|c| {
                let mut c = c.clone();
                c.run.seed = seed;
                c
            })
        })
        .collect()
}

/// Splits the results of [`paired_jobs`] into the A and B summaries.
pub fn split_pairs(results: &[Summary]) -> (Vec<Summary>, Vec<Summary>) {
    let a = results.iter().step_by(2).cloned().collect();
    let b = results.iter().skip(1).step_by(2).cloned().collect();
    (a, b)
}

/// One metric of a paired check.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CheckRow {
    /// The metric.
    pub metric: MetricId,
    /// Means of A and B and the paired difference with its 95% interval.
    pub paired: PairedDifference,
    /// How much the metric moved across the summary window, relative to its
    /// mean, for A and for B: the seeds' mean of [`Summary::slope`].
    pub slopes: [f32; 2],
    /// The standard errors of those slopes ([`Summary::slope_error`] of the
    /// seeds' mean summary).
    pub slope_errors: [f32; 2],
}

impl CheckRow {
    /// Whether the metric was still moving in A's runs and in B's, as
    /// [`Summary::is_moving`] judges the seeds' mean summary.
    pub fn moving(&self) -> [bool; 2] {
        [0, 1].map(|i| incentive_core::is_moving_slope(self.slopes[i], self.slope_errors[i]))
    }
}

/// The rows of a paired check: one per metric, from the per-seed summaries.
pub fn check_rows(metrics: &[MetricId], a: &[Summary], b: &[Summary]) -> Vec<CheckRow> {
    let (mean_a, mean_b) = (Summary::mean(a), Summary::mean(b));
    let slope =
        |mean: &Option<Summary>, metric| mean.as_ref().map_or(f32::NAN, |s| s.slope(metric));
    let error =
        |mean: &Option<Summary>, metric| mean.as_ref().map_or(f32::NAN, |s| s.slope_error(metric));
    metrics
        .iter()
        .map(|&metric| {
            let values = |s: &[Summary]| -> Vec<f32> { s.iter().map(|x| x.get(metric)).collect() };
            CheckRow {
                metric,
                paired: PairedDifference::of(&values(a), &values(b)),
                slopes: [slope(&mean_a, metric), slope(&mean_b, metric)],
                slope_errors: [error(&mean_a, metric), error(&mean_b, metric)],
            }
        })
        .collect()
}

/// Two designs on several seeds, time-sliced, with the paired statistics
/// once every run has finished.
#[derive(Clone, Debug)]
pub struct SeedCheck {
    /// The seeds, paired across A and B.
    pub seeds: Vec<u64>,
    /// Individuals per run.
    pub population: u32,
    /// Days per run.
    pub days: u32,
    /// The runs.
    pub batch: BatchRun,
    /// The statistics, once finished.
    pub rows: Option<Vec<CheckRow>>,
    /// Days in each run's summary window, once finished.
    pub window: u32,
}

impl SeedCheck {
    /// Queues A and B on every seed.
    pub fn new(a: &Config, b: &Config, seeds: Vec<u64>) -> Self {
        Self {
            population: a.run.population_size,
            days: a.run.days,
            batch: BatchRun::new(paired_jobs(a, b, &seeds)),
            seeds,
            rows: None,
            window: 0,
        }
    }

    /// Works on the runs; computes the statistics of `metrics` when the last
    /// run finishes.
    pub fn work(&mut self, metrics: &[MetricId], keep_going: &mut dyn FnMut() -> bool) {
        if self.rows.is_some() {
            return;
        }
        if self.batch.work(keep_going) && self.batch.error().is_none() {
            let (a, b) = split_pairs(self.batch.results());
            self.window = a.first().map_or(0, |s| s.days);
            self.rows = Some(check_rows(metrics, &a, &b));
        }
    }
}

/// The outcome of a scenario's confirmation.
#[derive(Clone, Debug, PartialEq)]
pub struct Confirmed {
    /// Every goal judged on its per-seed margins with a 95% interval
    /// ([`Scenario::judge`]), and the scenario's verdict.
    pub verdict: ScenarioVerdict,
    /// The mean of the player's summaries over the seeds; its slopes say
    /// which metrics were still moving.
    pub player: Summary,
    /// The mean of the baseline's summaries over the seeds.
    pub baseline: Summary,
}

/// A scenario judged on paired seeds: the player's run against the
/// baseline's on each seed.
#[derive(Clone, Debug)]
pub struct ScenarioConfirm {
    /// The seeds.
    pub seeds: Vec<u64>,
    /// The runs: the player's design and the baseline on each seed.
    pub batch: BatchRun,
    /// The verdict, once finished.
    pub results: Option<Confirmed>,
}

impl ScenarioConfirm {
    /// Queues the player's configuration and the baseline on every seed.
    pub fn new(player: &Config, baseline: &Config, seeds: Vec<u64>) -> Self {
        Self {
            batch: BatchRun::new(paired_jobs(player, baseline, &seeds)),
            seeds,
            results: None,
        }
    }

    /// Works on the runs; judges the goals on the paired seeds when the last
    /// run finishes.
    pub fn work(&mut self, scenario: &Scenario, keep_going: &mut dyn FnMut() -> bool) {
        if self.results.is_some() {
            return;
        }
        if self.batch.work(keep_going) && self.batch.error().is_none() {
            let (player, baseline) = split_pairs(self.batch.results());
            if let (Some(p), Some(b)) = (Summary::mean(&player), Summary::mean(&baseline)) {
                self.results = Some(Confirmed {
                    verdict: scenario.judge(&player, &baseline),
                    player: p,
                    baseline: b,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use incentive_core::{Verdict, preset, scenario};

    fn tiny(days: u32) -> Config {
        let mut config = Config::default();
        config.run.population_size = 200;
        config.run.days = days;
        config
    }

    #[test]
    fn a_batch_advances_a_day_per_call_when_the_budget_is_spent() {
        let mut batch = BatchRun::new(vec![tiny(3), tiny(2)]);
        let mut calls = 0;
        while !batch.work(&mut || false) {
            calls += 1;
            assert!(calls < 10);
        }
        assert_eq!(calls, 4, "five days, one per call");
        assert_eq!(batch.counts(), (2, 2));
        assert_eq!(batch.progress(), 1.0);
        assert!(batch.error().is_none());
    }

    #[test]
    fn a_batch_matches_whole_runs() {
        let mut batch = BatchRun::new(vec![tiny(4)]);
        let mut budget = 2;
        let mut keep_going = || {
            budget -= 1;
            budget > 0
        };
        assert!(!batch.work(&mut keep_going));
        assert!(batch.work(&mut || true));
        let mut config = tiny(4);
        config.run.tracked_agents = 0;
        let whole = Simulation::new(config).expect("valid").run_to_end();
        // Through JSON, where the missing values (NaN) compare equal as null.
        let json = |s: &[Summary]| serde_json::to_string(s).unwrap_or_default();
        assert_eq!(batch.results().len(), 1);
        assert_eq!(json(batch.results()), json(&[whole]));
    }

    #[test]
    fn a_bad_configuration_stops_the_batch_with_an_error() {
        let mut bad = tiny(2);
        bad.design.ads_share = 3.0;
        let mut batch = BatchRun::new(vec![bad]);
        assert!(batch.work(&mut || true));
        assert!(batch.error().is_some());
    }

    #[test]
    fn paired_checks_interleave_seeds_and_compute_rows() {
        let a = tiny(3);
        let mut b = tiny(3);
        preset("chronological").expect("preset").apply_to(&mut b);
        let jobs = paired_jobs(&a, &b, &[1, 2]);
        let seeds: Vec<u64> = jobs.iter().map(|c| c.run.seed).collect();
        assert_eq!(seeds, [1, 1, 2, 2]);
        assert!(jobs[1].design.chronological && !jobs[0].design.chronological);
        let mut check = SeedCheck::new(&a, &b, vec![1, 2]);
        for _ in 0..1_000 {
            if check.rows.is_some() || check.batch.error().is_some() {
                break;
            }
            check.work(&[MetricId::EngagementIndex], &mut || true);
        }
        let rows = check.rows.as_deref().unwrap_or_default();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].paired.pairs, 2);
        assert_eq!(check.window, 3, "a short run is summarised whole");
        assert!(rows[0].paired.mean_a > rows[0].paired.mean_b);
    }

    #[test]
    fn a_confirmation_judges_every_goal() {
        let sc = scenario("engagement-trap").expect("scenario");
        let mut baseline = sc.config();
        baseline.run.population_size = 200;
        baseline.run.days = 3;
        let mut confirm = ScenarioConfirm::new(&baseline, &baseline, vec![1, 2]);
        for _ in 0..1_000 {
            if confirm.results.is_some() || confirm.batch.error().is_some() {
                break;
            }
            confirm.work(sc, &mut || true);
        }
        let confirmed = confirm.results.expect("the confirmation finishes");
        assert_eq!(confirmed.verdict.goals.len(), sc.goals.len());
        assert!(confirmed.verdict.goals.iter().all(|g| g.margin.n == 2));
        // The baseline against itself cannot halve its own toxicity.
        assert_ne!(confirmed.verdict.goals[1].verdict, Verdict::Pass);
        assert_ne!(confirmed.verdict.verdict, Verdict::Pass);
        assert!(confirmed.player.days > 0 && confirmed.baseline.days > 0);
    }

    #[test]
    fn a_confirmation_keeps_the_player_and_the_baseline_apart() {
        let sc = scenario("engagement-trap").expect("scenario");
        let mut baseline = sc.config();
        baseline.run.population_size = 200;
        baseline.run.days = 12;
        let mut player = baseline.clone();
        player.design.chronological = true;
        let seeds = vec![1, 2];
        let mut confirm = ScenarioConfirm::new(&player, &baseline, seeds.clone());
        for _ in 0..1_000 {
            if confirm.results.is_some() || confirm.batch.error().is_some() {
                break;
            }
            confirm.work(sc, &mut || true);
        }
        // The same judgement made by hand, side by side.
        let runs = |config: &Config| -> Vec<Summary> {
            seeds
                .iter()
                .map(|&seed| {
                    let mut c = config.clone();
                    c.run.seed = seed;
                    c.run.tracked_agents = 0;
                    let mut sim = Simulation::new(c).expect("valid configuration");
                    sim.run_to_end()
                })
                .collect()
        };
        let (p, b) = (runs(&player), runs(&baseline));
        let expected = sc.judge(&p, &b);
        let got = confirm.results.expect("the confirmation finishes");
        assert_eq!(got.verdict.goals.len(), expected.goals.len());
        for (g, e) in got.verdict.goals.iter().zip(&expected.goals) {
            assert_eq!(g.player.to_bits(), e.player.to_bits(), "{}", e.description);
            assert_eq!(
                g.baseline.to_bits(),
                e.baseline.to_bits(),
                "{}",
                e.description
            );
            assert_eq!(g.margin.mean.to_bits(), e.margin.mean.to_bits());
            assert_eq!(g.verdict, e.verdict);
        }
        assert!(
            got.verdict.goals.iter().any(|g| g.player != g.baseline),
            "the two sides ran different designs"
        );
        let mean = Summary::mean(&p).expect("two summaries");
        let id = got.verdict.goals[0].metric;
        assert_eq!(got.player.slope(id).to_bits(), mean.slope(id).to_bits());
    }

    #[test]
    fn check_rows_flag_metrics_still_moving() {
        let mut a = tiny(40);
        a.run.tracked_agents = 0;
        let mut b = a.clone();
        b.design.chronological = true;
        let summaries: Vec<Summary> = [a, b]
            .into_iter()
            .map(|c| Simulation::new(c).expect("valid").run_to_end())
            .collect();
        let metrics = [MetricId::UserShare, MetricId::ToxicShare];
        let rows = check_rows(&metrics, &summaries[..1], &summaries[1..]);
        for (row, id) in rows.iter().zip(metrics) {
            assert_eq!(row.slopes[0].to_bits(), summaries[0].slope(id).to_bits());
            assert_eq!(row.slopes[1].to_bits(), summaries[1].slope(id).to_bits());
            assert_eq!(
                row.moving(),
                [summaries[0].is_moving(id), summaries[1].is_moving(id)]
            );
        }
        let row = CheckRow {
            slopes: [0.2, 0.2],
            slope_errors: [0.05, 0.15],
            ..rows[0]
        };
        assert_eq!(row.moving(), [true, false]);
    }
}
