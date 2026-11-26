//! Accounts and the follow graph (docs/model.md, 3.4, 6.6 and 7.6).
//!
//! Account ids below the population size are individuals; the ones above are
//! bot slots.

use crate::bots::Bot;
use crate::content::Group;
use crate::person::Person;
use crate::population::Archetype;
use crate::rng::{self, StreamRng};
use crate::world::World;

/// A share of a post by an account, visible to that account's followers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Reshare {
    pub post: u32,
    pub depth: u8,
    pub day: u32,
}

/// Per-account data shared by individuals and bots.
#[derive(Clone, Debug, Default)]
pub(crate) struct Accounts {
    pub followers: Vec<u32>,
    /// Moving average of see-less marks per impression (observable).
    pub see_less_rate: Vec<f32>,
    pub see_less_today: Vec<u32>,
    pub impressions_today: Vec<u32>,
    /// Ids of the account's live posts, oldest first.
    pub recent_posts: Vec<Vec<u32>>,
    /// The account's live reshares, oldest first.
    pub reshares: Vec<Vec<Reshare>>,
    /// The account's reports that human reviewers judged (observable).
    pub reports_reviewed: Vec<u32>,
    /// Of those, the ones reviewers upheld: the post was toxic.
    pub reports_upheld: Vec<u32>,
}

/// Weight of the see-less marks of the latest day in the moving average.
const SEE_LESS_SMOOTHING: f32 = 0.1;
/// Creators are this much likelier to be followed by preferential attachment.
const CREATOR_ATTACHMENT: f32 = 5.0;

impl Accounts {
    pub(crate) fn new(count: usize) -> Self {
        Self {
            followers: vec![0; count],
            see_less_rate: vec![0.0; count],
            see_less_today: vec![0; count],
            impressions_today: vec![0; count],
            recent_posts: vec![Vec::new(); count],
            reshares: vec![Vec::new(); count],
            reports_reviewed: vec![0; count],
            reports_upheld: vec![0; count],
        }
    }

    /// Drops references to pruned posts.
    pub(crate) fn prune(&mut self, first_live_id: u32) {
        for posts in &mut self.recent_posts {
            let dead = posts.partition_point(|&id| id < first_live_id);
            posts.drain(..dead);
        }
        for shares in &mut self.reshares {
            shares.retain(|r| r.post >= first_live_id);
        }
    }

    /// The account's upheld rate, `(upheld + 1) / (reviewed + 2)`: 0.5
    /// for an account without judged reports (docs/model.md, 6.7).
    pub(crate) fn upheld_rate(&self, account: u32) -> f32 {
        let a = account as usize;
        (self.reports_upheld[a] + 1) as f32 / (self.reports_reviewed[a] + 2) as f32
    }

    /// Folds today's see-less marks into the moving average.
    pub(crate) fn end_day(&mut self) {
        for i in 0..self.followers.len() {
            let impressions = self.impressions_today[i];
            if impressions > 0 {
                let rate = self.see_less_today[i] as f32 / impressions as f32;
                self.see_less_rate[i] += SEE_LESS_SMOOTHING * (rate - self.see_less_rate[i]);
            }
            self.see_less_today[i] = 0;
            self.impressions_today[i] = 0;
        }
    }
}

/// Makes individual `v` follow `account`; false when nothing changed.
pub(crate) fn follow(
    people: &mut [Person],
    accounts: &mut Accounts,
    v: usize,
    account: u32,
) -> bool {
    if account as usize == v || !people[v].add_follow(account) {
        return false;
    }
    accounts.followers[account as usize] += 1;
    true
}

/// Makes individual `v` unfollow `account`; false when nothing changed.
pub(crate) fn unfollow(
    people: &mut [Person],
    accounts: &mut Accounts,
    v: usize,
    account: u32,
) -> bool {
    if !people[v].remove_follow(account) {
        return false;
    }
    let count = &mut accounts.followers[account as usize];
    *count = count.saturating_sub(1);
    true
}

/// Gives an account a fresh start: nobody follows or blocks it any more.
pub(crate) fn reset_account(people: &mut [Person], accounts: &mut Accounts, account: u32) {
    for person in people.iter_mut() {
        person.remove_follow(account);
        person.state.blocked.retain(|&b| b != account);
    }
    let a = account as usize;
    accounts.followers[a] = 0;
    accounts.see_less_rate[a] = 0.0;
    accounts.reshares[a].clear();
    accounts.reports_reviewed[a] = 0;
    accounts.reports_upheld[a] = 0;
}

/// How many accounts preferential attachment should bring a person to.
#[derive(Clone, Copy, Debug)]
pub(crate) enum FollowTarget {
    /// Follow this many accounts in total, offline ties included.
    Total(usize),
    /// Follow this many accounts beyond the offline ties.
    Extra(usize),
}

/// Follows `v`'s offline ties who are users, then accounts by preferential
/// attachment until the target is met (docs/model.md, 3.4 and 7.4).
pub(crate) fn attach_follows(
    world: &mut World,
    v: usize,
    target: FollowTarget,
    homophily: f32,
    rng: &mut StreamRng,
) {
    let World {
        people,
        bots,
        accounts,
        ties,
        ..
    } = world;
    for &tie in &ties[v] {
        if people[tie as usize].state.is_user {
            follow(people, accounts, v, tie);
        }
    }
    let wanted = match target {
        FollowTarget::Total(total) => total.saturating_sub(people[v].state.follows.len()),
        FollowTarget::Extra(extra) => extra,
    };
    if wanted == 0 {
        return;
    }
    let pools = attachment_pools(people, bots, accounts, v);
    let group = people[v].traits.group;
    let mut added = 0;
    let mut attempts = 0;
    while added < wanted && attempts < wanted * 10 {
        attempts += 1;
        let same = rng::chance(rng, homophily);
        let pool = &pools[if same {
            group.index()
        } else {
            group.other().index()
        }];
        let Some(pick) = rng::pick_cumulative(rng, &pool.cumulative) else {
            continue;
        };
        if follow(people, accounts, v, pool.accounts[pick]) {
            added += 1;
        }
    }
}

/// Candidate accounts of one group with cumulative attachment weights.
struct Pool {
    accounts: Vec<u32>,
    cumulative: Vec<f32>,
}

fn attachment_pools(people: &[Person], bots: &[Bot], accounts: &Accounts, v: usize) -> [Pool; 2] {
    let mut pools = [Group::A, Group::B].map(|_| Pool {
        accounts: Vec::new(),
        cumulative: Vec::new(),
    });
    let mut push = |group: Group, account: u32, weight: f32| {
        let pool = &mut pools[group.index()];
        let total = pool.cumulative.last().copied().unwrap_or(0.0);
        pool.accounts.push(account);
        pool.cumulative.push(total + weight);
    };
    for (i, person) in people.iter().enumerate() {
        if i == v || !person.state.is_user {
            continue;
        }
        let boost = if person.traits.archetype == Archetype::Creator {
            CREATOR_ATTACHMENT
        } else {
            1.0
        };
        push(
            person.traits.group,
            i as u32,
            (accounts.followers[i] as f32 + 1.0) * boost,
        );
    }
    let base = people.len();
    for (b, bot) in bots.iter().enumerate() {
        if bot.active {
            let account = base + b;
            push(
                bot.group,
                account as u32,
                accounts.followers[account] as f32 + 1.0,
            );
        }
    }
    pools
}
