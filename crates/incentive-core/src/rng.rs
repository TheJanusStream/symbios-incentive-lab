//! Counter-based random streams (docs/model.md, section 12).
//!
//! Every draw site seeds its own generator from `(seed, day, entity, stream)`,
//! so a run is a pure function of its configuration and two runs that differ in
//! one lever share every draw that lever does not touch (common random numbers).

use rand::Rng;
use rand_distr::{Beta, Distribution, Gamma, StandardNormal};
use rand_pcg::Pcg64Mcg;

/// The generator type behind every stream.
pub(crate) type StreamRng = Pcg64Mcg;

/// The purpose of a random stream. The discriminant enters the seed, so the
/// values must never be reordered or reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stream {
    Population = 1,
    Ties = 2,
    Follows = 3,
    Bots = 4,
    Joining = 5,
    Activity = 6,
    Posting = 7,
    Moderation = 8,
    Harassment = 9,
    Feed = 10,
    Engagement = 11,
    BotBoosts = 12,
    Notes = 13,
    Review = 14,
    Economy = 15,
    Learning = 16,
    Churn = 18,
    Spillover = 19,
    Survey = 20,
    Explain = 21,
    /// Per-post draws, keyed on the author and the author's own post ordinal.
    PostDraw = 22,
    /// The archetype race of each individual.
    Archetype = 23,
    /// Reports and see-less marks of brigading bots, per bot account.
    Brigade = 24,
    /// Assignment to a deactivation experiment's break group, per person, on
    /// the experiment's first day.
    Experiment = 25,
}

/// SplitMix64 finaliser: a cheap bijective mixer with good avalanche.
const fn splitmix64(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// A generator for one `(seed, day, entity, stream)` cell.
pub(crate) fn stream_rng(seed: u64, day: u32, entity: u64, stream: Stream) -> StreamRng {
    let a = splitmix64(seed ^ splitmix64(stream as u64));
    let b = splitmix64(a ^ u64::from(day));
    let hi = splitmix64(b ^ entity);
    let lo = splitmix64(hi ^ 0xD1B5_4A32_D192_ED03);
    Pcg64Mcg::new((u128::from(hi) << 64) | u128::from(lo))
}

/// The entity of a post's own draws: the author's account in the high half
/// and the author's own post ordinal in the low half, so that a lever that
/// changes how many posts someone writes never reshuffles anyone else's draws.
pub(crate) fn post_key(author: u32, ordinal: u32) -> u64 {
    (u64::from(author) << 32) | u64::from(ordinal)
}

/// The generator of a post's own draws (style, polish, topic, content noise
/// and Attack target). The ordinal counts the author's posts over the whole
/// run, so the day is not needed to keep keys apart.
pub(crate) fn post_rng(seed: u64, author: u32, ordinal: u32) -> StreamRng {
    stream_rng(seed, 0, post_key(author, ordinal), Stream::PostDraw)
}

/// An `Exp(1)` draw by inversion.
pub(crate) fn exponential(rng: &mut StreamRng) -> f32 {
    -(1.0 - uniform(rng)).ln()
}

/// A uniform draw in `[0, 1)`.
pub(crate) fn uniform(rng: &mut StreamRng) -> f32 {
    rng.random::<f32>()
}

/// True with probability `p`; values outside `[0, 1]` act as certain or impossible.
pub(crate) fn chance(rng: &mut StreamRng, p: f32) -> bool {
    uniform(rng) < p
}

/// A normal draw with mean 0 and standard deviation `sd`.
pub(crate) fn normal(rng: &mut StreamRng, sd: f32) -> f32 {
    let z: f32 = StandardNormal.sample(rng);
    z * sd
}

/// A uniform index in `0..n`; `n` must be positive.
pub(crate) fn index(rng: &mut StreamRng, n: usize) -> usize {
    rng.random_range(0..n)
}

/// A Poisson draw by Knuth's product method; the model's rates are small, and
/// the cap keeps a pathological configuration from looping for long.
pub(crate) fn poisson(rng: &mut StreamRng, lambda: f32) -> u32 {
    const CAP: u32 = 30;
    if lambda <= 0.0 {
        return 0;
    }
    let limit = (-lambda).exp();
    let mut product = uniform(rng);
    let mut count = 0;
    while product > limit && count < CAP {
        product *= uniform(rng);
        count += 1;
    }
    count
}

/// A Beta draw with mean `m` and concentration `k`, that is `Beta(k m, k (1 - m))`.
pub(crate) fn beta_mean(rng: &mut StreamRng, m: f32, k: f32) -> f32 {
    let m = m.clamp(0.01, 0.99);
    Beta::new(k * m, k * (1.0 - m)).map_or(m, |d| d.sample(rng))
}

/// A Gamma draw with unit scale; the fallback is the distribution's mean.
pub(crate) fn gamma(rng: &mut StreamRng, shape: f32) -> f32 {
    Gamma::new(shape, 1.0).map_or(shape, |d| d.sample(rng))
}

/// Picks an index from cumulative weights by binary search; `None` when empty
/// or when the total weight is zero.
pub(crate) fn pick_cumulative(rng: &mut StreamRng, cumulative: &[f32]) -> Option<usize> {
    let total = *cumulative.last()?;
    if total <= 0.0 {
        return None;
    }
    let target = uniform(rng) * total;
    let pos = cumulative.partition_point(|&c| c <= target);
    Some(pos.min(cumulative.len() - 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streams_are_reproducible_and_distinct() {
        let mut a = stream_rng(7, 3, 11, Stream::Feed);
        let mut b = stream_rng(7, 3, 11, Stream::Feed);
        let mut c = stream_rng(7, 3, 11, Stream::Engagement);
        let xa: u64 = a.random();
        assert_eq!(xa, b.random::<u64>());
        assert_ne!(xa, c.random::<u64>());
    }

    #[test]
    fn poisson_mean_is_close() {
        let mut rng = stream_rng(1, 0, 0, Stream::Posting);
        let n = 20_000;
        let total: u32 = (0..n).map(|_| poisson(&mut rng, 1.5)).sum();
        let mean = total as f32 / n as f32;
        assert!((mean - 1.5).abs() < 0.05, "mean {mean}");
    }

    #[test]
    fn cumulative_pick_respects_weights() {
        let mut rng = stream_rng(1, 0, 0, Stream::Feed);
        let cumulative = [1.0, 1.0, 4.0];
        let mut counts = [0; 3];
        for _ in 0..4_000 {
            if let Some(i) = pick_cumulative(&mut rng, &cumulative) {
                counts[i] += 1;
            }
        }
        assert_eq!(counts[1], 0);
        assert!(counts[2] > counts[0] * 2);
    }
}
