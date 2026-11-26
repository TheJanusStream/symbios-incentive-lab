//! Automated moderation, harassment delivery, community notes, human review
//! and sanctions (docs/model.md, 6.7 to 6.9).

use crate::behaviour;
use crate::bots;
use crate::config::{ModerationAction, Sanctions};
use crate::content::{EVASION_TOXICITY, TOXIC_THRESHOLD};
use crate::rng::{self, Stream};
use crate::world::{DayCtx, World};

/// Confirmed misses per 1,000 users and day at which the classifier retrains
/// at half of `classifier_retraining`.
const RETRAINING_HALF_MISSES: f32 = 1.0;
/// Impressions added to a post's own in the review queue's priority,
/// `W / (impressions + K_QUEUE)`, so that one report on a post few people
/// saw does not jump the queue (docs/model.md, 6.7).
pub(crate) const K_QUEUE: f32 = 20.0;
/// Sanction severities for deterrence (docs/model.md, 7.5).
const SEVERITY_WARNING: u8 = 1;
const SEVERITY_SUSPENSION: u8 = 2;
const SEVERITY_BAN: u8 = 4;
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
    let mut evading_authors = Vec::new();
    let mut wronged_authors = Vec::new();
    for post in world.posts.iter_from_mut(first_new) {
        // Keyed on the author and the author's post ordinal, not the global
        // post id, so other people's posting never reshuffles this draw.
        let mut rng = ctx.rng(post.draw_key(), Stream::Moderation);
        let flag = (0.7 * post.toxicity + 0.3 * post.outrage) * post.detectability
            + rng::normal(&mut rng, 0.1);
        if flag <= design.classifier_threshold {
            continue;
        }
        if post.toxicity < TOXIC_THRESHOLD {
            world.tally.false_positives += 1;
            wronged_authors.push(post.author);
        }
        match design.moderation_action {
            ModerationAction::Remove => {
                post.removed = true;
                world.tally.auto_removals += 1;
            }
            ModerationAction::Demote => post.demoted = true,
        }
        if post.toxicity > EVASION_TOXICITY {
            evading_authors.push(post.author);
        }
    }
    for author in evading_authors {
        learn_evasion(world, ctx, author);
    }
    for author in wronged_authors {
        wrongful_sanction(world, ctx, author);
    }
}

/// An automated removal or demotion of a toxic post teaches its (human)
/// author to evade classifiers. Human reviewers judge true toxicity, so their
/// removals teach nothing.
fn learn_evasion(world: &mut World, ctx: &DayCtx, author: u32) {
    if let Some(person) = world.people.get_mut(author as usize) {
        let e = &mut person.state.evasion;
        *e += ctx.config.world.evasion_rate * (1.0 - *e);
    }
}

/// A (human) author whose harmless post the classifier removed or demoted
/// feels it like harassment, without a harassment event.
fn wrongful_sanction(world: &mut World, ctx: &DayCtx, author: u32) {
    if let Some(person) = world.people.get_mut(author as usize) {
        let hurt = ctx.config.world.wrongful_sanction_cost * (0.5 + person.traits.vulnerability);
        person.state.today.e_ref -= hurt;
    }
}

/// Phase 11: everyone's evasion fades by `evasion_decay` a day.
pub(crate) fn fade_evasion(world: &mut World, ctx: &DayCtx) {
    let keep = 1.0 - ctx.config.world.evasion_decay;
    for person in &mut world.people {
        person.state.evasion *= keep;
    }
}

/// End of phase 9: the classifier retrains on the toxic posts reviewers
/// confirmed it missed. Its counter-evasion `c` moves toward the mean evasion
/// `m` the missed posts were written with by `classifier_retraining * x /
/// (x + 1)` of the gap, where `x` is the misses per 1,000 users: no misses,
/// no retraining.
fn retrain_classifier(world: &mut World, ctx: &DayCtx) {
    let misses = world.tally.confirmed_misses;
    let users = world.user_count();
    if misses == 0 || users == 0 {
        return;
    }
    let x = 1000.0 * misses as f32 / users as f32;
    let rate = ctx.config.world.classifier_retraining * x / (x + RETRAINING_HALF_MISSES);
    let target = world.tally.miss_evasion / misses as f32;
    let c = &mut world.platform.counter_evasion;
    *c = (*c + rate * (target - *c)).clamp(0.0, 1.0);
}

/// Phase 5: each surviving Attack written today reaches its target unless a
/// block stops it; the target may report it (7.1, with the target's bonus)
/// and block the author.
pub(crate) fn deliver_harassment(world: &mut World, ctx: &DayCtx, first_new: u32) {
    let attacks: Vec<(u32, u64, u32, u32, f32)> = world
        .posts
        .iter()
        .filter(|p| p.id >= first_new && !p.removed)
        .filter_map(|p| {
            p.target
                .map(|t| (p.id, p.draw_key(), p.author, t, p.toxicity))
        })
        .collect();
    let assume = &ctx.config.world;
    for (id, key, author, target, toxicity) in attacks {
        let mut rng = ctx.rng(key, Stream::Harassment);
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
            && post.add_report(target)
        {
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
        let mut rng = ctx.rng(post.draw_key(), Stream::Notes);
        if rng::chance(&mut rng, 0.3 * (1.0 - 0.6 * post.outrage)) {
            post.noted = true;
            world.tally.notes += 1;
        }
    }
}

/// The review queue's priority of each reported post not yet reviewed or
/// removed (docs/model.md, 6.7): the summed weight of its reports per
/// impression, `W / (impressions + K_QUEUE)`. A report weighs
/// `(1 - report_trust) + report_trust * upheld_rate` of its reporter. Highest
/// priority first, ties to the older post.
pub(crate) fn review_queue(world: &World, report_trust: f32) -> Vec<(f32, u32)> {
    let accounts = &world.accounts;
    let mut queue: Vec<(f32, u32)> = world
        .posts
        .iter()
        .filter(|p| p.reports > 0 && !p.removed && !p.reviewed)
        .map(|p| {
            let weight: f32 = p
                .reporters
                .iter()
                .map(|&a| (1.0 - report_trust) + report_trust * accounts.upheld_rate(a))
                .sum();
            (weight / (p.impressions as f32 + K_QUEUE), p.id)
        })
        .collect();
    queue.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    queue
}

/// Phase 9: reviewers take reported posts in the order of
/// [`review_queue`] and judge truthfully; a toxic post is removed and its
/// author struck. Every reporter of a reviewed post adds a judged report to
/// its record, and an upheld one when the post was toxic.
pub(crate) fn human_review(world: &mut World, ctx: &DayCtx) {
    let classifier_on = ctx.config.design.classifier_enabled;
    let capacity = ctx.config.design.review_capacity * world.user_count() as f32 / 1000.0;
    // Entity u64::MAX keeps this draw apart from the per-person ban draws.
    let mut rng = ctx.rng(u64::MAX, Stream::Review);
    let reviews = capacity.floor() as usize + usize::from(rng::chance(&mut rng, capacity.fract()));
    let queue = review_queue(world, ctx.config.design.report_trust);
    for &(_, id) in queue.iter().take(reviews) {
        let Some(post) = world.posts.get_mut(id) else {
            continue;
        };
        post.reviewed = true;
        let toxic = post.toxicity >= TOXIC_THRESHOLD;
        for &reporter in &post.reporters {
            let a = reporter as usize;
            world.accounts.reports_reviewed[a] += 1;
            world.accounts.reports_upheld[a] += u32::from(toxic);
        }
        if !toxic {
            continue;
        }
        post.removed = true;
        post.struck = true;
        let author = post.author;
        let missed = classifier_on && !post.demoted;
        world.tally.human_removals += 1;
        if missed {
            // The evasion the missed post was written with: the platform
            // holds the post, not its author's present skill.
            world.tally.confirmed_misses += 1;
            world.tally.miss_evasion += post.evasion;
        }
        let severity = strike(world, ctx, author);
        if let Some(post) = world.posts.get_mut(id) {
            post.sanction_severity = severity;
        }
    }
    if classifier_on {
        retrain_classifier(world, ctx);
    }
}

/// A strike from human review against any account; returns the severity of
/// the sanction it drew for a person (0 for a bot or without sanctions).
fn strike(world: &mut World, ctx: &DayCtx, account: u32) -> u8 {
    if let Some(slot) = world.bot_slot(account) {
        if bots::strike(&mut world.bots[slot], ctx) {
            world.tally.departures += 1;
        }
        return 0;
    }
    let state = &mut world.people[account as usize].state;
    state.strikes += 1;
    let (ban, severity) = match ctx.config.design.sanctions {
        Sanctions::None => (false, 0),
        Sanctions::Strict => (true, SEVERITY_BAN),
        Sanctions::Graduated => {
            let severity = match state.strikes {
                1 => SEVERITY_WARNING,
                2 => {
                    state.suspended_until = ctx.day + 1 + 3;
                    SEVERITY_SUSPENSION
                }
                3 => {
                    state.suspended_until = ctx.day + 1 + 14;
                    SEVERITY_SUSPENSION
                }
                _ => SEVERITY_BAN,
            };
            (state.strikes >= 4, severity)
        }
    };
    if ban && state.is_user {
        ban_person(world, ctx, account as usize);
    }
    severity
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::content::{AuthorProfile, Group, Style, generate_post};
    use crate::init;

    fn small_config() -> Config {
        let mut config = Config::default();
        config.run.population_size = 200;
        config.design.review_capacity = 10.0;
        config.design.classifier_threshold = 0.2;
        config
    }

    fn first_user(world: &World) -> u32 {
        world
            .people
            .iter()
            .position(|p| p.state.is_user)
            .map_or(0, |i| i as u32)
    }

    /// Adds a post by `author` with the given toxicity and outrage; returns its id.
    fn add_post(world: &mut World, author: u32, toxicity: f32, outrage: f32) -> u32 {
        let mut rng = rng::stream_rng(3, 0, u64::from(author), Stream::Posting);
        let id = world.posts.next_id();
        let profile = AuthorProfile::plain(Group::A, 0.5, 0.5);
        let assume = crate::config::WorldAssumptions::default();
        let mut post = generate_post(
            &mut rng,
            id,
            0,
            &profile,
            Style::Attack,
            &[1.0 / 6.0; 6],
            &assume,
        );
        post.author = author;
        post.toxicity = toxicity;
        post.outrage = outrage;
        post.detectability = 0.9;
        post.target = None;
        world.posts.push(post);
        id
    }

    /// Three distinct people report post `id`.
    fn report(world: &mut World, id: u32) {
        if let Some(post) = world.posts.get_mut(id) {
            for reporter in 1..=3 {
                post.add_report(reporter);
            }
        }
    }

    #[test]
    fn evasion_fades_every_day() {
        let mut config = small_config();
        config.world.evasion_decay = 0.1;
        let mut world = init::build_world(&config);
        world.people[0].state.evasion = 0.5;
        let ctx = DayCtx {
            config: &config,
            day: 0,
        };
        fade_evasion(&mut world, &ctx);
        assert!((world.people[0].state.evasion - 0.45).abs() < 1e-6);
    }

    #[test]
    fn a_human_removal_teaches_no_evasion_but_the_classifier_does() {
        let config = small_config();
        let ctx = DayCtx {
            config: &config,
            day: 0,
        };
        let mut world = init::build_world(&config);
        let author = first_user(&world);
        let id = add_post(&mut world, author, 0.9, 0.8);
        report(&mut world, id);
        human_review(&mut world, &ctx);
        assert!(world.posts.get(id).is_some_and(|p| p.struck));
        assert_eq!(world.people[author as usize].state.evasion, 0.0);

        let mut world = init::build_world(&config);
        let id = add_post(&mut world, author, 0.9, 0.8);
        classify_new_posts(&mut world, &ctx, id);
        assert!(world.posts.get(id).is_some_and(|p| p.removed));
        let evasion = world.people[author as usize].state.evasion;
        assert!(
            (evasion - config.world.evasion_rate).abs() < 1e-6,
            "{evasion}"
        );
    }

    #[test]
    fn retraining_moves_only_with_confirmed_misses() {
        let config = small_config();
        let ctx = DayCtx {
            config: &config,
            day: 0,
        };
        let mut world = init::build_world(&config);
        let author = first_user(&world);
        world.people[author as usize].state.evasion = 0.6;
        // No reports, no reviews: nothing to learn from.
        human_review(&mut world, &ctx);
        assert_eq!(world.platform.counter_evasion, 0.0);
        // A reviewed toxic post the classifier had flagged is no miss.
        let flagged = add_post(&mut world, author, 0.9, 0.8);
        report(&mut world, flagged);
        if let Some(p) = world.posts.get_mut(flagged) {
            p.demoted = true;
        }
        human_review(&mut world, &ctx);
        assert!(world.posts.get(flagged).is_some_and(|p| p.struck));
        assert_eq!(world.platform.counter_evasion, 0.0);
        // A confirmed miss pulls the classifier toward its author's evasion.
        world.tally.confirmed_misses = 0;
        world.tally.miss_evasion = 0.0;
        let missed = add_post(&mut world, author, 0.9, 0.8);
        set_evasion(&mut world, missed, 0.6);
        report(&mut world, missed);
        human_review(&mut world, &ctx);
        let c = world.platform.counter_evasion;
        assert!(c > 0.0 && c < 0.6, "{c}");
        // Retraining 0 switches it off.
        let mut off = small_config();
        off.world.classifier_retraining = 0.0;
        let ctx = DayCtx {
            config: &off,
            day: 0,
        };
        let mut world = init::build_world(&off);
        let missed = add_post(&mut world, author, 0.9, 0.8);
        set_evasion(&mut world, missed, 0.6);
        report(&mut world, missed);
        human_review(&mut world, &ctx);
        assert_eq!(world.platform.counter_evasion, 0.0);
    }

    fn set_evasion(world: &mut World, id: u32, evasion: f32) {
        if let Some(p) = world.posts.get_mut(id) {
            p.evasion = evasion;
        }
    }

    #[test]
    fn retraining_learns_the_evasion_a_missed_post_was_written_with() {
        let config = small_config();
        let ctx = DayCtx {
            config: &config,
            day: 0,
        };
        // A veiled post records its author's evasion when it is written.
        let mut world = init::build_world(&config);
        let author = first_user(&world);
        let mut rng = rng::stream_rng(3, 0, 0, Stream::Posting);
        let profile = AuthorProfile {
            evasion: 0.3,
            written_evasion: 0.7,
            ..AuthorProfile::plain(Group::A, 0.5, 1.0)
        };
        let assume = crate::config::WorldAssumptions::default();
        let id = world.posts.next_id();
        let post = generate_post(
            &mut rng,
            id,
            0,
            &profile,
            Style::Attack,
            &[1.0 / 6.0; 6],
            &assume,
        );
        assert!(post.toxicity > EVASION_TOXICITY);
        assert_eq!(post.evasion, 0.7);
        // The author's evasion then fades to nothing before review; the
        // retraining still learns the 0.7 the post carries.
        let missed = add_post(&mut world, author, 0.9, 0.8);
        set_evasion(&mut world, missed, 0.7);
        world.people[author as usize].state.evasion = 0.0;
        report(&mut world, missed);
        human_review(&mut world, &ctx);
        assert_eq!(world.tally.confirmed_misses, 1);
        assert!((world.tally.miss_evasion - 0.7).abs() < 1e-6);
        let c = world.platform.counter_evasion;
        assert!(c > 0.0 && c < 0.7, "{c}");
    }

    #[test]
    fn an_account_reports_a_post_once() {
        let config = small_config();
        let mut world = init::build_world(&config);
        let author = first_user(&world);
        let id = add_post(&mut world, author, 0.2, 0.2);
        let post = world.posts.get_mut(id).expect("the post");
        assert!(post.add_report(7));
        assert!(!post.add_report(7));
        assert!(post.add_report(8));
        assert_eq!(post.reports, 2);
        assert_eq!(post.reporters, vec![7, 8]);
        let queue = review_queue(&world, 0.0);
        let expected = 2.0 / (world.posts.get(id).map_or(0, |p| p.impressions) as f32 + K_QUEUE);
        assert_eq!(queue, vec![(expected, id)]);
    }

    #[test]
    fn a_toxic_low_reach_post_outranks_a_popular_harmless_one() {
        let mut config = small_config();
        let mut world = init::build_world(&config);
        let author = first_user(&world);
        // Seen 2,000 times and reported by 14 people, about 0.007 a view.
        let popular = add_post(&mut world, author, 0.1, 0.3);
        if let Some(p) = world.posts.get_mut(popular) {
            p.impressions = 2000;
            for reporter in 0..14 {
                p.add_report(10 + reporter);
            }
        }
        // An Attack seen once, by its target, who reported it.
        let attack = add_post(&mut world, author, 0.9, 0.6);
        if let Some(p) = world.posts.get_mut(attack) {
            p.impressions = 1;
            p.add_report(40);
        }
        let queue = review_queue(&world, 0.0);
        assert_eq!(
            queue.iter().map(|q| q.1).collect::<Vec<_>>(),
            [attack, popular]
        );
        // With room for one review a day, the reviewer removes the Attack.
        let users = world.user_count() as f32;
        config.design.review_capacity = 1000.0 / users;
        let ctx = DayCtx {
            config: &config,
            day: 0,
        };
        human_review(&mut world, &ctx);
        assert!(world.posts.get(attack).is_some_and(|p| p.struck));
        assert!(world.posts.get(popular).is_some_and(|p| !p.reviewed));
        assert_eq!(world.tally.human_removals, 1);
    }

    #[test]
    fn report_trust_weighs_reports_by_their_upheld_rate() {
        let mut config = small_config();
        config.design.review_capacity = 0.0;
        let mut world = init::build_world(&config);
        let author = first_user(&world);
        // Account 5's ten judged reports were all rejected, account 6's all
        // upheld.
        world.accounts.reports_reviewed[5] = 10;
        world.accounts.reports_reviewed[6] = 10;
        world.accounts.reports_upheld[6] = 10;
        assert!((world.accounts.upheld_rate(5) - 1.0 / 12.0).abs() < 1e-6);
        assert!((world.accounts.upheld_rate(6) - 11.0 / 12.0).abs() < 1e-6);
        assert!((world.accounts.upheld_rate(7) - 0.5).abs() < 1e-6);
        let distrusted = add_post(&mut world, author, 0.2, 0.2);
        let trusted = add_post(&mut world, author, 0.2, 0.2);
        for (id, reporter) in [(distrusted, 5), (trusted, 6)] {
            if let Some(p) = world.posts.get_mut(id) {
                p.add_report(reporter);
            }
        }
        // Without trust every report weighs 1 and the older post goes first.
        let plain = review_queue(&world, 0.0);
        assert_eq!(plain[0].1, distrusted);
        assert_eq!(plain[0].0, plain[1].0);
        // With full trust the rejected reporter's report weighs 1/12.
        let trusting = review_queue(&world, 1.0);
        assert_eq!(trusting[0].1, trusted);
        assert!((trusting[1].0 * K_QUEUE - 1.0 / 12.0).abs() < 1e-6);

        // Reviews update the record of every reporter of a judged post.
        config.design.review_capacity = 1000.0;
        let ctx = DayCtx {
            config: &config,
            day: 0,
        };
        let toxic = add_post(&mut world, author, 0.9, 0.6);
        if let Some(p) = world.posts.get_mut(toxic) {
            p.add_report(7);
            p.add_report(5);
        }
        human_review(&mut world, &ctx);
        assert_eq!(world.accounts.reports_reviewed[5], 12);
        assert_eq!(world.accounts.reports_upheld[5], 1);
        assert_eq!(world.accounts.reports_reviewed[7], 1);
        assert_eq!(world.accounts.reports_upheld[7], 1);
        assert_eq!(world.accounts.reports_upheld[6], 10);
    }

    #[test]
    fn a_wrongful_sanction_hurts_its_author() {
        let config = small_config();
        let ctx = DayCtx {
            config: &config,
            day: 0,
        };
        let mut world = init::build_world(&config);
        let author = first_user(&world);
        let id = add_post(&mut world, author, 0.45, 1.0);
        classify_new_posts(&mut world, &ctx, id);
        assert_eq!(world.tally.false_positives, 1);
        let person = &world.people[author as usize];
        let expected = -config.world.wrongful_sanction_cost * (0.5 + person.traits.vulnerability);
        assert!((person.state.today.e_ref - expected).abs() < 1e-6);
        assert_eq!(person.state.evasion, config.world.evasion_rate);
    }

    #[test]
    fn sanctions_are_graded_by_severity() {
        let mut config = small_config();
        let mut world = init::build_world(&config);
        let author = first_user(&world);
        let ctx = DayCtx {
            config: &config,
            day: 0,
        };
        let graded: Vec<u8> = (0..4).map(|_| strike(&mut world, &ctx, author)).collect();
        assert_eq!(graded, vec![1, 2, 2, 4]);
        assert!(world.people[author as usize].state.banned);
        config.design.sanctions = Sanctions::Strict;
        let mut world = init::build_world(&config);
        let ctx = DayCtx {
            config: &config,
            day: 0,
        };
        assert_eq!(strike(&mut world, &ctx, author), SEVERITY_BAN);
    }

    #[test]
    fn classifier_draws_follow_the_author_not_the_post_id() {
        let mut config = small_config();
        config.design.classifier_threshold = 0.6;
        let ctx = DayCtx {
            config: &config,
            day: 0,
        };
        // The same author's posts, with and without other people's posts
        // written in between, which renumbers every global id.
        let decisions = |interleave: bool| -> Vec<bool> {
            let mut world = init::build_world(&config);
            let author = first_user(&world);
            let first = world.posts.next_id();
            let mut ids = Vec::new();
            for ordinal in 0..200 {
                if interleave {
                    add_post(&mut world, author + 1, 0.1, 0.1);
                }
                let id = add_post(&mut world, author, 0.75, 0.6);
                if let Some(p) = world.posts.get_mut(id) {
                    p.ordinal = ordinal;
                }
                ids.push(id);
            }
            classify_new_posts(&mut world, &ctx, first);
            ids.iter()
                .map(|&id| world.posts.get(id).is_some_and(|p| p.removed))
                .collect()
        };
        let plain = decisions(false);
        let removed = plain.iter().filter(|&&r| r).count();
        assert!(removed > 20 && removed < 180, "{removed}");
        assert_eq!(plain, decisions(true));
    }
}
