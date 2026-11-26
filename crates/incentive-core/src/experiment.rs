//! The deactivation experiment (docs/model.md, 13.2, and fact 6 of section
//! 15): a researcher's intervention, not a platform feature. At the start of
//! its first day the regular users are found, and each joins the break group
//! with the configured chance; the rest of them are the stayers. People on
//! the break do not open the app, receive no Attacks and do not churn, like
//! the paid participants of a field experiment. In a placebo run the
//! participants and the break group are drawn the same way, but nobody goes
//! on the break. Its state lives on the truth side of the world, and nothing
//! the platform reads depends on it.

use crate::config::{Config, Deactivation};
use crate::rng::{self, Stream};
use crate::world::{DayCtx, World};

/// Days looked back on to find the regular users.
const REGULAR_WINDOW_DAYS: u32 = 7;
/// Days of that window a regular user opened the app on, at least.
const REGULAR_MIN_DAYS: u32 = 4;

/// One participant of the experiment.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Participant {
    /// Assigned to the break group; in a placebo run nobody takes the break.
    assigned: bool,
    /// Wellbeing at the start of the first day.
    start_wellbeing: f32,
}

/// The running experiment: who takes part, in which group, and the
/// wellbeing they started from.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Experiment {
    first_day: u32,
    /// A placebo run: the groups are drawn, but nobody takes the break.
    placebo: bool,
    /// By individual index; `None` for people who do not take part.
    participants: Vec<Option<Participant>>,
}

impl Experiment {
    /// Whether individual `i` is on the break: assigned to it in a run that
    /// is not a placebo.
    pub(crate) fn on_break(&self, i: usize) -> bool {
        !self.placebo
            && self
                .participants
                .get(i)
                .copied()
                .flatten()
                .is_some_and(|p| p.assigned)
    }

    /// Standard deviation of the participants' wellbeing at the start of the
    /// first day; `None` with fewer than two participants or none at all.
    fn start_sd(&self) -> Option<f64> {
        let start: Vec<f64> = self
            .participants
            .iter()
            .flatten()
            .map(|p| f64::from(p.start_wellbeing))
            .collect();
        if start.len() < 2 {
            return None;
        }
        let m = mean(&start);
        let var = start.iter().map(|w| (w - m).powi(2)).sum::<f64>() / (start.len() - 1) as f64;
        let sd = var.sqrt();
        (sd > 0.0).then_some(sd)
    }

    /// The experiment, on the run's last day only.
    fn on_last_day<'a>(world: &'a World, ctx: &DayCtx) -> Option<&'a Self> {
        let experiment = world.truth.experiment.as_ref()?;
        (ctx.day + 1 == ctx.config.run.days).then_some(experiment)
    }
}

/// Mean of a non-empty slice; NaN when it is empty.
fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len() as f64
}

/// Whether a person with these activity bits (yesterday in the lowest bit)
/// opened the app on at least 4 of the last 7 days.
fn is_regular(active_bits: u64) -> bool {
    let window = (1u64 << REGULAR_WINDOW_DAYS) - 1;
    (active_bits & window).count_ones() >= REGULAR_MIN_DAYS
}

/// The experiment configured for this run, if any.
fn configured(config: &Config) -> Option<&Deactivation> {
    config.run.deactivation.as_ref()
}

/// Start of a day, before anything happens: on the experiment's first day,
/// finds the eligible users and assigns each to the break with the
/// configured chance, on the experiment stream keyed on the person and the
/// day. A placebo run draws the same. Without an experiment, or on any other
/// day, it draws nothing.
pub(crate) fn begin_day(world: &mut World, ctx: &DayCtx) {
    let Some(d) = configured(ctx.config) else {
        return;
    };
    let first_day = d.first_day(ctx.config.run.days);
    if ctx.day != first_day {
        return;
    }
    let participants = world
        .people
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let s = &p.state;
            (s.is_user && is_regular(s.active_bits)).then(|| {
                let mut rng = ctx.rng(i as u64, Stream::Experiment);
                Participant {
                    assigned: rng::chance(&mut rng, d.share),
                    start_wellbeing: s.wellbeing,
                }
            })
        })
        .collect();
    world.truth.experiment = Some(Experiment {
        first_day,
        placebo: d.placebo,
        participants,
    });
}

/// The experiment's effect on the run's last day: the mean wellbeing change
/// of the break group minus that of the stayers, in standard deviations of
/// the participants' wellbeing at the start of the first day. A change is
/// the wellbeing at the end of the last day minus that at the start of the
/// first. Participants who left during the experiment are left out, since
/// their wellbeing froze when they left. NaN on every other day, in a run
/// without the experiment, and when a group is empty (as at share 1). In a
/// placebo run it compares the groups as drawn.
pub(crate) fn effect(world: &World, ctx: &DayCtx) -> f32 {
    let Some(experiment) = Experiment::on_last_day(world, ctx) else {
        return f32::NAN;
    };
    let mut changes: [Vec<f64>; 2] = Default::default();
    for (p, person) in experiment.participants.iter().zip(&world.people) {
        let Some(p) = p else {
            continue;
        };
        let s = &person.state;
        let stayed = s.is_user && s.left_day.is_none_or(|d| d < experiment.first_day);
        if stayed {
            let change = f64::from(s.wellbeing) - f64::from(p.start_wellbeing);
            changes[usize::from(p.assigned)].push(change);
        }
    }
    let [stayers, on_break] = &changes;
    match experiment.start_sd() {
        Some(sd) if !stayers.is_empty() && !on_break.is_empty() => {
            ((mean(on_break) - mean(stayers)) / sd) as f32
        }
        _ => f32::NAN,
    }
}

/// The break group's own change on the run's last day: the mean wellbeing
/// change of everyone assigned to the break, in standard deviations of all
/// participants' wellbeing at the start of the first day. Every member
/// counts, including those who left during the experiment, whose wellbeing
/// froze when they left; in a real run nobody on the break leaves, in a
/// placebo run some do. Freezing is the model's simplification (former
/// users' wellbeing is not updated), where a field survey would follow them;
/// docs/model.md, 13.2, sizes it. The paired difference of a real run and its
/// placebo is the break's effect on the same people. NaN on every other day,
/// in a run without the experiment, and when the break group is empty.
pub(crate) fn break_change(world: &World, ctx: &DayCtx) -> f32 {
    let Some(experiment) = Experiment::on_last_day(world, ctx) else {
        return f32::NAN;
    };
    let changes: Vec<f64> = experiment
        .participants
        .iter()
        .zip(&world.people)
        .filter_map(|(p, person)| {
            p.filter(|p| p.assigned)
                .map(|p| f64::from(person.state.wellbeing) - f64::from(p.start_wellbeing))
        })
        .collect();
    match experiment.start_sd() {
        Some(sd) if !changes.is_empty() => (mean(&changes) / sd) as f32,
        _ => f32::NAN,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init;
    use crate::simulation::Simulation;

    fn config(days: u32, share: f32) -> Config {
        let mut config = Config::default();
        config.run.population_size = 400;
        config.run.days = days;
        config.run.deactivation = Some(Deactivation {
            days: 10,
            share,
            placebo: false,
        });
        config
    }

    /// The same run as a placebo.
    fn placebo(mut config: Config) -> Config {
        if let Some(d) = config.run.deactivation.as_mut() {
            d.placebo = true;
        }
        config
    }

    #[test]
    fn regular_users_opened_on_four_of_seven_days() {
        assert!(is_regular(0b1111));
        assert!(is_regular(0b101_0101));
        assert!(!is_regular(0b111));
        // Days before the window do not count.
        assert!(!is_regular(0b1111_0000_0011));
        assert!(is_regular(u64::MAX));
    }

    #[test]
    fn eligibility_and_assignment_follow_the_rules() {
        let config = config(30, 0.5);
        let mut world = init::build_world(&config);
        // Users 0 to 199 are regular, 200 and up opened on three days only.
        for (i, p) in world.people.iter_mut().enumerate() {
            p.state.active_bits = if i < 200 { 0b1111 } else { 0b111 };
        }
        let ctx = DayCtx {
            config: &config,
            day: 19,
        };
        begin_day(&mut world, &ctx);
        assert!(world.truth.experiment.is_none(), "not the first day");
        let ctx = DayCtx {
            config: &config,
            day: 20,
        };
        begin_day(&mut world, &ctx);
        let e = world.truth.experiment.as_ref().expect("started");
        for (i, (p, person)) in e.participants.iter().zip(&world.people).enumerate() {
            let eligible = i < 200 && person.state.is_user;
            assert_eq!(p.is_some(), eligible, "person {i}");
            if let Some(p) = p {
                assert_eq!(p.start_wellbeing, person.state.wellbeing);
            }
        }
        let n = e.participants.iter().flatten().count();
        let k = e
            .participants
            .iter()
            .flatten()
            .filter(|p| p.assigned)
            .count();
        assert!(n > 50, "{n} eligible");
        // Binomial(n, 0.5): within four standard deviations.
        let sd = (n as f32 * 0.25).sqrt();
        assert!((k as f32 - 0.5 * n as f32).abs() < 4.0 * sd, "{k} of {n}");
    }

    #[test]
    fn the_assignment_share_is_the_configured_chance() {
        let mut total = (0, 0);
        for seed in 1..=4 {
            let mut config = config(30, 0.1);
            config.run.population_size = 2_000;
            config.run.seed = seed;
            let mut world = init::build_world(&config);
            for p in &mut world.people {
                p.state.active_bits = u64::MAX;
            }
            let ctx = DayCtx {
                config: &config,
                day: 20,
            };
            begin_day(&mut world, &ctx);
            let e = world.truth.experiment.as_ref().expect("started");
            total.0 += e.participants.iter().flatten().count();
            total.1 += e
                .participants
                .iter()
                .flatten()
                .filter(|p| p.assigned)
                .count();
        }
        let (n, k) = (total.0 as f32, total.1 as f32);
        let sd = (n * 0.1 * 0.9).sqrt();
        assert!((k - 0.1 * n).abs() < 4.0 * sd, "{k} of {n}");
    }

    #[test]
    fn the_break_group_neither_opens_posts_is_attacked_nor_churns() {
        let mut config = config(40, 0.5);
        // Many trolls and harsh churn, so attacks and departures happen.
        config.population.archetype_shares.troll = 0.3;
        config.world.churn_base = 0.1;
        config.world.harassment_exit = 0.2;
        let mut sim = Simulation::new(config).expect("valid");
        let mut stayer_left = false;
        while sim.step().is_some() {
            let world = sim.world();
            let Some(e) = world.truth.experiment.as_ref() else {
                continue;
            };
            for (i, p) in world.people.iter().enumerate() {
                let state = &p.state;
                if e.on_break(i) {
                    assert!(!state.today.opened, "person {i} opened");
                    assert_eq!(state.today.posts, 0, "person {i} posted");
                    assert_eq!(state.today.items, 0, "person {i} read");
                    assert_eq!(state.today.harassment, 0, "person {i} was attacked");
                    assert!(
                        state.left_day.is_none_or(|d| d < e.first_day) || state.banned,
                        "person {i} churned"
                    );
                } else if e.participants[i].is_some() && state.left_day >= Some(e.first_day) {
                    stayer_left = true;
                }
            }
        }
        assert!(stayer_left, "the stayers churn, so the check has teeth");
    }

    #[test]
    fn the_effect_exists_on_the_last_day_only() {
        use crate::metrics::MetricId;
        let history = |config: Config| {
            let mut sim = Simulation::new(config).expect("valid");
            sim.run_to_end();
            sim.history().to_vec()
        };
        let ids = [MetricId::DeactivationEffect, MetricId::BreakChange];
        for config in [config(40, 0.5), placebo(config(40, 0.5))] {
            let h = history(config);
            let (last, rest) = h.split_last().expect("days");
            for id in ids {
                assert!(rest.iter().all(|d| d.get(id).is_nan()), "{id:?}");
                assert!(last.get(id).is_finite(), "{id:?}: {}", last.get(id));
            }
        }
        // With everyone on the break there are no stayers to compare with,
        // but the break group's own change exists.
        let h = history(config(40, 1.0));
        let last = h.last().expect("days");
        assert!(last.get(MetricId::DeactivationEffect).is_nan());
        assert!(last.get(MetricId::BreakChange).is_finite());
        // Without the experiment neither ever exists.
        let mut plain = config(40, 0.5);
        plain.run.deactivation = None;
        let h = history(plain);
        assert!(ids.iter().all(|&id| h.iter().all(|d| d.get(id).is_nan())));
    }

    #[test]
    fn a_placebo_assigns_the_same_people_and_keeps_nobody_off() {
        let mut real = config(40, 0.5);
        // Many trolls and harsh churn, as in the test of the real break.
        real.population.archetype_shares.troll = 0.3;
        real.world.churn_base = 0.1;
        real.world.harassment_exit = 0.2;
        let mut a = Simulation::new(real.clone()).expect("valid");
        let mut b = Simulation::new(placebo(real)).expect("valid");
        let mut assigned_opened = false;
        let mut assigned_attacked_or_left = false;
        let mut started = false;
        while a.step().is_some() {
            b.step().expect("same length");
            let (Some(ea), Some(eb)) = (
                a.world().truth.experiment.as_ref(),
                b.world().truth.experiment.as_ref(),
            ) else {
                assert!(b.world().truth.experiment.is_none(), "same first day");
                continue;
            };
            assert!(!ea.placebo && eb.placebo);
            assert_eq!(ea.first_day, eb.first_day);
            assert_eq!(ea.participants, eb.participants, "same people, same groups");
            started = true;
            for (i, p) in b.world().people.iter().enumerate() {
                assert!(!eb.on_break(i), "person {i} is kept off in a placebo");
                if eb.participants[i].is_some_and(|p| p.assigned) {
                    let s = &p.state;
                    assigned_opened |= s.today.opened;
                    assigned_attacked_or_left |=
                        s.today.harassment > 0 || s.left_day >= Some(eb.first_day);
                }
            }
        }
        assert!(started, "the experiment ran");
        let n = b.world().truth.experiment.as_ref().map_or(0, |e| {
            e.participants
                .iter()
                .flatten()
                .filter(|p| p.assigned)
                .count()
        });
        assert!(n > 20, "{n} assigned");
        assert!(
            assigned_opened,
            "the placebo break group keeps opening the app"
        );
        assert!(
            assigned_attacked_or_left,
            "the placebo break group is attacked or churns like anyone"
        );
    }

    #[test]
    fn the_break_change_counts_every_member_of_the_break_group() {
        let config = config(30, 0.5);
        let mut world = init::build_world(&config);
        for (i, p) in world.people.iter_mut().enumerate() {
            p.state.active_bits = u64::MAX;
            p.state.wellbeing = if i % 2 == 0 { 0.4 } else { 0.6 };
        }
        let first = DayCtx {
            config: &config,
            day: 20,
        };
        begin_day(&mut world, &first);
        let e = world.truth.experiment.clone().expect("started");
        // Every member gains 0.1, and one leaves: still counted.
        let mut left = false;
        for (i, p) in world.people.iter_mut().enumerate() {
            if e.participants[i].is_some_and(|p| p.assigned) {
                p.state.wellbeing += 0.1;
                if !left {
                    p.state.is_user = false;
                    p.state.left_day = Some(25);
                    left = true;
                }
            }
        }
        assert!(left);
        let sd = e.start_sd().expect("spread");
        let last = DayCtx {
            config: &config,
            day: 29,
        };
        let change = break_change(&world, &last);
        assert!(
            (f64::from(change) - 0.1 / sd).abs() < 1e-4,
            "{change} against {}",
            0.1 / sd
        );
        let before = DayCtx {
            config: &config,
            day: 28,
        };
        assert!(
            break_change(&world, &before).is_nan(),
            "only on the last day"
        );
    }
}
