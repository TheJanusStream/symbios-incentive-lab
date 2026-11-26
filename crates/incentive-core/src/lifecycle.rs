//! Joining, wellbeing, habit, stay utility and churn (docs/model.md, 7.2 to
//! 7.4).

use crate::behaviour::sig;
use crate::config::Config;
use crate::moderation;
use crate::network::{self, FollowTarget};
use crate::population::Archetype;
use crate::rng::{self, Stream};
use crate::world::{DayCtx, World};

/// Accounts a new user follows beyond its offline ties.
const NEW_USER_FOLLOWS: usize = 10;
/// Days before a former user may return, and its factor on the joining chance.
const RETURN_DELAY: u32 = 30;
const RETURN_FACTOR: f32 = 0.3;
/// Lowest reputation factor on joining.
const MIN_REPUTATION: f32 = 0.1;
/// Scales of the daily experience sums in the `tanh` saturations of 7.3. The
/// specification's text divides both by 5, which pins every engaged user at
/// the saturation; section 13 of docs/model.md records the calibration.
const IMPULSIVE_SCALE: f32 = 20.0;
const REFLECTIVE_SCALE: f32 = 10.0;
/// Daily wellbeing response to a fully positive or negative day (text: 0.05).
const WELLBEING_GAIN: f32 = 0.01;
/// Daily smoothing of recent impulsive pleasure, a mean over about a week.
const PLEASURE_SMOOTHING: f32 = 0.2;
/// Habit and stay utility of a new or returning user.
const START_HABIT: f32 = 0.1;
const START_STAY: f32 = 0.3;

/// Phase 2: non-users may join, former users may return, banned people may
/// come back with a new account.
pub(crate) fn joining(world: &mut World, ctx: &DayCtx) {
    let assume = &ctx.config.world;
    let reputation =
        (1.0 - assume.reputation_sensitivity * world.truth.toxic_reputation()).max(MIN_REPUTATION);
    let open = 1.0 - ctx.config.design.account_cost;
    let deterred = human_entry_factor(ctx.config);
    for i in 0..world.population() {
        let person = &world.people[i];
        if person.state.is_user {
            continue;
        }
        let mut rng = ctx.rng(i as u64, Stream::Joining);
        if person.state.banned {
            let factor = moderation::reentry_factor(person.traits.archetype == Archetype::Troll);
            if ctx.day >= person.state.reentry_day && rng::chance(&mut rng, factor * open) {
                reenter(world, ctx, i);
            }
            continue;
        }
        let ties = &world.ties[i];
        let users = ties
            .iter()
            .filter(|&&t| world.people[t as usize].state.is_user)
            .count();
        let share = if ties.is_empty() {
            0.0
        } else {
            users as f32 / ties.len() as f32
        };
        let mut p = assume.join_base * (0.2 + share) * reputation * deterred;
        if let Some(left) = person.state.left_day {
            if ctx.day < left + RETURN_DELAY {
                continue;
            }
            p *= RETURN_FACTOR;
        }
        if rng::chance(&mut rng, p) {
            let returning = person.state.ever_user;
            join(world, ctx, i, !returning);
        }
    }
}

/// The factor the account cost puts on human joining and returning,
/// `1 - account_cost_deterrence * account_cost`.
pub(crate) fn human_entry_factor(config: &Config) -> f32 {
    (1.0 - config.world.account_cost_deterrence * config.design.account_cost).clamp(0.0, 1.0)
}

/// Makes individual `i` a user. A first-time user follows its ties and some
/// accounts; a returning user's dormant follows wake up.
fn join(world: &mut World, ctx: &DayCtx, i: usize, fresh: bool) {
    let state = &mut world.people[i].state;
    state.is_user = true;
    state.ever_user = true;
    state.joined_day = Some(ctx.day);
    state.habit = START_HABIT;
    state.stay_ema = START_STAY;
    world.tally.new_accounts += 1;
    if fresh {
        let mut rng = ctx.rng(i as u64, Stream::Follows);
        let target = FollowTarget::Extra(NEW_USER_FOLLOWS);
        network::attach_follows(world, i, target, ctx.config.world.homophily, &mut rng);
    }
}

/// A banned person returns with a new account: no followers, no strikes, and
/// the follows of a new user.
fn reenter(world: &mut World, ctx: &DayCtx, i: usize) {
    let follows = std::mem::take(&mut world.people[i].state.follows);
    for account in follows {
        let count = &mut world.accounts.followers[account as usize];
        *count = count.saturating_sub(1);
    }
    network::reset_account(&mut world.people, &mut world.accounts, i as u32);
    let state = &mut world.people[i].state;
    state.banned = false;
    state.strikes = 0;
    state.suspended_until = 0;
    join(world, ctx, i, true);
}

/// Phase 12: author feedback, wellbeing, habit, stay utility and churn.
pub(crate) fn daily_updates(world: &mut World, ctx: &DayCtx) {
    let assume = &ctx.config.world;
    let ni = ctx.config.design.notification_intensity;
    for i in 0..world.population() {
        let opened = world.people[i].state.today.opened;
        let state = &mut world.people[i].state;
        state.active_bits = (state.active_bits << 1) | u64::from(opened);
        if !state.is_user {
            continue;
        }
        let mutuals = if opened {
            active_mutual_follows(world, i)
        } else {
            0
        };
        let person = &mut world.people[i];
        let (traits, state) = (&person.traits, &mut person.state);
        let t = &mut state.today;
        let likes = (t.likes_received as f32).ln_1p();
        t.e_imp += 0.1 * traits.approval_sensitivity * likes;
        t.e_ref += 0.02 * likes + (0.1 * t.mutual_replies as f32).min(1.0)
            - 0.05 * (0.5 + traits.vulnerability) * t.angry_received as f32
            + traits.money_weight * t.income / assume.living_income;
        let (imp, refl) = (
            (t.e_imp / IMPULSIVE_SCALE).tanh(),
            (t.e_ref / REFLECTIVE_SCALE).tanh(),
        );
        let experienced = opened || t.harassment > 0;
        let recovery = 0.02 * (traits.wellbeing_baseline - state.wellbeing);
        let experience = if experienced {
            WELLBEING_GAIN * refl
        } else {
            0.0
        };
        state.wellbeing = (state.wellbeing + experience + recovery).clamp(0.0, 1.0);
        let decay = 0.05 * state.habit;
        if opened {
            state.habit += assume.habit_gain * (1.0 + 0.5 * ni) * imp;
            let utility =
                0.4 * imp + 0.6 * refl + 0.3 * (mutuals as f32 / 10.0).min(1.0) + 0.5 * state.habit
                    - 0.5 * state.today.harassment as f32;
            state.stay_ema = 0.9 * state.stay_ema + 0.1 * utility;
        }
        state.habit = (state.habit - decay).clamp(0.0, 1.0);
        // Recent pleasure, for the survey (9.1): the day's squashed impulsive
        // experience, zero on a day without a session, smoothed over days.
        let felt = if opened { imp.clamp(0.0, 1.0) } else { 0.0 };
        state.pleasure =
            (state.pleasure + PLEASURE_SMOOTHING * (felt - state.pleasure)).clamp(0.0, 1.0);
        state.eref_window += state.today.e_ref;
        let leave = assume.churn_base * sig(8.0 * (assume.leave_threshold - state.stay_ema))
            + assume.harassment_exit * state.today.harassment as f32 * traits.vulnerability;
        let mut rng = ctx.rng(i as u64, Stream::Churn);
        if rng::chance(&mut rng, leave) {
            state.is_user = false;
            state.subscriber = false;
            state.left_day = Some(ctx.day);
            world.tally.departures += 1;
        }
    }
}

/// Followed accounts who are users and follow back.
fn active_mutual_follows(world: &World, i: usize) -> usize {
    let me = i as u32;
    world.people[i]
        .state
        .follows
        .iter()
        .filter_map(|&a| world.people.get(a as usize))
        .filter(|other| other.state.is_user && other.follows(me))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init;

    fn joins(account_cost: f32) -> u32 {
        let mut config = Config::default();
        config.run.population_size = 400;
        config.run.initial_user_share = 0.1;
        config.world.join_base = 0.02;
        config.world.account_cost_deterrence = 0.8;
        config.design.account_cost = account_cost;
        let mut world = init::build_world(&config);
        let mut total = 0;
        for day in 0..20 {
            world.tally.new_accounts = 0;
            let ctx = DayCtx {
                config: &config,
                day,
            };
            joining(&mut world, &ctx);
            total += world.tally.new_accounts;
        }
        total
    }

    #[test]
    fn account_cost_lowers_human_joining() {
        let mut config = Config::default();
        config.world.account_cost_deterrence = 0.5;
        config.design.account_cost = 1.0;
        assert!((human_entry_factor(&config) - 0.5).abs() < 1e-6);
        config.design.account_cost = 0.0;
        assert_eq!(human_entry_factor(&config), 1.0);
        let (free, costly) = (joins(0.0), joins(1.0));
        assert!(
            free > 20 && (costly as f32) < 0.5 * free as f32,
            "{free} {costly}"
        );
    }
}
