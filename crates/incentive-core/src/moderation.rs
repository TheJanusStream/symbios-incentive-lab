//! Automated moderation, harassment delivery, community notes, human review
//! and sanctions (docs/model.md, 6.7 to 6.9).

use crate::behaviour;
use crate::bots;
use crate::config::{FeedExperimentKind, ModerationAction, Sanctions};
use crate::content::{EVASION_TOXICITY, Post, TOXIC_THRESHOLD};
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
/// Chance, before the partisan factor, that a false post gets a community
/// note (docs/model.md, 6.8): set so that the share of false posts noted
/// matches the daily rule it replaced (lab #34, 13.2).
pub(crate) const NOTE_CHANCE: f32 = 0.7;
/// Median hours from a post's creation to the display of its note, and the
/// log-scale spread of the lognormal delay (docs/model.md, 6.8 and 13.2).
pub(crate) const NOTE_DELAY_MEDIAN_HOURS: f32 = 16.0;
pub(crate) const NOTE_DELAY_SIGMA: f32 = 0.92;
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
        if hides_flagged(ctx) {
            for post in world.posts.iter_from_mut(first_new) {
                post.classifier_score = Some(classifier_score(post, ctx));
            }
        }
        return;
    }
    let mut evading_authors = Vec::new();
    let mut wronged_authors = Vec::new();
    for post in world.posts.iter_from_mut(first_new) {
        let flag = classifier_score(post, ctx);
        post.classifier_score = Some(flag);
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

/// The classifier's score of a new post (docs/model.md, 6.7), `flag =
/// (0.7 * toxicity + 0.3 * outrage) * detectability + N(0, 0.1)`. The draw
/// is keyed on the author and the author's post ordinal, not the global post
/// id, so other people's posting never reshuffles it.
fn classifier_score(post: &Post, ctx: &DayCtx) -> f32 {
    let mut rng = ctx.rng(post.draw_key(), Stream::Moderation);
    (0.7 * post.toxicity + 0.3 * post.outrage) * post.detectability + rng::normal(&mut rng, 0.1)
}

/// Whether a feed experiment that hides flagged posts is configured: it needs
/// every post's score even with the classifier switched off. The score's
/// draw is the classifier's own, which nothing else uses then, so no other
/// draw moves (docs/model.md, section 12).
fn hides_flagged(ctx: &DayCtx) -> bool {
    ctx.config
        .run
        .feed_experiment
        .as_ref()
        .is_some_and(|f| matches!(f.kind, FeedExperimentKind::HideFlagged { .. }))
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
/// and block the author. Targets on a deactivation experiment's break
/// (docs/model.md, 13.2) are off the platform and receive nothing.
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
        if world.truth.on_break(target as usize) {
            continue;
        }
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

/// The chance that a false post gets a community note: [`NOTE_CHANCE`]
/// times the partisan factor `1 - 0.6 * outrage`, since a note needs raters
/// from both groups to agree (docs/model.md, 6.8).
pub(crate) fn note_chance(outrage: f32) -> f32 {
    NOTE_CHANCE * (1.0 - 0.6 * outrage)
}

/// A note's delay in hours from a standard normal draw `z`: the lognormal
/// of docs/model.md, 6.8, `exp(ln(median) + sigma * z)`.
pub(crate) fn note_delay(z: f32) -> f32 {
    (NOTE_DELAY_MEDIAN_HOURS.ln() + NOTE_DELAY_SIGMA * z).exp()
}

/// Decides, when it is written, whether each new false post gets a
/// community note and when the note will be displayed (docs/model.md, 6.8).
/// The draws come from the post's own note-timing stream, keyed on its
/// author and ordinal, once per post: one uniform for the chance and, for a
/// post that gets a note, one normal for its delay. Without community notes
/// nothing is drawn.
pub(crate) fn decide_notes(world: &mut World, ctx: &DayCtx, first_new: u32) {
    if !ctx.config.design.community_notes {
        return;
    }
    let seed = ctx.config.run.seed;
    for post in world.posts.iter_mut().filter(|p| p.id >= first_new) {
        if !post.is_misinformation() {
            continue;
        }
        let mut rng = rng::stream_rng(seed, 0, post.draw_key(), Stream::NoteTiming);
        if rng::chance(&mut rng, note_chance(post.outrage)) {
            post.note_delay = Some(note_delay(rng::normal(&mut rng, 1.0)));
        }
    }
}

/// Phase 8: a post whose note's display time has come by the end of the day,
/// while it had at least 50 impressions, counts as noted from then on, even
/// if no impression carried the note yet (docs/model.md, 6.8). Impressions
/// carry the note by their own time of day (6.8); this draws nothing.
pub(crate) fn community_notes(world: &mut World, ctx: &DayCtx) {
    if !ctx.config.design.community_notes {
        return;
    }
    for post in world.posts.iter_mut() {
        if !post.noted && post.note_shown(ctx.day, 24.0) {
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
    use crate::content::{AuthorProfile, Group, NOTE_MIN_IMPRESSIONS, Style, generate_post};
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

    #[test]
    fn the_classifier_keeps_each_new_posts_score() {
        let mut config = small_config();
        config.design.classifier_threshold = 0.5;
        let ctx = DayCtx {
            config: &config,
            day: 0,
        };
        let mut world = init::build_world(&config);
        let author = first_user(&world);
        let first = world.posts.next_id();
        let ids: Vec<u32> = (0..100)
            .map(|k| {
                let id = add_post(&mut world, author, 0.01 * k as f32, 0.5);
                if let Some(p) = world.posts.get_mut(id) {
                    p.ordinal = k;
                }
                id
            })
            .collect();
        classify_new_posts(&mut world, &ctx, first);
        let mut removed = 0;
        for &id in &ids {
            let post = world.posts.get(id).expect("live");
            let score = post.classifier_score.expect("screened");
            // The score is the one that decided the removal.
            assert_eq!(post.removed, score > 0.5, "post {id}");
            removed += usize::from(post.removed);
        }
        assert!(removed > 10 && removed < 90, "{removed}");
        // Switched off, the classifier screens nothing, unless a feed
        // experiment hides flagged posts: then the same scores are kept and
        // nothing is removed.
        config.design.classifier_enabled = false;
        let scores = |config: &Config| {
            let ctx = DayCtx { config, day: 0 };
            let mut other = init::build_world(config);
            let first = other.posts.next_id();
            for k in 0..100 {
                let id = add_post(&mut other, author, 0.01 * k as f32, 0.5);
                if let Some(p) = other.posts.get_mut(id) {
                    p.ordinal = k;
                }
            }
            classify_new_posts(&mut other, &ctx, first);
            assert!(other.posts.iter().all(|p| !p.removed && !p.demoted));
            other
                .posts
                .iter()
                .map(|p| p.classifier_score)
                .collect::<Vec<_>>()
        };
        assert!(scores(&config).iter().all(Option::is_none));
        config.run.feed_experiment = Some(crate::config::FeedExperiment {
            kind: FeedExperimentKind::HideFlagged { threshold: 0.5 },
            ..crate::config::FeedExperiment::default()
        });
        let kept: Vec<Option<f32>> = ids
            .iter()
            .map(|&id| world.posts.get(id).and_then(|p| p.classifier_score))
            .collect();
        assert_eq!(scores(&config), kept);
    }

    /// A post with truth 0.2, `impressions` impressions and a note due
    /// `delay` hours after its creation on day 0.
    fn noted_post(world: &mut World, impressions: u32, delay: f32) -> u32 {
        let author = first_user(world);
        let id = add_post(world, author, 0.1, 0.0);
        if let Some(p) = world.posts.get_mut(id) {
            p.truth = 0.2;
            p.impressions = impressions;
            p.note_delay = Some(delay);
        }
        id
    }

    #[test]
    fn a_note_shows_from_its_display_time_on() {
        let config = small_config();
        let mut world = init::build_world(&config);
        let id = noted_post(&mut world, 60, 30.0);
        let post = world.posts.get(id).cloned().expect("post");
        assert!(!post.note_shown(0, 23.9));
        assert!(!post.note_shown(1, 5.99));
        assert!(post.note_shown(1, 6.0));
        assert!(post.note_shown(2, 0.0));
        assert_eq!(post.age_hours(1, 6.0), 30.0);
        let mut removed = post.clone();
        removed.removed = true;
        assert!(!removed.note_shown(2, 0.0));
        let mut none = post;
        none.note_delay = None;
        assert!(!none.note_shown(2, 23.0));
    }

    #[test]
    fn a_note_needs_fifty_impressions_at_display_time() {
        let mut config = small_config();
        config.design.community_notes = true;
        let mut world = init::build_world(&config);
        let id = noted_post(&mut world, NOTE_MIN_IMPRESSIONS - 1, 2.0);
        let ctx = DayCtx {
            config: &config,
            day: 0,
        };
        community_notes(&mut world, &ctx);
        let post = world.posts.get(id).expect("post");
        assert!(!post.note_shown(0, 12.0) && !post.noted);
        if let Some(p) = world.posts.get_mut(id) {
            p.impressions = NOTE_MIN_IMPRESSIONS;
        }
        let post = world.posts.get(id).expect("post");
        assert!(post.note_shown(0, 2.0) && !post.note_shown(0, 1.9));
        community_notes(&mut world, &ctx);
        assert!(world.posts.get(id).is_some_and(|p| p.noted));
        assert_eq!(world.tally.notes, 1);
        community_notes(&mut world, &ctx);
        assert_eq!(world.tally.notes, 1, "a post is noted once");
    }

    #[test]
    fn the_end_of_day_marks_a_note_due_that_day_and_draws_nothing() {
        let mut config = small_config();
        config.design.community_notes = true;
        let mut world = init::build_world(&config);
        let late = noted_post(&mut world, 80, 40.0);
        let ctx = |day| DayCtx {
            config: &config,
            day,
        };
        community_notes(&mut world, &ctx(0));
        assert!(world.posts.get(late).is_some_and(|p| !p.noted));
        community_notes(&mut world, &ctx(1));
        let post = world.posts.get(late).expect("post");
        assert!(post.noted);
        assert_eq!(post.note_delay, Some(40.0), "the delay is drawn once");
    }

    /// A small world on day 0, with community notes on or off, after
    /// everyone wrote their posts and the day's note decisions, with its
    /// configuration.
    fn decided(notes: bool) -> (World, Config) {
        let mut config = small_config();
        config.run.population_size = 400;
        config.design.community_notes = notes;
        let mut world = init::build_world(&config);
        for person in &mut world.people {
            person.state.today.opened = person.state.is_user;
        }
        let ctx = DayCtx {
            config: &config,
            day: 0,
        };
        let first = world.posts.next_id();
        crate::posting::write_posts(&mut world, &ctx);
        // Every other post is made false, so the rule has enough to decide.
        for p in world.posts.iter_mut().filter(|p| p.id % 2 == 1) {
            p.truth = 0.3;
        }
        decide_notes(&mut world, &ctx, first);
        (world, config)
    }

    #[test]
    fn notes_are_decided_once_per_false_post_and_never_without_notes() {
        let (off, _) = decided(false);
        assert!(off.posts.iter().all(|p| p.note_delay.is_none()));
        let (mut world, config) = decided(true);
        let on: Vec<Post> = world.posts.iter().cloned().collect();
        assert_eq!(off.posts.len(), on.len());
        // Same posts either way: the decision draws on its own stream.
        for (a, b) in off.posts.iter().zip(&on) {
            assert_eq!((a.style, a.truth, a.quality), (b.style, b.truth, b.quality));
        }
        assert!(
            on.iter()
                .all(|p| p.is_misinformation() || p.note_delay.is_none())
        );
        let false_posts: Vec<&Post> = on.iter().filter(|p| p.is_misinformation()).collect();
        let expected: f32 = false_posts.iter().map(|p| note_chance(p.outrage)).sum();
        let got = false_posts
            .iter()
            .filter(|p| p.note_delay.is_some())
            .count() as f32;
        let sd = expected.sqrt().max(1.0);
        assert!(false_posts.len() > 30, "{}", false_posts.len());
        assert!(
            (got - expected).abs() < 4.0 * sd,
            "{got} against {expected}"
        );
        // Later days decide only for their own new posts, and the end of a
        // day draws nothing: every delay stays as drawn.
        let before: Vec<Option<f32>> = on.iter().map(|p| p.note_delay).collect();
        for day in 1..3 {
            let ctx = DayCtx {
                config: &config,
                day,
            };
            let next = world.posts.next_id();
            decide_notes(&mut world, &ctx, next);
            community_notes(&mut world, &ctx);
        }
        let after: Vec<Option<f32>> = world.posts.iter().map(|p| p.note_delay).collect();
        assert_eq!(after, before);
    }

    #[test]
    fn note_delays_follow_the_fitted_lognormal() {
        assert!((note_delay(0.0) - NOTE_DELAY_MEDIAN_HOURS).abs() < 1e-4);
        let mut rng = rng::stream_rng(9, 0, 0, Stream::NoteTiming);
        let n = 40_000;
        let delays: Vec<f32> = (0..n)
            .map(|_| note_delay(rng::normal(&mut rng, 1.0)))
            .collect();
        let within = |h: f32| delays.iter().filter(|&&d| d <= h).count() as f32 / n as f32;
        // The fit's quantiles (docs/model.md, 13.2): mu = ln 16, sigma 0.92.
        for (hours, share) in [(4.0, 0.066), (6.0, 0.143), (16.0, 0.5), (36.0, 0.811)] {
            let got = within(hours);
            assert!((got - share).abs() < 0.01, "{hours} h: {got}");
        }
    }

    #[test]
    fn the_partisan_factor_scales_the_note_chance() {
        assert!((note_chance(0.0) - NOTE_CHANCE).abs() < 1e-6);
        assert!((note_chance(1.0) - 0.4 * NOTE_CHANCE).abs() < 1e-6);
    }
}
