//! The researcher's experiments (docs/model.md, 13.2, and facts 6 and 7 of
//! section 15): interventions, not platform features. At most one runs in a
//! run, at its end.
//!
//! - The deactivation experiment: at the start of its first day the regular
//!   users are found, and each joins the break group with the configured
//!   chance; the rest of them are the stayers. People on the break do not
//!   open the app, receive no Attacks and do not churn, like the paid
//!   participants of a field experiment. The break may end before the run
//!   does; the break group then uses the platform again under the usual
//!   rules, and its time on it is logged.
//! - The notifications-off experiment: the regular users are found the same
//!   way, and each has the platform's notifications switched off with the
//!   configured chance. For them notification intensity is 0; everything
//!   else follows the usual rules. Each participant's time on the platform
//!   and sessions a day over the 7 days before are kept as their baseline.
//! - The feed experiment: the regular users are found the same way, and each
//!   treated viewer's feed changes as the experiment's kind says (hidden
//!   flagged posts, no reshares, a penalty on posts by the viewer's own
//!   group, or the chronological feed of their in-network posts). Each
//!   participant's use of the platform and exposure are logged over the
//!   experiment's days.
//!
//! In a placebo run the participants and the groups are drawn the same way,
//! but nobody is treated. The experiments' state lives on the truth side of
//! the world, and nothing the platform reads depends on it.

use crate::config::{Config, FeedExperimentKind, NOTIFICATIONS_OFF_BASELINE_DAYS};
use crate::content::Post;
use crate::person::{Person, PersonState};
use crate::rng::{self, Stream};
use crate::world::{DayCtx, World, partisan_band};

/// Days looked back on to find the regular users.
const REGULAR_WINDOW_DAYS: u32 = 7;
/// Days of that window a regular user opened the app on, at least.
const REGULAR_MIN_DAYS: u32 = 4;

/// Which experiment runs.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Kind {
    /// A break from the platform (fact 6).
    #[default]
    Deactivation,
    /// The platform's notifications switched off (fact 7).
    NotificationsOff,
    /// The treated viewers' feeds changed.
    Feed(FeedExperimentKind),
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

/// One person's use of the platform and exposure over a feed experiment's
/// days, summed; a day off the platform adds nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Exposure {
    /// Dwell, the time on the platform.
    dwell: f64,
    /// Engagement actions: likes, replies and shares given.
    actions: u32,
    /// Impressions, and of them the toxic ones, the misinformation and
    /// those of posts by the person's own group.
    impressions: u32,
    toxic: u32,
    misinformation: u32,
    same_group: u32,
    /// Of the impressions, those a hide-flagged experiment's threshold
    /// flags (its dose; counted in a placebo run too).
    flagged: u32,
    /// Of the impressions, those of posts by human authors in the more
    /// partisan half, the most partisan tenth and the middle fifth by the
    /// partisanship trait.
    partisan_half: u32,
    extreme: u32,
    middle: u32,
    /// Posts written and their summed toxicity.
    posts: u32,
    post_toxicity: f64,
    /// Shares made and the summed truth of the shared posts.
    shares: u32,
    shared_truth: f64,
}

impl Exposure {
    /// Adds the day that just ended for `person`.
    fn add_day(&mut self, person: &Person) {
        let today = &person.state.today;
        self.dwell += f64::from(today.dwell);
        self.actions += today.actions;
        self.impressions += today.items;
        self.toxic += today.toxic_items;
        self.misinformation += today.misinformation_items;
        self.same_group += today.same_group_items;
        self.flagged += today.flagged_items;
        self.partisan_half += today.partisan_half_items;
        self.extreme += today.extreme_items;
        self.middle += today.middle_items;
        self.posts += today.posts;
        self.post_toxicity += f64::from(today.post_toxicity);
        self.shares += today.shares;
        self.shared_truth += f64::from(today.shared_truth);
    }
}

/// One participant of the experiment.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Participant {
    /// Assigned to the treated group; in a placebo run nobody is treated.
    assigned: bool,
    /// Wellbeing at the start of the first day.
    start_wellbeing: f32,
    /// Affective polarisation at the start of the first day.
    start_polarisation: f32,
    /// Informedness at the start of the first day.
    start_informedness: f32,
    /// Mean daily time on the platform over the 7 days before the first day
    /// (notifications-off experiment; 0 otherwise).
    base_dwell: f32,
    /// Mean sessions a day over the same days (likewise).
    base_sessions: f32,
}

/// A deactivation experiment's measures at the end of its break: the field
/// design's effect, and the break group's changes in wellbeing,
/// polarisation and informedness, each in standard deviations.
#[derive(Clone, Copy, Debug, PartialEq)]
struct BreakEnd {
    effect: f32,
    wellbeing: f32,
    polarisation: f32,
    informedness: f32,
    /// The polarisation and informedness changes unscaled, in the
    /// quantities' own units.
    polarisation_raw: f32,
    informedness_raw: f32,
    /// The standard deviations the scaled changes divide by: of all
    /// participants' wellbeing, polarisation and informedness at the start.
    sds: [f32; 3],
}

/// Impressions of a feed experiment's treated group, pooled, by the
/// author's partisanship: the more partisan half, the most partisan tenth,
/// the middle fifth, and all impressions.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Reach {
    half: u64,
    extreme: u64,
    middle: u64,
    all: u64,
}

impl Reach {
    /// Adds `n` impressions of a post by `author`.
    fn add(&mut self, partisanship_rank: &[u32], author: usize, n: u64) {
        if let Some(band) = partisan_band(partisanship_rank, author) {
            self.half += if band.half { n } else { 0 };
            self.extreme += if band.extreme { n } else { 0 };
            self.middle += if band.middle { n } else { 0 };
        }
        self.all += n;
    }
}

/// The running experiment: which one, who takes part, in which group, and
/// where they started from.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Experiment {
    kind: Kind,
    first_day: u32,
    /// The first day after the experiment: the run's length, except for a
    /// break that ends before the run.
    end_day: u32,
    /// A placebo run: the groups are drawn, but nobody is treated.
    placebo: bool,
    /// The experiment is over (a break that ended before the run did).
    over: bool,
    /// By individual index; `None` for people who do not take part.
    participants: Vec<Option<Participant>>,
    /// By individual index, each person's usage since the first day
    /// (notifications-off experiment), or since the end of a break that
    /// ended before the run; empty otherwise.
    usage: Vec<Usage>,
    /// By individual index, each person's exposure since the first day
    /// (feed experiment only; empty otherwise).
    exposure: Vec<Exposure>,
    /// The measures taken at the end of a break that ended before the run.
    break_end: Option<BreakEnd>,
    /// A feed experiment's treated group's impressions of posts that had
    /// been removed or demoted by the end of their life, counted as each
    /// such post ends (feed experiment only; zero otherwise).
    removed_or_demoted: Reach,
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
    /// assigned to it in a run that is not a placebo, while it lasts.
    pub(crate) fn on_break(&self, i: usize) -> bool {
        self.kind == Kind::Deactivation && !self.over && self.treated(i)
    }

    /// Whether individual `i` has notifications switched off by a
    /// notifications-off experiment: assigned in a run that is not a placebo.
    pub(crate) fn notifications_off(&self, i: usize) -> bool {
        self.kind == Kind::NotificationsOff && self.treated(i)
    }

    /// Whether individual `i` is assigned to a feed experiment's treated
    /// group, in a real run or its placebo: whose impressions its measures
    /// count.
    pub(crate) fn feed_assigned(&self, i: usize) -> bool {
        matches!(self.kind, Kind::Feed(_))
            && self
                .participants
                .get(i)
                .copied()
                .flatten()
                .is_some_and(|p| p.assigned)
    }

    /// Counts a post at the end of its life: a feed experiment's treated
    /// group's impressions of it, if it had been removed or demoted
    /// (docs/model.md, 9.2). Measurement only.
    pub(crate) fn count_ending_post(&mut self, partisanship_rank: &[u32], post: &Post) {
        if matches!(self.kind, Kind::Feed(_))
            && (post.removed || post.demoted)
            && post.assigned_impressions > 0
        {
            self.removed_or_demoted.add(
                partisanship_rank,
                post.author as usize,
                u64::from(post.assigned_impressions),
            );
        }
    }

    /// How a feed experiment changes individual `i`'s feed: its kind for a
    /// viewer assigned in a run that is not a placebo, `None` otherwise.
    pub(crate) fn feed_treatment(&self, i: usize) -> Option<FeedExperimentKind> {
        match self.kind {
            Kind::Feed(kind) if self.treated(i) => Some(kind),
            _ => None,
        }
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

    /// The experiment, if its kind passes `is`, on the run's last day only.
    fn on_last_day<'a>(
        world: &'a World,
        ctx: &DayCtx,
        is: impl Fn(Kind) -> bool,
    ) -> Option<&'a Self> {
        let experiment = world.truth.experiment.as_ref()?;
        (is(experiment.kind) && ctx.day + 1 == ctx.config.run.days).then_some(experiment)
    }

    /// The participants assigned to the treated group, with their index.
    fn assigned(&self) -> impl Iterator<Item = (usize, &Participant)> {
        self.participants
            .iter()
            .enumerate()
            .filter_map(|(i, p)| p.as_ref().filter(|p| p.assigned).map(|p| (i, p)))
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

/// The experiment a run sets up: which, from which day to which, with what
/// chance of treatment, and whether it is a placebo.
#[derive(Clone, Copy, Debug)]
struct Setup {
    kind: Kind,
    first_day: u32,
    end_day: u32,
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
            end_day: d.end_day(days),
            share: d.share,
            placebo: d.placebo,
        });
    }
    if let Some(f) = &config.run.feed_experiment {
        return Some(Setup {
            kind: Kind::Feed(f.kind),
            first_day: f.first_day(days),
            end_day: days,
            share: f.share,
            placebo: f.placebo,
        });
    }
    config.run.notifications_off.as_ref().map(|n| Setup {
        kind: Kind::NotificationsOff,
        first_day: n.first_day(days),
        end_day: days,
        share: n.share,
        placebo: n.placebo,
    })
}

/// Start of a day, before anything happens: on the experiment's first day,
/// finds the eligible users and assigns each to the treated group with the
/// configured chance, on the experiment stream keyed on the person and the
/// day. A placebo run draws the same. A notifications-off experiment also
/// turns each participant's usage over the 7 days before into their
/// baseline. On the first day after a break that ends before the run, the
/// break is over. Without an experiment, or on any other day, it draws
/// nothing.
pub(crate) fn begin_day(world: &mut World, ctx: &DayCtx) {
    let Some(setup) = configured(ctx.config) else {
        return;
    };
    if ctx.day == setup.end_day
        && let Some(experiment) = world.truth.experiment.as_mut()
    {
        experiment.over = true;
    }
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
                    start_polarisation: s.affective_polarisation,
                    start_informedness: s.informedness,
                    base_dwell: (base.dwell / days) as f32,
                    base_sessions: (f64::from(base.sessions) / days) as f32,
                }
            })
        })
        .collect();
    let n = world.population();
    let after_break = setup.end_day < ctx.config.run.days;
    let usage = match setup.kind {
        Kind::NotificationsOff => vec![Usage::default(); n],
        Kind::Deactivation if after_break => vec![Usage::default(); n],
        Kind::Deactivation | Kind::Feed(_) => Vec::new(),
    };
    let exposure = match setup.kind {
        Kind::Feed(_) => vec![Exposure::default(); n],
        Kind::Deactivation | Kind::NotificationsOff => Vec::new(),
    };
    world.truth.experiment = Some(Experiment {
        kind: setup.kind,
        first_day: setup.first_day,
        end_day: setup.end_day,
        placebo: setup.placebo,
        over: false,
        participants,
        usage,
        exposure,
        break_end: None,
        removed_or_demoted: Reach::default(),
    });
}

/// End of a day, after the daily updates and before the metrics. A
/// notifications-off experiment adds the day's time on the platform and
/// sessions of every individual to the baseline over the 7 days before its
/// first day, and to the usage since then from the first day on. A feed
/// experiment adds every individual's use and exposure from its first day
/// on. A break that ends before the run takes its measures on its last day
/// and logs the usage from the day after. Draws nothing; without an
/// experiment it does nothing.
pub(crate) fn end_day(world: &mut World, ctx: &DayCtx) {
    let Some(setup) = configured(ctx.config) else {
        return;
    };
    match setup.kind {
        Kind::NotificationsOff => log_notifications_off(world, ctx, setup),
        Kind::Feed(_) => {
            if let Some(e) = world.truth.experiment.as_mut() {
                e.exposure.resize(world.people.len(), Exposure::default());
                for (exposure, person) in e.exposure.iter_mut().zip(&world.people) {
                    exposure.add_day(person);
                }
            }
        }
        Kind::Deactivation if setup.end_day < ctx.config.run.days => {
            let Some(e) = world.truth.experiment.as_ref() else {
                return;
            };
            if ctx.day + 1 == setup.end_day {
                let measures = break_measures(e, world);
                if let Some(e) = world.truth.experiment.as_mut() {
                    e.break_end = Some(measures);
                }
            } else if ctx.day >= setup.end_day
                && let Some(e) = world.truth.experiment.as_mut()
            {
                e.usage.resize(world.people.len(), Usage::default());
                for (usage, person) in e.usage.iter_mut().zip(&world.people) {
                    usage.add_day(person);
                }
            }
        }
        Kind::Deactivation => {}
    }
}

/// A notifications-off experiment's log: the baseline over the 7 days
/// before its first day, the usage since then from the first day on.
fn log_notifications_off(world: &mut World, ctx: &DayCtx, setup: Setup) {
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

/// The field design's effect of a deactivation experiment, now: the mean
/// wellbeing change of the break group minus that of the stayers, in
/// standard deviations of the participants' wellbeing at the start of the
/// first day. Participants who left during the experiment are left out. NaN
/// when a group is empty.
fn field_effect(experiment: &Experiment, world: &World) -> f32 {
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

/// The mean change of a quantity, now against the start of the first day,
/// over everyone assigned to the treated group (leavers included), in
/// standard deviations of all participants' values at the start; `None`
/// when the group is empty or there is no spread.
fn assigned_change(
    experiment: &Experiment,
    world: &World,
    now: impl Fn(&PersonState) -> f32,
    start: impl Fn(&Participant) -> f32,
) -> Option<f64> {
    let sd = experiment.participants_sd(&start)?;
    assigned_raw_change(experiment, world, now, start).map(|c| c / sd)
}

/// The mean change of a quantity as [`assigned_change`] measures it, but
/// unscaled, in the quantity's own units; `None` when the group is empty.
fn assigned_raw_change(
    experiment: &Experiment,
    world: &World,
    now: impl Fn(&PersonState) -> f32,
    start: impl Fn(&Participant) -> f32,
) -> Option<f64> {
    let changes: Vec<f64> = experiment
        .participants
        .iter()
        .zip(&world.people)
        .filter_map(|(p, person)| {
            p.filter(|p| p.assigned)
                .map(|p| f64::from(now(&person.state)) - f64::from(start(&p)))
        })
        .collect();
    (!changes.is_empty()).then(|| mean(&changes))
}

/// The mean wellbeing change of everyone assigned to the treated group, in
/// standard deviations of all participants' wellbeing at the start of the
/// first day; `None` when the group is empty or there is no spread.
fn assigned_wellbeing_change(experiment: &Experiment, world: &World) -> Option<f64> {
    assigned_change(experiment, world, |s| s.wellbeing, |p| p.start_wellbeing)
}

/// A deactivation experiment's measures, now.
fn break_measures(experiment: &Experiment, world: &World) -> BreakEnd {
    let sds = |c: Option<f64>| c.map_or(f32::NAN, |c| c as f32);
    BreakEnd {
        effect: field_effect(experiment, world),
        wellbeing: sds(assigned_wellbeing_change(experiment, world)),
        polarisation: sds(assigned_change(
            experiment,
            world,
            |s| s.affective_polarisation,
            |p| p.start_polarisation,
        )),
        informedness: sds(assigned_change(
            experiment,
            world,
            |s| s.informedness,
            |p| p.start_informedness,
        )),
        polarisation_raw: sds(assigned_raw_change(
            experiment,
            world,
            |s| s.affective_polarisation,
            |p| p.start_polarisation,
        )),
        informedness_raw: sds(assigned_raw_change(
            experiment,
            world,
            |s| s.informedness,
            |p| p.start_informedness,
        )),
        sds: start_sds(experiment),
    }
}

/// The standard deviations of all participants' wellbeing, affective
/// polarisation and informedness at the start of the first day, NaN where
/// there are fewer than two participants or no spread.
fn start_sds(experiment: &Experiment) -> [f32; 3] {
    let sd = |v: Option<f64>| v.map_or(f32::NAN, |v| v as f32);
    [
        sd(experiment.start_sd()),
        sd(experiment.participants_sd(|p| p.start_polarisation)),
        sd(experiment.participants_sd(|p| p.start_informedness)),
    ]
}

/// A deactivation experiment's measures at the end of its break, read on
/// the run's last day: taken now when the break ends with the run, kept
/// from the break's last day when it ended before. `None` on every other
/// day and in a run without the experiment.
fn break_end(world: &World, ctx: &DayCtx) -> Option<BreakEnd> {
    let e = Experiment::on_last_day(world, ctx, |k| k == Kind::Deactivation)?;
    if e.end_day >= ctx.config.run.days {
        Some(break_measures(e, world))
    } else {
        e.break_end
    }
}

/// The deactivation experiment's effect on the run's last day: the mean
/// wellbeing change of the break group minus that of the stayers, in
/// standard deviations of the participants' wellbeing at the start of the
/// first day. A change is the wellbeing at the end of the break's last day
/// minus that at the start of the first. Participants who left during the
/// experiment are left out: the stayers are the participants still on the
/// platform at the break's end, as a comparison of the people still using
/// it would count them. NaN on every other day, in a run without the
/// experiment, and when a group is empty (as at share 1). In a placebo run
/// it compares the groups as drawn.
pub(crate) fn effect(world: &World, ctx: &DayCtx) -> f32 {
    break_end(world, ctx).map_or(f32::NAN, |b| b.effect)
}

/// The break group's own change on the run's last day: the mean wellbeing
/// change of everyone assigned to the break, from the start of its first day
/// to the end of its last, in standard deviations of all participants'
/// wellbeing at the start of the first day. Every member counts, including
/// those who left during the experiment; in a real run nobody on the break
/// leaves, in a placebo run some do. Off the platform a leaver's wellbeing
/// recovers toward its baseline as a person's on the break does
/// (docs/model.md, 7.3), so the measure follows leavers as a field survey
/// would. The paired difference of a real run and its placebo is the break's
/// effect on the same people. NaN on every other day, in a run without the
/// experiment, and when the break group is empty.
pub(crate) fn break_change(world: &World, ctx: &DayCtx) -> f32 {
    break_end(world, ctx).map_or(f32::NAN, |b| b.wellbeing)
}

/// The break group's changes in affective polarisation and informedness
/// over the break, measured like [`break_change`], each in standard
/// deviations of all participants' values at the start of the first day,
/// then the same two changes unscaled: `[polarisation, informedness,
/// polarisation_raw, informedness_raw]`, read on the run's last day; NaN as
/// [`break_change`] is.
pub(crate) fn break_society_changes(world: &World, ctx: &DayCtx) -> [f32; 4] {
    break_end(world, ctx).map_or([f32::NAN; 4], |b| {
        [
            b.polarisation,
            b.informedness,
            b.polarisation_raw,
            b.informedness_raw,
        ]
    })
}

/// The standard deviations the deactivation experiment's scaled changes
/// divide by, read with them on the run's last day: of all participants'
/// wellbeing, affective polarisation and informedness at the start of the
/// first day, `[wellbeing, polarisation, informedness]`. NaN as
/// [`break_change`] is, and where there is no spread.
pub(crate) fn break_sds(world: &World, ctx: &DayCtx) -> [f32; 3] {
    break_end(world, ctx).map_or([f32::NAN; 3], |b| b.sds)
}

/// The break group's mean daily time on the platform over the days after a
/// break that ended before the run, to the run's end, read on the run's last
/// day. Every member counts, with no time on days without a session and
/// after leaving. NaN on every other day, when the break ends with the run,
/// in a run without the experiment, and when the break group is empty.
pub(crate) fn post_break_dwell(world: &World, ctx: &DayCtx) -> f32 {
    let Some(e) = Experiment::on_last_day(world, ctx, |k| k == Kind::Deactivation) else {
        return f32::NAN;
    };
    if e.end_day >= ctx.config.run.days {
        return f32::NAN;
    }
    let days = f64::from(ctx.config.run.days - e.end_day);
    let dwell: Vec<f64> = e
        .assigned()
        .map(|(i, _)| e.usage.get(i).map_or(0.0, |u| u.dwell) / days)
        .collect();
    if dwell.is_empty() {
        f32::NAN
    } else {
        mean(&dwell) as f32
    }
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
    let Some(experiment) = Experiment::on_last_day(world, ctx, |k| k == Kind::NotificationsOff)
    else {
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

/// The standard deviations a notifications-off experiment's changes divide
/// by, on the run's last day: of the participants' baseline daily time and
/// sessions a day, and of their wellbeing at the start of the first day,
/// `[time, sessions, wellbeing]`. NaN on every other day, in a run without
/// the experiment, and where there is no spread.
pub(crate) fn notifications_off_sds(world: &World, ctx: &DayCtx) -> [f32; 3] {
    let Some(e) = Experiment::on_last_day(world, ctx, |k| k == Kind::NotificationsOff) else {
        return [f32::NAN; 3];
    };
    let sd = |v: Option<f64>| v.map_or(f32::NAN, |v| v as f32);
    [
        sd(e.participants_sd(|p| p.base_dwell)),
        sd(e.participants_sd(|p| p.base_sessions)),
        sd(e.start_sd()),
    ]
}

/// Number of a feed experiment's measures.
pub(crate) const FEED_MEASURES: usize = 26;

/// A feed experiment's measures on the run's last day, over the
/// experiment's days, for everyone assigned to the treated group, leavers
/// included with no use after leaving:
///
/// - mean daily time on the platform, and mean daily engagement actions
///   (likes, replies and shares given);
/// - the toxic, misinformation and same-group shares of their impressions,
///   pooled over the group's impressions;
/// - the mean toxicity of the posts they wrote and the mean truth of the
///   posts they shared, pooled likewise;
/// - their mean changes in affective polarisation, informedness and
///   wellbeing, from the start of the first day to the end of the last, in
///   standard deviations of all participants' values at the start;
/// - the share of their impressions whose classifier score exceeds a
///   hide-flagged experiment's threshold, pooled likewise: the dose, read in
///   the placebo run (in a real run the treated see none of them), NaN for
///   the other kinds;
/// - their mean changes in affective polarisation and informedness unscaled,
///   in the quantities' own units;
/// - the standard deviations the scaled changes divide by: of all
///   participants' polarisation, informedness and wellbeing at the start;
/// - their reach by the author's partisanship, per treated viewer and day,
///   pooled over the group and the days: impressions of posts by the more
///   partisan half of authors, by the most partisan tenth, by the middle
///   fifth (40% to 60%) and of all posts, then the most partisan tenth's
///   impressions over the middle fifth's;
/// - the same five without the impressions of posts removed or demoted by
///   the end of their life (or by the run's end, for posts still live).
///
/// In that order. The paired comparison of a real run and its placebo is
/// the experiment's effect on the same people. Each NaN on every other day,
/// in a run without the experiment, when the treated group is empty, and
/// when its denominator is 0 or the start has no spread.
pub(crate) fn feed_measures(world: &World, ctx: &DayCtx) -> [f32; FEED_MEASURES] {
    let Some(e) = Experiment::on_last_day(world, ctx, |k| matches!(k, Kind::Feed(_))) else {
        return [f32::NAN; FEED_MEASURES];
    };
    let days = f64::from(ctx.day + 1 - e.first_day);
    let mut total = Exposure::default();
    let (mut dwell, mut actions) = (Vec::new(), Vec::new());
    let mut reach = Reach::default();
    for (i, _) in e.assigned() {
        let x = e.exposure.get(i).copied().unwrap_or_default();
        dwell.push(x.dwell / days);
        actions.push(f64::from(x.actions) / days);
        total.impressions += x.impressions;
        total.toxic += x.toxic;
        total.misinformation += x.misinformation;
        total.same_group += x.same_group;
        total.flagged += x.flagged;
        total.posts += x.posts;
        total.post_toxicity += x.post_toxicity;
        total.shares += x.shares;
        total.shared_truth += x.shared_truth;
        reach.half += u64::from(x.partisan_half);
        reach.extreme += u64::from(x.extreme);
        reach.middle += u64::from(x.middle);
        reach.all += u64::from(x.impressions);
    }
    // Posts still live: those removed or demoted so far count as such.
    let mut excluded = e.removed_or_demoted;
    for post in world.posts.iter() {
        if (post.removed || post.demoted) && post.assigned_impressions > 0 {
            excluded.add(
                &world.truth.partisanship_rank,
                post.author as usize,
                u64::from(post.assigned_impressions),
            );
        }
    }
    let viewer_days = dwell.len() as f64 * days;
    let per_viewer_day = |n: u64| {
        if viewer_days > 0.0 {
            (n as f64 / viewer_days) as f32
        } else {
            f32::NAN
        }
    };
    let over = |a: u64, b: u64| {
        if b > 0 {
            (a as f64 / b as f64) as f32
        } else {
            f32::NAN
        }
    };
    let kept = Reach {
        half: reach.half.saturating_sub(excluded.half),
        extreme: reach.extreme.saturating_sub(excluded.extreme),
        middle: reach.middle.saturating_sub(excluded.middle),
        all: reach.all.saturating_sub(excluded.all),
    };
    let reach_measures = |r: Reach| {
        [
            per_viewer_day(r.half),
            per_viewer_day(r.extreme),
            per_viewer_day(r.middle),
            per_viewer_day(r.all),
            over(r.extreme, r.middle),
        ]
    };
    let [half, extreme, middle, all, ratio] = reach_measures(reach);
    let [half_kept, extreme_kept, middle_kept, all_kept, ratio_kept] = reach_measures(kept);
    let [sd_wellbeing, sd_polarisation, sd_informedness] = start_sds(e);
    let per_day = |v: &[f64]| {
        if v.is_empty() {
            f32::NAN
        } else {
            mean(v) as f32
        }
    };
    let share = |part: f64, whole: u32| {
        if whole > 0 {
            (part / f64::from(whole)) as f32
        } else {
            f32::NAN
        }
    };
    let sds = |c: Option<f64>| c.map_or(f32::NAN, |c| c as f32);
    [
        per_day(&dwell),
        per_day(&actions),
        share(f64::from(total.toxic), total.impressions),
        share(f64::from(total.misinformation), total.impressions),
        share(f64::from(total.same_group), total.impressions),
        share(total.post_toxicity, total.posts),
        share(total.shared_truth, total.shares),
        sds(assigned_change(
            e,
            world,
            |s| s.affective_polarisation,
            |p| p.start_polarisation,
        )),
        sds(assigned_change(
            e,
            world,
            |s| s.informedness,
            |p| p.start_informedness,
        )),
        sds(assigned_wellbeing_change(e, world)),
        if matches!(e.kind, Kind::Feed(FeedExperimentKind::HideFlagged { .. })) {
            share(f64::from(total.flagged), total.impressions)
        } else {
            f32::NAN
        },
        sds(assigned_raw_change(
            e,
            world,
            |s| s.affective_polarisation,
            |p| p.start_polarisation,
        )),
        sds(assigned_raw_change(
            e,
            world,
            |s| s.informedness,
            |p| p.start_informedness,
        )),
        sd_polarisation,
        sd_informedness,
        sd_wellbeing,
        half,
        extreme,
        middle,
        all,
        ratio,
        half_kept,
        extreme_kept,
        middle_kept,
        all_kept,
        ratio_kept,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Deactivation, FeedExperiment, NotificationsOff};
    use crate::init;
    use crate::metrics::{DailyMetrics, MetricId};
    use crate::ranking::{self, Candidate, FeedCtx, FeedScratch, Trending};
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
            ends_before_end: 0,
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
        let sds = notifications_off_sds(&world, &last);
        for (got, want) in sds.iter().zip([sd_dwell, sd_sessions, sd_start]) {
            assert!(
                (f64::from(*got) - want).abs() < 1e-6,
                "{got} against {want}"
            );
        }
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
            ends_before_end: 0,
        });
        let day = last(c);
        assert!(quiet_ids.iter().all(|id| day.get(*id).is_nan()));
        assert!(break_ids.iter().all(|id| day.get(*id).is_finite()));
    }

    /// A run of 30 days with a feed experiment of `kind` over its last 10.
    fn feed_config(kind: FeedExperimentKind, share: f32, placebo: bool) -> Config {
        let mut config = Config::default();
        config.run.population_size = 400;
        config.run.days = 30;
        config.run.feed_experiment = Some(FeedExperiment {
            kind,
            days: 10,
            share,
            placebo,
        });
        config
    }

    const HIDE: FeedExperimentKind = FeedExperimentKind::HideFlagged { threshold: 0.3 };
    const PENALTY: FeedExperimentKind = FeedExperimentKind::FewerSameGroup { penalty: 0.5 };
    const CHRONO: FeedExperimentKind = FeedExperimentKind::Chronological;
    const FEED_KINDS: [FeedExperimentKind; 4] =
        [HIDE, FeedExperimentKind::NoReshares, PENALTY, CHRONO];

    const FEED_IDS: [MetricId; FEED_MEASURES] = [
        MetricId::FeedExperimentDwell,
        MetricId::FeedExperimentEngagement,
        MetricId::FeedExperimentToxicShare,
        MetricId::FeedExperimentMisinformationShare,
        MetricId::FeedExperimentSameGroupShare,
        MetricId::FeedExperimentPostToxicity,
        MetricId::FeedExperimentSharedTruth,
        MetricId::FeedExperimentPolarisation,
        MetricId::FeedExperimentInformedness,
        MetricId::FeedExperimentWellbeing,
        MetricId::FeedExperimentFlaggedShare,
        MetricId::FeedExperimentPolarisationRaw,
        MetricId::FeedExperimentInformednessRaw,
        MetricId::FeedExperimentPolarisationSd,
        MetricId::FeedExperimentInformednessSd,
        MetricId::FeedExperimentWellbeingSd,
        MetricId::FeedExperimentPartisanHalfImpressions,
        MetricId::FeedExperimentExtremeImpressions,
        MetricId::FeedExperimentMiddleImpressions,
        MetricId::FeedExperimentImpressions,
        MetricId::FeedExperimentExtremeMiddleRatio,
        MetricId::FeedExperimentPartisanHalfImpressionsKept,
        MetricId::FeedExperimentExtremeImpressionsKept,
        MetricId::FeedExperimentMiddleImpressionsKept,
        MetricId::FeedExperimentImpressionsKept,
        MetricId::FeedExperimentExtremeMiddleRatioKept,
    ];

    #[test]
    fn a_feed_experiment_assigns_the_regular_users_on_its_first_day() {
        for kind in FEED_KINDS {
            let config = feed_config(kind, 0.5, false);
            let mut world = init::build_world(&config);
            let n = world.population();
            for (i, p) in world.people.iter_mut().enumerate() {
                p.state.active_bits = if i < 200 { 0b1111 } else { 0b111 };
            }
            let on = |day| DayCtx {
                config: &config,
                day,
            };
            begin_day(&mut world, &on(19));
            assert!(world.truth.experiment.is_none(), "not the first day");
            begin_day(&mut world, &on(20));
            let e = world.truth.experiment.as_ref().expect("started");
            assert_eq!((e.kind, e.first_day, e.end_day), (Kind::Feed(kind), 20, 30));
            assert_eq!(e.exposure.len(), n);
            assert!(e.usage.is_empty());
            let mut assigned = 0;
            for (i, (p, person)) in e.participants.iter().zip(&world.people).enumerate() {
                assert_eq!(p.is_some(), i < 200 && person.state.is_user, "person {i}");
                assert!(!e.on_break(i) && !e.notifications_off(i));
                let treated = p.is_some_and(|p| p.assigned);
                assert_eq!(e.feed_treatment(i), treated.then_some(kind), "person {i}");
                assert_eq!(world.truth.feed_treatment(i), treated.then_some(kind));
                assigned += usize::from(treated);
            }
            let eligible = e.participants.iter().flatten().count();
            assert!(eligible > 50, "{eligible} eligible");
            let sd = (eligible as f32 * 0.25).sqrt();
            assert!((assigned as f32 - 0.5 * eligible as f32).abs() < 4.0 * sd);
            // The same draws in a placebo, and nobody treated.
            let placebo = feed_config(kind, 0.5, true);
            let mut other = init::build_world(&placebo);
            for (i, p) in other.people.iter_mut().enumerate() {
                p.state.active_bits = if i < 200 { 0b1111 } else { 0b111 };
            }
            begin_day(
                &mut other,
                &DayCtx {
                    config: &placebo,
                    day: 20,
                },
            );
            let ep = other.truth.experiment.as_ref().expect("started");
            assert_eq!(ep.participants, e.participants, "same people, same groups");
            assert!((0..n).all(|i| ep.feed_treatment(i).is_none()));
        }
    }

    #[test]
    fn a_feed_placebo_matches_a_run_without_the_experiment() {
        let history = |config: Config| {
            let mut sim = Simulation::new(config).expect("valid");
            sim.run_to_end();
            (sim.history().to_vec(), sim.world().truth.experiment.clone())
        };
        let differs = |a: &[DailyMetrics], b: &[DailyMetrics], days: usize| {
            a.iter().zip(b).take(days).any(|(x, y)| {
                MetricId::all()
                    .filter(|id| !id.needs_experiment())
                    .any(|id| x.get(id).to_bits() != y.get(id).to_bits())
            })
        };
        let mut none = feed_config(HIDE, 0.5, false);
        none.run.feed_experiment = None;
        let (plain, _) = history(none);
        for kind in FEED_KINDS {
            let (real, er) = history(feed_config(kind, 0.5, false));
            let (placebo, ep) = history(feed_config(kind, 0.5, true));
            let (er, ep) = (er.expect("ran"), ep.expect("ran"));
            assert_eq!(er.participants, ep.participants, "same people, same groups");
            // Identical before the first day; the placebo identical to a run
            // without the experiment throughout; the real run differs after.
            assert!(!differs(&real, &placebo, 20), "{kind:?}");
            assert!(!differs(&placebo, &plain, 30), "{kind:?}");
            assert!(differs(&real, &placebo, 30), "{kind:?}");
            // The measures exist on the last day only, in both runs.
            for h in [&real, &placebo] {
                let (last, rest) = h.split_last().expect("days");
                for id in FEED_IDS {
                    assert!(rest.iter().all(|d| d.get(id).is_nan()), "{id:?}");
                    // Only a hide-flagged experiment has a flagged share.
                    let exists = id != MetricId::FeedExperimentFlaggedShare || kind == HIDE;
                    assert_eq!(last.get(id).is_finite(), exists, "{kind:?} {id:?}");
                }
            }
        }
        for id in FEED_IDS {
            assert!(plain.iter().all(|d| d.get(id).is_nan()), "{id:?}");
        }
    }

    /// With the classifier switched off, a hide-flagged experiment still
    /// scores every post (on the classifier's own draws), and its placebo
    /// still matches a run without the experiment.
    #[test]
    fn a_hide_flagged_placebo_without_the_classifier_matches_a_run_without_it() {
        let history = |mut config: Config| {
            config.design.classifier_enabled = false;
            let mut sim = Simulation::new(config).expect("valid");
            sim.run_to_end();
            sim.history().to_vec()
        };
        let differs = |a: &[DailyMetrics], b: &[DailyMetrics], days: usize| {
            a.iter().zip(b).take(days).any(|(x, y)| {
                MetricId::all()
                    .filter(|id| !id.needs_experiment())
                    .any(|id| x.get(id).to_bits() != y.get(id).to_bits())
            })
        };
        let mut none = feed_config(HIDE, 0.5, false);
        none.run.feed_experiment = None;
        let plain = history(none);
        let real = history(feed_config(HIDE, 0.5, false));
        let placebo = history(feed_config(HIDE, 0.5, true));
        assert!(!differs(&placebo, &plain, 30));
        assert!(!differs(&real, &placebo, 20));
        assert!(differs(&real, &placebo, 30));
        let last = placebo.last().expect("days");
        let dose = last.get(MetricId::FeedExperimentFlaggedShare);
        assert!(dose > 0.0 && dose < 1.0, "{dose}");
    }

    /// Every user's feed in `world`, built as the run builds it and as its
    /// placebo would on the same draws: `(treated, real, placebo)`.
    fn feeds_both_ways(
        config: &Config,
        world: &World,
        day: u32,
        limit: usize,
    ) -> Vec<(bool, Vec<Candidate>, Vec<Candidate>)> {
        let mut placebo = world.clone();
        if let Some(e) = placebo.truth.experiment.as_mut() {
            e.placebo = true;
        }
        let trending = Trending::build(world);
        let build = |w: &World, v: usize| {
            let mut scratch = FeedScratch::default();
            scratch.reset(w.posts.len());
            let mut rng = rng::stream_rng(config.run.seed, day, v as u64, Stream::Feed);
            let ctx = FeedCtx {
                world: w,
                config,
                day,
                viewer: v,
            };
            ranking::build_feed(&ctx, limit, &trending, &mut rng, &mut scratch);
            scratch.candidates.clone()
        };
        (0..world.population())
            .filter(|&v| world.people[v].state.is_user)
            .map(|v| {
                let treated = world.truth.feed_treatment(v).is_some();
                (treated, build(world, v), build(&placebo, v))
            })
            .collect()
    }

    /// The world of a real run with a feed experiment of `kind`, 5 days into
    /// it, and the day after.
    fn world_in_experiment(kind: FeedExperimentKind) -> (Config, World, u32) {
        let config = feed_config(kind, 0.5, false);
        let mut sim = Simulation::new(config.clone()).expect("valid");
        for _ in 0..25 {
            sim.step().expect("a day");
        }
        (config, sim.world().clone(), sim.day())
    }

    /// The same feed: the same posts with bit-identical scores.
    fn same_feed(a: &[Candidate], b: &[Candidate]) -> bool {
        a.len() == b.len()
            && a.iter()
                .zip(b)
                .all(|(x, y)| x.post == y.post && x.score.to_bits() == y.score.to_bits())
    }

    #[test]
    fn hidden_flagged_posts_never_reach_the_treated() {
        let (config, world, day) = world_in_experiment(HIDE);
        let hidden = |c: &Candidate| {
            world
                .posts
                .get(c.post)
                .and_then(|p| p.classifier_score)
                .is_some_and(|s| s > 0.3)
        };
        let (mut treated, mut would_see) = (0, 0);
        for (is_treated, real, placebo) in feeds_both_ways(&config, &world, day, 10_000) {
            if is_treated {
                treated += 1;
                assert!(
                    !real.iter().any(hidden),
                    "a hidden post reached a treated viewer"
                );
                would_see += placebo.iter().filter(|c| hidden(c)).count();
            } else {
                assert!(same_feed(&real, &placebo), "an untreated feed changed");
            }
        }
        assert!(treated > 20, "{treated} treated users");
        assert!(would_see > 20, "the filter has teeth: {would_see}");
    }

    /// The flagged share is the dose a placebo run measures; in a real run
    /// the treated see no flagged post, and the other kinds have none.
    #[test]
    fn the_flagged_share_is_the_placebos_dose() {
        let last = |kind, placebo| {
            let mut sim = Simulation::new(feed_config(kind, 0.5, placebo)).expect("valid");
            sim.run_to_end();
            sim.history()
                .last()
                .map_or(f32::NAN, |d| d.get(MetricId::FeedExperimentFlaggedShare))
        };
        let placebo = last(HIDE, true);
        assert!(placebo > 0.01 && placebo < 0.9, "{placebo}");
        assert_eq!(last(HIDE, false), 0.0);
        assert!(last(PENALTY, true).is_nan());
    }

    #[test]
    fn the_treated_get_no_reshares_and_keep_original_posts() {
        let (config, world, day) = world_in_experiment(FeedExperimentKind::NoReshares);
        let (mut originals, mut would_see) = (0, 0);
        for (is_treated, real, placebo) in feeds_both_ways(&config, &world, day, 10_000) {
            if is_treated {
                assert!(
                    real.iter().all(|c| c.via.is_none()),
                    "a reshare reached them"
                );
                originals += real.iter().filter(|c| c.in_network).count();
                would_see += placebo.iter().filter(|c| c.via.is_some()).count();
            } else {
                assert!(same_feed(&real, &placebo), "an untreated feed changed");
            }
        }
        assert!(originals > 100, "in-network originals stay: {originals}");
        assert!(would_see > 20, "the filter has teeth: {would_see}");
    }

    /// The treated read the chronological feed of their in-network posts:
    /// no recommendation, newest first, nothing scored. Everyone else's feed,
    /// and the treated's in the placebo, is the ranked one, on the same draws.
    #[test]
    fn the_treated_read_the_chronological_feed_and_nobody_else_does() {
        let (config, world, day) = world_in_experiment(CHRONO);
        let (mut treated, mut recommended_in_placebo) = (0, 0);
        for (is_treated, real, placebo) in feeds_both_ways(&config, &world, day, 10_000) {
            if is_treated {
                treated += 1;
                assert!(real.iter().all(|c| c.in_network), "a recommendation");
                assert!(
                    real.windows(2)
                        .all(|w| (w[0].item_day, w[0].post) >= (w[1].item_day, w[1].post)),
                    "not newest first"
                );
                assert!(real.iter().all(|c| c.score == 0.0));
                recommended_in_placebo += placebo.iter().filter(|c| !c.in_network).count();
                assert!(
                    placebo.iter().any(|c| c.score != 0.0),
                    "the placebo is ranked"
                );
            } else {
                assert!(same_feed(&real, &placebo), "an untreated feed changed");
            }
        }
        assert!(treated > 20, "{treated} treated users");
        assert!(recommended_in_placebo > 20, "the switch has teeth");
    }

    /// Under the integrity preset, which demotes, the treated's reach
    /// without posts removed or demoted lies below the reach with them, in
    /// a real run and in its placebo.
    #[test]
    fn the_kept_reach_leaves_out_demoted_posts_in_a_run() {
        for placebo in [false, true] {
            let mut config = feed_config(CHRONO, 0.5, placebo);
            crate::presets::integrity_ranking(&mut config.design);
            // At 400 people, with the exposure contagion of lab #38, the
            // placebo's treated saw no demoted post in these ten days.
            config.run.population_size = 1000;
            let mut sim = Simulation::new(config).expect("valid");
            sim.run_to_end();
            let last = sim.history().last().expect("days").clone();
            let all = last.get(MetricId::FeedExperimentImpressions);
            let kept = last.get(MetricId::FeedExperimentImpressionsKept);
            assert!(
                kept > 0.0 && kept < all,
                "placebo {placebo}: {kept} of {all}"
            );
            let half = last.get(MetricId::FeedExperimentPartisanHalfImpressions);
            let half_kept = last.get(MetricId::FeedExperimentPartisanHalfImpressionsKept);
            assert!(half_kept <= half && half <= all);
        }
    }

    #[test]
    fn same_group_posts_are_pushed_down_for_the_treated() {
        let (config, world, day) = world_in_experiment(PENALTY);
        let top = 20;
        let same = |feed: &[Candidate]| feed.iter().filter(|c| c.signals.same_group).count();
        let (mut real_same, mut placebo_same) = (0, 0);
        for (is_treated, real, placebo) in feeds_both_ways(&config, &world, day, top) {
            if is_treated {
                for c in &real {
                    let expected = if c.signals.same_group { -0.5 } else { 0.0 };
                    assert_eq!(c.components.experiment, expected);
                }
                assert!(
                    same(&real) <= same(&placebo),
                    "more same-group posts on top"
                );
                real_same += same(&real);
                placebo_same += same(&placebo);
            } else {
                assert!(same_feed(&real, &placebo), "an untreated feed changed");
                assert!(real.iter().all(|c| c.components.experiment == 0.0));
            }
        }
        assert!(
            (real_same as f32) < 0.9 * placebo_same as f32,
            "{real_same} against {placebo_same}"
        );
    }

    #[test]
    fn the_feed_measures_follow_their_definitions() {
        let config = feed_config(FeedExperimentKind::NoReshares, 0.5, false);
        let mut world = init::build_world(&config);
        for (i, p) in world.people.iter_mut().enumerate() {
            let s = &mut p.state;
            s.active_bits = u64::MAX;
            s.wellbeing = if i % 2 == 0 { 0.4 } else { 0.6 };
            s.affective_polarisation = if i % 3 == 0 { 0.2 } else { 0.5 };
            s.informedness = if i % 4 == 0 { 0.3 } else { 0.7 };
        }
        begin_day(
            &mut world,
            &DayCtx {
                config: &config,
                day: 20,
            },
        );
        let e = world.truth.experiment.as_mut().expect("started");
        // Over the 10 days each assigned person but the first spends 20
        // units of time, takes 30 actions, sees 40 items (4 toxic, 2
        // misinformation, 10 by their own group), writes 2 posts of summed
        // toxicity 0.6 and shares 4 posts of summed truth 2; the first left
        // and did nothing. Everyone else did something else entirely.
        let mut first = true;
        let mut assigned = 0.0;
        for i in 0..e.participants.len() {
            let Some(p) = e.participants[i] else {
                continue;
            };
            e.exposure[i] = if !p.assigned {
                Exposure {
                    dwell: 99.0,
                    actions: 99,
                    impressions: 10,
                    toxic: 10,
                    ..Exposure::default()
                }
            } else if first {
                first = false;
                assigned += 1.0;
                Exposure::default()
            } else {
                assigned += 1.0;
                Exposure {
                    dwell: 20.0,
                    actions: 30,
                    impressions: 40,
                    toxic: 4,
                    misinformation: 2,
                    same_group: 10,
                    flagged: 0,
                    partisan_half: 20,
                    extreme: 6,
                    middle: 8,
                    posts: 2,
                    post_toxicity: 0.6,
                    shares: 4,
                    shared_truth: 2.0,
                }
            };
        }
        let e = e.clone();
        for (i, p) in world.people.iter_mut().enumerate() {
            if e.participants[i].is_some_and(|p| p.assigned) {
                p.state.wellbeing += 0.1;
                p.state.affective_polarisation -= 0.05;
                p.state.informedness += 0.02;
            }
        }
        let last = DayCtx {
            config: &config,
            day: 29,
        };
        let m = feed_measures(&world, &last);
        let active = (assigned - 1.0) / assigned;
        let sd_pol = e.participants_sd(|p| p.start_polarisation).expect("spread");
        let sd_inf = e.participants_sd(|p| p.start_informedness).expect("spread");
        let sd_wb = e.start_sd().expect("spread");
        // Per treated viewer and day: 20, 6, 8 and 40 impressions over 10
        // days for all but the first; no post was removed or demoted, so
        // the kept measures are the same.
        let reach = [2.0 * active, 0.6 * active, 0.8 * active, 4.0 * active, 0.75];
        let mut expected = vec![
            2.0 * active,
            3.0 * active,
            0.1,
            0.05,
            0.25,
            0.3,
            0.5,
            -0.05 / sd_pol,
            0.02 / sd_inf,
            0.1 / sd_wb,
            f64::NAN,
            -0.05,
            0.02,
            sd_pol,
            sd_inf,
            sd_wb,
        ];
        expected.extend(reach);
        expected.extend(reach);
        assert_eq!(expected.len(), FEED_MEASURES);
        for (k, (got, want)) in m.iter().zip(expected).enumerate() {
            if want.is_nan() {
                assert!(got.is_nan(), "measure {k}: {got}, not NaN");
                continue;
            }
            assert!(
                (f64::from(*got) - want).abs() < 1e-4,
                "measure {k}: {got} against {want}"
            );
        }
        let before = DayCtx {
            config: &config,
            day: 28,
        };
        assert!(feed_measures(&world, &before).iter().all(|v| v.is_nan()));
        assert!(
            notifications_off_changes(&world, &last)
                .iter()
                .all(|v| v.is_nan())
        );
        assert!(break_change(&world, &last).is_nan() && post_break_dwell(&world, &last).is_nan());
    }

    /// The reach measures on a hand-checkable case: two treated viewers,
    /// a removed post and a demoted one, a post that ended removed before
    /// the last day, and a bot's post.
    #[test]
    fn the_reach_measures_count_by_partisanship_with_and_without_removed_posts() {
        let config = feed_config(CHRONO, 0.5, false);
        let mut world = init::build_world(&config);
        for p in &mut world.people {
            p.state.active_bits = u64::MAX;
        }
        begin_day(
            &mut world,
            &DayCtx {
                config: &config,
                day: 20,
            },
        );
        let n = world.population();
        // Authors by partisanship rank: the top author of the population is
        // in the half and the tenth, the median one in the half and the
        // middle fifth.
        let rank = &world.truth.partisanship_rank;
        let by_rank = |r: u32| rank.iter().position(|&x| x == r).expect("ranked");
        let top = by_rank(n as u32 - 1);
        let median = by_rank(n as u32 / 2);
        let e = world.truth.experiment.as_mut().expect("started");
        let treated: Vec<usize> = e.assigned().map(|(i, _)| i).take(2).collect();
        e.participants.iter_mut().enumerate().for_each(|(i, p)| {
            if let Some(p) = p {
                p.assigned = treated.contains(&i);
            }
        });
        // Over the 10 days the two treated saw 100 items each: 30 by the
        // top tenth, 20 by the middle fifth (all in the half too, with 10
        // more), the rest by the lower half or bots.
        for &i in &treated {
            e.exposure[i] = Exposure {
                impressions: 100,
                partisan_half: 60,
                extreme: 30,
                middle: 20,
                ..Exposure::default()
            };
        }
        // An ended post by the top author, removed, seen 6 times by them.
        e.removed_or_demoted.add(rank, top, 6);
        // Live posts: by the median author demoted (4), by the top author
        // untouched (50), by a bot removed (3).
        let mut live = |author: usize, impressions: u32, removed: bool, demoted: bool| {
            let mut rng = rng::stream_rng(1, 0, author as u64, Stream::PostDraw);
            let profile = crate::content::AuthorProfile::plain(crate::content::Group::A, 0.5, 0.5);
            let mut post = crate::content::generate_post(
                &mut rng,
                world.posts.next_id(),
                29,
                &profile,
                crate::content::Style::Outrage,
                &[1.0 / 6.0; 6],
                &config.world,
            );
            post.author = author as u32;
            post.assigned_impressions = impressions;
            post.removed = removed;
            post.demoted = demoted;
            world.posts.push(post);
        };
        live(median, 4, false, true);
        live(top, 50, false, false);
        live(n + 5, 3, true, false);
        let m = feed_measures(
            &world,
            &DayCtx {
                config: &config,
                day: 29,
            },
        );
        let got = |k: usize| f64::from(m[16 + k]);
        // 2 viewers x 10 days = 20 viewer-days: the half 120 / 20, the tenth
        // 60 / 20, the middle fifth 40 / 20, all 200 / 20, and the tenth
        // over the middle fifth 60 / 40.
        for (k, want) in [6.0, 3.0, 2.0, 10.0, 1.5].into_iter().enumerate() {
            assert!((got(k) - want).abs() < 1e-5, "reach {k}: {}", got(k));
        }
        // Without posts removed or demoted: the half loses 6 + 4, the tenth
        // 6, the middle fifth 4, all posts 6 + 4 + 3; the untouched post
        // stays.
        for (k, want) in [5.5, 2.7, 1.8, 9.35, 1.5].into_iter().enumerate() {
            assert!((got(5 + k) - want).abs() < 1e-5, "kept {k}: {}", got(5 + k));
        }
    }

    #[test]
    fn the_exposure_log_sums_each_day_of_the_experiment() {
        let mut sim = Simulation::new(feed_config(PENALTY, 0.5, false)).expect("valid");
        let n = sim.world().population();
        let mut sums = vec![Exposure::default(); n];
        while sim.step().is_some() {
            let world = sim.world();
            if world.truth.experiment.is_none() {
                continue;
            }
            for (sum, person) in sums.iter_mut().zip(&world.people) {
                sum.add_day(person);
            }
        }
        let e = sim.world().truth.experiment.as_ref().expect("ran");
        assert_eq!(e.exposure, sums);
        let seen: u32 = sums.iter().map(|x| x.impressions).sum();
        let shares: u32 = sums.iter().map(|x| x.shares).sum();
        let same: u32 = sums.iter().map(|x| x.same_group).sum();
        assert!(seen > 1_000 && shares > 10 && same > 100 && same < seen);
    }

    /// A run of 40 days with a break of 10 days that ends 8 days before the
    /// run: days 22 to 31.
    fn early_break(placebo: bool) -> Config {
        let mut config = config(40, 0.5);
        config.run.deactivation = Some(Deactivation {
            days: 10,
            share: 0.5,
            placebo,
            ends_before_end: 8,
        });
        config
    }

    #[test]
    fn a_break_that_ends_early_lets_its_group_back_and_logs_their_time() {
        let mut sim = Simulation::new(early_break(false)).expect("valid");
        let n = sim.world().population();
        let mut dwell = vec![0.0f64; n];
        let mut back = 0;
        let mut at_end = None;
        while sim.step().is_some() {
            let day = sim.day() - 1;
            let world = sim.world();
            let Some(e) = world.truth.experiment.as_ref() else {
                assert!(day < 22, "day {day}");
                continue;
            };
            assert_eq!((e.first_day, e.end_day), (22, 32));
            for (i, p) in world.people.iter().enumerate() {
                let assigned = e.participants[i].is_some_and(|p| p.assigned);
                assert_eq!(e.on_break(i), assigned && day < 32, "person {i} day {day}");
                if assigned && day >= 32 {
                    dwell[i] += f64::from(p.state.today.dwell);
                    back += usize::from(p.state.today.opened);
                }
            }
            if day == 31 {
                let measures = break_measures(e, world);
                assert_eq!(e.break_end, Some(measures), "kept at the break's end");
                at_end = Some(measures);
            } else if day < 31 {
                assert!(e.break_end.is_none());
            }
        }
        assert!(back > 20, "the break group comes back: {back}");
        let at_end = at_end.expect("the break ended");
        let h = sim.history();
        let (last, rest) = h.split_last().expect("days");
        let ids = [
            MetricId::DeactivationEffect,
            MetricId::BreakChange,
            MetricId::BreakPolarisation,
            MetricId::BreakInformedness,
            MetricId::PostBreakDwell,
        ];
        for id in ids {
            assert!(rest.iter().all(|d| d.get(id).is_nan()), "{id:?}");
        }
        let reported = |id| last.get(id).to_bits();
        assert_eq!(
            reported(MetricId::DeactivationEffect),
            at_end.effect.to_bits()
        );
        assert_eq!(reported(MetricId::BreakChange), at_end.wellbeing.to_bits());
        assert_eq!(
            reported(MetricId::BreakPolarisation),
            at_end.polarisation.to_bits()
        );
        assert_eq!(
            reported(MetricId::BreakInformedness),
            at_end.informedness.to_bits()
        );
        let e = sim.world().truth.experiment.as_ref().expect("ran");
        let per_day: Vec<f64> = e.assigned().map(|(i, _)| dwell[i] / 8.0).collect();
        let expected = mean(&per_day);
        let got = f64::from(last.get(MetricId::PostBreakDwell));
        assert!(
            expected > 0.0 && (got - expected).abs() < 1e-4,
            "{got} against {expected}"
        );
        // A break that ends with the run has no time after it.
        let mut sim = Simulation::new(config(40, 0.5)).expect("valid");
        sim.run_to_end();
        let last = sim.history().last().expect("days").clone();
        assert!(last.get(MetricId::PostBreakDwell).is_nan());
        assert!(last.get(MetricId::BreakPolarisation).is_finite());
        assert!(last.get(MetricId::BreakInformedness).is_finite());
    }

    #[test]
    fn an_early_break_placebo_assigns_the_same_people_and_keeps_nobody_off() {
        let run = |config: Config| {
            let mut sim = Simulation::new(config).expect("valid");
            sim.run_to_end();
            (sim.history().to_vec(), sim.world().truth.experiment.clone())
        };
        let (real, er) = run(early_break(false));
        let (placebo, ep) = run(early_break(true));
        let mut none = early_break(true);
        none.run.deactivation = None;
        let (plain, _) = run(none);
        let (er, ep) = (er.expect("ran"), ep.expect("ran"));
        assert_eq!(er.participants, ep.participants);
        let same = |a: &[DailyMetrics], b: &[DailyMetrics], days: usize| {
            a.iter().zip(b).take(days).all(|(x, y)| {
                MetricId::all()
                    .filter(|id| !id.needs_experiment())
                    .all(|id| x.get(id).to_bits() == y.get(id).to_bits())
            })
        };
        assert!(same(&real, &placebo, 22) && !same(&real, &placebo, 40));
        assert!(same(&placebo, &plain, 40));
        assert!(
            placebo
                .last()
                .is_some_and(|d| d.get(MetricId::PostBreakDwell) > 0.0)
        );
    }

    #[test]
    fn the_break_measures_polarisation_and_informedness_in_participant_sds() {
        let config = config(30, 0.5);
        let mut world = init::build_world(&config);
        for (i, p) in world.people.iter_mut().enumerate() {
            let s = &mut p.state;
            s.active_bits = u64::MAX;
            s.affective_polarisation = if i % 3 == 0 { 0.2 } else { 0.5 };
            s.informedness = if i % 2 == 0 { 0.3 } else { 0.6 };
        }
        begin_day(
            &mut world,
            &DayCtx {
                config: &config,
                day: 20,
            },
        );
        let e = world.truth.experiment.clone().expect("started");
        for (i, p) in world.people.iter_mut().enumerate() {
            if e.participants[i].is_some_and(|p| p.assigned) {
                p.state.affective_polarisation -= 0.04;
                p.state.informedness -= 0.03;
            } else if e.participants[i].is_some() {
                p.state.informedness += 0.5;
            }
        }
        let sd_pol = e.participants_sd(|p| p.start_polarisation).expect("spread");
        let sd_inf = e.participants_sd(|p| p.start_informedness).expect("spread");
        let last = DayCtx {
            config: &config,
            day: 29,
        };
        let [pol, inf, pol_raw, inf_raw] = break_society_changes(&world, &last);
        assert!((f64::from(pol) + 0.04 / sd_pol).abs() < 1e-4, "{pol}");
        assert!((f64::from(inf) + 0.03 / sd_inf).abs() < 1e-4, "{inf}");
        assert!((pol_raw + 0.04).abs() < 1e-5, "{pol_raw}");
        assert!((inf_raw + 0.03).abs() < 1e-5, "{inf_raw}");
        // The SDs the scaled changes divide by are reported with them.
        let [sd_wb, sd_p, sd_i] = break_sds(&world, &last);
        assert!((f64::from(sd_p) - sd_pol).abs() < 1e-6);
        assert!((f64::from(sd_i) - sd_inf).abs() < 1e-6);
        assert!((f64::from(sd_wb) - e.start_sd().expect("spread")).abs() < 1e-6);
        let before = DayCtx {
            config: &config,
            day: 28,
        };
        assert!(
            break_society_changes(&world, &before)
                .iter()
                .all(|v| v.is_nan())
        );
    }
}
