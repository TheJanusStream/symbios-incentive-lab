//! Automated moderation, harassment delivery, community notes, human review
//! and sanctions (docs/model.md, 6.7 to 6.9).

use crate::behaviour;
use crate::bots;
use crate::config::{ModerationAction, Sanctions};
use crate::content::TOXIC_THRESHOLD;
use crate::rng::{self, Stream};
use crate::world::{DayCtx, World};

/// Toxicity above which a removal teaches the author to evade.
const EVASION_TOXICITY: f32 = 0.3;
/// Impressions a misleading post needs before raters notice it.
const NOTE_MIN_IMPRESSIONS: u32 = 50;
/// Re-entry chance factor of Trolls and of everyone else.
const TROLL_REENTRY: f32 = 0.5;
const OTHER_REENTRY: f32 = 0.05;
/// Shortest and longest wait before a banned person may return.
const REENTRY_MIN_DAYS: u32 = 7;
const REENTRY_MAX_DAYS: u32 = 30;

/// Phase 4: the classifier screens the posts written today.
pub(crate) fn classify_new_posts(world: &mut World, ctx: &DayCtx, first_new: u32) {
    let design = &ctx.config.design;
    if !design.classifier_enabled {
        return;
    }
    let mut removed_authors = Vec::new();
    for post in world.posts.iter_from_mut(first_new) {
        let mut rng = ctx.rng(u64::from(post.id), Stream::Moderation);
        let flag = (0.7 * post.toxicity + 0.3 * post.outrage) * post.detectability
            + rng::normal(&mut rng, 0.1);
        if flag <= design.classifier_threshold {
            continue;
        }
        world.tally.false_positives += u32::from(post.toxicity < TOXIC_THRESHOLD);
        match design.moderation_action {
            ModerationAction::Remove => {
                post.removed = true;
                world.tally.auto_removals += 1;
                if post.toxicity > EVASION_TOXICITY {
                    removed_authors.push(post.author);
                }
            }
            ModerationAction::Demote => post.demoted = true,
        }
    }
    for author in removed_authors {
        learn_evasion(world, ctx, author);
    }
}

/// A removal of a toxic post teaches its (human) author to evade classifiers.
fn learn_evasion(world: &mut World, ctx: &DayCtx, author: u32) {
    if let Some(person) = world.people.get_mut(author as usize) {
        let e = &mut person.state.evasion;
        *e += ctx.config.world.evasion_rate * (1.0 - *e);
    }
}

/// Phase 5: each surviving Attack written today reaches its target unless a
/// block stops it; the target may report it (7.1, with the target's bonus)
/// and block the author.
pub(crate) fn deliver_harassment(world: &mut World, ctx: &DayCtx, first_new: u32) {
    let attacks: Vec<(u32, u32, u32, f32)> = world
        .posts
        .iter()
        .filter(|p| p.id >= first_new && !p.removed)
        .filter_map(|p| p.target.map(|t| (p.id, p.author, t, p.toxicity)))
        .collect();
    let assume = &ctx.config.world;
    for (id, author, target, toxicity) in attacks {
        let mut rng = ctx.rng(u64::from(id), Stream::Harassment);
        let person = &mut world.people[target as usize];
        if !person.state.is_user {
            continue;
        }
        if person.has_blocked(author)
            && rng::chance(&mut rng, ctx.config.design.block_effectiveness)
        {
            continue;
        }
        let harm = assume.harassment_harm * toxicity * (0.5 + person.traits.vulnerability);
        person.state.today.e_ref -= harm;
        person.state.today.harassment += 1;
        world.tally.harassment += 1;
        let report = behaviour::report_probability(&person.traits, toxicity, true);
        if rng::chance(&mut rng, report)
            && let Some(post) = world.posts.get_mut(id)
        {
            post.reports += 1;
            world.tally.reports += 1;
        }
        if rng::chance(&mut rng, 0.3 + 0.5 * person.traits.vulnerability) {
            person.block(author);
        }
    }
}

/// Phase 8: misleading posts with enough impressions may get a note; partisan
/// posts are harder to note because raters from both groups must agree.
pub(crate) fn community_notes(world: &mut World, ctx: &DayCtx) {
    if !ctx.config.design.community_notes {
        return;
    }
    for post in world.posts.iter_mut() {
        if post.noted
            || post.removed
            || !post.is_misinformation()
            || post.impressions < NOTE_MIN_IMPRESSIONS
        {
            continue;
        }
        let mut rng = ctx.rng(u64::from(post.id), Stream::Notes);
        if rng::chance(&mut rng, 0.3 * (1.0 - 0.6 * post.outrage)) {
            post.noted = true;
            world.tally.notes += 1;
        }
    }
}

/// Phase 9: reviewers take reported posts in order of report count and judge
/// truthfully; a toxic post is removed and its author struck.
pub(crate) fn human_review(world: &mut World, ctx: &DayCtx) {
    let capacity = ctx.config.design.review_capacity * world.user_count() as f32 / 1000.0;
    // Entity u64::MAX keeps this draw apart from the per-person ban draws.
    let mut rng = ctx.rng(u64::MAX, Stream::Review);
    let reviews = capacity.floor() as usize + usize::from(rng::chance(&mut rng, capacity.fract()));
    let mut queue: Vec<(u32, u32)> = world
        .posts
        .iter()
        .filter(|p| p.reports > 0 && !p.removed && !p.reviewed)
        .map(|p| (p.reports, p.id))
        .collect();
    queue.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    for &(_, id) in queue.iter().take(reviews) {
        let Some(post) = world.posts.get_mut(id) else {
            continue;
        };
        post.reviewed = true;
        if post.toxicity < TOXIC_THRESHOLD {
            continue;
        }
        post.removed = true;
        post.struck = true;
        let author = post.author;
        world.tally.human_removals += 1;
        learn_evasion(world, ctx, author);
        strike(world, ctx, author);
    }
}

/// A strike from human review against any account.
fn strike(world: &mut World, ctx: &DayCtx, account: u32) {
    if let Some(slot) = world.bot_slot(account) {
        if bots::strike(&mut world.bots[slot], ctx) {
            world.tally.departures += 1;
        }
        return;
    }
    let state = &mut world.people[account as usize].state;
    state.strikes += 1;
    let ban = match ctx.config.design.sanctions {
        Sanctions::None => false,
        Sanctions::Strict => true,
        Sanctions::Graduated => {
            match state.strikes {
                2 => state.suspended_until = ctx.day + 1 + 3,
                3 => state.suspended_until = ctx.day + 1 + 14,
                _ => {}
            }
            state.strikes >= 4
        }
    };
    if ban && state.is_user {
        ban_person(world, ctx, account as usize);
    }
}

/// A banned person leaves and may return with a new account after 7 to 30 days.
fn ban_person(world: &mut World, ctx: &DayCtx, i: usize) {
    let mut rng = ctx.rng(i as u64, Stream::Review);
    let wait = REENTRY_MIN_DAYS
        + rng::index(&mut rng, (REENTRY_MAX_DAYS - REENTRY_MIN_DAYS + 1) as usize) as u32;
    let state = &mut world.people[i].state;
    state.is_user = false;
    state.banned = true;
    state.subscriber = false;
    state.left_day = Some(ctx.day);
    state.reentry_day = ctx.day + wait;
    world.tally.departures += 1;
}

/// The re-entry chance factor of a banned person.
pub(crate) fn reentry_factor(is_troll: bool) -> f32 {
    if is_troll {
        TROLL_REENTRY
    } else {
        OTHER_REENTRY
    }
}
