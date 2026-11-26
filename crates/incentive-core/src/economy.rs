//! Phase 10: revenue, subscriptions and creator payouts (docs/model.md, 8.3).

use crate::behaviour::sig;
use crate::config::{PayoutScheme, REFERENCE_SUBSCRIPTION_PRICE, WorldAssumptions};
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
    revenue(world, ctx);
    let revenue = world.tally.revenue;
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

/// Every 30 days each user decides afresh whether to subscribe to the
/// ad-free tier (docs/model.md, 8.3). The value `V` is the mean daily
/// reflective utility since the last decision without the ad annoyance felt,
/// so that a subscriber and a non-subscriber with the same feed value it
/// alike, plus the annoyance the tier spares at the current ad load and the
/// person's recent items a day. On day 0 there is no history, so `V` is taken
/// to equal the hurdle.
fn decide_subscriptions(world: &mut World, ctx: &DayCtx) {
    let assume = &ctx.config.world;
    let spared_per_item = assume.ad_annoyance * ctx.config.design.ads_share;
    let price_factor = price_factor(assume);
    for (i, person) in world.people.iter_mut().enumerate() {
        let s = &mut person.state;
        let value = if ctx.day == 0 {
            assume.subscription_hurdle
        } else {
            subscription_value(
                s.eref_window,
                s.annoyance_window,
                s.items_window,
                spared_per_item,
            )
        };
        s.eref_window = 0.0;
        s.annoyance_window = 0.0;
        s.items_window = 0;
        if !s.is_user {
            s.subscriber = false;
            continue;
        }
        let mut rng = ctx.rng(i as u64, Stream::Economy);
        s.subscriber = rng::chance(&mut rng, uptake(assume, value, price_factor));
    }
}

/// The mean daily value of the platform with the tier: reflective utility
/// before the time cost, without the ad annoyance felt, plus the annoyance
/// the tier spares at the person's recent items a day.
fn subscription_value(eref: f32, annoyance: f32, items: u32, spared_per_item: f32) -> f32 {
    let days = SUBSCRIPTION_PERIOD as f32;
    (eref + annoyance + spared_per_item * items as f32) / days
}

/// `(p_ref / p)^e`: how the price moves uptake, 1 at the reference price.
fn price_factor(assume: &WorldAssumptions) -> f32 {
    let price = assume.subscription_price.max(f32::MIN_POSITIVE);
    (REFERENCE_SUBSCRIPTION_PRICE / price).powf(assume.subscription_elasticity)
}

/// The chance of subscribing: `s * sig((V - hurdle) / 0.1) * (p_ref / p)^e`,
/// at most 1.
fn uptake(assume: &WorldAssumptions, value: f32, price_factor: f32) -> f32 {
    let willing = sig((value - assume.subscription_hurdle) / 0.1);
    (assume.subscription_uptake * willing * price_factor).min(1.0)
}

/// What a subscriber pays a day: the price over 30 days, whatever the ad
/// load.
fn subscription_revenue_per_user(ctx: &DayCtx) -> f32 {
    ctx.config.world.subscription_price / SUBSCRIPTION_PERIOD as f32
}

/// Ad revenue per impression that carries ads.
fn ad_revenue_per_impression(ctx: &DayCtx) -> f32 {
    ctx.config.design.ads_share * ctx.config.world.ad_rate
}

/// Today's revenue: ads on the impressions of people who do not subscribe,
/// and the tier's price from every subscriber.
fn revenue(world: &mut World, ctx: &DayCtx) {
    let subscribers = world
        .people
        .iter()
        .filter(|p| p.state.is_user && p.state.subscriber)
        .count();
    let ads = ad_revenue_per_impression(ctx) * world.tally.ad_impressions as f32;
    let tier = subscription_revenue_per_user(ctx) * subscribers as f32;
    let t = &mut world.tally;
    t.subscribers = subscribers as u32;
    t.ad_revenue = ads;
    t.subscription_revenue = tier;
    t.revenue = ads + tier;
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

/// The revenue a person brought in today: the ads on the items they saw
/// while not subscribing, and the tier's daily price if they subscribe.
fn brought(state: &crate::person::PersonState, per_impression: f32, subscription: f32) -> f32 {
    per_impression * state.today.ad_items as f32 + if state.subscriber { subscription } else { 0.0 }
}

/// Fan funding: each user's share of the revenue they brought in goes to the
/// posts that gave them positive reflective value, in proportion to it.
fn pay_fans(world: &mut World, ctx: &DayCtx) {
    let design = &ctx.config.design;
    let per_impression = ad_revenue_per_impression(ctx);
    let subscription = subscription_revenue_per_user(ctx);
    for v in 0..world.population() {
        let credits = std::mem::take(&mut world.people[v].state.today.fan_credits);
        let state = &world.people[v].state;
        let brought = brought(state, per_impression, subscription);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::metrics::MetricId;
    use crate::simulation::Simulation;

    /// A small world with half the full ad load in which, at the given
    /// uptake scale and half the reference price, every user subscribes on
    /// day 0 (the day-0 value equals the hurdle, so the chance is
    /// `uptake * 0.5 * 2`).
    fn config(uptake: f32) -> Config {
        let mut config = Config::default();
        config.run.population_size = 300;
        config.design.ads_share = 0.5;
        config.world.subscription_uptake = uptake;
        config.world.subscription_price = 0.5 * REFERENCE_SUBSCRIPTION_PRICE;
        config
    }

    /// Runs two days and returns the world and the second day's metrics.
    fn second_day(config: Config) -> (World, crate::metrics::DailyMetrics) {
        let mut sim = Simulation::new(config).expect("valid");
        sim.step();
        let metrics = sim.step().expect("a second day");
        (sim.world().clone(), metrics)
    }

    #[test]
    fn subscribers_see_no_ads_earn_none_and_feel_no_annoyance() {
        let (paid, paid_m) = second_day(config(1.0));
        let (free, free_m) = second_day(config(0.0));
        let annoyance = 0.5 * Config::default().world.ad_annoyance;
        let mut subscribers = 0;
        let mut ad_items = 0;
        for (p, f) in paid.people.iter().zip(&free.people) {
            let (ps, fs) = (&p.state, &f.state);
            assert!(!fs.subscriber, "nobody subscribes at uptake 0");
            // The day-1 feeds are the same: subscribing changed nothing
            // before the second day's sessions but the ads.
            assert_eq!(ps.today.items, fs.today.items);
            assert_eq!(fs.today.ad_items, fs.today.items);
            if ps.subscriber {
                subscribers += 1;
                assert_eq!(ps.today.ad_items, 0);
                assert_eq!(ps.today.ad_annoyance, 0.0);
                let spared = annoyance * fs.today.items as f32;
                assert!((fs.today.ad_annoyance - spared).abs() < 1e-4);
                assert!((ps.today.e_ref - fs.today.e_ref - spared).abs() < 1e-3);
            } else {
                ad_items += ps.today.ad_items;
            }
        }
        assert!(
            subscribers > 100,
            "every day-0 user subscribes: {subscribers}"
        );
        assert_eq!(paid.tally.ad_impressions, ad_items);
        assert!(paid.tally.ad_impressions < paid.tally.impressions);
        assert_eq!(free.tally.ad_impressions, free.tally.impressions);
        // Ads earn only on the impressions that carried them.
        let per_impression = 0.5 * Config::default().world.ad_rate;
        assert_eq!(
            paid_m.get(MetricId::AdRevenue),
            per_impression * ad_items as f32
        );
        assert_eq!(free_m.get(MetricId::SubscriptionRevenue), 0.0);
    }

    #[test]
    fn revenue_is_ads_on_ad_impressions_plus_the_price_over_30_days() {
        let config = config(1.0);
        let price = config.world.subscription_price;
        let (world, m) = second_day(config.clone());
        let t = &world.tally;
        let subscribers = world
            .people
            .iter()
            .filter(|p| p.state.is_user && p.state.subscriber)
            .count() as f32;
        let ads = 0.5 * config.world.ad_rate * t.ad_impressions as f32;
        let tier = price / 30.0 * subscribers;
        assert_eq!(m.get(MetricId::AdRevenue), ads);
        assert_eq!(m.get(MetricId::SubscriptionRevenue), tier);
        assert_eq!(m.get(MetricId::Revenue), ads + tier);
        let dau = m.get(MetricId::ReportedDailyActives);
        assert_eq!(m.get(MetricId::SubscriberShare), subscribers / dau);
        // The price is paid whatever the ad load.
        let mut no_ads = config;
        no_ads.design.ads_share = 0.0;
        let (world, m) = second_day(no_ads);
        let subscribers = world
            .people
            .iter()
            .filter(|p| p.state.is_user && p.state.subscriber)
            .count() as f32;
        assert_eq!(m.get(MetricId::AdRevenue), 0.0);
        assert_eq!(m.get(MetricId::Revenue), price / 30.0 * subscribers);
    }

    #[test]
    fn uptake_falls_with_price_at_the_stated_elasticity() {
        let mut assume = WorldAssumptions::default();
        assert_eq!(assume.subscription_price, REFERENCE_SUBSCRIPTION_PRICE);
        assert_eq!(price_factor(&assume), 1.0);
        let value = assume.subscription_hurdle + 0.3;
        let at_reference = uptake(&assume, value, 1.0);
        for elasticity in [0.5, 1.0, 2.0] {
            assume.subscription_elasticity = elasticity;
            assume.subscription_price = 2.0 * REFERENCE_SUBSCRIPTION_PRICE;
            let doubled = uptake(&assume, value, price_factor(&assume));
            let expected = at_reference * 0.5f32.powf(elasticity);
            assert!(
                (doubled - expected).abs() < 1e-6,
                "{doubled} against {expected}"
            );
            assume.subscription_price = 0.5 * REFERENCE_SUBSCRIPTION_PRICE;
            let halved = uptake(&assume, value, price_factor(&assume));
            assert!(halved > at_reference);
        }
        // The chance never exceeds 1, even for a free tier.
        assume.subscription_uptake = 1.0;
        assume.subscription_price = 0.0;
        assert_eq!(uptake(&assume, value, price_factor(&assume)), 1.0);
    }

    #[test]
    fn the_value_counts_the_feed_without_ads_plus_the_ads_spared() {
        // 30 days: utility 3 after 1.5 of annoyance felt, 300 items, 0.01 of
        // annoyance spared per item at the current ad load.
        assert_eq!(
            subscription_value(3.0, 1.5, 300, 0.01),
            (3.0 + 1.5 + 3.0) / 30.0
        );
        // A subscriber with the same feed felt no annoyance and values it alike.
        assert_eq!(subscription_value(4.5, 0.0, 300, 0.01), (4.5 + 3.0) / 30.0);
        // Without ads, only the platform's value counts.
        assert_eq!(subscription_value(4.5, 0.0, 300, 0.0), 4.5 / 30.0);
        // A higher price draws fewer subscribers in a simulated world.
        let count = |price: f32| {
            let mut c = config(0.5);
            c.world.subscription_price = price;
            let mut sim = Simulation::new(c).expect("valid");
            sim.step();
            sim.world().tally.subscribers
        };
        let (cheap, dear) = (
            count(REFERENCE_SUBSCRIPTION_PRICE),
            count(2.0 * REFERENCE_SUBSCRIPTION_PRICE),
        );
        assert!(dear < cheap && cheap > 0, "{dear} against {cheap}");
    }

    #[test]
    fn fans_bring_their_ads_or_their_tier_payment() {
        let world = crate::init::build_world(&Config::default());
        let mut state = world.people[0].state.clone();
        state.today.items = 40;
        state.today.ad_items = 40;
        state.subscriber = false;
        assert_eq!(brought(&state, 0.001, 0.2), 0.001 * 40.0);
        state.subscriber = true;
        state.today.ad_items = 0;
        assert_eq!(brought(&state, 0.001, 0.2), 0.2);
    }
}
