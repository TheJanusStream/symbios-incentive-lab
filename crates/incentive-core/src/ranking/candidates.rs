//! Gathering feed candidates (docs/model.md, 6.2): in-network posts and
//! reshares, out-of-network recommendations, and the viewer's true responses.

use super::{Candidate, FeedCtx, FeedKind, ScoreComponents};
use crate::behaviour::{self, BehaviourEnv, Relation, Signals};
use crate::content::{Post, Style};
use crate::rng::{self, StreamRng};
use crate::world::World;

/// Posts sampled for out-of-network recommendation, weighted by observed
/// engagement, the platform's "trending" signal.
#[derive(Clone, Debug, Default)]
pub(crate) struct Trending {
    ids: Vec<u32>,
    cumulative: Vec<f32>,
}

impl Trending {
    /// Snapshot of live, unremoved posts; weights are `1 + observed engagement`.
    pub(crate) fn build(world: &World) -> Self {
        let mut trending = Self::default();
        let mut total = 0.0;
        for post in world.posts.iter().filter(|p| !p.removed) {
            total += 1.0 + post.observed_engagement() as f32;
            trending.ids.push(post.id);
            trending.cumulative.push(total);
        }
        trending
    }

    fn sample(&self, rng: &mut StreamRng) -> Option<u32> {
        rng::pick_cumulative(rng, &self.cumulative).map(|i| self.ids[i])
    }
}

/// Reusable per-session buffers. `seen` holds, per live post slot, the stamp of
/// the latest session that took it as a candidate.
#[derive(Clone, Debug, Default)]
pub(crate) struct FeedScratch {
    pub seen: Vec<u32>,
    pub stamp: u32,
    pub candidates: Vec<Candidate>,
    pub adjust: Vec<f32>,
    raw: Vec<Origin>,
}

impl FeedScratch {
    /// Prepares the buffers for a phase in which no posts are created.
    pub(crate) fn reset(&mut self, post_count: usize) {
        self.seen.clear();
        self.seen.resize(post_count, 0);
        self.stamp = 0;
    }
}

/// A gathered post and where it came from; compact, so that ranking the many
/// in-network posts by recency stays cheap.
#[derive(Clone, Copy, Debug)]
struct Origin {
    post: u32,
    in_network: bool,
    via: Option<u32>,
    depth: u8,
    item_day: u32,
}

pub(super) fn gather(
    ctx: &FeedCtx,
    kind: FeedKind,
    trending: &Trending,
    rng: &mut StreamRng,
    scratch: &mut FeedScratch,
) {
    let cap = ctx.config.run.candidate_cap as usize;
    let recommended = if kind == FeedKind::Chronological {
        0.0
    } else {
        ctx.config.design.recommended_share
    };
    let in_cap = ((cap as f32) * (1.0 - recommended)).round() as usize;
    scratch.raw.clear();
    gather_in_network(ctx, scratch);
    if scratch.raw.len() > in_cap {
        let newest_first =
            |a: &Origin, b: &Origin| b.item_day.cmp(&a.item_day).then(b.post.cmp(&a.post));
        if in_cap == 0 {
            scratch.raw.clear();
        } else {
            scratch.raw.select_nth_unstable_by(in_cap - 1, newest_first);
            scratch.raw.truncate(in_cap);
            scratch.raw.sort_unstable_by(newest_first);
        }
    }
    let draws = (recommended * cap as f32).ceil() as usize;
    let viewer = &ctx.world.people[ctx.viewer];
    for _ in 0..draws {
        let Some(id) = trending.sample(rng) else {
            break;
        };
        let Some(post) = ctx.world.posts.get(id) else {
            continue;
        };
        if post.author as usize != ctx.viewer && !viewer.has_blocked(post.author) {
            let origin = Origin {
                post: id,
                in_network: false,
                via: None,
                depth: 0,
                item_day: post.day,
            };
            push_origin(ctx, post, origin, scratch);
        }
    }
    let FeedScratch {
        raw, candidates, ..
    } = scratch;
    candidates.extend(
        raw.iter()
            .filter_map(|o| ctx.world.posts.get(o.post).map(|p| candidate(p, o))),
    );
}

fn gather_in_network(ctx: &FeedCtx, scratch: &mut FeedScratch) {
    let world = ctx.world;
    let viewer = &world.people[ctx.viewer];
    for &account in &viewer.state.follows {
        if viewer.has_blocked(account) {
            continue;
        }
        for &id in &world.accounts.recent_posts[account as usize] {
            if let Some(post) = world.posts.get(id) {
                let origin = Origin {
                    post: id,
                    in_network: true,
                    via: None,
                    depth: 0,
                    item_day: post.day,
                };
                push_origin(ctx, post, origin, scratch);
            }
        }
        for reshare in &world.accounts.reshares[account as usize] {
            let Some(post) = world.posts.get(reshare.post) else {
                continue;
            };
            if post.author as usize != ctx.viewer && !viewer.has_blocked(post.author) {
                let origin = Origin {
                    post: reshare.post,
                    in_network: true,
                    via: Some(account),
                    depth: reshare.depth,
                    item_day: reshare.day,
                };
                push_origin(ctx, post, origin, scratch);
            }
        }
    }
}

/// Adds a post unless it is removed or already gathered in this session.
fn push_origin(ctx: &FeedCtx, post: &Post, origin: Origin, scratch: &mut FeedScratch) {
    if post.removed {
        return;
    }
    let Some(slot) = ctx.world.posts.slot(post.id) else {
        return;
    };
    if scratch.seen.len() <= slot {
        scratch.seen.resize(slot + 1, 0);
    }
    if scratch.seen[slot] == scratch.stamp {
        return;
    }
    scratch.seen[slot] = scratch.stamp;
    scratch.raw.push(origin);
}

/// A candidate whose true responses and score are still to be computed.
fn candidate(post: &Post, origin: &Origin) -> Candidate {
    Candidate {
        post: post.id,
        author: post.author,
        topic: post.topic,
        hostile: post.style.is_hostile(),
        in_network: origin.in_network,
        via: origin.via,
        depth: origin.depth,
        item_day: origin.item_day,
        relation: Relation::default(),
        signals: Signals::default(),
        components: ScoreComponents::default(),
        score: 0.0,
    }
}

/// Computes the viewer's true responses for the kept candidates only, so the
/// many in-network posts dropped by the cap cost nothing.
pub(super) fn fill_signals(ctx: &FeedCtx, scratch: &mut FeedScratch) {
    let env = BehaviourEnv {
        design: &ctx.config.design,
        world: &ctx.config.world,
    };
    let traits = &ctx.world.people[ctx.viewer].traits;
    for c in &mut scratch.candidates {
        if let Some(post) = ctx.world.posts.get(c.post) {
            c.relation = relation(ctx, post, c.depth);
            c.signals = behaviour::signals(traits, post, c.relation, env);
        }
    }
}

fn relation(ctx: &FeedCtx, post: &Post, depth: u8) -> Relation {
    let v = ctx.viewer;
    let viewer = &ctx.world.people[v];
    let follows_author = viewer.follows(post.author);
    let mutual = post.style == Style::Personal
        && follows_author
        && ctx
            .world
            .people
            .get(post.author as usize)
            .is_some_and(|a| a.follows(v as u32));
    Relation {
        mutual,
        follows_author,
        is_target: post.target == Some(v as u32),
        share_capped: ctx
            .config
            .design
            .reshare_depth_cap
            .is_some_and(|cap| u32::from(depth) >= cap),
    }
}
