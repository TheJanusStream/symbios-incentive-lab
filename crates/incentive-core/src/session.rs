//! Opening the app, the day's sessions and consuming a feed (docs/model.md,
//! 6.1, 6.6, 7.1, 7.2 and 7.6).

use crate::behaviour::{like_terms, sig, weighted_quality};
use crate::config::{Config, FeedExperimentKind, WorldAssumptions};
use crate::content::{Group, Style};
use crate::inspector::{self, FeedExplanation, Inspector, ItemActions, ServedSession, SessionKind};
use crate::network::{self, Reshare};
use crate::ranking::{self, Candidate, FeedCtx, FeedKind, FeedScratch, Trending};
use crate::rng::{self, Stream, StreamRng};
use crate::world::{DayCtx, World, partisan_band};

/// Informedness lost per believed misinformation item, per unit of falsehood,
/// and gained per good Informative item, per unit of interest-weighted
/// quality. The text's 0.05 and 0.01 drive informedness to its bounds within
/// weeks; docs/model.md, section 13, records the calibration.
pub(crate) const MISBELIEF_LOSS: f32 = 0.008;
const INFORMATION_GAIN: f32 = 0.0004;
/// Highest chance of opening the app on a day.
const MAX_OPEN_CHANCE: f32 = 0.98;
/// How much notifications at full intensity raise the chance of opening the
/// app, `1 + NOTIFICATION_OPEN_GAIN * notification_intensity` (docs/model.md,
/// 6.1 and 13.1).
const NOTIFICATION_OPEN_GAIN: f32 = 0.15;
/// How much notifications at full intensity lengthen an impulsive person's
/// first session, `1 + NOTIFICATION_LENGTH_GAIN * notification_intensity *
/// impulsivity` (docs/model.md, 6.1 and 13.1).
const NOTIFICATION_LENGTH_GAIN: f32 = 0.0;

/// Partisanship above which a viewer hardens on seeing a calm out-group post
/// (docs/model.md, 8.1).
pub(crate) const BACKFIRE_PARTISANSHIP: f32 = 0.7;

/// The polarisation change from one impression of a calm (not hostile)
/// out-group post (docs/model.md, 8.1): liking it depolarises by
/// `contact_rate * quality` (contact); a strongly partisan viewer also gains
/// `polarisation_backfire * contact_rate` from seeing it at all (backfire).
pub(crate) fn contact_change(
    partisanship: f32,
    liked: bool,
    quality: f32,
    assume: &WorldAssumptions,
) -> f32 {
    let mut change = 0.0;
    if liked {
        change -= assume.contact_rate * quality;
    }
    if partisanship > BACKFIRE_PARTISANSHIP {
        change += assume.polarisation_backfire * assume.contact_rate;
    }
    change
}

/// The reflective value of the time an item with dwell `dwell` takes,
/// `time_cost * D` (docs/model.md, 7.2).
pub(crate) fn time_cost(dwell: f32, time_cost: f32) -> f32 {
    time_cost * dwell
}

/// Whether an item is worth less on reflection than the time it takes,
/// `R < time_cost * D`: the time the metric `low_value_time` counts
/// (docs/model.md, 9.2). At `time_cost` 0 it is an item with negative `R`.
pub(crate) fn is_low_value(reflective: f32, dwell: f32, cost: f32) -> bool {
    reflective < time_cost(dwell, cost)
}

/// Decides who opens the app today, and how many extra sessions each opener
/// has after the first (docs/model.md, 6.1). Posting and sessions both use
/// it. People on a deactivation experiment's break (docs/model.md, 13.2) do
/// not open it and draw nothing; people whose notifications a
/// notifications-off experiment switched off open it with intensity 0.
pub(crate) fn decide_openers(world: &mut World, ctx: &DayCtx) {
    let design_ni = ctx.config.design.notification_intensity;
    let assume = &ctx.config.world;
    let extra_sessions = assume.recheck_rate > 0.0 || assume.notification_pull > 0.0;
    for (i, person) in world.people.iter_mut().enumerate() {
        if !person.state.is_user || world.truth.on_break(i) {
            continue;
        }
        let ni = world.truth.notification_intensity(i, design_ni);
        let mut rng = ctx.rng(i as u64, Stream::Activity);
        let p = (person.traits.activity
            * (0.6 + 0.8 * person.state.habit)
            * (1.0 + NOTIFICATION_OPEN_GAIN * ni))
            .clamp(0.0, MAX_OPEN_CHANCE);
        if rng::chance(&mut rng, p) {
            person.state.today.opened = true;
            world.tally.openers += 1;
            if extra_sessions {
                let habitual = assume.recheck_rate * person.state.habit;
                let notified = assume.notification_pull * ni;
                let mut rng = ctx.rng(i as u64, Stream::Sessions);
                let today = &mut person.state.today;
                today.habitual_sessions = rng::poisson(&mut rng, habitual);
                today.notified_sessions = rng::poisson(&mut rng, notified);
            }
        }
    }
}

/// The session's item cap `L` (docs/model.md, 6.1), with the viewer's own
/// notification intensity.
pub(crate) fn session_limit(world: &World, config: &Config, v: usize) -> usize {
    let person = &world.people[v];
    let habit = person.state.habit;
    let ni = world
        .truth
        .notification_intensity(v, config.design.notification_intensity);
    let raw = config.world.base_session_items
        * (0.7 + 0.6 * habit)
        * (1.0 + NOTIFICATION_LENGTH_GAIN * ni * person.traits.impulsivity);
    let mut limit = raw.round().max(1.0) as usize;
    if person.state.time_limited {
        limit = limit.min(person.traits.time_budget as usize);
    }
    limit
}

/// An extra session's item cap: `round(extra_session_share * L)`, at least 1
/// (docs/model.md, 6.1).
pub(crate) fn extra_session_cap(first_cap: usize, share: f32) -> usize {
    (share * first_cap as f32).round().max(1.0) as usize
}

/// The most items viewer `v` may see in a day across all sessions: the time
/// budget of a user who opted into time limits (6.10), else no limit.
fn daily_item_limit(world: &World, v: usize) -> Option<usize> {
    let person = &world.people[v];
    person
        .state
        .time_limited
        .then_some(person.traits.time_budget as usize)
}

/// Hours in a day: a user's sessions get a time of day uniform over them
/// (docs/model.md, 6.8).
const HOURS_PER_DAY: f32 = 24.0;

/// Runs every session of the day in ascending individual index. Each opener
/// gets one ranked feed for the day, long enough for every session; the first
/// session reads from its top, and each extra session continues down the list
/// after the items seen before it, so no item is shown twice in a day. The
/// feeds of tracked individuals go to the inspector as they were served, with
/// the day's sessions and what the viewer did with each item.
pub(crate) fn run_sessions(
    world: &mut World,
    ctx: &DayCtx,
    scratch: &mut FeedScratch,
    inspector: &mut Inspector,
) {
    let trending = Trending::build(world);
    scratch.reset(world.posts.len());
    let assume = &ctx.config.world;
    for v in 0..world.population() {
        let today = &world.people[v].state.today;
        if !today.opened {
            continue;
        }
        let (habitual, notified) = (today.habitual_sessions, today.notified_sessions);
        let first = session_limit(world, ctx.config, v);
        let extra = extra_session_cap(first, assume.extra_session_share);
        let daily_limit = daily_item_limit(world, v);
        let mut length = first + (habitual + notified) as usize * extra;
        if let Some(limit) = daily_limit {
            length = length.min(limit.max(first));
        }
        if ctx.config.design.community_notes {
            let mut clock = ctx.rng(v as u64, Stream::SessionClock);
            world.people[v].state.today.session_hour = HOURS_PER_DAY * rng::uniform(&mut clock);
        }
        let mut feed_rng = ctx.rng(v as u64, Stream::Feed);
        let feed_ctx = FeedCtx {
            world,
            config: ctx.config,
            day: ctx.day,
            viewer: v,
        };
        let kind = ranking::build_feed(&feed_ctx, length, &trending, &mut feed_rng, scratch);
        let mut served = inspector
            .is_tracked(v as u32)
            .then(|| served_feed(&feed_ctx, ctx, kind, first, scratch));
        // The extra sessions draw after the first on the same streams, so the
        // first session's draws do not depend on them.
        let mut rng = ctx.rng(v as u64, Stream::Engagement);
        let sessions = std::iter::once((SessionKind::First, first))
            .chain(std::iter::repeat_n(
                (SessionKind::Habitual, extra),
                habitual as usize,
            ))
            .chain(std::iter::repeat_n(
                (SessionKind::Notification, extra),
                notified as usize,
            ));
        let mut next = 0;
        for (index, (kind, cap)) in sessions.enumerate() {
            let cap = daily_limit.map_or(cap, |limit| cap.min(limit.saturating_sub(next)));
            let start = next;
            let read = Read {
                start,
                cap,
                session: index,
            };
            next = read_session(world, ctx, v, scratch, read, &mut rng, served.as_mut());
            if let Some(feed) = served.as_mut() {
                feed.sessions.push(ServedSession {
                    kind,
                    cap,
                    start,
                    seen: next - start,
                });
            }
        }
        world.tally.feeds += 1;
        world.tally.candidates += scratch.gathered as u32;
        world.tally.in_network_candidates += scratch.gathered_in_network;
        world.tally.in_network_outgroup_candidates += scratch.gathered_in_network_outgroup;
        world.tally.sessions += 1 + habitual + notified;
        world.tally.notification_sessions += notified;
        world.people[v].state.today.interruption_cost += notified as f32 * assume.interruption_cost;
        if let Some(feed) = served {
            inspector.record_feed(feed);
        }
    }
}

/// Where a session starts in the day's feed, its item cap, and its index
/// among the day's sessions.
#[derive(Clone, Copy)]
struct Read {
    start: usize,
    cap: usize,
    session: usize,
}

/// One session of viewer `v` over the day's feed: from `read.start`, at most
/// `read.cap` items, each followed by the stop draw of 6.1. Returns the
/// position after the last item seen, where the next session starts. A
/// session that finds the feed used up sees nothing.
fn read_session(
    world: &mut World,
    ctx: &DayCtx,
    v: usize,
    scratch: &FeedScratch,
    read: Read,
    rng: &mut StreamRng,
    mut served: Option<&mut FeedExplanation>,
) -> usize {
    let len = scratch.candidates.len();
    let end = read.start.saturating_add(read.cap).min(len).max(read.start);
    for rank in read.start..end {
        let c = &scratch.candidates[rank];
        let appeal = c.signals.appeal;
        let mut actions = impression(world, ctx, v, c, rng);
        actions.session = read.session;
        if let Some(item) = served
            .as_deref_mut()
            .and_then(|feed| feed.items.iter_mut().find(|item| item.rank == rank))
        {
            item.actions = Some(actions);
        }
        let habit = world.people[v].state.habit;
        if rng::chance(rng, sig(ctx.config.world.stop_bias - 1.5 * appeal - habit)) {
            return rank + 1;
        }
    }
    end
}

/// The record of the feed just built for a tracked viewer, before any item is
/// seen and before any session runs; `limit` is the first session's cap. A
/// chronological feed also gets the main ranking's scores, drawn on
/// the explanation stream; the session reads neither scores nor components,
/// so recording never changes the run.
fn served_feed(
    feed_ctx: &FeedCtx,
    ctx: &DayCtx,
    kind: FeedKind,
    limit: usize,
    scratch: &mut FeedScratch,
) -> FeedExplanation {
    if kind == FeedKind::Chronological {
        let mut rng = ctx.rng(feed_ctx.viewer as u64, Stream::Explain);
        ranking::score_for_explanation(feed_ctx, &mut rng, scratch);
    }
    FeedExplanation {
        viewer: feed_ctx.viewer as u32,
        day: ctx.day,
        served: true,
        kind,
        session_limit: limit,
        sessions: Vec::new(),
        items: inspector::explained_items(
            feed_ctx.world,
            &scratch.candidates,
            Some(ItemActions::default()),
            ctx.config.world.time_cost,
        ),
    }
}

/// A follow-graph change decided during an impression, applied after it.
enum FollowChange {
    None,
    Follow(u32),
    Unfollow(u32),
}

/// One item seen by viewer `v`: engagement draws, experience and learning of
/// the viewer, counters of the post. Returns what the viewer did.
fn impression(
    world: &mut World,
    ctx: &DayCtx,
    v: usize,
    c: &Candidate,
    rng: &mut StreamRng,
) -> ItemActions {
    let (change, mut actions) = record_impression(world, ctx, v, c, rng);
    match change {
        FollowChange::Follow(author) => {
            actions.followed = network::follow(&mut world.people, &mut world.accounts, v, author);
            if actions.followed
                && let Some(post) = world.posts.get_mut(c.post)
            {
                post.followers_gained += 1;
            }
        }
        FollowChange::Unfollow(author) => {
            actions.unfollowed =
                network::unfollow(&mut world.people, &mut world.accounts, v, author);
        }
        FollowChange::None => {}
    }
    actions
}

/// The threshold of a hide-flagged feed experiment, when the run sets one
/// up (a placebo included): impressions of posts scored above it are its
/// dose, counted for everyone and read for the treated (docs/model.md, 9.2).
fn flag_threshold(ctx: &DayCtx) -> Option<f32> {
    match ctx.config.run.feed_experiment.as_ref()?.kind {
        FeedExperimentKind::HideFlagged { threshold } => Some(threshold),
        _ => None,
    }
}

fn record_impression(
    world: &mut World,
    ctx: &DayCtx,
    v: usize,
    c: &Candidate,
    rng: &mut StreamRng,
) -> (FollowChange, ItemActions) {
    let World {
        people,
        posts,
        accounts,
        tally,
        truth,
        ..
    } = world;
    let Some(post) = posts.get_mut(c.post) else {
        return (FollowChange::None, ItemActions::default());
    };
    let mut actions = ItemActions {
        seen: true,
        ..ItemActions::default()
    };
    let design = &ctx.config.design;
    let assume = &ctx.config.world;
    let person = &mut people[v];
    let (traits, state) = (&person.traits, &mut person.state);
    let s = &c.signals;
    let style = post.style.index();
    let author = post.author as usize;

    state.today.items += 1;
    state.today.dwell += s.dwell;
    // An impression that carries the note marks the post noted, if the end
    // of an earlier day has not already (docs/model.md, 6.8).
    if c.relation.noted && !post.noted {
        post.noted = true;
        tally.notes += 1;
    }
    post.impressions += 1;
    post.impressions_today += 1;
    post.dwell += s.dwell;
    accounts.impressions_today[author] += 1;
    tally.impressions += 1;
    tally.dwell += s.dwell;
    tally.impressions_by_style[style] += 1;
    tally.dwell_by_style[style] += s.dwell;
    tally.reflective_sum += s.reflective;
    tally.toxic_impressions += u32::from(post.is_toxic());
    tally.outrage_impressions += u32::from(post.outrage > 0.5);
    tally.misinformation_impressions += u32::from(post.is_misinformation());
    if post.is_misinformation() {
        tally.misinformation_by_style[style] += 1;
        tally.misinformation_bot_impressions += u32::from(post.author_is_bot);
    }
    state.today.toxic_items += u32::from(post.is_toxic());
    state.today.misinformation_items += u32::from(post.is_misinformation());
    state.today.hostile_items += u32::from(post.style.is_hostile());
    state.today.same_group_items += u32::from(s.same_group);
    if let Some(threshold) = flag_threshold(ctx) {
        state.today.flagged_items +=
            u32::from(post.classifier_score.is_some_and(|f| f > threshold));
    }
    if c.in_network {
        tally.in_network_impressions += 1;
        tally.in_network_outgroup_impressions += u32::from(!s.same_group);
    }
    // Truth-side measurement only (docs/model.md, 9.2): reach by the
    // author's partisanship, impressions by a feed experiment's treated
    // group, and the like probability's terms. Nothing reads them back.
    if let Some(band) = partisan_band(&truth.partisanship_rank, author) {
        state.today.partisan_half_items += u32::from(band.half);
        state.today.extreme_items += u32::from(band.extreme);
        state.today.middle_items += u32::from(band.middle);
    }
    if truth
        .experiment
        .as_ref()
        .is_some_and(|e| e.feed_assigned(v))
    {
        post.assigned_impressions += 1;
    }
    let terms = like_terms(traits, post, s, assume);
    tally.like_terms[usize::from(!s.same_group)].add(&terms, s.p_like);

    let liked = rng::chance(rng, s.p_like);
    actions.liked = liked;
    if liked {
        match traits.group {
            Group::A => post.likes_a += 1,
            Group::B => post.likes_b += 1,
        }
        post.likes_today += 1;
        post.engagement_today += 1;
        tally.likes += 1;
        tally.likes_by_style[style] += 1;
        tally.cross_group_likes += u32::from(!s.same_group);
        state.today.actions += 1;
        if c.in_network {
            tally.in_network_likes += 1;
            tally.in_network_outgroup_likes += u32::from(!s.same_group);
        }
        if post.style == Style::Personal && c.relation.mutual {
            state.today.meaningful += 1;
        }
    }
    state.today.e_imp += s.appeal * if liked { 1.0 } else { 0.5 };
    // Subscribers see no ads, so they bring no ad revenue and feel no
    // annoyance (docs/model.md, 7.2 and 8.3).
    let annoyance = if state.subscriber {
        0.0
    } else {
        state.today.ad_items += 1;
        tally.ad_impressions += 1;
        assume.ad_annoyance * design.ads_share
    };
    state.today.ad_annoyance += annoyance;
    state.today.e_ref += s.reflective - annoyance;
    state.today.time_cost += time_cost(s.dwell, assume.time_cost);
    if is_low_value(s.reflective, s.dwell, assume.time_cost) {
        tally.low_value_dwell += s.dwell;
    }

    if rng::chance(rng, s.p_reply) {
        actions.replied = true;
        state.today.actions += 1;
        post.engagement_today += 1;
        tally.replies += 1;
        tally.replies_by_style[style] += 1;
        if rng::chance(rng, s.angry_share) {
            actions.angry = true;
            post.angry_replies += 1;
            post.angry_today += 1;
            state.today.e_ref -= 0.3;
        } else {
            post.positive_replies += 1;
            if post.style == Style::Personal && c.relation.mutual {
                post.mutual_replies_today += 1;
                state.today.meaningful += 1;
            }
        }
    }
    if rng::chance(rng, s.p_share) {
        actions.shared = true;
        state.today.actions += 1;
        state.today.shares += 1;
        state.today.shared_truth += post.truth;
        tally.shared_truth += f64::from(post.truth);
        post.shares += 1;
        post.human_shares_today += 1;
        post.engagement_today += 1;
        tally.shares += 1;
        tally.shares_by_style[style] += 1;
        let depth = c.depth.saturating_add(1);
        accounts.reshares[v].push(Reshare {
            post: post.id,
            depth,
            day: ctx.day,
        });
        if post.is_misinformation() {
            state
                .today
                .misinformation_shares
                .push((post.truth, c.relation.noted));
        }
        if post.note_delay.is_some() && !c.relation.noted {
            post.shares_at_note += 1;
        }
    }
    // The draw comes first, so a repeat report changes no later draw.
    if rng::chance(rng, s.p_report) && post.add_report(v as u32) {
        actions.reported = true;
        tally.reports += 1;
    }
    if rng::chance(rng, s.p_see_less) {
        actions.see_less = true;
        post.see_less += 1;
        accounts.see_less_today[author] += 1;
        tally.see_less += 1;
    }

    if design.like_counts_visible
        && !post.author_is_creator
        && post.likes() as f32 > state.typical_likes
    {
        let visible = ((post.likes() as f32).ln_1p() / 1000f32.ln()).min(1.0);
        state.today.e_ref -= assume.comparison_cost * traits.vulnerability * visible;
    }
    if state.today.items > traits.time_budget {
        state.today.e_ref -= assume.regret_cost;
        tally.regret_items += 1;
    }
    if post.is_misinformation() {
        let noted = if c.relation.noted { 0.3 } else { 1.0 };
        if rng::chance(rng, traits.credulity * assume.credulity_scale * noted) {
            state.informedness =
                (state.informedness - MISBELIEF_LOSS * (0.5 - post.truth)).clamp(0.0, 1.0);
        }
    } else if post.style == Style::Informative && post.truth > 0.7 {
        state.informedness = (state.informedness
            + INFORMATION_GAIN * weighted_quality(post, s.interest))
        .clamp(0.0, 1.0);
    }
    let mut polarisation = assume.polarisation_rate * (s.hostility + 0.5 * s.provocation);
    if !s.same_group && !post.style.is_hostile() {
        polarisation += contact_change(traits.partisanship, liked, post.quality, assume);
    }
    state.affective_polarisation = (state.affective_polarisation + polarisation).clamp(0.0, 1.0);
    if s.reflective > 0.0 {
        state.today.fan_credits.push((post.id, s.reflective));
    }

    if liked
        && !c.relation.follows_author
        && author != v
        && rng::chance(rng, 0.05 * (1.0 + post.quality))
    {
        return (FollowChange::Follow(post.author), actions);
    }
    if c.relation.follows_author
        && post.toxicity * (1.0 - traits.toxicity_tolerance) > 0.5
        && rng::chance(rng, 0.2)
    {
        return (FollowChange::Unfollow(post.author), actions);
    }
    (FollowChange::None, actions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::MetricId;
    use crate::simulation::Simulation;

    /// The world after one simulated day with the given time cost.
    fn first_day(time_cost: f32) -> World {
        let mut config = Config::default();
        config.run.population_size = 300;
        config.world.time_cost = time_cost;
        let mut sim = Simulation::new(config).expect("valid");
        sim.step();
        sim.world().clone()
    }

    #[test]
    fn each_impression_costs_its_dwell_at_the_time_cost() {
        assert_eq!(time_cost(1.5, 0.2), 0.2 * 1.5);
        let (free, charged) = (first_day(0.0), first_day(0.2));
        assert!(free.tally.impressions > 100);
        assert_eq!(free.tally.impressions, charged.tally.impressions);
        assert_eq!(free.tally.dwell, charged.tally.dwell);
        let mut charged_total = 0.0f64;
        for (a, b) in free.people.iter().zip(&charged.people) {
            let (a, b) = (&a.state.today, &b.state.today);
            // The time cost leaves the gross reflective utility alone, which
            // subscriptions read, and lowers the net that wellbeing reads.
            assert_eq!(a.e_ref, b.e_ref);
            assert_eq!(a.time_cost, 0.0);
            assert_eq!(b.net_reflective(), b.e_ref - b.time_cost);
            assert!(b.time_cost > 0.0 || b.items == 0);
            charged_total += f64::from(b.time_cost);
        }
        let expected = 0.2 * f64::from(charged.tally.dwell);
        assert!(
            (charged_total - expected).abs() < 1e-4 * expected,
            "{charged_total} against {expected}"
        );
        // Subscription windows are filled from the gross value.
        for (a, b) in free.people.iter().zip(&charged.people) {
            assert_eq!(a.state.eref_window, b.state.eref_window);
        }
    }

    /// A configuration whose users check the app several times a day.
    fn checking(recheck: f32, pull: f32) -> Config {
        let mut config = Config::default();
        config.run.population_size = 300;
        config.run.tracked_agents = 40;
        config.world.recheck_rate = recheck;
        config.world.notification_pull = pull;
        config
    }

    #[test]
    fn an_extra_session_caps_a_share_of_the_first() {
        assert_eq!(extra_session_cap(20, 0.5), 10);
        assert_eq!(extra_session_cap(25, 0.5), 13, "12.5 rounds up");
        assert_eq!(extra_session_cap(24, 0.25), 6);
        assert_eq!(extra_session_cap(3, 0.0), 1, "at least one item");
        assert_eq!(extra_session_cap(1, 0.2), 1);
    }

    #[test]
    fn extra_sessions_continue_down_the_feed_without_repeats() {
        let config = checking(3.0, 3.0);
        let mut sim = Simulation::new(config.clone()).expect("valid");
        sim.step();
        let mut extra_sessions = 0;
        for id in sim.tracked() {
            let Some(feed) = sim.served_feed(id) else {
                continue;
            };
            let first = feed.sessions[0];
            assert_eq!(first.kind, SessionKind::First);
            assert_eq!((first.cap, first.start), (feed.session_limit, 0));
            let extra = extra_session_cap(feed.session_limit, config.world.extra_session_share);
            let mut next = 0;
            for (k, session) in feed.sessions.iter().enumerate() {
                assert_eq!(
                    session.start, next,
                    "session {k} starts after the items seen"
                );
                assert!(session.seen <= session.cap);
                if k > 0 {
                    assert_eq!(session.cap, extra);
                    assert_ne!(session.kind, SessionKind::First);
                }
                next += session.seen;
                for item in &feed.items {
                    if (session.start..next).contains(&item.rank) {
                        let actions = item.actions.expect("a served feed");
                        assert!(actions.seen);
                        assert_eq!(actions.session, k);
                    }
                }
            }
            // Habitual sessions come before the notification-driven ones.
            let kinds: Vec<SessionKind> = feed.sessions.iter().map(|s| s.kind).collect();
            let mut sorted = kinds.clone();
            sorted.sort_by_key(|k| *k as u8);
            assert_eq!(kinds, sorted);
            // Nothing beyond the last session was reached, and the day's
            // impressions are exactly the items the sessions read.
            let seen = feed
                .items
                .iter()
                .filter(|i| i.actions.is_some_and(|a| a.seen))
                .count();
            assert_eq!(seen, next);
            let person = &sim.world().people[id as usize];
            assert_eq!(person.state.today.items as usize, next);
            // No post appears twice in the day's feed.
            let mut posts: Vec<u32> = feed.items.iter().map(|i| i.post.id).collect();
            posts.sort_unstable();
            posts.dedup();
            assert_eq!(posts.len(), feed.items.len());
            extra_sessions += feed.sessions.len() - 1;
        }
        assert!(extra_sessions > 20, "{extra_sessions} extra sessions");
    }

    #[test]
    fn a_time_limit_caps_the_items_of_the_whole_day() {
        let mut config = checking(3.0, 3.0);
        config.design.time_limit_share = 1.0;
        let mut sim = Simulation::new(config).expect("valid");
        sim.step();
        let world = sim.world();
        let limited: Vec<_> = world
            .people
            .iter()
            .filter(|p| p.state.time_limited && p.state.today.opened)
            .collect();
        assert!(limited.len() > 50);
        assert!(
            limited
                .iter()
                .all(|p| p.state.today.items <= p.traits.time_budget)
        );
        assert!(
            limited
                .iter()
                .any(|p| p.state.today.items == p.traits.time_budget),
            "some reach the limit"
        );
        assert_eq!(world.tally.regret_items, 0, "nobody passes the budget");
    }

    #[test]
    fn notification_sessions_cost_an_interruption_that_subscriptions_ignore() {
        let first_day_of = |config: Config| {
            let mut sim = Simulation::new(config).expect("valid");
            let metrics = sim.step().expect("a day");
            (sim.world().clone(), metrics)
        };
        let mut charged = checking(0.0, 2.0);
        charged.world.interruption_cost = 0.1;
        let ((free, _), (dear, metrics)) =
            (first_day_of(checking(0.0, 2.0)), first_day_of(charged));
        let mut notified = 0;
        for (a, b) in free.people.iter().zip(&dear.people) {
            let (ta, tb) = (&a.state.today, &b.state.today);
            assert_eq!(ta.notified_sessions, tb.notified_sessions);
            assert_eq!(ta.habitual_sessions, 0);
            assert_eq!((ta.e_ref, ta.time_cost), (tb.e_ref, tb.time_cost));
            assert_eq!(ta.interruption_cost, 0.0);
            assert_eq!(tb.interruption_cost, tb.notified_sessions as f32 * 0.1);
            assert_eq!(
                tb.net_reflective(),
                tb.e_ref - tb.time_cost - tb.interruption_cost
            );
            // Subscription windows are filled from the gross value.
            assert_eq!(a.state.eref_window, b.state.eref_window);
            notified += tb.notified_sessions;
        }
        assert!(notified > 50, "{notified}");
        assert_eq!(dear.tally.notification_sessions, notified);
        assert_eq!(dear.tally.sessions, dear.tally.openers + notified);
        assert!(metrics.get(MetricId::SessionsPerActive) > 1.0);
        let share = metrics.get(MetricId::NotificationSessionShare);
        assert!(share > 0.0 && share < 1.0, "{share}");
        // Habitual sessions cost no interruption.
        let mut habitual = checking(3.0, 0.0);
        habitual.world.interruption_cost = 0.1;
        let (world, metrics) = first_day_of(habitual);
        assert!(
            world
                .people
                .iter()
                .any(|p| p.state.today.habitual_sessions > 0)
        );
        assert!(
            world
                .people
                .iter()
                .all(|p| p.state.today.interruption_cost == 0.0)
        );
        assert!(metrics.get(MetricId::SessionsPerActive) > 1.0);
        assert_eq!(metrics.get(MetricId::NotificationSessionShare), 0.0);
    }

    #[test]
    fn people_who_did_not_open_have_no_extra_sessions() {
        let mut sim = Simulation::new(checking(5.0, 5.0)).expect("valid");
        sim.step();
        let world = sim.world();
        assert!(
            world
                .people
                .iter()
                .any(|p| p.state.is_user && !p.state.today.opened)
        );
        for p in world.people.iter().filter(|p| !p.state.today.opened) {
            assert_eq!(p.state.today.habitual_sessions, 0);
            assert_eq!(p.state.today.notified_sessions, 0);
        }
    }

    #[test]
    fn low_value_time_counts_items_worth_less_than_their_time() {
        assert!(is_low_value(0.1, 1.5, 0.1), "0.1 < 0.15");
        assert!(!is_low_value(0.1, 1.5, 0.0));
        assert!(
            is_low_value(-0.01, 1.0, 0.0),
            "at zero cost, negative value"
        );
        assert!(!is_low_value(0.0, 1.0, 0.0));
        let (free, dear) = (first_day(0.0), first_day(0.4));
        assert!(free.tally.low_value_dwell > 0.0);
        assert!(dear.tally.low_value_dwell > free.tally.low_value_dwell);
        assert!(dear.tally.low_value_dwell <= dear.tally.dwell);
    }

    #[test]
    fn backfire_hardens_strong_partisans_only_when_positive() {
        let mut assume = WorldAssumptions::default();
        let contact = assume.contact_rate;
        // At the default 0, seeing a calm out-group post changes nothing and
        // liking it depolarises through contact.
        assert_eq!(contact_change(0.9, false, 0.6, &assume), 0.0);
        assert!((contact_change(0.9, true, 0.6, &assume) + contact * 0.6).abs() < 1e-9);
        assume.polarisation_backfire = 0.5;
        assert!((contact_change(0.9, false, 0.6, &assume) - 0.5 * contact).abs() < 1e-9);
        // Below the partisanship threshold there is no backfire.
        assert_eq!(
            contact_change(BACKFIRE_PARTISANSHIP, false, 0.6, &assume),
            0.0
        );
        assert_eq!(contact_change(0.3, false, 0.6, &assume), 0.0);
        // A strong partisan who likes the post still gains from contact.
        let liked = contact_change(0.9, true, 0.8, &assume);
        assert!((liked - (0.5 * contact - 0.8 * contact)).abs() < 1e-9);
        assert!(liked < 0.0);
    }

    /// The prevalence target of lab #32 (docs/model.md, 4.1 and 13.1): the
    /// misinformation share is the misinformation impressions over all
    /// impressions, and its diagnostic counts split it by style and pick
    /// out the bots' part.
    #[test]
    fn the_misinformation_share_is_false_impressions_over_all() {
        let mut config = Config::default();
        config.run.population_size = 300;
        let mut sim = Simulation::new(config).expect("valid");
        let mut seen = 0;
        for _ in 0..6 {
            let m = sim.step().expect("a day");
            let t = &sim.world().tally;
            let by_style: u32 = t.misinformation_by_style.iter().sum();
            assert_eq!(by_style, t.misinformation_impressions);
            assert!(t.misinformation_bot_impressions <= t.misinformation_impressions);
            let counted: f32 = Style::ALL
                .iter()
                .map(|&s| m.get(MetricId::misinformation_count(s)))
                .sum();
            assert_eq!(counted, t.misinformation_impressions as f32);
            let share = t.misinformation_impressions as f32 / t.impressions as f32;
            assert!((m.get(MetricId::MisinformationShare) - share).abs() < 1e-6);
            assert_eq!(
                m.get(MetricId::MisinformationCountBots),
                t.misinformation_bot_impressions as f32
            );
            seen += t.misinformation_impressions;
        }
        assert!(seen > 0);
    }

    /// Each user who opens the app gets a time of day for the day's
    /// sessions, uniform over 24 hours from the session-clock stream keyed on
    /// the person and the day; without community notes nobody does.
    #[test]
    fn session_times_are_drawn_per_person_and_day_with_notes_only() {
        let run = |notes: bool| {
            let mut config = Config::default();
            config.run.population_size = 300;
            config.design.community_notes = notes;
            let mut sim = Simulation::new(config.clone()).expect("valid");
            let mut hours = Vec::new();
            for day in 0..3 {
                sim.step();
                for (v, p) in sim.world().people.iter().enumerate() {
                    let h = p.state.today.session_hour;
                    if !notes || !p.state.today.opened {
                        assert_eq!(h, 0.0);
                        continue;
                    }
                    let mut clock = crate::rng::stream_rng(
                        config.run.seed,
                        day,
                        v as u64,
                        Stream::SessionClock,
                    );
                    assert_eq!(h, HOURS_PER_DAY * rng::uniform(&mut clock));
                    assert!((0.0..HOURS_PER_DAY).contains(&h));
                    hours.push(h);
                }
            }
            hours
        };
        assert!(run(false).is_empty());
        let hours = run(true);
        assert!(hours.len() > 200, "{}", hours.len());
        let mean = hours.iter().sum::<f32>() / hours.len() as f32;
        assert!((mean - 12.0).abs() < 1.0, "{mean}");
        let morning = hours.iter().filter(|&&h| h < 6.0).count() as f32 / hours.len() as f32;
        assert!((morning - 0.25).abs() < 0.06, "{morning}");
    }

    #[test]
    fn each_persons_day_adds_up_to_the_days_tallies() {
        let mut config = Config::default();
        config.run.population_size = 300;
        let mut sim = Simulation::new(config).expect("valid");
        for _ in 0..3 {
            sim.step();
        }
        let world = sim.world();
        let t = &world.tally;
        let sum = |of: fn(&crate::person::DayAccumulators) -> u32| -> u32 {
            world.people.iter().map(|p| of(&p.state.today)).sum()
        };
        assert_eq!(sum(|d| d.items), t.impressions);
        assert_eq!(sum(|d| d.toxic_items), t.toxic_impressions);
        assert_eq!(
            sum(|d| d.misinformation_items),
            t.misinformation_impressions
        );
        assert_eq!(sum(|d| d.shares), t.shares - t.bot_shares);
        let human = t.likes + t.replies + t.shares - t.bot_likes - t.bot_shares;
        assert_eq!(sum(|d| d.actions), human);
        let same = sum(|d| d.same_group_items);
        assert!(same > 0 && same < t.impressions);
        let truth: f64 = world
            .people
            .iter()
            .map(|p| f64::from(p.state.today.shared_truth))
            .sum();
        assert!(
            (truth - t.shared_truth).abs() < 1e-3,
            "{truth} {}",
            t.shared_truth
        );
        assert!(t.in_network_impressions > 0 && t.in_network_impressions < t.impressions);
        assert!(t.in_network_outgroup_impressions < t.in_network_impressions);
        assert!(t.in_network_likes <= t.likes - t.bot_likes);
        assert!(t.in_network_outgroup_likes <= t.in_network_likes);
        assert!(t.in_network_candidates >= t.in_network_impressions);
        assert!(t.in_network_outgroup_candidates < t.in_network_candidates);
        let posts = sum(|d| d.posts);
        let toxicity: f32 = world
            .people
            .iter()
            .map(|p| p.state.today.post_toxicity)
            .sum();
        assert!(posts > 0 && toxicity > 0.0 && toxicity < posts as f32);
    }
}
