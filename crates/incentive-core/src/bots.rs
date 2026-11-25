//! Bot accounts run by engagement farms (docs/model.md, 3.5).
//!
//! Bots have no experience, wellbeing or learning. Their engagement is
//! observable and feeds ranking, but truth metrics leave it out.

use crate::config::{Config, PopulationSettings};
use crate::content::{AuthorProfile, Group, Style, TOPIC_COUNT, generate_post};
use crate::network::{self, Reshare};
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
}

/// Slots allocated for bots: the most bots the population could ever have.
pub(crate) fn slot_count(population: &PopulationSettings, n: usize) -> usize {
    (population.bot_share * n as f32).round() as usize
}

/// Bots the current account cost lets in.
pub(crate) fn target_count(config: &Config, n: usize) -> usize {
    let open = 1.0 - config.design.account_cost;
    (config.population.bot_share * n as f32 * open * open).round() as usize
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
            }
        })
        .collect()
}

/// Phase 1: slots beyond the account cost's target go dormant; empty slots
/// within it are refilled with probability `1 - account_cost` per day.
pub(crate) fn replace_banned(world: &mut World, ctx: &DayCtx) {
    let target = target_count(ctx.config, world.population());
    let open = 1.0 - ctx.config.design.account_cost;
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

/// Bots write their posts (part of phase 3).
pub(crate) fn write_posts(world: &mut World, ctx: &DayCtx) {
    let uniform_topics = [1.0 / TOPIC_COUNT as f32; TOPIC_COUNT];
    let pop = &ctx.config.population;
    for slot in 0..world.bots.len() {
        let bot = &world.bots[slot];
        if !bot.active || ctx.day < bot.suspended_until {
            continue;
        }
        let account = world.bot_account(slot);
        let author = AuthorProfile {
            account,
            is_bot: true,
            is_creator: false,
            group: bot.group,
            skill: BOT_SKILL,
            temperament: BOT_TEMPERAMENT,
            evasion: 0.0,
        };
        let mut rng = ctx.rng(u64::from(account), Stream::Posting);
        for _ in 0..pop.bot_posts_per_day {
            let style = if rng::chance(&mut rng, pop.bot_clickbait_share) {
                Style::Clickbait
            } else {
                Style::Outrage
            };
            let id = world.posts.next_id();
            let post = generate_post(&mut rng, id, ctx.day, &author, style, &uniform_topics);
            world.posts.push(post);
            world.accounts.recent_posts[account as usize].push(id);
            world.tally.posts_by_style[style.index()] += 1;
        }
    }
}

/// Phase 7: each active bot likes and shares random recent posts by bots or
/// Clickbait posts.
pub(crate) fn boost(world: &mut World, ctx: &DayCtx) {
    let boosted: Vec<u32> = world
        .posts
        .iter()
        .filter(|p| !p.removed && (p.author_is_bot || p.style == Style::Clickbait))
        .map(|p| p.id)
        .collect();
    if boosted.is_empty() {
        return;
    }
    for slot in 0..world.bots.len() {
        let bot = &world.bots[slot];
        if !bot.active {
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
            post.likes_today += 1;
            post.shares += 1;
            post.bot_shares += 1;
            post.engagement_today += 2;
            world.tally.likes += 1;
            world.tally.shares += 1;
            world.tally.bot_likes += 1;
            world.tally.bot_shares += 1;
            world.accounts.reshares[account as usize].push(Reshare {
                post: id,
                depth: 1,
                day: ctx.day,
            });
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
