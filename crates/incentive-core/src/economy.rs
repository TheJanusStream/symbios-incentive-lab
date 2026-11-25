//! Phase 10: revenue, subscriptions and creator payouts (docs/model.md, 8.3).

use crate::behaviour::sig;
use crate::config::PayoutScheme;
use crate::rng::{self, Stream};
use crate::world::{DayCtx, World};

/// Days between subscription decisions.
const SUBSCRIPTION_PERIOD: u32 = 30;
/// Days of the income moving average.
const INCOME_DAYS: f32 = 30.0;

/// Runs the day's economy: subscriptions, revenue, payouts and incomes.
pub(crate) fn run(world: &mut World, ctx: &DayCtx) {
    if ctx.day.is_multiple_of(SUBSCRIPTION_PERIOD) {
        decide_subscriptions(world, ctx);
    }
    let revenue = revenue(world, ctx);
    world.tally.revenue = revenue;
    let pool = ctx.config.design.payout_share * revenue;
    if pool > 0.0 {
        match ctx.config.design.payout_scheme {
            PayoutScheme::FanFunded => pay_fans(world, ctx),
            scheme => pay_by_units(world, ctx, scheme, pool),
        }
    }
    for person in &mut world.people {
        let s = &mut person.state;
        s.income_ema += (s.today.income - s.income_ema) / INCOME_DAYS;
        s.income_total += s.today.income;
    }
}

/// Every 30 days each user decides afresh whether to subscribe, by the mean
/// daily reflective utility since the last decision. On day 0 there is no
/// history, so the mean is taken to equal the hurdle.
fn decide_subscriptions(world: &mut World, ctx: &DayCtx) {
    let hurdle = ctx.config.world.subscription_hurdle;
    for (i, person) in world.people.iter_mut().enumerate() {
        let s = &mut person.state;
        let mean = if ctx.day == 0 {
            hurdle
        } else {
            s.eref_window / SUBSCRIPTION_PERIOD as f32
        };
        s.eref_window = 0.0;
        if !s.is_user {
            s.subscriber = false;
            continue;
        }
        let mut rng = ctx.rng(i as u64, Stream::Economy);
        s.subscriber = rng::chance(&mut rng, 0.5 * sig((mean - hurdle) / 0.1));
    }
}

fn subscription_revenue_per_user(ctx: &DayCtx) -> f32 {
    (1.0 - ctx.config.design.ads_share) * ctx.config.world.subscription_price
        / SUBSCRIPTION_PERIOD as f32
}

fn revenue(world: &World, ctx: &DayCtx) -> f32 {
    let subscribers = world
        .people
        .iter()
        .filter(|p| p.state.is_user && p.state.subscriber)
        .count();
    let ads =
        ctx.config.design.ads_share * ctx.config.world.ad_rate * world.tally.impressions as f32;
    ads + subscription_revenue_per_user(ctx) * subscribers as f32
}

/// Whether the author of a post can be paid.
fn eligible(world: &World, ctx: &DayCtx, author: u32) -> bool {
    world.accounts.followers[author as usize] >= ctx.config.design.payout_threshold_followers
}

/// Credits a payout to a post and its author.
fn credit(world: &mut World, post_id: u32, amount: f32) {
    let Some(post) = world.posts.get_mut(post_id) else {
        return;
    };
    post.payout += amount;
    let (author, style) = (post.author, post.style.index());
    world.tally.payouts += amount;
    world.tally.payout_by_style[style] += amount;
    match world.bot_slot(author) {
        Some(slot) => world.bots[slot].income_total += amount,
        None => world.people[author as usize].state.today.income += amount,
    }
}

/// Views, engagement or quality-weighted engagement: the pool is divided in
/// proportion to each eligible post's units today.
fn pay_by_units(world: &mut World, ctx: &DayCtx, scheme: PayoutScheme, pool: f32) {
    let units: Vec<(u32, f32)> = world
        .posts
        .iter()
        .filter(|p| !p.removed && eligible(world, ctx, p.author))
        .map(|p| {
            let u = match scheme {
                PayoutScheme::PerView => p.impressions_today as f32,
                PayoutScheme::QualityWeighted => p.engagement_today as f32 * p.quality_signal,
                PayoutScheme::PerEngagement | PayoutScheme::FanFunded => p.engagement_today as f32,
            };
            (p.id, u)
        })
        .filter(|&(_, u)| u > 0.0)
        .collect();
    let total: f32 = units.iter().map(|&(_, u)| u).sum();
    if total <= 0.0 {
        return;
    }
    for (id, u) in units {
        credit(world, id, pool * u / total);
    }
}

/// Fan funding: each user's share of the revenue they brought in goes to the
/// posts that gave them positive reflective value, in proportion to it.
fn pay_fans(world: &mut World, ctx: &DayCtx) {
    let design = &ctx.config.design;
    let per_impression = design.ads_share * ctx.config.world.ad_rate;
    let subscription = subscription_revenue_per_user(ctx);
    for v in 0..world.population() {
        let credits = std::mem::take(&mut world.people[v].state.today.fan_credits);
        let state = &world.people[v].state;
        let brought = per_impression * state.today.items as f32
            + if state.subscriber { subscription } else { 0.0 };
        let share = design.payout_share * brought;
        let paid: Vec<(u32, f32)> = credits
            .iter()
            .copied()
            .filter(|&(id, _)| {
                world
                    .posts
                    .get(id)
                    .is_some_and(|p| !p.removed && eligible(world, ctx, p.author))
            })
            .collect();
        let total: f32 = paid.iter().map(|&(_, r)| r).sum();
        if share > 0.0 && total > 0.0 {
            for &(id, r) in &paid {
                credit(world, id, share * r / total);
            }
        }
        world.people[v].state.today.fan_credits = credits;
    }
}
