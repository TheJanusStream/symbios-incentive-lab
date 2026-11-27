//! The out-of-sample validation battery: its doses at a small size, and the
//! harness that calibrated them at full size (ignored by default).
//! `incentive validate` judges the checks at full size.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use incentive_core::{
    Config, DailyMetrics, FactRun, Simulation, V1_DOSE_TARGET, V1_PENALTY, V3_DOSE_TARGET,
    V3_THRESHOLD, Validation, validations,
};

fn validation(id: &str) -> &'static Validation {
    validations()
        .iter()
        .find(|v| v.id == id)
        .expect("known check")
}

/// Runs the configurations on up to 8 threads, in input order.
fn run_all(configs: Vec<Config>) -> Vec<Vec<DailyMetrics>> {
    let next = AtomicUsize::new(0);
    let out = Mutex::new(vec![Vec::new(); configs.len()]);
    std::thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(config) = configs.get(i) else {
                        break;
                    };
                    let mut sim = Simulation::new(config.clone()).expect("valid");
                    sim.run_to_end();
                    let history = sim.history().to_vec();
                    if let Ok(mut out) = out.lock() {
                        out[i] = history;
                    }
                }
            });
        }
    });
    out.into_inner().unwrap_or_default()
}

/// The dose of `id` at `value` on paired real and placebo runs, at the
/// defaults under the integrity preset: the dose's effect on the seed means
/// and the half-width of its 95% interval.
fn dose_at(id: &str, value: f32, run: &FactRun, placebo_only: bool) -> (f32, f32) {
    let v = validation(id);
    let dose = v.dose.expect("has a dose");
    let base = Config::default();
    let arms: Vec<usize> = if placebo_only { vec![1] } else { vec![0, 1] };
    let mut configs = Vec::new();
    for &seed in &run.seeds {
        let all = v.configs_with_dose(&base, run, seed, value);
        configs.extend(arms.iter().map(|&a| all[a].clone()));
    }
    let outputs = run_all(configs);
    let mut histories = vec![Vec::new(); 2];
    for (k, h) in outputs.into_iter().enumerate() {
        histories[arms[k % arms.len()]].push(h);
    }
    let r = dose.evaluate(&histories);
    (r.effect, r.interval.half_width())
}

/// The doses move the right way at a small size: the penalty lowers the
/// treated's same-group share, and the threshold flags a few percent of
/// their impressions in the placebo run. Full-size values are calibrated
/// by the ignored harness below.
#[test]
fn the_doses_act_at_a_small_size() {
    let run = FactRun {
        seeds: vec![1, 2],
        population: 500,
        days: 120,
    };
    let (v1, _) = dose_at("V1", V1_PENALTY, &run, false);
    assert!(v1 < -0.1 && v1 > -0.7, "V1 dose {v1}");
    let (v3, _) = dose_at("V3", V3_THRESHOLD, &run, true);
    assert!(v3 > 0.02 && v3 < 0.2, "V3 dose {v3}");
}

/// Reads a comma-separated list of numbers from an environment variable.
fn values(name: &str) -> Vec<f32> {
    std::env::var(name)
        .unwrap_or_default()
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect()
}

/// The dose calibration (lab #31), a manipulation check: at the default
/// world under the integrity preset, full size, seeds 1 to 8, prints the
/// dose of each candidate in `DOSE_V1` (penalties) and `DOSE_V3`
/// (thresholds). The outcome checks are not run. Run it with
/// `DOSE_V1=0.5,1 DOSE_V3=0.4,0.5 cargo test --release -p incentive-core
/// --test validation -- --ignored --nocapture`.
#[test]
#[ignore = "full size: minutes per candidate"]
fn calibrate_doses() {
    let run = FactRun {
        seeds: (1..=8).collect(),
        population: 2_000,
        days: 365,
    };
    for value in values("DOSE_V1") {
        let (dose, half) = dose_at("V1", value, &run, false);
        println!("V1 penalty {value}: dose {dose:.4} +- {half:.4} (target {V1_DOSE_TARGET})");
    }
    for value in values("DOSE_V3") {
        let (dose, half) = dose_at("V3", value, &run, true);
        println!("V3 threshold {value}: dose {dose:.4} +- {half:.4} (target {V3_DOSE_TARGET})");
    }
}
