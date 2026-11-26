//! The researcher's experiments (docs/model.md, 13.2, and facts 6 and 7 of
//! section 15): interventions, not platform features. At most one runs in a
//! run, at its end.
//!
//! - The deactivation experiment: at the start of its first day the regular
//!   users are found, and each joins the break group with the configured
//!   chance; the rest of them are the stayers. People on the break do not
//!   open the app, receive no Attacks and do not churn, like the paid
//!   participants of a field experiment.
//! - The notifications-off experiment: the regular users are found the same
//!   way, and each has the platform's notifications switched off with the
//!   configured chance. For them notification intensity is 0; everything
//!   else follows the usual rules. Each participant's time on the platform
//!   and sessions a day over the 7 days before are kept as their baseline.
//!
//! In a placebo run the participants and the groups are drawn the same way,
//! but nobody is treated. The experiments' state lives on the truth side of
//! the world, and nothing the platform reads depends on it.

use crate::config::{Config, NOTIFICATIONS_OFF_BASELINE_DAYS};
use crate::person::Person;
use crate::rng::{self, Stream};
use crate::world::{DayCtx, World};

/// Days looked back on to find the regular users.
const REGULAR_WINDOW_DAYS: u32 = 7;
/// Days of that window a regular user opened the app on, at least.
const REGULAR_MIN_DAYS: u32 = 4;

/// Which experiment runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Kind {
    /// A break from the platform (fact 6).
    #[default]
    Deactivation,
    /// The platform's notifications switched off (fact 7).
    NotificationsOff,
}

/// One person's time on the platform and sessions, summed over days.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Usage {
    /// Dwell, the time on the platform (docs/model.md, 7.1).
    dwell: f64,
    /// Sessions: on a day the person opened the app, the first one and the
    /// extra ones (6.1); none on other days.
    sessions: u32,
}

impl Usage {
    /// Adds the day that just ended for `person`.
    fn add_day(&mut self, person: &Person) {
        let today = &person.state.today;
        self.dwell += f64::from(today.dwell);
        if today.opened {
            self.sessions += 1 + today.habitual_sessions + today.notified_sessions;
        }
    }
}

/// One participant of the experiment.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Participant {
    /// Assigned to the treated group; in a placebo run nobody is treated.
    assigned: bool,
    /// Wellbeing at the start of the first day.
    start_wellbeing: f32,
    /// Mean daily time on the platform over the 7 days before the first day
    /// (notifications-off experiment; 0 in a deactivation experiment).
    base_dwell: f32,
    /// Mean sessions a day over the same days (likewise).
    base_sessions: f32,
}

/// The running experiment: which one, who takes part, in which group, and
/// where they started from.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Experiment {
    kind: Kind,
    first_day: u32,
    /// A placebo run: the groups are drawn, but nobody is treated.
    placebo: bool,
    /// By individual index; `None` for people who do not take part.
    participants: Vec<Option<Participant>>,
    /// By individual index, each person's usage since the first day
    /// (notifications-off experiment only; empty otherwise).
    usage: Vec<Usage>,
}

impl Experiment {
    /// Whether individual `i` is in the treated group of a run that is not a
    /// placebo.
    fn treated(&self, i: usize) -> bool {
        !self.placebo
            && self
                .participants
                .get(i)
                .copied()
                .flatten()
                .is_some_and(|p| p.assigned)
    }

    /// Whether individual `i` is on a deactivation experiment's break:
    /// assigned to it in a run that is not a placebo.
    pub(crate) fn on_break(&self, i: usize) -> bool {
        self.kind == Kind::Deactivation && self.treated(i)
    }

    /// Whether individual `i` has notifications switched off by a
    /// notifications-off experiment: assigned in a run that is not a placebo.
    pub(crate) fn notifications_off(&self, i: usize) -> bool {
        self.kind == Kind::NotificationsOff && self.treated(i)
    }

    /// Standard deviation of a quantity over all participants; `None` with
    /// fewer than two participants or no spread.
    fn participants_sd(&self, of: impl Fn(&Participant) -> f32) -> Option<f64> {
        let values: Vec<f64> = self
            .participants
            .iter()
            .flatten()
            .map(|p| f64::from(of(p)))
            .collect();
        sample_sd(&values)
    }

    /// Standard deviation of the participants' wellbeing at the start of the
    /// first day.
    fn start_sd(&self) -> Option<f64> {
        self.participants_sd(|p| p.start_wellbeing)
    }

    /// The experiment of the given kind, on the run's last day only.
    fn on_last_day<'a>(world: &'a World, ctx: &DayCtx, kind: Kind) -> Option<&'a Self> {
        let experiment = world.truth.experiment.as_ref()?;
        (experiment.kind == kind && ctx.day + 1 == ctx.config.run.days).then_some(experiment)
    }
}

/// Mean of a non-empty slice; NaN when it is empty.
fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len() as f64
}

/// Sample standard deviation; `None` with fewer than two values or none of
/// them apart.
fn sample_sd(v: &[f64]) -> Option<f64> {
    if v.len() < 2 {
        return None;
    }
    let m = mean(v);
    let var = v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (v.len() - 1) as f64;
    let sd = var.sqrt();
    (sd > 0.0).then_some(sd)
}

/// Whether a person with these activity bits (yesterday in the lowest bit)
/// opened the app on at least 4 of the last 7 days.
fn is_regular(active_bits: u64) -> bool {
    let window = (1u64 << REGULAR_WINDOW_DAYS) - 1;
    (active_bits & window).count_ones() >= REGULAR_MIN_DAYS
}

/// The experiment a run sets up: which, from which day, with what chance of
/// treatment, and whether it is a placebo.
#[derive(Clone, Copy, Debug)]
struct Setup {
    kind: Kind,
    first_day: u32,
    share: f32,
    placebo: bool,
}

/// The experiment configured for this run, if any; validation keeps at most
/// one.
fn configured(config: &Config) -> Option<Setup> {
    let days = config.run.days;
    if let Some(d) = &config.run.deactivation {
        return Some(Setup {
            kind: Kind::Deactivation,
            first_day: d.first_day(days),
            share: d.share,
            placebo: d.placebo,
        });
    }
    config.run.notifications_off.as_ref().map(|n| Setup {
        kind: Kind::NotificationsOff,
        first_day: n.first_day(days),
        share: n.share,
        placebo: n.placebo,
    })
}

/// Start of a day, before anything happens: on the experiment's first day,
/// finds the eligible users and assigns each to the treated group with the
/// configured chance, on the experiment stream keyed on the person and the
/// day. A placebo run draws the same. A notifications-off experiment also
/// turns each participant's usage over the 7 days before into their
/// baseline. Without an experiment, or on any other day, it draws nothing.
pub(crate) fn begin_day(world: &mut World, ctx: &DayCtx) {
    let Some(setup) = configured(ctx.config) else {
        return;
    };
    if ctx.day != setup.first_day {
        return;
    }
    let baseline = std::mem::take(&mut world.truth.baseline_usage);
    let days = f64::from(NOTIFICATIONS_OFF_BASELINE_DAYS);
    let participants = world
        .people
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let s = &p.state;
            (s.is_user && is_regular(s.active_bits)).then(|| {
                let mut rng = ctx.rng(i as u64, Stream::Experiment);
                let base = baseline.get(i).copied().unwrap_or_default();
                Participant {
                    assigned: rng::chance(&mut rng, setup.share),
                    start_wellbeing: s.wellbeing,
                    base_dwell: (base.dwell / days) as f32,
                    base_sessions: (f64::from(base.sessions) / days) as f32,
                }
            })
        })
        .collect();
    let usage = match setup.kind {
        Kind::Deactivation => Vec::new(),
        Kind::NotificationsOff => vec![Usage::default(); world.population()],
    };
    world.truth.experiment = Some(Experiment {
        kind: setup.kind,
        first_day: setup.first_day,
        placebo: setup.placebo,
        participants,
        usage,
    });
}

/// End of a day, after the daily updates and before the metrics: a
/// notifications-off experiment adds the day's time on the platform and
/// sessions of every individual to the baseline over the 7 days before its
/// first day, and to the usage since then from the first day on. Draws
/// nothing; without that experiment it does nothing.
pub(crate) fn end_day(world: &mut World, ctx: &DayCtx) {
    let Some(setup) = configured(ctx.config) else {
        return;
    };
    if setup.kind != Kind::NotificationsOff {
        return;
    }
    let baseline_from = setup
        .first_day
        .saturating_sub(NOTIFICATIONS_OFF_BASELINE_DAYS);
    let log = if ctx.day >= setup.first_day {
        match world.truth.experiment.as_mut() {
            Some(e) => &mut e.usage,
            None => return,
        }
    } else if ctx.day >= baseline_from {
        &mut world.truth.baseline_usage
    } else {
        return;
    };
    log.resize(world.people.len(), Usage::default());
    for (usage, person) in log.iter_mut().zip(&world.people) {
        usage.add_day(person);
    }
}

/// The deactivation experiment's effect on the run's last day: the mean wellbeing change
/// of the break group minus that of the stayers, in standard deviations of
/// the participants' wellbeing at the start of the first day. A change is
/// the wellbeing at the end of the last day minus that at the start of the
/// first. Participants who left during the experiment are left out: the
/// stayers are the participants still on the platform at the end, as a
/// comparison of the people still using it would count them. NaN on every
/// other day, in a run
/// without the experiment, and when a group is empty (as at share 1). In a
/// placebo run it compares the groups as drawn.
pub(crate) fn effect(world: &World, ctx: &DayCtx) -> f32 {
    let Some(experiment) = Experiment::on_last_day(world, ctx, Kind::Deactivation) else {
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

/// The mean wellbeing change of everyone assigned to the treated group, in
/// standard deviations of all participants' wellbeing at the start of the
/// first day; `None` when the group is empty or there is no spread.
fn assigned_wellbeing_change(experiment: &Experiment, world: &World) -> Option<f64> {
    let changes: Vec<f64> = experiment
        .participants
        .iter()
        .zip(&world.people)
        .filter_map(|(p, person)| {
            p.filter(|p| p.assigned)
                .map(|p| f64::from(person.state.wellbeing) - f64::from(p.start_wellbeing))
        })
        .collect();
    let sd = experiment.start_sd()?;
    (!changes.is_empty()).then(|| mean(&changes) / sd)
}

/// The break group's own change on the run's last day: the mean wellbeing
/// change of everyone assigned to the break, in standard deviations of all
/// participants' wellbeing at the start of the first day. Every member
/// counts, including those who left during the experiment; in a real run
/// nobody on the break leaves, in a placebo run some do. Off the platform a
/// leaver's wellbeing recovers toward its baseline as a person's on the
/// break does (docs/model.md, 7.3), so the measure follows leavers as a field
/// survey would. The paired difference of a real run and its placebo is the
/// break's effect on the same people. NaN on every other day, in a run
/// without the experiment, and when the break group is empty.
pub(crate) fn break_change(world: &World, ctx: &DayCtx) -> f32 {
    Experiment::on_last_day(world, ctx, Kind::Deactivation)
        .and_then(|e| assigned_wellbeing_change(e, world))
        .map_or(f32::NAN, |c| c as f32)
}

/// A notifications-off experiment's measures on the run's last day, for
/// everyone assigned to the treated group, each in standard deviations of
/// the participants' baseline: the mean change in daily time on the platform
/// and in sessions a day, from each participant's mean over the 7 days
/// before the first day to their mean over the experiment's days, and the
/// mean change in wellbeing, from the start of the first day to the end of
/// the last (as fact 6's). Members who leave count, with no time and no
/// sessions from then on, as a phone's log would record them. The paired
/// difference of a real run and its placebo is the effect of switching the
/// notifications off on the same people. `[time, sessions, wellbeing]`; each
/// NaN on every other day, in a run without the experiment, when the treated
/// group is empty, and when its baseline has no spread.
pub(crate) fn notifications_off_changes(world: &World, ctx: &DayCtx) -> [f32; 3] {
    let Some(experiment) = Experiment::on_last_day(world, ctx, Kind::NotificationsOff) else {
        return [f32::NAN; 3];
    };
    let days = f64::from(ctx.day + 1 - experiment.first_day);
    let mut time = Vec::new();
    let mut sessions = Vec::new();
    for (i, p) in experiment.participants.iter().enumerate() {
        let Some(p) = p.filter(|p| p.assigned) else {
            continue;
        };
        let usage = experiment.usage.get(i).copied().unwrap_or_default();
        time.push(usage.dwell / days - f64::from(p.base_dwell));
        sessions.push(f64::from(usage.sessions) / days - f64::from(p.base_sessions));
    }
    let in_sds = |changes: &[f64], sd: Option<f64>| match sd {
        Some(sd) if !changes.is_empty() => (mean(changes) / sd) as f32,
        _ => f32::NAN,
    };
    [
        in_sds(&time, experiment.participants_sd(|p| p.base_dwell)),
        in_sds(&sessions, experiment.participants_sd(|p| p.base_sessions)),
        assigned_wellbeing_change(experiment, world).map_or(f32::NAN, |c| c as f32),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Deactivation, NotificationsOff};
    use crate::init;
    use crate::metrics::{DailyMetrics, MetricId};
    use crate::session::session_limit;
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

    /// A run of `days` days with a notifications-off experiment over its
    /// last week, with extra sessions so that notifications pull people back.
    fn quiet(days: u32, share: f32, placebo: bool) -> Config {
        let mut config = Config::default();
        config.run.population_size = 400;
        config.run.days = days;
        config.world.recheck_rate = 2.0;
        config.world.notification_pull = 2.0;
        config.run.notifications_off = Some(NotificationsOff {
            days: 7,
            share,
            placebo,
        });
        config
    }

    #[test]
    fn notifications_off_assigns_the_regular_users_with_their_baseline() {
        let config = quiet(30, 0.5, false);
        let mut world = init::build_world(&config);
        let n = world.population();
        for (i, p) in world.people.iter_mut().enumerate() {
            p.state.active_bits = if i < 200 { 0b1111 } else { 0b111 };
        }
        // The baseline week as `end_day` sums it: person i spent 7 i in all.
        world.truth.baseline_usage = (0..n)
            .map(|i| Usage {
                dwell: 7.0 * i as f64,
                sessions: 14,
            })
            .collect();
        begin_day(
            &mut world,
            &DayCtx {
                config: &config,
                day: 22,
            },
        );
        assert!(world.truth.experiment.is_none(), "not the first day");
        begin_day(
            &mut world,
            &DayCtx {
                config: &config,
                day: 23,
            },
        );
        assert!(world.truth.baseline_usage.is_empty(), "the baseline moved");
        let e = world.truth.experiment.as_ref().expect("started");
        assert_eq!((e.kind, e.first_day), (Kind::NotificationsOff, 23));
        assert_eq!(e.usage.len(), n);
        let mut assigned = 0;
        for (i, (p, person)) in e.participants.iter().zip(&world.people).enumerate() {
            let eligible = i < 200 && person.state.is_user;
            assert_eq!(p.is_some(), eligible, "person {i}");
            assert!(!e.on_break(i), "nobody takes a break");
            assert_eq!(e.notifications_off(i), p.is_some_and(|p| p.assigned));
            if let Some(p) = p {
                assert_eq!(p.start_wellbeing, person.state.wellbeing);
                assert!((p.base_dwell - i as f32).abs() < 1e-4, "person {i}");
                assert!((p.base_sessions - 2.0).abs() < 1e-6);
                assigned += usize::from(p.assigned);
            }
        }
        let eligible = e.participants.iter().flatten().count();
        assert!(eligible > 50, "{eligible} eligible");
        let sd = (eligible as f32 * 0.25).sqrt();
        assert!((assigned as f32 - 0.5 * eligible as f32).abs() < 4.0 * sd);
    }

    #[test]
    fn the_baseline_is_each_persons_mean_over_the_week_before() {
        let mut sim = Simulation::new(quiet(30, 1.0, false)).expect("valid");
        let n = sim.world().population();
        let mut dwell = vec![0.0f64; n];
        let mut sessions = vec![0u32; n];
        for day in 0..=23 {
            sim.step().expect("a day");
            let world = sim.world();
            if day < 16 {
                assert!(world.truth.baseline_usage.is_empty(), "day {day}");
            } else if day < 23 {
                for (i, p) in world.people.iter().enumerate() {
                    let t = &p.state.today;
                    dwell[i] += f64::from(t.dwell);
                    if t.opened {
                        sessions[i] += 1 + t.habitual_sessions + t.notified_sessions;
                    }
                }
            }
        }
        let e = sim.world().truth.experiment.as_ref().expect("started");
        let mut checked = 0;
        for (i, p) in e.participants.iter().enumerate() {
            let Some(p) = p else {
                continue;
            };
            assert!((f64::from(p.base_dwell) - dwell[i] / 7.0).abs() < 1e-3);
            assert!((f64::from(p.base_sessions) - f64::from(sessions[i]) / 7.0).abs() < 1e-5);
            checked += 1;
        }
        assert!(checked > 50, "{checked} participants");
        assert!(
            e.participants
                .iter()
                .flatten()
                .any(|p| p.base_sessions > 1.0)
        );
    }

    #[test]
    fn notification_intensity_is_zero_for_the_treated_only() {
        let config = quiet(30, 0.5, false);
        let mut silent = config.clone();
        silent.design.notification_intensity = 0.0;
        let ni = config.design.notification_intensity;
        let mut sim = Simulation::new(config.clone()).expect("valid");
        let (mut treated_days, mut other_notified) = (0, 0);
        while sim.step().is_some() {
            let world = sim.world();
            let Some(e) = world.truth.experiment.as_ref() else {
                continue;
            };
            for (i, p) in world.people.iter().enumerate() {
                let t = &p.state.today;
                let own = world.truth.notification_intensity(i, ni);
                let cap = session_limit(world, &config, i);
                if e.notifications_off(i) {
                    treated_days += 1;
                    assert_eq!(own, 0.0);
                    assert_eq!(t.notified_sessions, 0, "person {i}");
                    // The session length's term goes too, whatever its
                    // constant (0 since lab #30, docs/model.md 13.1).
                    assert_eq!(cap, session_limit(world, &silent, i));
                } else {
                    assert_eq!(own, ni);
                    other_notified += t.notified_sessions;
                    assert!(cap >= session_limit(world, &silent, i));
                }
            }
        }
        assert!(treated_days > 100, "{treated_days}");
        assert!(other_notified > 100, "the others are still pulled back");
        // In a placebo nobody's notifications change.
        let mut sim = Simulation::new(quiet(30, 0.5, true)).expect("valid");
        let mut assigned_notified = 0;
        while sim.step().is_some() {
            let world = sim.world();
            let Some(e) = world.truth.experiment.as_ref() else {
                continue;
            };
            for (i, p) in world.people.iter().enumerate() {
                assert!(!e.notifications_off(i));
                assert_eq!(world.truth.notification_intensity(i, ni), ni);
                if e.participants[i].is_some_and(|p| p.assigned) {
                    assigned_notified += p.state.today.notified_sessions;
                }
            }
        }
        assert!(assigned_notified > 50, "{assigned_notified}");
    }

    #[test]
    fn a_notifications_off_placebo_matches_the_real_run_until_the_switch() {
        let history = |config: Config| {
            let mut sim = Simulation::new(config).expect("valid");
            sim.run_to_end();
            (sim.history().to_vec(), sim.world().truth.experiment.clone())
        };
        let (real, er) = history(quiet(30, 0.5, false));
        let (placebo, ep) = history(quiet(30, 0.5, true));
        let mut none = quiet(30, 0.5, true);
        none.run.notifications_off = None;
        let (plain, _) = history(none);
        let (er, ep) = (er.expect("ran"), ep.expect("ran"));
        assert_eq!(er.participants, ep.participants, "same people, same groups");
        let differs = |a: &[DailyMetrics], b: &[DailyMetrics], days: usize| {
            a.iter().zip(b).take(days).any(|(x, y)| {
                MetricId::all()
                    .filter(|id| !id.needs_experiment())
                    .any(|id| x.get(id).to_bits() != y.get(id).to_bits())
            })
        };
        // Identical before the first day; the placebo identical to a run
        // without the experiment throughout; the real run differs after.
        assert!(!differs(&real, &placebo, 23));
        assert!(!differs(&placebo, &plain, 30));
        assert!(differs(&real, &placebo, 30));
    }

    #[test]
    fn the_notifications_off_measures_follow_their_definitions() {
        let config = quiet(30, 0.5, false);
        let mut world = init::build_world(&config);
        let n = world.population();
        for (i, p) in world.people.iter_mut().enumerate() {
            p.state.active_bits = u64::MAX;
            p.state.wellbeing = if i % 2 == 0 { 0.4 } else { 0.6 };
        }
        world.truth.baseline_usage = (0..n)
            .map(|i| Usage {
                dwell: 7.0 * (i % 4) as f64,
                sessions: 7 * (i % 3) as u32,
            })
            .collect();
        begin_day(
            &mut world,
            &DayCtx {
                config: &config,
                day: 23,
            },
        );
        let e = world.truth.experiment.as_mut().expect("started");
        // Each treated person spends one more unit of time and has two more
        // sessions a day over the week; everyone else keeps their baseline.
        for (i, p) in e.participants.iter().enumerate() {
            let Some(p) = p else {
                continue;
            };
            let extra = if p.assigned { (1.0, 2.0) } else { (0.0, 0.0) };
            e.usage[i] = Usage {
                dwell: 7.0 * (f64::from(p.base_dwell) + extra.0),
                sessions: (7.0 * (p.base_sessions + extra.1)).round() as u32,
            };
        }
        let e = e.clone();
        for (i, p) in world.people.iter_mut().enumerate() {
            if e.participants[i].is_some_and(|p| p.assigned) {
                p.state.wellbeing += 0.1;
            }
        }
        let sd_dwell = e.participants_sd(|p| p.base_dwell).expect("spread");
        let sd_sessions = e.participants_sd(|p| p.base_sessions).expect("spread");
        let sd_start = e.start_sd().expect("spread");
        let last = DayCtx {
            config: &config,
            day: 29,
        };
        let [time, sessions, wellbeing] = notifications_off_changes(&world, &last);
        assert!((f64::from(time) - 1.0 / sd_dwell).abs() < 1e-4, "{time}");
        assert!(
            (f64::from(sessions) - 2.0 / sd_sessions).abs() < 1e-4,
            "{sessions}"
        );
        assert!(
            (f64::from(wellbeing) - 0.1 / sd_start).abs() < 1e-4,
            "{wellbeing}"
        );
        // Only on the last day, and the deactivation measures stay empty.
        let before = DayCtx {
            config: &config,
            day: 28,
        };
        assert!(
            notifications_off_changes(&world, &before)
                .iter()
                .all(|v| v.is_nan())
        );
        assert!(effect(&world, &last).is_nan() && break_change(&world, &last).is_nan());
    }

    #[test]
    fn each_experiment_fills_only_its_own_metrics() {
        let quiet_ids = [
            MetricId::NotificationsOffTime,
            MetricId::NotificationsOffSessions,
            MetricId::NotificationsOffWellbeing,
        ];
        let break_ids = [MetricId::DeactivationEffect, MetricId::BreakChange];
        let last = |config: Config| {
            let mut sim = Simulation::new(config).expect("valid");
            sim.run_to_end();
            let h = sim.history().to_vec();
            let (last, rest) = h.split_last().expect("days");
            for id in quiet_ids.iter().chain(&break_ids) {
                assert!(rest.iter().all(|d| d.get(*id).is_nan()), "{id:?}");
            }
            last.clone()
        };
        for placebo in [false, true] {
            let day = last(quiet(30, 0.5, placebo));
            assert!(quiet_ids.iter().all(|id| day.get(*id).is_finite()));
            assert!(break_ids.iter().all(|id| day.get(*id).is_nan()));
        }
        let mut c = quiet(30, 0.5, false);
        c.run.notifications_off = None;
        c.run.deactivation = Some(Deactivation {
            days: 7,
            share: 0.5,
            placebo: false,
        });
        let day = last(c);
        assert!(quiet_ids.iter().all(|id| day.get(*id).is_nan()));
        assert!(break_ids.iter().all(|id| day.get(*id).is_finite()));
    }
}
