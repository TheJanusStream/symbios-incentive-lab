//! Feed candidates, the platform's predictions and ranking (docs/model.md,
//! 6.2 to 6.5). Everything here reads the world without changing it, so the
//! inspector can explain a feed with the same code a session uses.

use serde::Serialize;

use crate::behaviour::{Relation, Signals};
use crate::config::{AltFeed, Config, DesignLevers};
use crate::content::Post;
use crate::rng::{self, StreamRng};
use crate::world::{StyleRates, World};

mod candidates;

pub(crate) use candidates::{FeedScratch, Trending};
use candidates::{fill_signals, gather};

/// Which feed a viewer gets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum FeedKind {
    /// The main ranking with the design's weights.
    Ranked,
    /// Followed accounts only, newest first.
    Chronological,
    /// The alternative quality-ranked feed.
    Quality,
    /// The alternative bridging-ranked feed.
    Bridging,
}

/// The weighted contribution of each ranking term to a post's score
/// (docs/model.md, 6.4). Penalties are negative.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct ScoreComponents {
    /// Weighted predicted like probability.
    pub like: f32,
    /// Weighted predicted reply probability.
    pub reply: f32,
    /// Weighted predicted share probability.
    pub share: f32,
    /// Weighted predicted dwell time.
    pub dwell: f32,
    /// Weighted quality classifier score.
    pub quality: f32,
    /// Weighted bridging score.
    pub bridging: f32,
    /// Weighted freshness.
    pub recency: f32,
    /// Bonus for followed accounts.
    pub in_network: f32,
    /// Penalty for the author's see-less rate.
    pub see_less: f32,
    /// Penalty for demotion.
    pub demoted: f32,
    /// Penalty for a community note.
    pub noted: f32,
    /// Adjustment by diversity re-ranking at the moment the post was picked.
    pub diversity: f32,
}

impl ScoreComponents {
    /// The score: the sum of all components.
    pub fn total(&self) -> f32 {
        self.like
            + self.reply
            + self.share
            + self.dwell
            + self.quality
            + self.bridging
            + self.recency
            + self.in_network
            + self.see_less
            + self.demoted
            + self.noted
            + self.diversity
    }
}

/// Ranking weights of one feed.
#[derive(Clone, Copy, Debug)]
struct RankWeights {
    like: f32,
    reply: f32,
    share: f32,
    dwell: f32,
    quality: f32,
    bridging: f32,
    recency: f32,
    in_network: f32,
    see_less: f32,
    demote: f32,
    noted: f32,
}

impl RankWeights {
    fn for_feed(kind: FeedKind, d: &DesignLevers) -> Self {
        let main = Self {
            like: d.w_like,
            reply: d.w_reply,
            share: d.w_share,
            dwell: d.w_dwell,
            quality: d.w_quality,
            bridging: d.w_bridging,
            recency: d.w_recency,
            in_network: d.w_in_network,
            see_less: d.w_see_less,
            demote: d.demote_penalty,
            noted: d.noted_penalty,
        };
        let plain = Self {
            like: 0.3,
            reply: 0.0,
            share: 0.0,
            dwell: 0.0,
            quality: 0.0,
            bridging: 0.0,
            recency: 0.3,
            in_network: 0.0,
            see_less: 0.0,
            ..main
        };
        match kind {
            FeedKind::Ranked | FeedKind::Chronological => main,
            FeedKind::Quality => Self {
                quality: 1.5,
                ..plain
            },
            FeedKind::Bridging => Self {
                bridging: 1.5,
                quality: 0.5,
                ..plain
            },
        }
    }
}

/// The ranking weights users meet on average: the main ranking's for those
/// on the main feed and the alternative feed's for its share of users. A
/// chronological feed weighs nothing. Adaptive bots read these to decide
/// what to game (docs/model.md, 3.5).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct MeanWeights {
    pub like: f32,
    pub share: f32,
    pub quality: f32,
    pub bridging: f32,
    pub see_less: f32,
}

impl MeanWeights {
    pub(crate) fn of(design: &DesignLevers) -> Self {
        let weights = |kind: FeedKind| {
            if kind == FeedKind::Chronological {
                return Self::default();
            }
            let w = RankWeights::for_feed(kind, design);
            Self {
                like: w.like,
                share: w.share,
                quality: w.quality,
                bridging: w.bridging,
                see_less: w.see_less,
            }
        };
        let main = weights(if design.chronological {
            FeedKind::Chronological
        } else {
            FeedKind::Ranked
        });
        let alt = weights(match design.alt_feed {
            AltFeed::Chronological => FeedKind::Chronological,
            AltFeed::Quality => FeedKind::Quality,
            AltFeed::Bridging => FeedKind::Bridging,
        });
        let a = design.alt_feed_share;
        let mix = |m: f32, x: f32| (1.0 - a) * m + a * x;
        Self {
            like: mix(main.like, alt.like),
            share: mix(main.share, alt.share),
            quality: mix(main.quality, alt.quality),
            bridging: mix(main.bridging, alt.bridging),
            see_less: mix(main.see_less, alt.see_less),
        }
    }
}

/// A feed candidate with everything a session or an explanation needs.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Candidate {
    pub post: u32,
    pub author: u32,
    pub topic: u8,
    pub hostile: bool,
    pub in_network: bool,
    /// The followed account whose reshare brought the post, if any.
    pub via: Option<u32>,
    pub depth: u8,
    pub item_day: u32,
    pub relation: Relation,
    pub signals: Signals,
    pub components: ScoreComponents,
    pub score: f32,
}

/// The feed a viewer gets.
pub(crate) fn feed_kind(world: &World, design: &DesignLevers, v: usize) -> FeedKind {
    if world.people[v].state.alt_feed_user {
        return match design.alt_feed {
            AltFeed::Chronological => FeedKind::Chronological,
            AltFeed::Quality => FeedKind::Quality,
            AltFeed::Bridging => FeedKind::Bridging,
        };
    }
    if design.chronological {
        FeedKind::Chronological
    } else {
        FeedKind::Ranked
    }
}

/// Who is asking for a feed, on which day, in which world.
#[derive(Clone, Copy)]
pub(crate) struct FeedCtx<'a> {
    pub world: &'a World,
    pub config: &'a Config,
    pub day: u32,
    pub viewer: usize,
}

/// Builds, scores and orders the viewer's feed of at most `limit` items into
/// `scratch.candidates`.
pub(crate) fn build_feed(
    ctx: &FeedCtx,
    limit: usize,
    trending: &Trending,
    rng: &mut StreamRng,
    scratch: &mut FeedScratch,
) -> FeedKind {
    let kind = feed_kind(ctx.world, &ctx.config.design, ctx.viewer);
    scratch.stamp += 1;
    scratch.candidates.clear();
    gather(ctx, kind, trending, rng, scratch);
    fill_signals(ctx, scratch);
    if kind != FeedKind::Chronological {
        let weights = RankWeights::for_feed(kind, &ctx.config.design);
        for c in &mut scratch.candidates {
            if let Some(post) = ctx.world.posts.get(c.post) {
                c.components = score(ctx, post, c, &weights, rng);
                c.score = c.components.total();
            }
        }
    }
    order(kind, ctx.config.design.diversity, limit, scratch);
    kind
}

/// Fills in ranking components for an already gathered chronological feed, so
/// an explanation can show what the main ranking would have thought of it.
pub(crate) fn score_for_explanation(ctx: &FeedCtx, rng: &mut StreamRng, scratch: &mut FeedScratch) {
    let weights = RankWeights::for_feed(FeedKind::Ranked, &ctx.config.design);
    for c in &mut scratch.candidates {
        if let Some(post) = ctx.world.posts.get(c.post) {
            c.components = score(ctx, post, c, &weights, rng);
            c.score = c.components.total();
        }
    }
}

fn score(
    ctx: &FeedCtx,
    post: &Post,
    c: &Candidate,
    w: &RankWeights,
    rng: &mut StreamRng,
) -> ScoreComponents {
    let q = ctx.config.design.predictor_quality;
    let observed = observed_rates(post, &ctx.world.platform.style_rates[post.style.index()]);
    let s = &c.signals;
    let mut predict = |truth: f32, seen: f32| q * truth + (1.0 - q) * seen + rng::normal(rng, 0.02);
    let like = predict(s.p_like, observed.like).clamp(0.0, 1.0);
    let reply = predict(s.p_reply, observed.reply).clamp(0.0, 1.0);
    let share = predict(s.p_share, observed.share).clamp(0.0, 1.0);
    let dwell = predict(s.dwell, observed.dwell).max(0.0);
    let age = ctx.day.saturating_sub(post.day) as f32;
    let see_less_rate = ctx.world.accounts.see_less_rate[post.author as usize];
    ScoreComponents {
        like: w.like * like,
        reply: w.reply * reply,
        share: w.share * share,
        dwell: w.dwell * dwell / 2.0,
        quality: w.quality * post.quality_signal,
        bridging: w.bridging * post.bridging(),
        recency: w.recency * (-age).exp2(),
        in_network: if c.in_network { w.in_network } else { 0.0 },
        see_less: -w.see_less * see_less_rate,
        demoted: if post.demoted { -w.demote } else { 0.0 },
        noted: if post.noted { -w.noted } else { 0.0 },
        diversity: 0.0,
    }
}

/// Impressions of prior weight behind the style rate in a post's observed
/// rates (docs/model.md, 6.3): a post with this many impressions counts its
/// own counts and the style's rate equally.
pub(crate) const OBSERVED_PRIOR_IMPRESSIONS: f32 = 20.0;

/// A post's observed rates per human impression, `P_obs` of docs/model.md,
/// 6.3: its counts, bot likes and shares included because the platform
/// cannot tell them from people's, shrunk toward the style's rates with a
/// prior of [`OBSERVED_PRIOR_IMPRESSIONS`] impressions,
/// `(count + K * P_style) / (impressions + K)`. Bots boost without a feed
/// impression, so their boosts can lift a rate above what any person would.
pub(crate) fn observed_rates(post: &Post, style: &StyleRates) -> StyleRates {
    let k = OBSERVED_PRIOR_IMPRESSIONS;
    let n = post.impressions as f32 + k;
    let shrink = |count: f32, prior: f32| (count + k * prior) / n;
    StyleRates {
        like: shrink(post.likes() as f32, style.like),
        reply: shrink(post.replies() as f32, style.reply),
        share: shrink(post.shares as f32, style.share),
        dwell: shrink(post.dwell, style.dwell),
    }
}

/// Orders the candidates and keeps the first `limit`.
fn order(kind: FeedKind, diversity: f32, limit: usize, scratch: &mut FeedScratch) {
    let cands = &mut scratch.candidates;
    match kind {
        FeedKind::Chronological => {
            cands.sort_by(|a, b| b.item_day.cmp(&a.item_day).then(b.post.cmp(&a.post)));
        }
        _ if diversity <= 0.0 => {
            cands.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.post.cmp(&b.post)));
        }
        _ => diversify(diversity, limit, scratch),
    }
    scratch.candidates.truncate(limit);
}

/// Greedy diversity re-ranking (docs/model.md, 6.4): each pick lowers the
/// remaining posts by the same author and on the same topic. Calm posts from
/// the viewer's out-group get a one-time lift.
fn diversify(diversity: f32, limit: usize, scratch: &mut FeedScratch) {
    let FeedScratch {
        candidates, adjust, ..
    } = scratch;
    adjust.clear();
    adjust.extend(candidates.iter().map(|c| {
        if !c.signals.same_group && !c.hostile {
            0.3 * diversity
        } else {
            0.0
        }
    }));
    let picks = limit.min(candidates.len());
    for k in 0..picks {
        let mut best = k;
        for i in k + 1..candidates.len() {
            let (bi, bb) = (
                candidates[i].score + adjust[i],
                candidates[best].score + adjust[best],
            );
            if bi > bb || (bi == bb && candidates[i].post < candidates[best].post) {
                best = i;
            }
        }
        candidates.swap(k, best);
        adjust.swap(k, best);
        candidates[k].components.diversity = adjust[k];
        candidates[k].score = candidates[k].components.total();
        let (author, topic) = (candidates[k].author, candidates[k].topic);
        for i in k + 1..candidates.len() {
            if candidates[i].author == author {
                adjust[i] -= 0.5 * diversity;
            }
            if candidates[i].topic == topic {
                adjust[i] -= 0.2 * diversity;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::content::{AuthorProfile, Group, Style, generate_post};
    use crate::rng::Stream;

    fn post() -> Post {
        let mut rng = rng::stream_rng(1, 0, 0, Stream::PostDraw);
        let profile = AuthorProfile::plain(Group::A, 0.5, 0.5);
        let config = Config::default();
        generate_post(
            &mut rng,
            0,
            0,
            &profile,
            Style::Clickbait,
            &[1.0 / 6.0; 6],
            &config.world,
        )
    }

    #[test]
    fn observed_rates_shrink_toward_the_style_and_count_bots() {
        let style = StyleRates::default();
        let mut p = post();
        // No impressions: the style's rates.
        let none = observed_rates(&p, &style);
        assert!((none.like - style.like).abs() < 1e-6 && (none.share - style.share).abs() < 1e-6);
        assert!((none.reply - style.reply).abs() < 1e-6 && (none.dwell - style.dwell).abs() < 1e-6);
        p.impressions = 100;
        p.likes_a = 30;
        p.positive_replies = 4;
        p.angry_replies = 2;
        p.shares = 5;
        p.dwell = 180.0;
        let k = OBSERVED_PRIOR_IMPRESSIONS;
        let seen = observed_rates(&p, &style);
        assert!((seen.like - (30.0 + k * style.like) / (100.0 + k)).abs() < 1e-6);
        assert!((seen.reply - (6.0 + k * style.reply) / (100.0 + k)).abs() < 1e-6);
        assert!((seen.share - (5.0 + k * style.share) / (100.0 + k)).abs() < 1e-6);
        assert!((seen.dwell - (180.0 + k * style.dwell) / (100.0 + k)).abs() < 1e-5);
        // The rate lies between the post's own rate and the style's.
        assert!(seen.like < 0.3 && seen.like > style.like);
        // Twenty bot boosts lift the like and share rates as human ones would.
        p.likes_b += 20;
        p.bot_likes += 20;
        p.shares += 20;
        p.bot_shares += 20;
        let boosted = observed_rates(&p, &style);
        assert!((boosted.like - seen.like - 20.0 / (100.0 + k)).abs() < 1e-6);
        assert!((boosted.share - seen.share - 20.0 / (100.0 + k)).abs() < 1e-6);
        assert_eq!(boosted.dwell, seen.dwell);
    }

    #[test]
    fn mean_weights_blend_the_feeds_users_get() {
        let mut design = DesignLevers::default();
        let main = MeanWeights::of(&design);
        assert_eq!((main.like, main.share, main.bridging), (1.0, 2.0, 0.0));
        design.alt_feed_share = 0.5;
        design.alt_feed = AltFeed::Bridging;
        let mixed = MeanWeights::of(&design);
        assert!((mixed.bridging - 0.75).abs() < 1e-6);
        assert!((mixed.like - 0.65).abs() < 1e-6);
        design.chronological = true;
        design.alt_feed_share = 0.0;
        assert_eq!(MeanWeights::of(&design), MeanWeights::default());
    }
}
