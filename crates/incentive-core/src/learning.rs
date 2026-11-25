//! Phase 11: authors learn what pays (docs/model.md, 7.5), and posts at the end
//! of their life get their final accounting and leave the store.

use crate::content::STYLE_COUNT;
use crate::rng::{self, Stream};
use crate::world::{DayCtx, World};

/// Share of the day's most-viewed posts that the discourse metrics look at.
const TOP_POST_SHARE: f32 = 0.05;
/// Daily drift of unwritten styles back toward their intrinsic value.
const STYLE_DRIFT: f32 = 0.01;
/// Share of the way an imitator moves toward the imitated account's values.
const IMITATION_STEP: f32 = 0.2;
/// Penalty in the reward of a removed or struck post.
const SANCTION_PENALTY: f32 = 1.0;
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

/// The platform's running per-style rates, from observed counts (6.3).
fn update_style_rates(world: &mut World) {
    const SMOOTHING: f32 = 0.1;
    let t = &world.tally;
    for (s, rates) in world.platform.style_rates.iter_mut().enumerate() {
        let impressions = t.impressions_by_style[s];
        if impressions == 0 {
            continue;
        }
        let n = impressions as f32;
        rates.like += SMOOTHING * (t.likes_by_style[s] as f32 / n - rates.like);
        rates.reply += SMOOTHING * (t.replies_by_style[s] as f32 / n - rates.reply);
        rates.share += SMOOTHING * (t.shares_by_style[s] as f32 / n - rates.share);
        rates.dwell += SMOOTHING * (t.dwell_by_style[s] / n - rates.dwell);
    }
}

/// The reward of a post at the end of its life, with its final counts
/// (docs/model.md, 7.5; the timing choice is recorded in section 13).
fn reward_author(world: &mut World, ctx: &DayCtx, id: u32) {
    let Some(post) = world.posts.get(id) else {
        return;
    };
    let Some(person) = world.people.get_mut(post.author as usize) else {
        return;
    };
    if !person.state.is_user {
        return;
    }
    let traits = &person.traits;
    let social =
        (post.likes() as f32 + 2.0 * post.positive_replies as f32 + 3.0 * post.shares as f32)
            .ln_1p()
            + traits.temperament * (post.angry_replies as f32).ln_1p()
            + 0.5 * (post.followers_gained as f32).ln_1p();
    let money = post.payout / (ctx.config.world.living_income / MONEY_DIVISOR);
    let sanction = if post.removed || post.struck {
        SANCTION_PENALTY
    } else {
        0.0
    };
    let s = post.style.index();
    let reward = traits.intrinsic[s]
        + traits.approval_sensitivity * social / 3.0
        + traits.money_weight * money
        - sanction;
    let state = &mut person.state;
    state.style_value[s] += ctx.config.world.learning_rate * (reward - state.style_value[s]);
    state.reward_ema += REWARD_SMOOTHING * (reward - state.reward_ema);
    state.typical_likes += TYPICAL_LIKES_SMOOTHING * (post.likes() as f32 - state.typical_likes);
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
