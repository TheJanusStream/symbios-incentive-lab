//! The public API the command-line runner and the game app rely on.

use incentive_core::{
    ARCHETYPE_COUNT, Config, ConfigError, FeedExplanation, FeedKind, MetricId, ParamKind,
    Simulation, Visibility, param, params, preset, presets,
};

fn small() -> Config {
    let mut config = Config::default();
    config.run.population_size = 300;
    config.run.days = 40;
    config
}

#[test]
fn presets_have_the_documented_ids() {
    let ids: Vec<&str> = presets().iter().map(|p| p.id).collect();
    assert_eq!(
        ids,
        [
            "engagement-maximiser",
            "chronological",
            "quality-bridging",
            "calm-by-design",
            "subscription",
            "engagement-paid-creators",
            "integrity-ranking",
            "max-engagement"
        ]
    );
    assert!(
        presets()
            .iter()
            .all(|p| !p.name.is_empty() && p.description.len() > 80)
    );
}

#[test]
fn every_parameter_accepts_its_whole_range() {
    for p in params() {
        let mut config = Config::default();
        match p.kind {
            ParamKind::Range { min, max, .. } => {
                assert!(config.set_path(p.path, min).is_ok(), "{} min", p.path);
                assert!(config.set_path(p.path, max).is_ok(), "{} max", p.path);
                assert!(
                    config.set_path(p.path, max + 1.0).is_err(),
                    "{} above max",
                    p.path
                );
            }
            ParamKind::Integer { min, max } | ParamKind::OptionalInteger { min, max } => {
                assert!(
                    config.set_path(p.path, f64::from(min)).is_ok(),
                    "{} min",
                    p.path
                );
                assert!(
                    config.set_path(p.path, f64::from(max)).is_ok(),
                    "{} max",
                    p.path
                );
            }
            ParamKind::Toggle => assert!(config.set_path(p.path, true).is_ok(), "{}", p.path),
            ParamKind::Choice(choices) => {
                for c in choices {
                    assert!(
                        config.set_path(p.path, c.id).is_ok(),
                        "{} = {}",
                        p.path,
                        c.id
                    );
                }
            }
        }
    }
    assert!(param("design.reshare_depth_cap").is_some());
}

#[test]
fn invalid_configurations_are_rejected_with_typed_errors() {
    let mut config = small();
    config.design.ads_share = 1.5;
    assert!(matches!(
        Simulation::new(config),
        Err(ConfigError::OutOfRange { .. })
    ));
    let mut config = small();
    config.population.archetype_shares.casual = 0.0;
    config.population.archetype_shares.creator = 0.0;
    config.population.archetype_shares.partisan = 0.0;
    config.population.archetype_shares.troll = 0.0;
    config.population.archetype_shares.vulnerable = 0.0;
    assert!(matches!(
        Simulation::new(config),
        Err(ConfigError::Invalid(_))
    ));
}

#[test]
fn every_metric_is_labelled_and_measured() {
    let mut sim = Simulation::new(small()).expect("valid configuration");
    let day = sim.step().expect("first day");
    for id in MetricId::all() {
        let info = id.info();
        assert!(!info.key.is_empty() && !info.description.is_empty());
        // The survey is weekly, retention needs 30 days, and the default
        // design pays no creators.
        let missing_allowed = matches!(
            id,
            MetricId::SurveySatisfaction | MetricId::Retention30 | MetricId::HarmfulPayoutShare
        );
        assert!(
            missing_allowed || day.get_opt(id).is_some(),
            "{} missing on day 0",
            info.key
        );
    }
    assert_eq!(
        MetricId::EngagementIndex.info().visibility,
        Visibility::Observable
    );
    assert_eq!(MetricId::ToxicShare.info().visibility, Visibility::Truth);
}

#[test]
fn the_survey_exists_only_on_survey_days() {
    let mut sim = Simulation::new(small()).expect("valid configuration");
    sim.run_to_end();
    let history = sim.history();
    assert!(history[0].get_opt(MetricId::SurveySatisfaction).is_none());
    assert!(history[6].get_opt(MetricId::SurveySatisfaction).is_some());
    assert!(
        sim.summary()
            .get_opt(MetricId::SurveySatisfaction)
            .is_some()
    );
}

#[test]
fn payout_shares_exist_only_when_creators_are_paid() {
    let mut unpaid = Simulation::new(small()).expect("valid configuration");
    unpaid.run_to_end();
    assert!(
        unpaid
            .history()
            .iter()
            .all(|d| d.get_opt(MetricId::HarmfulPayoutShare).is_none())
    );
    assert!(
        unpaid
            .summary()
            .get_opt(MetricId::HarmfulPayoutShare)
            .is_none()
    );
    let mut config = small();
    preset("engagement-paid-creators")
        .expect("documented preset")
        .apply_to(&mut config);
    let mut paid = Simulation::new(config).expect("valid configuration");
    paid.run_to_end();
    let share = paid.summary().get_opt(MetricId::HarmfulPayoutShare);
    assert!(share.is_some_and(|s| (0.0..=1.0).contains(&s)), "{share:?}");
}

#[test]
fn design_changes_apply_from_the_next_day() {
    let mut sim = Simulation::new(small()).expect("valid configuration");
    sim.step();
    let mut design = sim.config().design.clone();
    design.chronological = true;
    assert!(sim.set_design(design).is_ok());
    assert!(!sim.config().design.chronological);
    sim.step();
    assert!(sim.config().design.chronological);
    let viewer = sim
        .tracked()
        .into_iter()
        .find(|&id| sim.explain_feed(id).is_some());
    let kind = viewer.and_then(|id| sim.explain_feed(id)).map(|f| f.kind);
    assert_eq!(kind, Some(FeedKind::Chronological));
    let mut bad = sim.config().design.clone();
    bad.share_friction = -1.0;
    assert!(sim.set_design(bad).is_err());
}

#[test]
fn the_inspector_tracks_two_per_archetype_and_explains_feeds() {
    let mut sim = Simulation::new(small()).expect("valid configuration");
    let tracked = sim.tracked();
    assert_eq!(tracked.len(), 2 * ARCHETYPE_COUNT);
    for _ in 0..10 {
        sim.step();
    }
    let id = tracked[0];
    assert_eq!(sim.agent_history(id).map(Iterator::count), Some(10));
    let newcomer = (0..300)
        .find(|i| !tracked.contains(i))
        .expect("an untracked individual");
    assert!(sim.track(newcomer));
    assert!(!sim.track(newcomer));
    sim.step();
    assert_eq!(sim.agent_history(newcomer).map(Iterator::count), Some(1));
    let view = sim.agent_view(id).expect("known individual");
    assert!((view.style_mix.iter().sum::<f32>() - 1.0).abs() < 1e-4);

    let viewer = (0..300)
        .find(|&v| sim.explain_feed(v).is_some())
        .expect("a user");
    let feed = sim.explain_feed(viewer).expect("a user's feed");
    assert_eq!(feed.kind, FeedKind::Ranked);
    assert!(!feed.items.is_empty() && feed.items.len() <= feed.session_limit);
    for pair in feed.items.windows(2) {
        assert!(pair[0].score >= pair[1].score, "ranked feed out of order");
    }
    for item in &feed.items {
        assert!((item.components.total() - item.score).abs() < 1e-5);
        assert_eq!(sim.post(item.post.id).map(|p| p.id), Some(item.post.id));
    }
}

/// Scores are the sum of their components and fall with rank in a ranked
/// feed, the diversity adjustment included.
fn assert_ranked_in_order(feed: &FeedExplanation) {
    assert_eq!(feed.kind, FeedKind::Ranked);
    assert!(!feed.items.is_empty() && feed.items.len() <= feed.session_limit);
    for item in &feed.items {
        assert!((item.components.total() - item.score).abs() < 1e-5);
    }
    for pair in feed.items.windows(2) {
        assert!(pair[0].score >= pair[1].score, "ranked feed out of order");
    }
}

#[test]
fn served_feeds_record_what_tracked_viewers_saw_and_did() {
    // Quality and bridging re-ranks for diversity, so the explained scores
    // must include the diversity adjustment to stay in order.
    let mut config = small();
    preset("quality-bridging")
        .expect("documented preset")
        .apply_to(&mut config);
    assert!(config.design.diversity > 0.0);
    let mut sim = Simulation::new(config).expect("valid configuration");
    for _ in 0..10 {
        sim.step();
    }
    let feeds: Vec<&FeedExplanation> = sim
        .tracked()
        .into_iter()
        .filter_map(|id| sim.served_feed(id))
        .collect();
    assert!(!feeds.is_empty(), "no tracked individual opened the app");
    for feed in feeds {
        assert!(feed.served && feed.day < sim.day());
        assert_ranked_in_order(feed);
        let seen: Vec<bool> = feed
            .items
            .iter()
            .map(|item| item.actions.expect("a served item has a record").seen)
            .collect();
        assert!(seen[0], "the first item is always seen");
        assert!(
            seen.windows(2).all(|w| w[0] || !w[1]),
            "seen items are not a prefix: {seen:?}"
        );
        for (item, _) in feed.items.iter().zip(&seen).filter(|(_, seen)| !**seen) {
            assert_eq!(item.actions, Some(Default::default()));
        }
    }
    let untracked = (0..300)
        .find(|id| !sim.tracked().contains(id))
        .expect("an untracked individual");
    assert!(sim.served_feed(untracked).is_none());
    let preview = (0..300)
        .find_map(|v| sim.explain_feed(v))
        .expect("a user's preview");
    assert!(!preview.served && preview.items.iter().all(|i| i.actions.is_none()));
    assert_ranked_in_order(&preview);
}

#[test]
fn live_posts_stay_bounded() {
    let mut config = small();
    config.run.days = 200;
    let mut sim = Simulation::new(config).expect("valid configuration");
    let mut most = 0;
    while sim.step().is_some() {
        most = most.max(sim.posts().count());
    }
    let lifetime = sim.config().run.post_lifetime_days as usize;
    assert!(most < lifetime * 3 * sim.population(), "{most} live posts");
    let oldest = sim.posts().map(|p| p.day).min().unwrap_or(0);
    assert!(sim.day() - oldest <= lifetime as u32);
}
