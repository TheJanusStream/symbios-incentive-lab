//! A run is a pure function of its configuration (docs/model.md, section 12).

use incentive_core::{Config, DailyMetrics, Deactivation, MetricId, NotificationsOff, Simulation};

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

#[test]
fn a_deactivation_experiment_changes_nothing_before_it_starts() {
    let mut with = small(3);
    with.run.days = 40;
    with.run.deactivation = Some(Deactivation {
        days: 10,
        share: 0.5,
        placebo: false,
    });
    let mut without = with.clone();
    without.run.deactivation = None;
    let (a, b) = (run(with), run(without));
    // The experiment draws on its own stream from its first day on, so the
    // thirty days before it are bit for bit those of a run without it.
    assert_identical(&a[..30], &b[..30]);
    assert_ne!(
        a[35].get(MetricId::HumanDailyActives),
        b[35].get(MetricId::HumanDailyActives),
        "the break keeps people away"
    );
}

#[test]
fn a_placebo_experiment_changes_nothing_but_its_own_metrics() {
    let mut placebo = small(3);
    placebo.run.days = 40;
    placebo.run.deactivation = Some(Deactivation {
        days: 10,
        share: 0.5,
        placebo: true,
    });
    let mut without = placebo.clone();
    without.run.deactivation = None;
    let (a, b) = (run(placebo), run(without));
    // Nobody takes the break, and the assignment draws on its own stream,
    // so every other metric is bit for bit that of a run without it.
    for (da, db) in a.iter().zip(&b) {
        for id in MetricId::all().filter(|id| !id.needs_experiment()) {
            assert_eq!(
                da.get(id).to_bits(),
                db.get(id).to_bits(),
                "day {} metric {} differs",
                da.day,
                id.info().key
            );
        }
    }
    let last = a.last().expect("days");
    assert!(last.get(MetricId::BreakChange).is_finite());
    assert!(last.get(MetricId::DeactivationEffect).is_finite());
}

#[test]
fn extra_sessions_switched_off_change_nothing() {
    // With both extra-session rates at 0 nothing is drawn, and the other
    // settings of the extra sessions leave the run untouched.
    let mut base = small(3);
    base.world.recheck_rate = 0.0;
    base.world.notification_pull = 0.0;
    let mut off = base.clone();
    off.world.extra_session_share = 0.9;
    off.world.interruption_cost = 0.5;
    let base = run(base);
    assert_identical(&base, &run(off));
    assert!(
        base.iter()
            .all(|d| d.get(MetricId::SessionsPerActive) == 1.0)
    );
    assert!(
        base.iter()
            .all(|d| d.get(MetricId::NotificationSessionShare) == 0.0)
    );
}

#[test]
fn extra_sessions_are_deterministic() {
    let mut config = small(4);
    config.world.recheck_rate = 1.0;
    config.world.notification_pull = 1.0;
    config.world.interruption_cost = 0.1;
    assert_identical(&run(config.clone()), &run(config));
}

#[test]
fn a_notifications_off_placebo_changes_nothing_but_its_own_metrics() {
    let mut placebo = small(3);
    placebo.run.days = 40;
    placebo.run.notifications_off = Some(NotificationsOff {
        days: 7,
        share: 0.5,
        placebo: true,
    });
    let mut without = placebo.clone();
    without.run.notifications_off = None;
    let (a, b) = (run(placebo.clone()), run(without));
    // Nobody's notifications change, the assignment draws on its own stream
    // and the baseline is only read, so every other metric is bit for bit
    // that of a run without it.
    for (da, db) in a.iter().zip(&b) {
        for id in MetricId::all().filter(|id| !id.needs_experiment()) {
            assert_eq!(
                da.get(id).to_bits(),
                db.get(id).to_bits(),
                "day {} metric {} differs",
                da.day,
                id.info().key
            );
        }
    }
    let last = a.last().expect("days");
    assert!(last.get(MetricId::NotificationsOffTime).is_finite());
    assert!(last.get(MetricId::NotificationsOffSessions).is_finite());
    assert!(last.get(MetricId::NotificationsOffWellbeing).is_finite());
    // The real run is deterministic too.
    let mut real = placebo;
    if let Some(n) = real.run.notifications_off.as_mut() {
        n.placebo = false;
    }
    assert_identical(&run(real.clone()), &run(real));
}
