//! A run is a pure function of its configuration (docs/model.md, section 12).

use incentive_core::{Config, DailyMetrics, MetricId, Simulation};

fn small(seed: u64) -> Config {
    let mut config = Config::default();
    config.run.seed = seed;
    config.run.population_size = 400;
    config.run.days = 60;
    config
}

/// Compares every metric of every day bit for bit (NaN included).
fn assert_identical(a: &[DailyMetrics], b: &[DailyMetrics]) {
    assert_eq!(a.len(), b.len());
    for (da, db) in a.iter().zip(b) {
        for id in MetricId::all() {
            assert_eq!(
                da.get(id).to_bits(),
                db.get(id).to_bits(),
                "day {} metric {} differs",
                da.day,
                id.info().key
            );
        }
        let bits = |d: &DailyMetrics| -> Vec<u32> {
            d.wellbeing_by_archetype
                .iter()
                .chain(&d.payout_share_by_style)
                .chain(&d.impression_share_by_style)
                .map(|v| v.to_bits())
                .collect()
        };
        assert_eq!(bits(da), bits(db), "breakdowns differ on day {}", da.day);
    }
}

fn run(config: Config) -> Vec<DailyMetrics> {
    let mut sim = Simulation::new(config).expect("valid configuration");
    sim.run_to_end();
    sim.history().to_vec()
}

#[test]
fn identical_configurations_give_identical_histories() {
    for seed in [1, 7] {
        assert_identical(&run(small(seed)), &run(small(seed)));
    }
}

#[test]
fn different_seeds_give_different_histories() {
    let (a, b) = (run(small(1)), run(small(2)));
    assert_ne!(
        a.last().map(|d| d.get(MetricId::Impressions)),
        b.last().map(|d| d.get(MetricId::Impressions))
    );
}

#[test]
fn inspecting_never_changes_the_run() {
    let plain = run(small(3));
    let mut sim = Simulation::new(small(3)).expect("valid configuration");
    while !sim.is_finished() {
        for id in sim.tracked() {
            let _ = sim.explain_feed(id);
            let _ = sim.agent_view(id);
        }
        sim.track(sim.day() % 400);
        sim.step();
    }
    assert_identical(&plain, sim.history());
}

/// Compares the state of every individual and every live post.
fn assert_same_world(a: &Simulation, b: &Simulation) {
    // Debug output compares NaN fields as equal, unlike `==`.
    assert!(
        format!("{:?}", a.people()) == format!("{:?}", b.people()),
        "individuals differ"
    );
    let posts = |sim: &Simulation| format!("{:?}", sim.posts().collect::<Vec<_>>());
    assert!(posts(a) == posts(b), "live posts differ");
}

#[test]
fn a_clone_continues_identically() {
    let mut a = Simulation::new(small(4)).expect("valid configuration");
    for _ in 0..20 {
        a.step();
    }
    let mut b = a.clone();
    a.run_to_end();
    b.run_to_end();
    assert_identical(a.history(), b.history());
    assert_same_world(&a, &b);
}

#[test]
fn identical_design_changes_give_identical_runs() {
    let changed = || {
        let mut sim = Simulation::new(small(5)).expect("valid configuration");
        while !sim.is_finished() {
            let mut design = sim.config().design.clone();
            match sim.day() {
                10 => design.chronological = true,
                25 => {
                    design.chronological = false;
                    design.w_reply = 0.0;
                    design.diversity = 0.3;
                }
                _ => {}
            }
            if matches!(sim.day(), 10 | 25) {
                sim.set_design(design).expect("valid design");
            }
            sim.step();
        }
        sim
    };
    let (a, b) = (changed(), changed());
    assert_identical(a.history(), b.history());
    assert_same_world(&a, &b);
    // The first change applies from day 10: the days before it match an
    // unchanged run, day 10 does not.
    let plain = run(small(5));
    assert_identical(&plain[..10], &a.history()[..10]);
    assert_ne!(
        plain[10].get(MetricId::Impressions).to_bits(),
        a.history()[10].get(MetricId::Impressions).to_bits()
    );
}
