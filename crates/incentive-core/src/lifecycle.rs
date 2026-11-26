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
/// Share of the gap to the wellbeing baseline that closes each day, for
/// users and non-users alike (docs/model.md, 7.3).
const WELLBEING_RECOVERY: f32 = 0.02;
/// Daily smoothing of recent impulsive pleasure, a mean over about a week.
const PLEASURE_SMOOTHING: f32 = 0.2;
/// Habit and stay utility of a new or returning user.
const START_HABIT: f32 = 0.1;
/// How much notifications at full intensity raise the habit gain,
/// `habit_gain * (1 + NOTIFICATION_HABIT_GAIN * notification_intensity)`
/// (docs/model.md, 7.3 and 13.1).
const NOTIFICATION_HABIT_GAIN: f32 = 0.5;
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

/// The daily pull of wellbeing toward its baseline,
/// `0.02 * (baseline - wellbeing)` (docs/model.md, 7.3).
fn recovery(wellbeing: f32, baseline: f32) -> f32 {
    WELLBEING_RECOVERY * (baseline - wellbeing)
}

/// Phase 12: author feedback, wellbeing, habit, stay utility and churn.
/// Wellbeing and stay utility read the day's reflective utility net of the
/// time cost and the interruption cost; the subscription window gathers it
/// before both. Non-users (former users, banned people and people who never
/// joined) only recover toward their wellbeing baseline.
pub(crate) fn daily_updates(world: &mut World, ctx: &DayCtx) {
    let assume = &ctx.config.world;
    let design_ni = ctx.config.design.notification_intensity;
    for i in 0..world.population() {
        let ni = world.truth.notification_intensity(i, design_ni);
        let opened = world.people[i].state.today.opened;
        let person = &mut world.people[i];
        let (traits, state) = (&person.traits, &mut person.state);
        state.active_bits = (state.active_bits << 1) | u64::from(opened);
        if !state.is_user {
            let pull = recovery(state.wellbeing, traits.wellbeing_baseline);
            state.wellbeing = (state.wellbeing + pull).clamp(0.0, 1.0);
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
        // Wellbeing and stay utility read the day's reflective utility after
        // the time cost and the interruption cost; subscriptions read it
        // before (`eref_window` below).
        let (imp, refl) = (
            (t.e_imp / IMPULSIVE_SCALE).tanh(),
            (t.net_reflective() / REFLECTIVE_SCALE).tanh(),
        );
        let experienced = opened || t.harassment > 0;
        let recovery = recovery(state.wellbeing, traits.wellbeing_baseline);
        let experience = if experienced {
            WELLBEING_GAIN * refl
        } else {
            0.0
        };
        state.wellbeing = (state.wellbeing + experience + recovery).clamp(0.0, 1.0);
        let decay = 0.05 * state.habit;
        if opened {
            state.habit += assume.habit_gain * (1.0 + NOTIFICATION_HABIT_GAIN * ni) * imp;
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
        state.annoyance_window += state.today.ad_annoyance;
        state.items_window += state.today.items;
        let leave = assume.churn_base * sig(8.0 * (assume.leave_threshold - state.stay_ema))
            + assume.harassment_exit * state.today.harassment as f32 * traits.vulnerability;
        // People on a deactivation experiment's break are paid participants
        // and stay (docs/model.md, 13.2).
        let on_break = world.truth.on_break(i);
        let mut rng = ctx.rng(i as u64, Stream::Churn);
        if !on_break && rng::chance(&mut rng, leave) {
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
    fn wellbeing_reads_the_time_cost_and_subscriptions_do_not() {
        let mut config = Config::default();
        config.run.population_size = 200;
        let mut world = init::build_world(&config);
        let i = world
            .people
            .iter()
            .position(|p| p.state.is_user)
            .expect("a user");
        let ctx = DayCtx {
            config: &config,
            day: 5,
        };
        // The same day twice: reflective utility 2.0 with a time cost of
        // 0.5, and 1.5 with none.
        let mut charged = world.clone();
        let mut gross = world.clone();
        let t = &mut gross.people[i].state.today;
        t.opened = true;
        t.e_ref = 2.0;
        let t = &mut charged.people[i].state.today;
        t.opened = true;
        t.e_ref = 2.0;
        t.time_cost = 0.5;
        assert_eq!(t.net_reflective(), 1.5);
        let t = &mut world.people[i].state.today;
        t.opened = true;
        t.e_ref = 1.5;
        daily_updates(&mut charged, &ctx);
        daily_updates(&mut world, &ctx);
        daily_updates(&mut gross, &ctx);
        let (a, b) = (&charged.people[i].state, &world.people[i].state);
        assert_eq!(a.wellbeing, b.wellbeing);
        assert_eq!(a.stay_ema, b.stay_ema);
        // Without the time cost the same day would have felt better.
        assert!(gross.people[i].state.wellbeing > a.wellbeing);
        // The subscription window gathers the value before the time cost.
        assert_eq!(a.eref_window - b.eref_window, 0.5);
    }

    #[test]
    fn non_users_recover_toward_their_baseline() {
        let mut config = Config::default();
        config.run.population_size = 200;
        let mut world = init::build_world(&config);
        let ctx = DayCtx {
            config: &config,
            day: 40,
        };
        let mut non_users = world
            .people
            .iter()
            .enumerate()
            .filter(|(_, p)| !p.state.is_user)
            .map(|(i, _)| i);
        let (former, banned, never) = (
            non_users.next().expect("a non-user"),
            non_users.next().expect("a second non-user"),
            non_users.next().expect("a third non-user"),
        );
        let s = &mut world.people[former].state;
        s.ever_user = true;
        s.left_day = Some(10);
        s.wellbeing = 0.2;
        let s = &mut world.people[banned].state;
        s.ever_user = true;
        s.banned = true;
        s.wellbeing = 0.95;
        let before: Vec<f32> = world.people.iter().map(|p| p.state.wellbeing).collect();
        daily_updates(&mut world, &ctx);
        for i in [former, banned] {
            let baseline = world.people[i].traits.wellbeing_baseline;
            let expected = before[i] + 0.02 * (baseline - before[i]);
            assert_eq!(world.people[i].state.wellbeing, expected.clamp(0.0, 1.0));
        }
        assert!(world.people[former].state.wellbeing > 0.2);
        assert!(world.people[banned].state.wellbeing < 0.95);
        // Someone who never joined sits at the baseline and stays there.
        let p = &world.people[never];
        assert_eq!(p.state.wellbeing, p.traits.wellbeing_baseline);
        assert_eq!(before[never], p.traits.wellbeing_baseline);
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
