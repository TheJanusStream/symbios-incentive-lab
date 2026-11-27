//! Bot accounts run by engagement farms (docs/model.md, 3.5).
//!
//! Bots have no experience, wellbeing or learning. Their engagement is
//! observable and feeds ranking, but truth metrics leave it out.

use crate::config::{BotStrategy, Config, DesignLevers, PayoutScheme, PopulationSettings};
use crate::content::{AuthorProfile, Group, NEUTRAL_INTENSITY, Style, TOPIC_COUNT, generate_post};
use crate::network::{self, Reshare};
use crate::ranking::MeanWeights;
use crate::rng::{self, Stream};
use crate::world::{DayCtx, World};

/// Days a banned bot's slot stays empty before it can be refilled.
const REPLACEMENT_DELAY: u32 = 3;
/// Writing skill of bot posts.
const BOT_SKILL: f32 = 0.3;
/// Temperament of bot posts.
const BOT_TEMPERAMENT: f32 = 0.5;

/// One bot slot. A slot is filled by one bot account at a time.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Bot {
    pub group: Group,
    pub active: bool,
    pub banned: bool,
    pub banned_day: u32,
    pub strikes: u32,
    pub suspended_until: u32,
    pub income_total: f32,
    /// Posts the slot's accounts wrote over the run; the next post's ordinal.
    pub posts_written: u32,
}

/// Slots allocated for bots: the most bots the population could ever have.
pub(crate) fn slot_count(population: &PopulationSettings, n: usize) -> usize {
    (population.bot_share * n as f32).round() as usize
}

/// Share of the bot slots the current account cost lets in: farms that buy
/// verified accounts keep `bot_farm_floor` of them however costly accounts are.
pub(crate) fn open_share(config: &Config) -> f32 {
    let floor = config.world.bot_farm_floor;
    let open = 1.0 - config.design.account_cost;
    floor + (1.0 - floor) * open * open
}

/// Daily chance that an empty slot within the target is refilled; it never
/// falls below the farm floor either.
fn refill_chance(config: &Config) -> f32 {
    let floor = config.world.bot_farm_floor;
    floor + (1.0 - floor) * (1.0 - config.design.account_cost)
}

/// Bots the current account cost lets in.
pub(crate) fn target_count(config: &Config, n: usize) -> usize {
    (config.population.bot_share * n as f32 * open_share(config)).round() as usize
}

/// Creates the bot slots; the first `target_count` start active.
pub(crate) fn create(config: &Config, n: usize) -> Vec<Bot> {
    let target = target_count(config, n);
    (0..slot_count(&config.population, n))
        .map(|slot| {
            let mut rng = rng::stream_rng(config.run.seed, 0, slot as u64, Stream::Bots);
            Bot {
                group: if rng::chance(&mut rng, 0.5) {
                    Group::A
                } else {
                    Group::B
                },
                active: slot < target,
                banned: false,
                banned_day: 0,
                strikes: 0,
                suspended_until: 0,
                income_total: 0.0,
                posts_written: 0,
            }
        })
        .collect()
}

/// Phase 1: slots beyond the account cost's target go dormant; empty slots
/// within it are refilled with probability
/// `floor + (1 - floor) * (1 - account_cost)` per day.
pub(crate) fn replace_banned(world: &mut World, ctx: &DayCtx) {
    let target = target_count(ctx.config, world.population());
    let open = refill_chance(ctx.config);
    for slot in 0..world.bots.len() {
        let bot = &mut world.bots[slot];
        if slot >= target {
            bot.active = false;
            continue;
        }
        if bot.active || (bot.banned && ctx.day < bot.banned_day + REPLACEMENT_DELAY) {
            continue;
        }
        let mut rng = ctx.rng(slot as u64, Stream::Bots);
        if rng::chance(&mut rng, open) {
            *bot = Bot {
                active: true,
                banned: false,
                strikes: 0,
                suspended_until: 0,
                ..bot.clone()
            };
            let account = world.bot_account(slot);
            network::reset_account(&mut world.people, &mut world.accounts, account);
            world.tally.new_accounts += 1;
        }
    }
}

/// Bots write their posts (part of phase 3). Adaptive bots polish them when
/// the design reads the quality signal, other bots never do; each post draws
/// from its own stream, keyed on the bot account and its post ordinal. A
/// nonzero `population.bot_truth_shift` moves each post's truth after its
/// draws (docs/model.md, 3.5).
pub(crate) fn write_posts(world: &mut World, ctx: &DayCtx) {
    let uniform_topics = [1.0 / TOPIC_COUNT as f32; TOPIC_COUNT];
    let pop = &ctx.config.population;
    let polished = polishes(ctx.config);
    for slot in 0..world.bots.len() {
        let bot = &world.bots[slot];
        if !bot.active || ctx.day < bot.suspended_until {
            continue;
        }
        let account = world.bot_account(slot);
        let first = bot.posts_written;
        let mut author = AuthorProfile {
            account,
            is_bot: true,
            is_creator: false,
            group: bot.group,
            skill: BOT_SKILL,
            temperament: BOT_TEMPERAMENT,
            evasion: 0.0,
            written_evasion: 0.0,
            veil: 0.0,
            intensity: NEUTRAL_INTENSITY,
            polished,
            ordinal: first,
        };
        for k in 0..pop.bot_posts_per_day {
            author.ordinal = first + k;
            let mut rng = rng::post_rng(ctx.config.run.seed, account, author.ordinal);
            let style = if rng::chance(&mut rng, pop.bot_clickbait_share) {
                Style::Clickbait
            } else {
                Style::Outrage
            };
            let id = world.posts.next_id();
            let mut post = generate_post(
                &mut rng,
                id,
                ctx.day,
                &author,
                style,
                &uniform_topics,
                &ctx.config.world,
            );
            // A rule, not a draw: at the default of 0 the post is untouched.
            if pop.bot_truth_shift != 0.0 {
                post.truth = (post.truth + pop.bot_truth_shift).clamp(0.0, 1.0);
            }
            world.posts.push(post);
            world.accounts.recent_posts[account as usize].push(id);
            world.tally.bot_posts += 1;
        }
        world.bots[slot].posts_written += pop.bot_posts_per_day;
    }
}

/// Reports and see-less marks one adaptive bot files per day at
/// `bot_brigading` 1, of each kind (docs/model.md, 3.5).
pub(crate) const BRIGADE_BASE: f32 = 3.0;
/// Popular posts per group that brigading bots aim at.
pub(crate) const BRIGADE_TARGETS: usize = 20;

/// What a bot boosts from: the shared pool of bot posts and Clickbait posts,
/// and each group's own part of it (bot posts of the group and Clickbait
/// posts by its human authors).
struct BoostPools {
    shared: Vec<u32>,
    own: [Vec<u32>; 2],
}

impl BoostPools {
    fn build(world: &World) -> Self {
        let mut pools = Self {
            shared: Vec::new(),
            own: [Vec::new(), Vec::new()],
        };
        for p in world.posts.iter() {
            if p.removed || !(p.author_is_bot || p.style == Style::Clickbait) {
                continue;
            }
            pools.shared.push(p.id);
            pools.own[p.group.index()].push(p.id);
        }
        pools
    }
}

/// How an adaptive bot splits its daily boosts: own-side likes and shares
/// against cross-group coordination, in proportion to the ranking weights
/// of likes and shares against bridging. Returns the cross-group boosts.
pub(crate) fn cross_group_boosts(design: &DesignLevers, boosts: u32) -> u32 {
    let w = MeanWeights::of(design);
    let total = w.like + w.share + w.bridging;
    if total <= 0.0 {
        return 0;
    }
    (boosts as f32 * w.bridging / total).round() as u32
}

/// Whether adaptive bots polish their posts: when the ranking or the
/// payouts read the quality classifier's score.
pub(crate) fn polishes(config: &Config) -> bool {
    if config.world.bot_strategy != BotStrategy::Adaptive {
        return false;
    }
    let d = &config.design;
    MeanWeights::of(d).quality > 0.0
        || (d.payout_share > 0.0 && d.payout_scheme == PayoutScheme::QualityWeighted)
}

/// Phase 7: each active bot that is not suspended likes and shares posts by
/// the world's bot strategy (3.5); adaptive bots then brigade.
pub(crate) fn boost(world: &mut World, ctx: &DayCtx) {
    let pools = BoostPools::build(world);
    let strategy = ctx.config.world.bot_strategy;
    let boosts = ctx.config.population.bot_boosts;
    let cross = match strategy {
        BotStrategy::SharedPool => boosts,
        BotStrategy::OwnSide => 0,
        BotStrategy::Adaptive => cross_group_boosts(&ctx.config.design, boosts),
    };
    for slot in 0..world.bots.len() {
        let bot = &world.bots[slot];
        if !bot.active || ctx.day < bot.suspended_until {
            continue;
        }
        let group = bot.group;
        let account = world.bot_account(slot);
        let mut rng = ctx.rng(u64::from(account), Stream::BotBoosts);
        for k in 0..boosts {
            let pool = if k < boosts - cross {
                &pools.own[group.index()]
            } else {
                &pools.shared
            };
            if pool.is_empty() {
                continue;
            }
            let id = pool[rng::index(&mut rng, pool.len())];
            apply_boost(world, ctx, account, group, id);
        }
    }
    if strategy == BotStrategy::Adaptive && ctx.config.world.bot_brigading > 0.0 {
        brigade(world, ctx);
    }
}

/// One like and one share by a bot account.
fn apply_boost(world: &mut World, ctx: &DayCtx, account: u32, group: Group, id: u32) {
    let Some(post) = world.posts.get_mut(id) else {
        return;
    };
    if post.author == account {
        return;
    }
    match group {
        Group::A => post.likes_a += 1,
        Group::B => post.likes_b += 1,
    }
    post.bot_likes += 1;
    post.likes_today += 1;
    post.shares += 1;
    post.bot_shares += 1;
    post.engagement_today += 2;
    let style = post.style.index();
    let t = &mut world.tally;
    t.likes += 1;
    t.shares += 1;
    t.bot_likes += 1;
    t.bot_shares += 1;
    t.bot_likes_by_style[style] += 1;
    t.bot_shares_by_style[style] += 1;
    world.accounts.reshares[account as usize].push(Reshare {
        post: id,
        depth: 1,
        day: ctx.day,
    });
}

/// Each group's most popular live posts by human authors not yet reviewed,
/// by observed engagement (ties to the older post): what the other group's
/// bots brigade. A reviewed post cannot return to the review queue, so
/// reporting it would be wasted.
fn brigade_targets(world: &World) -> [Vec<u32>; 2] {
    let mut ranked: [Vec<(u32, u32)>; 2] = [Vec::new(), Vec::new()];
    for p in world.posts.iter() {
        if p.removed || p.reviewed || p.author_is_bot || p.impressions == 0 {
            continue;
        }
        ranked[p.group.index()].push((p.observed_engagement(), p.id));
    }
    ranked.map(|mut list| {
        list.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        list.truncate(BRIGADE_TARGETS);
        list.into_iter().map(|(_, id)| id).collect()
    })
}

/// Adaptive bots brigade (3.5): each files reports, when reviewers read
/// them, and see-less marks, when the ranking reads them, against distinct
/// popular posts of the other group's human authors, `bot_brigading *
/// BRIGADE_BASE` of each a day (the fraction is a chance of one more). A bot
/// reports or marks a post once at most over the run: it picks among the
/// targets it has not yet hit with that kind. Reports join the review queue
/// like anyone's; marks count in the authors' see-less rates.
fn brigade(world: &mut World, ctx: &DayCtx) {
    let targets = brigade_targets(world);
    let amount = ctx.config.world.bot_brigading * BRIGADE_BASE;
    let reports_read = ctx.config.design.review_capacity > 0.0;
    let marks_read = MeanWeights::of(&ctx.config.design).see_less > 0.0;
    if !reports_read && !marks_read {
        return;
    }
    let mut pool = Vec::new();
    let mut picks = Vec::new();
    for slot in 0..world.bots.len() {
        let bot = &world.bots[slot];
        if !bot.active || ctx.day < bot.suspended_until {
            continue;
        }
        let targets = &targets[bot.group.other().index()];
        if targets.is_empty() {
            continue;
        }
        let account = world.bot_account(slot);
        for (kind, read) in [(0, reports_read), (1, marks_read)] {
            if !read {
                continue;
            }
            // One stream per bot and kind, so switching one kind off leaves
            // the other's draws alone.
            let mut rng = ctx.rng((u64::from(account) << 1) | kind, Stream::Brigade);
            let count =
                amount.floor() as usize + usize::from(rng::chance(&mut rng, amount.fract()));
            pool.clear();
            pool.extend(targets.iter().copied().filter(|&id| {
                world.posts.get(id).is_some_and(|p| {
                    let done = if kind == 0 {
                        &p.reporters
                    } else {
                        &p.bot_markers
                    };
                    !done.contains(&account)
                })
            }));
            distinct_picks(&mut rng, &pool, count, &mut picks);
            for &id in &picks {
                let Some(post) = world.posts.get_mut(id) else {
                    continue;
                };
                if kind == 0 {
                    if post.add_report(account) {
                        world.tally.reports += 1;
                        world.tally.bot_reports += 1;
                    }
                } else if post.add_bot_mark(account) {
                    world.accounts.see_less_today[post.author as usize] += 1;
                    world.tally.see_less += 1;
                    world.tally.bot_see_less += 1;
                }
            }
        }
    }
}

/// Up to `count` distinct entries of `pool`, drawn uniformly, into `out`.
fn distinct_picks(rng: &mut crate::rng::StreamRng, pool: &[u32], count: usize, out: &mut Vec<u32>) {
    out.clear();
    let count = count.min(pool.len());
    while out.len() < count {
        let id = pool[rng::index(rng, pool.len())];
        if !out.contains(&id) {
            out.push(id);
        }
    }
}

/// A strike against a bot; returns whether the bot was banned.
pub(crate) fn strike(bot: &mut Bot, ctx: &DayCtx) -> bool {
    use crate::config::Sanctions;
    bot.strikes += 1;
    let ban = match ctx.config.design.sanctions {
        Sanctions::None => false,
        Sanctions::Strict => true,
        Sanctions::Graduated => {
            match bot.strikes {
                2 => bot.suspended_until = ctx.day + 1 + 3,
                3 => bot.suspended_until = ctx.day + 1 + 14,
                _ => {}
            }
            bot.strikes >= 4
        }
    };
    if ban {
        bot.active = false;
        bot.banned = true;
        bot.banned_day = ctx.day;
    }
    ban
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bots_never_fall_below_the_farm_floor() {
        let mut config = Config::default();
        config.world.bot_farm_floor = 0.2;
        let n = 2_000;
        let slots = slot_count(&config.population, n) as f32;
        for step in 0..=20 {
            config.design.account_cost = step as f32 / 20.0;
            let target = target_count(&config, n) as f32;
            assert!(target >= (0.2 * slots).round(), "{target}");
            assert!(refill_chance(&config) >= 0.2);
        }
        config.design.account_cost = 1.0;
        assert_eq!(target_count(&config, n), (0.2 * slots).round() as usize);
        config.design.account_cost = 0.0;
        assert_eq!(target_count(&config, n), slots as usize);
        config.world.bot_farm_floor = 0.0;
        config.design.account_cost = 1.0;
        assert_eq!(target_count(&config, n), 0);
    }

    #[test]
    fn suspended_bots_neither_post_nor_boost() {
        let mut config = Config::default();
        config.run.population_size = 400;
        let mut world = crate::init::build_world(&config);
        let ctx = DayCtx {
            config: &config,
            day: 5,
        };
        let active: Vec<usize> = (0..world.bots.len())
            .filter(|&s| world.bots[s].active)
            .collect();
        assert!(active.len() >= 2);
        write_posts(&mut world, &ctx);
        let boosts_with_all = {
            let mut w = world.clone();
            boost(&mut w, &ctx);
            w.tally.bot_likes
        };
        for &slot in &active {
            world.bots[slot].suspended_until = 6;
        }
        boost(&mut world, &ctx);
        assert!(boosts_with_all > 0);
        assert_eq!(world.tally.bot_likes, 0);
        // Bot posts count apart from the human content mix.
        assert!(world.tally.bot_posts > 0);
        assert_eq!(world.tally.posts_by_style.iter().sum::<u32>(), 0);
    }

    /// A small world on day 5 after everyone posted, with every post seen
    /// a few times, under `config`.
    fn posted_world(config: &Config) -> World {
        let mut world = crate::init::build_world(config);
        for person in &mut world.people {
            person.state.today.opened = person.state.is_user;
        }
        let ctx = DayCtx { config, day: 5 };
        crate::posting::write_posts(&mut world, &ctx);
        for post in world.posts.iter_mut() {
            post.impressions = 5 + post.id % 11;
        }
        world
    }

    fn small(strategy: BotStrategy) -> Config {
        let mut config = Config::default();
        config.run.population_size = 600;
        config.world.bot_strategy = strategy;
        config
    }

    /// The boosting of the shared-pool strategy as it was before the
    /// strategies existed.
    fn old_boost(world: &mut World, ctx: &DayCtx) {
        let boosted: Vec<u32> = world
            .posts
            .iter()
            .filter(|p| !p.removed && (p.author_is_bot || p.style == Style::Clickbait))
            .map(|p| p.id)
            .collect();
        for slot in 0..world.bots.len() {
            let bot = &world.bots[slot];
            if !bot.active || ctx.day < bot.suspended_until {
                continue;
            }
            let group = bot.group;
            let account = world.bot_account(slot);
            let mut rng = ctx.rng(u64::from(account), Stream::BotBoosts);
            for _ in 0..ctx.config.population.bot_boosts {
                let id = boosted[rng::index(&mut rng, boosted.len())];
                let Some(post) = world.posts.get_mut(id) else {
                    continue;
                };
                if post.author == account {
                    continue;
                }
                match group {
                    Group::A => post.likes_a += 1,
                    Group::B => post.likes_b += 1,
                }
                post.bot_likes += 1;
                post.shares += 1;
                post.bot_shares += 1;
            }
        }
    }

    #[test]
    fn shared_pool_reproduces_the_old_boosting() {
        let config = small(BotStrategy::SharedPool);
        let ctx = DayCtx {
            config: &config,
            day: 5,
        };
        let mut new = posted_world(&config);
        let mut old = new.clone();
        boost(&mut new, &ctx);
        old_boost(&mut old, &ctx);
        let counts = |w: &World| {
            w.posts
                .iter()
                .map(|p| (p.likes_a, p.likes_b, p.shares, p.bot_likes, p.reports))
                .collect::<Vec<_>>()
        };
        assert!(new.tally.bot_likes > 100);
        assert_eq!(counts(&new), counts(&old));
        assert_eq!(new.tally.bot_reports + new.tally.bot_see_less, 0);
    }

    #[test]
    fn own_side_bots_boost_only_their_own_group() {
        let config = small(BotStrategy::OwnSide);
        let ctx = DayCtx {
            config: &config,
            day: 5,
        };
        let mut world = posted_world(&config);
        boost(&mut world, &ctx);
        assert!(world.tally.bot_likes > 100);
        let mut human_clickbait = 0;
        for post in world.posts.iter() {
            let (own, other) = match post.group {
                Group::A => (post.likes_a, post.likes_b),
                Group::B => (post.likes_b, post.likes_a),
            };
            assert_eq!(other, 0, "post {} got likes from the other side", post.id);
            if own > 0 {
                assert!(post.author_is_bot || post.style == Style::Clickbait);
                human_clickbait += u32::from(!post.author_is_bot);
            }
        }
        assert!(human_clickbait > 0);
        assert_eq!(world.tally.bot_reports, 0);
    }

    #[test]
    fn adaptive_bots_follow_the_design() {
        let engagement = small(BotStrategy::Adaptive);
        let mut bridging = engagement.clone();
        if let Ok(preset) = crate::presets::preset("quality-bridging") {
            preset.apply_to(&mut bridging);
        }
        // The engagement ranking weighs no bridging and no quality signal.
        assert_eq!(cross_group_boosts(&engagement.design, 20), 0);
        assert!(!polishes(&engagement));
        // Quality and bridging: likes 0.5 and shares 0.5 against bridging 1.
        assert_eq!(cross_group_boosts(&bridging.design, 20), 10);
        assert!(polishes(&bridging));
        let mut chronological = bridging.clone();
        chronological.design.chronological = true;
        assert_eq!(cross_group_boosts(&chronological.design, 20), 0);
        assert!(!polishes(&chronological));
        // Other strategies never polish.
        bridging.world.bot_strategy = BotStrategy::OwnSide;
        assert!(!polishes(&bridging));
        bridging.world.bot_strategy = BotStrategy::Adaptive;

        // Polished bot posts score the polish gain on the classifier.
        let mut world = crate::init::build_world(&bridging);
        let ctx = DayCtx {
            config: &bridging,
            day: 0,
        };
        write_posts(&mut world, &ctx);
        assert!(world.posts.iter().all(|p| !p.author_is_bot || p.polished));
        let mut plain = crate::init::build_world(&engagement);
        write_posts(
            &mut plain,
            &DayCtx {
                config: &engagement,
                day: 0,
            },
        );
        assert!(plain.posts.iter().all(|p| !p.polished));
    }

    /// Runs the boost phase on a posted world in which every bot belongs to
    /// group A; returns the world.
    fn brigaded(config: &Config) -> World {
        let ctx = DayCtx { config, day: 5 };
        let mut world = posted_world(config);
        for bot in &mut world.bots {
            bot.group = Group::A;
        }
        boost(&mut world, &ctx);
        world
    }

    #[test]
    fn adaptive_bots_brigade_the_other_side_when_it_pays() {
        // Engagement ranking: reviewers read reports, the ranking ignores
        // see-less marks.
        let mut config = small(BotStrategy::Adaptive);
        let world = brigaded(&config);
        let active = world.bots.iter().filter(|b| b.active).count() as u32;
        assert_eq!(world.tally.bot_reports, active * BRIGADE_BASE as u32);
        assert_eq!(world.tally.bot_see_less, 0);
        let mut reported = 0;
        for post in world.posts.iter().filter(|p| p.reports > 0) {
            assert!(!post.author_is_bot && post.group == Group::B);
            reported += 1;
        }
        assert!(reported > 0 && reported <= BRIGADE_TARGETS);

        // A see-less penalty draws marks against the targeted authors.
        config.design.w_see_less = 1.0;
        let world = brigaded(&config);
        assert_eq!(world.tally.bot_see_less, active * BRIGADE_BASE as u32);
        let mut marked = 0;
        for post in world.posts.iter().filter(|p| p.see_less > 0) {
            assert!(!post.author_is_bot && post.group == Group::B);
            assert!(world.accounts.see_less_today[post.author as usize] >= post.see_less);
            marked += 1;
        }
        assert!(marked > 0);
        let mut accounts = world.accounts.clone();
        let targeted = world
            .posts
            .iter()
            .find(|p| p.see_less > 0)
            .map_or(0, |p| p.author as usize);
        accounts.impressions_today[targeted] = 10;
        accounts.end_day();
        assert!(accounts.see_less_rate[targeted] > 0.0);

        // Brigading scales with the assumption and stops at zero.
        config.world.bot_brigading = 2.0;
        assert_eq!(
            brigaded(&config).tally.bot_reports,
            2 * active * BRIGADE_BASE as u32
        );
        config.world.bot_brigading = 0.0;
        let world = brigaded(&config);
        assert_eq!(world.tally.bot_reports + world.tally.bot_see_less, 0);

        // Without reviewers or a see-less penalty there is nothing to game.
        config.world.bot_brigading = 1.0;
        config.design.w_see_less = 0.0;
        config.design.review_capacity = 0.0;
        let world = brigaded(&config);
        assert_eq!(world.tally.bot_reports + world.tally.bot_see_less, 0);
    }

    #[test]
    fn a_bot_never_reports_or_marks_a_post_twice() {
        let mut config = small(BotStrategy::Adaptive);
        config.design.w_see_less = 1.0;
        let mut world = posted_world(&config);
        for bot in &mut world.bots {
            bot.group = Group::A;
        }
        // Ten days of brigading against the same targets, with nobody
        // reviewing in between: 3 a day would exceed the 20 targets.
        for day in 5..15 {
            boost(
                &mut world,
                &DayCtx {
                    config: &config,
                    day,
                },
            );
        }
        let accounts: Vec<u32> = (0..world.bots.len())
            .filter(|&s| world.bots[s].active)
            .map(|s| world.bot_account(s))
            .collect();
        let (mut reports, mut marks) = (0, 0);
        for post in world.posts.iter() {
            for list in [&post.reporters, &post.bot_markers] {
                let mut sorted = list.clone();
                sorted.sort_unstable();
                sorted.dedup();
                assert_eq!(sorted.len(), list.len(), "post {} hit twice", post.id);
            }
            assert_eq!(post.reports as usize, post.reporters.len());
            reports += post
                .reporters
                .iter()
                .filter(|a| accounts.contains(a))
                .count();
            marks += post.bot_markers.len();
        }
        assert_eq!(reports as u32, world.tally.bot_reports);
        assert_eq!(marks as u32, world.tally.bot_see_less);
        assert!(world.tally.bot_reports <= accounts.len() as u32 * BRIGADE_TARGETS as u32);
        assert!(world.tally.bot_reports > accounts.len() as u32 * BRIGADE_BASE as u32);
    }

    #[test]
    fn reviewed_posts_are_not_brigade_targets() {
        let config = small(BotStrategy::Adaptive);
        let mut world = posted_world(&config);
        let before = brigade_targets(&world);
        assert!(!before[0].is_empty() && !before[1].is_empty());
        let reviewed: Vec<u32> = before.iter().map(|t| t[0]).collect();
        for &id in &reviewed {
            if let Some(p) = world.posts.get_mut(id) {
                p.reviewed = true;
            }
        }
        let after = brigade_targets(&world);
        for (group, targets) in after.iter().enumerate() {
            assert!(!targets.contains(&reviewed[group]));
            assert!(
                targets
                    .iter()
                    .all(|&id| world.posts.get(id).is_some_and(|p| !p.reviewed))
            );
            // The rest move up in the same order.
            let kept = before[group].len() - 1;
            assert_eq!(targets[..kept], before[group][1..]);
        }
    }

    #[test]
    fn brigade_reports_lose_weight_under_report_trust() {
        let mut config = small(BotStrategy::Adaptive);
        config.design.review_capacity = 1000.0;
        config.design.report_trust = 1.0;
        let mut world = brigaded(&config);
        let ctx = DayCtx {
            config: &config,
            day: 5,
        };
        crate::moderation::human_review(&mut world, &ctx);
        let bots: Vec<u32> = (0..world.bots.len())
            .map(|s| world.bot_account(s))
            .filter(|&a| world.accounts.reports_reviewed[a as usize] > 0)
            .collect();
        assert!(!bots.is_empty());
        // The brigaded posts were mostly harmless: the bots' records fall
        // below the 0.5 of an account without judged reports.
        let reviewed: u32 = bots
            .iter()
            .map(|&a| world.accounts.reports_reviewed[a as usize])
            .sum();
        let upheld: u32 = bots
            .iter()
            .map(|&a| world.accounts.reports_upheld[a as usize])
            .sum();
        assert!(2 * upheld < reviewed, "{upheld} of {reviewed} upheld");
        let mean_rate = bots
            .iter()
            .map(|&a| world.accounts.upheld_rate(a))
            .sum::<f32>()
            / bots.len() as f32;
        assert!(mean_rate < 0.5, "{mean_rate}");
        // A new post reported by such a bot now ranks below the same post
        // reported by a person without a record.
        let human = 1;
        let bot = bots
            .iter()
            .copied()
            .min_by(|&a, &b| {
                world
                    .accounts
                    .upheld_rate(a)
                    .total_cmp(&world.accounts.upheld_rate(b))
            })
            .unwrap_or(bots[0]);
        let first = world.posts.next_id();
        let ids: Vec<u32> = (0..2)
            .map(|_| {
                let mut post = world.posts.get(first - 1).cloned().expect("a post");
                post.id = world.posts.next_id();
                post.reporters.clear();
                post.reports = 0;
                post.reviewed = false;
                post.removed = false;
                post.impressions = 10;
                let id = post.id;
                world.posts.push(post);
                id
            })
            .collect();
        for (&id, reporter) in ids.iter().zip([bot, human]) {
            if let Some(p) = world.posts.get_mut(id) {
                p.add_report(reporter);
            }
        }
        let queue = crate::moderation::review_queue(&world, 1.0);
        let pos = |id: u32| queue.iter().position(|q| q.1 == id);
        assert!(pos(ids[1]) < pos(ids[0]));
        let plain = crate::moderation::review_queue(&world, 0.0);
        let pos = |id: u32| plain.iter().position(|q| q.1 == id);
        assert!(pos(ids[0]) < pos(ids[1]));
    }

    #[test]
    fn brigade_reports_reach_the_review_queue() {
        let mut config = small(BotStrategy::Adaptive);
        config.design.review_capacity = 1000.0;
        let mut world = brigaded(&config);
        let reported: Vec<u32> = world
            .posts
            .iter()
            .filter(|p| p.reports > 0)
            .map(|p| p.id)
            .collect();
        assert!(!reported.is_empty());
        let ctx = DayCtx {
            config: &config,
            day: 5,
        };
        crate::moderation::human_review(&mut world, &ctx);
        for id in reported {
            assert!(world.posts.get(id).is_some_and(|p| p.reviewed));
        }
    }
}
