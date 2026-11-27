//! Gathering feed candidates (docs/model.md, 6.2): in-network posts and
//! reshares, out-of-network recommendations, and the viewer's true responses.

use super::{Candidate, FeedCtx, FeedKind, ScoreComponents};
use crate::behaviour::{self, BehaviourEnv, Relation, Signals};
use crate::config::FeedExperimentKind;
use crate::content::{Post, Style};
use crate::rng::{self, StreamRng};
use crate::world::World;

/// Posts sampled for out-of-network recommendation, weighted by observed
/// engagement, the platform's "trending" signal.
#[derive(Clone, Debug, Default)]
pub(crate) struct Trending {
    ids: Vec<u32>,
    weights: Vec<f32>,
    cumulative: Vec<f32>,
}

impl Trending {
    /// Snapshot of live, unremoved posts; weights are `1 + observed engagement`.
    pub(crate) fn build(world: &World) -> Self {
        let mut trending = Self::default();
        let mut total = 0.0;
        for post in world.posts.iter().filter(|p| !p.removed) {
            let weight = 1.0 + post.observed_engagement() as f32;
            total += weight;
            trending.ids.push(post.id);
            trending.weights.push(weight);
            trending.cumulative.push(total);
        }
        trending
    }

    fn sample(&self, rng: &mut StreamRng) -> Option<u32> {
        rng::pick_cumulative(rng, &self.cumulative).map(|i| self.ids[i])
    }
}

/// What a feed experiment removes from a treated viewer's candidates
/// (docs/model.md, 13.2); nothing for everyone else.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct FeedFilter {
    /// Posts whose classifier score exceeds this are hidden.
    pub hide_above: Option<f32>,
    /// Reshares are dropped; original posts stay.
    pub no_reshares: bool,
}

impl FeedFilter {
    /// The filter of a viewer with this treatment, `None` for a viewer the
    /// experiment does not treat. A score penalty and the chronological feed
    /// filter nothing.
    pub(crate) fn of(treatment: Option<FeedExperimentKind>) -> Self {
        match treatment {
            Some(FeedExperimentKind::HideFlagged { threshold }) => Self {
                hide_above: Some(threshold),
                no_reshares: false,
            },
            Some(FeedExperimentKind::NoReshares) => Self {
                hide_above: None,
                no_reshares: true,
            },
            Some(FeedExperimentKind::FewerSameGroup { .. } | FeedExperimentKind::Chronological)
            | None => Self::default(),
        }
    }

    /// Whether the filter hides a post: its classifier score lies above
    /// the threshold. A post the classifier did not screen is never hidden.
    pub(crate) fn hides(&self, post: &Post) -> bool {
        self.hide_above
            .is_some_and(|t| post.classifier_score.is_some_and(|s| s > t))
    }
}

/// Weighted draws per recommendation slot before the sampler stops
/// rejecting duplicates and draws the rest exactly from what is left.
const REJECTION_DRAWS_PER_SLOT: usize = 4;

/// Reusable per-session buffers. `seen` holds, per live post slot, the stamp of
/// the latest session that took it as a candidate.
#[derive(Clone, Debug, Default)]
pub(crate) struct FeedScratch {
    pub seen: Vec<u32>,
    pub stamp: u32,
    pub candidates: Vec<Candidate>,
    /// Candidates the latest feed gathered, before ranking cut it to its
    /// length.
    pub gathered: usize,
    /// Of those, the in-network ones, and of them the ones by the viewer's
    /// out-group (docs/model.md, 9.2).
    pub gathered_in_network: u32,
    pub gathered_in_network_outgroup: u32,
    /// What a feed experiment removes from the latest viewer's candidates.
    pub filter: FeedFilter,
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
        restamp_kept(ctx, scratch);
    }
    let quota = (recommended * cap as f32).ceil() as usize;
    recommend(ctx, trending, quota, rng, scratch);
    let FeedScratch {
        raw, candidates, ..
    } = scratch;
    candidates.extend(
        raw.iter()
            .filter_map(|o| ctx.world.posts.get(o.post).map(|p| candidate(p, o))),
    );
}

/// Only the in-network posts the cap kept stay marked as gathered, so the
/// posts it cut can still be recommended.
fn restamp_kept(ctx: &FeedCtx, scratch: &mut FeedScratch) {
    scratch.stamp += 1;
    let FeedScratch {
        seen, stamp, raw, ..
    } = scratch;
    for origin in raw.iter() {
        if let Some(slot) = ctx.world.posts.slot(origin.post) {
            seen[slot] = *stamp;
        }
    }
}

/// Out-of-network recommendations (docs/model.md, 6.2): weighted draws
/// without replacement until `quota` posts are added or no eligible post is
/// left. Duplicates and ineligible posts are rejected and drawn again; when
/// rejections pile up, the rest is drawn exactly from the remaining eligible
/// posts with weighted random keys (Efraimidis and Spirakis), which samples
/// the same distribution.
fn recommend(
    ctx: &FeedCtx,
    trending: &Trending,
    quota: usize,
    rng: &mut StreamRng,
    scratch: &mut FeedScratch,
) {
    let mut added = 0;
    let mut draws = 0;
    while added < quota && draws < REJECTION_DRAWS_PER_SLOT * quota {
        draws += 1;
        let Some(id) = trending.sample(rng) else {
            return;
        };
        added += usize::from(try_recommend(ctx, id, scratch));
    }
    if added >= quota {
        return;
    }
    let mut keyed: Vec<(f32, u32)> = trending
        .ids
        .iter()
        .zip(&trending.weights)
        .filter(|&(&id, _)| eligible(ctx, id, scratch))
        .map(|(&id, &w)| ((1.0 - rng::uniform(rng)).ln() / w, id))
        .collect();
    keyed.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    for (_, id) in keyed {
        if added >= quota {
            break;
        }
        added += usize::from(try_recommend(ctx, id, scratch));
    }
}

/// Whether a post may be recommended to the viewer: live, not removed, not
/// the viewer's own, not by a blocked account and not gathered already.
fn eligible(ctx: &FeedCtx, id: u32, scratch: &FeedScratch) -> bool {
    let viewer = &ctx.world.people[ctx.viewer];
    let Some(post) = ctx.world.posts.get(id) else {
        return false;
    };
    let gathered = ctx
        .world
        .posts
        .slot(id)
        .is_none_or(|slot| scratch.seen.get(slot) == Some(&scratch.stamp));
    !post.removed
        && !gathered
        && post.author as usize != ctx.viewer
        && !viewer.has_blocked(post.author)
        && !scratch.filter.hides(post)
}

/// Adds a recommendation if the post is eligible; true when added.
fn try_recommend(ctx: &FeedCtx, id: u32, scratch: &mut FeedScratch) -> bool {
    if !eligible(ctx, id, scratch) {
        return false;
    }
    let Some(post) = ctx.world.posts.get(id) else {
        return false;
    };
    let origin = Origin {
        post: id,
        in_network: false,
        via: None,
        depth: 0,
        item_day: post.day,
    };
    push_origin(ctx, post, origin, scratch);
    true
}

/// In-network posts and reshares of the accounts the viewer follows, less
/// what a feed experiment's filter removes for a treated viewer.
fn gather_in_network(ctx: &FeedCtx, scratch: &mut FeedScratch) {
    let world = ctx.world;
    let viewer = &world.people[ctx.viewer];
    let filter = scratch.filter;
    for &account in &viewer.state.follows {
        if viewer.has_blocked(account) {
            continue;
        }
        for &id in &world.accounts.recent_posts[account as usize] {
            if let Some(post) = world.posts.get(id).filter(|p| !filter.hides(p)) {
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
        if filter.no_reshares {
            continue;
        }
        for reshare in &world.accounts.reshares[account as usize] {
            let Some(post) = world.posts.get(reshare.post).filter(|p| !filter.hides(p)) else {
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
        noted: post.note_shown(ctx.day, viewer.state.today.session_hour),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::init;
    use crate::rng::Stream;
    use crate::world::DayCtx;

    /// A world on day 0 after everyone wrote their posts.
    fn world_with_posts(config: &Config) -> World {
        let mut world = init::build_world(config);
        for p in &mut world.people {
            p.state.today.opened = p.state.is_user;
        }
        let ctx = DayCtx { config, day: 0 };
        crate::posting::write_posts(&mut world, &ctx);
        world
    }

    /// The candidates of the first user's feed, and how many post slots are
    /// marked as gathered.
    fn gathered(config: &Config, world: &World) -> (usize, Vec<Candidate>, usize) {
        let viewer = world
            .people
            .iter()
            .position(|p| p.state.is_user)
            .unwrap_or(0);
        let ctx = FeedCtx {
            world,
            config,
            day: 0,
            viewer,
        };
        let trending = Trending::build(world);
        let mut scratch = FeedScratch::default();
        scratch.reset(world.posts.len());
        scratch.stamp += 1;
        let mut rng = rng::stream_rng(1, 0, viewer as u64, Stream::Feed);
        gather(&ctx, FeedKind::Ranked, &trending, &mut rng, &mut scratch);
        let stamped = scratch.seen.iter().filter(|&&s| s == scratch.stamp).count();
        (viewer, scratch.candidates.clone(), stamped)
    }

    #[test]
    fn recommendations_fill_their_quota_without_duplicates() {
        let mut config = Config::default();
        config.run.population_size = 400;
        let world = world_with_posts(&config);
        let (_, cands, stamped) = gathered(&config, &world);
        let quota =
            (config.design.recommended_share * config.run.candidate_cap as f32).ceil() as usize;
        let recommended = cands.iter().filter(|c| !c.in_network).count();
        assert!(world.posts.len() > 4 * quota);
        assert_eq!(recommended, quota);
        let mut ids: Vec<u32> = cands.iter().map(|c| c.post).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), cands.len());
        assert_eq!(stamped, cands.len());
    }

    #[test]
    fn posts_cut_from_the_network_can_be_recommended_until_the_pool_runs_out() {
        let mut config = Config::default();
        config.run.population_size = 300;
        config.run.candidate_cap = 1_000;
        config.design.recommended_share = 1.0;
        let world = world_with_posts(&config);
        let (viewer, cands, stamped) = gathered(&config, &world);
        let own = world
            .posts
            .iter()
            .filter(|p| p.author as usize == viewer)
            .count();
        assert!(world.posts.len() < 1_000);
        // Every post but the viewer's own is recommended, followed accounts' too.
        assert_eq!(cands.len(), world.posts.len() - own);
        assert!(cands.iter().all(|c| !c.in_network));
        let person = &world.people[viewer];
        assert!(cands.iter().any(|c| person.follows(c.author)));
        assert_eq!(stamped, cands.len());
    }
}
