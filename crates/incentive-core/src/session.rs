//! Opening the app and consuming a feed (docs/model.md, 6.1, 6.6, 7.1, 7.2
//! and 7.6).

use crate::behaviour::{sig, weighted_quality};
use crate::config::{Config, WorldAssumptions};
use crate::content::{Group, Style};
use crate::inspector::{self, FeedExplanation, Inspector, ItemActions};
use crate::network::{self, Reshare};
use crate::ranking::{self, Candidate, FeedCtx, FeedKind, FeedScratch, Trending};
use crate::rng::{self, Stream, StreamRng};
use crate::world::{DayCtx, World};

/// Informedness lost per believed misinformation item, per unit of falsehood,
/// and gained per good Informative item, per unit of interest-weighted
/// quality. The text's 0.05 and 0.01 drive informedness to its bounds within
/// weeks; docs/model.md, section 13, records the calibration.
pub(crate) const MISBELIEF_LOSS: f32 = 0.008;
const INFORMATION_GAIN: f32 = 0.0004;
/// Highest chance of opening the app on a day.
const MAX_OPEN_CHANCE: f32 = 0.98;

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

/// Decides who opens the app today. Posting and sessions both use it.
/// People on a deactivation experiment's break (docs/model.md, 13.2) do not
/// open it and draw nothing.
pub(crate) fn decide_openers(world: &mut World, ctx: &DayCtx) {
    let ni = ctx.config.design.notification_intensity;
    for (i, person) in world.people.iter_mut().enumerate() {
        if !person.state.is_user || world.truth.on_break(i) {
            continue;
        }
        let mut rng = ctx.rng(i as u64, Stream::Activity);
        let p = (person.traits.activity * (0.6 + 0.8 * person.state.habit) * (1.0 + 0.3 * ni))
            .clamp(0.0, MAX_OPEN_CHANCE);
        if rng::chance(&mut rng, p) {
            person.state.today.opened = true;
            world.tally.openers += 1;
        }
    }
}

/// The session's item cap `L` (docs/model.md, 6.1).
pub(crate) fn session_limit(world: &World, config: &Config, v: usize) -> usize {
    let person = &world.people[v];
    let habit = person.state.habit;
    let ni = config.design.notification_intensity;
    let raw = config.world.base_session_items
        * (0.7 + 0.6 * habit)
        * (1.0 + 0.5 * ni * person.traits.impulsivity);
    let mut limit = raw.round().max(1.0) as usize;
    if person.state.time_limited {
        limit = limit.min(person.traits.time_budget as usize);
    }
    limit
}

/// Runs every session of the day in ascending individual index. The feeds of
/// tracked individuals go to the inspector as they were served, with what the
/// viewer did with each item.
pub(crate) fn run_sessions(
    world: &mut World,
    ctx: &DayCtx,
    scratch: &mut FeedScratch,
    inspector: &mut Inspector,
) {
    let trending = Trending::build(world);
    scratch.reset(world.posts.len());
    for v in 0..world.population() {
        if !world.people[v].state.today.opened {
            continue;
        }
        let limit = session_limit(world, ctx.config, v);
        let mut feed_rng = ctx.rng(v as u64, Stream::Feed);
        let feed_ctx = FeedCtx {
            world,
            config: ctx.config,
            day: ctx.day,
            viewer: v,
        };
        let kind = ranking::build_feed(&feed_ctx, limit, &trending, &mut feed_rng, scratch);
        let mut served = inspector
            .is_tracked(v as u32)
            .then(|| served_feed(&feed_ctx, ctx, kind, limit, scratch));
        let mut rng = ctx.rng(v as u64, Stream::Engagement);
        for (rank, c) in scratch.candidates.iter().enumerate() {
            let appeal = c.signals.appeal;
            let actions = impression(world, ctx, v, c, &mut rng);
            if let Some(item) = served
                .as_mut()
                .and_then(|feed| feed.items.iter_mut().find(|item| item.rank == rank))
            {
                item.actions = Some(actions);
            }
            let habit = world.people[v].state.habit;
            if rng::chance(
                &mut rng,
                sig(ctx.config.world.stop_bias - 1.5 * appeal - habit),
            ) {
                break;
            }
        }
        if let Some(feed) = served {
            inspector.record_feed(feed);
        }
    }
}

/// The record of the feed just built for a tracked viewer, before any item is
/// seen. A chronological feed also gets the main ranking's scores, drawn on
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
        if post.style == Style::Personal && c.relation.mutual {
            state.today.meaningful += 1;
        }
    }
    state.today.e_imp += s.appeal * if liked { 1.0 } else { 0.5 };
    state.today.e_ref += s.reflective - assume.ad_annoyance * design.ads_share;
    state.today.time_cost += time_cost(s.dwell, assume.time_cost);
    if is_low_value(s.reflective, s.dwell, assume.time_cost) {
        tally.low_value_dwell += s.dwell;
    }

    if rng::chance(rng, s.p_reply) {
        actions.replied = true;
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
                .push((post.truth, post.noted));
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
        let noted = if post.noted { 0.3 } else { 1.0 };
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
}
