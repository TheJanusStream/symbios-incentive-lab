//! Phase 11: authors learn what pays (docs/model.md, 7.5), and posts at the end
//! of their life get their final accounting and leave the store.

use crate::content::Post;
use crate::content::STYLE_COUNT;
use crate::moderation;
use crate::person::{POLISH_LEARNING_SHARE, PersonState};
use crate::rng::{self, Stream};
use crate::world::{DayCtx, DayTally, World, partisan_band};

/// Share of the day's most-viewed posts that the discourse metrics look at.
const TOP_POST_SHARE: f32 = 0.05;
/// Daily drift of unwritten styles back toward their intrinsic value.
const STYLE_DRIFT: f32 = 0.01;
/// Share of the way an imitator moves toward the imitated account's values.
const IMITATION_STEP: f32 = 0.2;
/// Penalty in the reward of a removed or struck post.
const SANCTION_PENALTY: f32 = 1.0;
/// Social reward `S` at which an unsanctioned hostile post raises its
/// author's intensity at half of `intensity_rate` (docs/model.md, 7.5).
const INTENSITY_HALF_SOCIAL: f32 = 3.0;
/// Smoothing of `reward_ema` (a 7-post mean) and of `typical_likes`.
const REWARD_SMOOTHING: f32 = 1.0 / 7.0;
const TYPICAL_LIKES_SMOOTHING: f32 = 0.2;
/// `M` divides a post's payout by `living_income / MONEY_DIVISOR`. The
/// specification's text says 30, which made money swamp every other reward;
/// section 13 of docs/model.md records the calibrated value.
const MONEY_DIVISOR: f32 = 1.0;

/// Runs the learning phase.
pub(crate) fn run(world: &mut World, ctx: &DayCtx) {
    collect_post_day(world, ctx);
    let lifetime = ctx.config.run.post_lifetime_days;
    let ending: Vec<u32> = world
        .posts
        .iter()
        .filter(|p| p.day + lifetime == ctx.day + 1)
        .map(|p| p.id)
        .collect();
    for id in ending {
        reward_author(world, ctx, id);
    }
    drift_and_imitate(world, ctx);
    moderation::fade_evasion(world, ctx);
    world.posts.prune_front(|p| p.day + lifetime <= ctx.day + 1);
    world.accounts.prune(world.posts.first_id());
    world.posts.iter_mut().for_each(|p| p.reset_today());
}

/// Gathers what the day's post counters say before posts are pruned: author
/// feedback, the most-viewed posts, the largest cascade and the platform's
/// per-style engagement rates.
fn collect_post_day(world: &mut World, ctx: &DayCtx) {
    let lifetime = ctx.config.run.post_lifetime_days;
    let mut viewed: Vec<(u32, bool, bool)> = Vec::new();
    for post in world.posts.iter() {
        if let Some(person) = world.people.get_mut(post.author as usize) {
            let today = &mut person.state.today;
            today.likes_received += post.likes_today;
            today.mutual_replies += post.mutual_replies_today;
            today.angry_received += post.angry_today;
        }
        if post.impressions_today > 0 {
            viewed.push((
                post.impressions_today,
                post.is_toxic(),
                !post.is_misinformation(),
            ));
        }
        if post.day + lifetime == ctx.day + 1 {
            world.tally.largest_cascade = world.tally.largest_cascade.max(post.human_shares());
            count_ending_post(&mut world.tally, &world.truth.partisanship_rank, post);
            if let Some(e) = world.truth.experiment.as_mut() {
                e.count_ending_post(&world.truth.partisanship_rank, post);
            }
        }
    }
    viewed.sort_by_key(|v| std::cmp::Reverse(v.0));
    let top = ((viewed.len() as f32) * TOP_POST_SHARE).ceil() as usize;
    if top > 0 {
        let top_posts = &viewed[..top];
        world.tally.top_toxic_share = top_posts.iter().filter(|p| p.1).count() as f32 / top as f32;
        world.tally.top_truthful_share =
            top_posts.iter().filter(|p| p.2).count() as f32 / top as f32;
    } else {
        world.tally.top_truthful_share = 1.0;
    }
    update_style_rates(world);
}

/// Adds a post at the end of its life to the descriptive truth tallies
/// (docs/model.md, 9.2): its lifetime human impressions by its human
/// author's partisanship, for a noted post its human shares before and after
/// the note, and whether a false post got a note. Measurement only: it draws nothing and changes nothing
/// else.
fn count_ending_post(tally: &mut DayTally, partisanship_rank: &[u32], post: &Post) {
    if post.is_misinformation() {
        tally.false_posts_ended += 1;
        tally.noted_false_posts_ended += u32::from(post.noted);
    }
    if post.noted {
        tally.noted_shares += post.human_shares();
        tally.noted_shares_before_note += post.shares_at_note;
    }
    let Some(band) = partisan_band(partisanship_rank, post.author as usize) else {
        return;
    };
    let impressions = u64::from(post.impressions);
    let kept = !post.removed && !post.demoted;
    if band.half {
        tally.partisan_half_posts += 1;
        tally.partisan_half_impressions += impressions;
        if kept {
            tally.partisan_half_kept_posts += 1;
            tally.partisan_half_kept_impressions += impressions;
        }
    }
    if band.extreme {
        tally.extreme_posts += 1;
        tally.extreme_impressions += impressions;
        if kept {
            tally.extreme_kept_posts += 1;
            tally.extreme_kept_impressions += impressions;
        }
    } else if band.middle {
        tally.middle_posts += 1;
        tally.middle_impressions += impressions;
        if kept {
            tally.middle_kept_posts += 1;
            tally.middle_kept_impressions += impressions;
        }
    }
}

/// The platform's running per-style rates, from observed counts per human
/// impression (6.3). Bot likes and shares count like anyone's: the platform
/// cannot tell them apart.
fn update_style_rates(world: &mut World) {
    const SMOOTHING: f32 = 0.1;
    let t = &world.tally;
    for (s, rates) in world.platform.style_rates.iter_mut().enumerate() {
        let impressions = t.impressions_by_style[s];
        if impressions == 0 {
            continue;
        }
        let n = impressions as f32;
        let likes = (t.likes_by_style[s] + t.bot_likes_by_style[s]) as f32;
        let shares = (t.shares_by_style[s] + t.bot_shares_by_style[s]) as f32;
        rates.like += SMOOTHING * (likes / n - rates.like);
        rates.reply += SMOOTHING * (t.replies_by_style[s] as f32 / n - rates.reply);
        rates.share += SMOOTHING * (shares / n - rates.share);
        rates.dwell += SMOOTHING * (t.dwell_by_style[s] / n - rates.dwell);
    }
}

/// The reward of a post at the end of its life, with its final counts
/// (docs/model.md, 7.5; the timing choice is recorded in section 13). A
/// banned author still learns from the post that drew the ban, so the ban
/// deters when the person comes back.
fn reward_author(world: &mut World, ctx: &DayCtx, id: u32) {
    let Some(post) = world.posts.get(id) else {
        return;
    };
    let Some(person) = world.people.get_mut(post.author as usize) else {
        return;
    };
    if !person.state.is_user && post.sanction_severity == 0 {
        return;
    }
    let traits = &person.traits;
    let social =
        (post.likes() as f32 + 2.0 * post.positive_replies as f32 + 3.0 * post.shares as f32)
            .ln_1p()
            + traits.temperament * (post.angry_replies as f32).ln_1p()
            + 0.5 * (post.followers_gained as f32).ln_1p();
    let money = post.payout / (ctx.config.world.living_income / MONEY_DIVISOR);
    let assume = &ctx.config.world;
    let removal = if post.removed || post.struck {
        SANCTION_PENALTY
    } else {
        0.0
    };
    let sanction = removal + assume.deterrence * f32::from(post.sanction_severity);
    let s = post.style.index();
    let polish = if post.polished {
        assume.polish_cost
    } else {
        0.0
    };
    let reward = traits.intrinsic[s]
        + traits.approval_sensitivity * social / 3.0
        + traits.money_weight * money
        - sanction
        - polish;
    let state = &mut person.state;
    let advantage = reward - state.style_value[s];
    learn_polish(
        state,
        post.polished,
        advantage,
        POLISH_LEARNING_SHARE * assume.learning_rate,
    );
    if post.style.is_hostile() {
        state.intensity = learn_intensity(
            state.intensity,
            post.removed || post.demoted || post.struck,
            social,
            assume.intensity_rate,
        );
    }
    state.style_value[s] += ctx.config.world.learning_rate * (reward - state.style_value[s]);
    state.reward_ema += REWARD_SMOOTHING * (reward - state.reward_ema);
    state.typical_likes += TYPICAL_LIKES_SMOOTHING * (post.likes() as f32 - state.typical_likes);
}

/// Polish learning (docs/model.md, 7.5): the value of the chosen option,
/// plain or polished, moves at `rate` (a share of the style learning rate)
/// toward the post's advantage, its reward less the style's value before
/// this post's update. Measuring against the style's value keeps the style
/// mix out of the comparison, so the gap between the two values estimates
/// what polish adds to a post's reward, its cost included.
fn learn_polish(state: &mut PersonState, polished: bool, advantage: f32, rate: f32) {
    let value = &mut state.polish_value[usize::from(polished)];
    *value += rate * (advantage - *value);
}

/// The intensity an author takes from one hostile post: a removal or
/// demotion lowers it by `rate * intensity`; an unsanctioned post raises it by
/// `rate * S / (S + 3) * (1 - intensity)`, more the better it did.
fn learn_intensity(intensity: f32, sanctioned: bool, social: f32, rate: f32) -> f32 {
    let next = if sanctioned {
        intensity - rate * intensity
    } else {
        let success = social.max(0.0) / (social.max(0.0) + INTENSITY_HALF_SOCIAL);
        intensity + rate * success * (1.0 - intensity)
    };
    next.clamp(0.0, 1.0)
}

/// Unwritten styles drift back toward intrinsic value; some users copy a more
/// successful account they follow.
fn drift_and_imitate(world: &mut World, ctx: &DayCtx) {
    let rate = ctx.config.world.imitation_rate;
    for i in 0..world.population() {
        if !world.people[i].state.is_user {
            continue;
        }
        let person = &mut world.people[i];
        for s in 0..STYLE_COUNT {
            if person.state.today.styles_written & (1 << s) == 0 {
                let intrinsic = person.traits.intrinsic[s];
                person.state.style_value[s] +=
                    STYLE_DRIFT * (intrinsic - person.state.style_value[s]);
            }
        }
        let mut rng = ctx.rng(i as u64, Stream::Learning);
        if !rng::chance(&mut rng, rate) || person.state.follows.is_empty() {
            continue;
        }
        let pick = person.state.follows[rng::index(&mut rng, person.state.follows.len())] as usize;
        let own = person.state.reward_ema;
        let Some(model) = world.people.get(pick) else {
            continue;
        };
        if model.state.reward_ema <= own {
            continue;
        }
        let target = model.state.style_value;
        let values = &mut world.people[i].state.style_value;
        for (v, t) in values.iter_mut().zip(target) {
            *v += IMITATION_STEP * (t - *v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::content::{AuthorProfile, Group, Style, generate_post};
    use crate::init;

    #[test]
    fn intensity_falls_after_a_sanction_and_rises_otherwise() {
        let down = learn_intensity(0.5, true, 4.0, 0.1);
        assert!((down - 0.45).abs() < 1e-6);
        let small = learn_intensity(0.5, false, 1.0, 0.1);
        let big = learn_intensity(0.5, false, 6.0, 0.1);
        assert!(small > 0.5 && big > small, "{small} {big}");
        assert_eq!(learn_intensity(0.5, false, 0.0, 0.1), 0.5);
        assert_eq!(learn_intensity(0.5, true, 4.0, 0.0), 0.5);
    }

    /// The style value of Attack after one struck Attack of the given severity.
    fn attack_value_after(severity: u8) -> f32 {
        let mut config = Config::default();
        config.run.population_size = 200;
        let mut world = init::build_world(&config);
        let author = world
            .people
            .iter()
            .position(|p| p.state.is_user)
            .unwrap_or(0);
        let mut rng = rng::stream_rng(5, 0, 0, Stream::Posting);
        let id = world.posts.next_id();
        let profile = AuthorProfile::plain(Group::A, 0.5, 0.5);
        let mut post = generate_post(
            &mut rng,
            id,
            0,
            &profile,
            Style::Attack,
            &[1.0 / 6.0; 6],
            &config.world,
        );
        post.author = author as u32;
        post.removed = true;
        post.struck = true;
        post.sanction_severity = severity;
        world.posts.push(post);
        // A banned author still learns from the post that drew the ban.
        world.people[author].state.is_user = severity < 4;
        let ctx = DayCtx {
            config: &config,
            day: 0,
        };
        reward_author(&mut world, &ctx, id);
        world.people[author].state.style_value[Style::Attack.index()]
    }

    #[test]
    fn a_ban_penalises_more_than_a_strike() {
        let strike = attack_value_after(1);
        let ban = attack_value_after(4);
        let none = attack_value_after(0);
        let config = Config::default();
        let step = config.world.learning_rate * config.world.deterrence;
        assert!((none - strike - step).abs() < 1e-5, "{none} {strike}");
        assert!((strike - ban - 3.0 * step).abs() < 1e-5, "{strike} {ban}");
    }

    #[test]
    fn polish_learning_charges_the_cost_and_moves_the_chosen_value() {
        let mut config = Config::default();
        config.run.population_size = 200;
        config.world.polish_cost = 0.4;
        let mut world = init::build_world(&config);
        let author = world
            .people
            .iter()
            .position(|p| p.state.is_user)
            .unwrap_or(0);
        let mut ids = Vec::new();
        for polished in [false, true] {
            let mut rng = rng::stream_rng(5, 0, 0, Stream::PostDraw);
            let id = world.posts.next_id();
            let profile = AuthorProfile {
                polished,
                ..AuthorProfile::plain(Group::A, 0.5, 0.5)
            };
            let mut post = generate_post(
                &mut rng,
                id,
                0,
                &profile,
                Style::Personal,
                &[1.0 / 6.0; 6],
                &config.world,
            );
            post.author = author as u32;
            world.posts.push(post);
            ids.push(id);
        }
        let ctx = DayCtx {
            config: &config,
            day: 0,
        };
        let s = Style::Personal.index();
        let rate = config.world.learning_rate;
        let polish_rate = POLISH_LEARNING_SHARE * rate;
        let before = world.people[author].state.clone();
        reward_author(&mut world, &ctx, ids[0]);
        let after_plain = world.people[author].state.clone();
        let plain_reward =
            before.style_value[s] + (after_plain.style_value[s] - before.style_value[s]) / rate;
        let moved = |old: f32, advantage: f32| old + polish_rate * (advantage - old);
        let expected = moved(before.polish_value[0], plain_reward - before.style_value[s]);
        assert!((after_plain.polish_value[0] - expected).abs() < 1e-5);
        assert_eq!(after_plain.polish_value[1], before.polish_value[1]);
        reward_author(&mut world, &ctx, ids[1]);
        let after = &world.people[author].state;
        let polished_reward =
            after_plain.style_value[s] + (after.style_value[s] - after_plain.style_value[s]) / rate;
        // Same counts, so the rewards differ by the cost alone.
        assert!(
            (plain_reward - polished_reward - 0.4).abs() < 1e-4,
            "{plain_reward} {polished_reward}"
        );
        let expected = moved(
            after_plain.polish_value[1],
            polished_reward - after_plain.style_value[s],
        );
        assert!((after.polish_value[1] - expected).abs() < 1e-5);
        assert_eq!(after.polish_value[0], after_plain.polish_value[0]);
    }

    #[test]
    fn polish_choice_follows_the_values_and_keeps_exploring() {
        let config = Config::default();
        let world = init::build_world(&config);
        let mut person = world.people[0].clone();
        let t = config.world.choice_temperature;
        person.state.polish_value = [0.0, 0.0];
        assert!((person.polish_probability(t) - 0.5).abs() < 1e-6);
        person.state.polish_value = [0.0, 0.3];
        let up = person.polish_probability(t);
        person.state.polish_value = [0.3, 0.0];
        let down = person.polish_probability(t);
        assert!(up > 0.7 && down < 0.3 && (up + down - 1.0).abs() < 1e-5);
        person.state.polish_value = [100.0, 0.0];
        assert!(person.polish_probability(t) >= crate::person::POLISH_EXPLORATION - 1e-6);
    }

    #[test]
    fn polish_is_rare_on_day_zero_at_any_temperature() {
        for t in [0.05, 0.25, 1.0] {
            let mut config = Config::default();
            config.run.population_size = 200;
            config.world.choice_temperature = t;
            let world = init::build_world(&config);
            for person in &world.people {
                let p = person.polish_probability(t);
                assert!(p < 0.1, "{t} {p}");
                assert!(person.state.polish_value[1] < person.state.polish_value[0]);
            }
        }
    }

    #[test]
    fn style_rates_count_bot_engagement() {
        let config = Config::default();
        let mut world = init::build_world(&config);
        let s = Style::Clickbait.index();
        let before = world.platform.style_rates[s];
        world.tally.impressions_by_style[s] = 100;
        world.tally.likes_by_style[s] = 10;
        world.tally.shares_by_style[s] = 1;
        world.tally.bot_likes_by_style[s] = 20;
        world.tally.bot_shares_by_style[s] = 20;
        update_style_rates(&mut world);
        let after = world.platform.style_rates[s];
        assert!((after.like - (before.like + 0.1 * (0.30 - before.like))).abs() < 1e-6);
        assert!((after.share - (before.share + 0.1 * (0.21 - before.share))).abs() < 1e-6);
        // Styles without impressions keep their rates.
        let other = Style::Informative.index();
        assert_eq!(world.platform.style_rates[other], before);
    }

    #[test]
    fn ending_posts_are_counted_by_partisanship_and_note() {
        let config = Config::default();
        let mut rng = rng::stream_rng(1, 0, 0, Stream::PostDraw);
        let profile = AuthorProfile::plain(Group::A, 0.5, 0.5);
        let mut post = generate_post(
            &mut rng,
            0,
            0,
            &profile,
            Style::Outrage,
            &[1.0 / 6.0; 6],
            &config.world,
        );
        // Ten people, ranked by partisanship: 9 is the most partisan, 5 lies
        // in the more partisan half and the middle fifth, 4 in the middle
        // fifth only, 0 in neither.
        let ranks: Vec<u32> = (0..10).collect();
        let count = |post: &Post, author: u32, impressions: u32| {
            let mut tally = DayTally::default();
            let mut p = post.clone();
            p.author = author;
            p.impressions = impressions;
            count_ending_post(&mut tally, &ranks, &p);
            tally
        };
        let top = count(&post, 9, 30);
        assert_eq!(
            (top.partisan_half_posts, top.partisan_half_impressions),
            (1, 30)
        );
        assert_eq!((top.extreme_posts, top.extreme_impressions), (1, 30));
        assert_eq!(top.middle_posts, 0);
        let five = count(&post, 5, 7);
        assert_eq!((five.partisan_half_posts, five.extreme_posts), (1, 0));
        assert_eq!((five.middle_posts, five.middle_impressions), (1, 7));
        let four = count(&post, 4, 7);
        assert_eq!((four.partisan_half_posts, four.middle_posts), (0, 1));
        let low = count(&post, 0, 7);
        assert_eq!(
            (low.partisan_half_posts, low.extreme_posts, low.middle_posts),
            (0, 0, 0)
        );
        // A bot's post has no partisanship; its note still counts.
        post.noted = true;
        post.shares = 12;
        post.bot_shares = 2;
        post.shares_at_note = 7;
        let bot = count(&post, 10, 99);
        assert_eq!(bot.partisan_half_posts + bot.middle_posts, 0);
        assert_eq!((bot.noted_shares, bot.noted_shares_before_note), (10, 7));
    }

    /// Reach both ways: a post never removed or demoted counts in the kept
    /// tallies too; a removed or demoted one only in the tallies that
    /// include it.
    #[test]
    fn kept_reach_leaves_out_removed_and_demoted_posts() {
        let config = Config::default();
        let mut rng = rng::stream_rng(1, 0, 0, Stream::PostDraw);
        let profile = AuthorProfile::plain(Group::A, 0.5, 0.5);
        let post = generate_post(
            &mut rng,
            0,
            0,
            &profile,
            Style::Outrage,
            &[1.0 / 6.0; 6],
            &config.world,
        );
        let ranks: Vec<u32> = (0..10).collect();
        let mut tally = DayTally::default();
        for (author, impressions, removed, demoted) in [
            (9, 30, false, false),
            (9, 20, true, false),
            (8, 10, false, true),
            (5, 7, false, false),
            (4, 3, true, false),
        ] {
            let mut p = post.clone();
            p.author = author;
            p.impressions = impressions;
            p.removed = removed;
            p.demoted = demoted;
            count_ending_post(&mut tally, &ranks, &p);
        }
        assert_eq!(
            (tally.partisan_half_posts, tally.partisan_half_impressions),
            (4, 67)
        );
        assert_eq!(
            (
                tally.partisan_half_kept_posts,
                tally.partisan_half_kept_impressions
            ),
            (2, 37)
        );
        assert_eq!((tally.extreme_posts, tally.extreme_impressions), (2, 50));
        assert_eq!(
            (tally.extreme_kept_posts, tally.extreme_kept_impressions),
            (1, 30)
        );
        // Author 8 sits at the 85th percentile: in the half, in neither the
        // tenth nor the middle fifth.
        assert_eq!((tally.middle_posts, tally.middle_impressions), (2, 10));
        assert_eq!(
            (tally.middle_kept_posts, tally.middle_kept_impressions),
            (1, 7)
        );
    }
}
