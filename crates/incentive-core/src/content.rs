//! Styles, topics, groups and posts (docs/model.md, section 4).

use serde::Serialize;

mod post;

pub use post::Post;
pub(crate) use post::{AuthorProfile, EVASION_TOXICITY, NEUTRAL_INTENSITY, generate_post};

/// Number of topics.
pub const TOPIC_COUNT: usize = 6;
/// Number of content styles.
pub const STYLE_COUNT: usize = 6;

/// Display names of the topics; topic 0 is public affairs.
pub const TOPICS: [&str; TOPIC_COUNT] = [
    "Public affairs",
    "Science",
    "Sports",
    "Arts",
    "Lifestyle",
    "Tech",
];

/// One of the two opinion groups.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
pub enum Group {
    /// Positive opinions.
    A,
    /// Negative opinions.
    B,
}

impl Group {
    /// 0 for A, 1 for B.
    pub fn index(self) -> usize {
        self as usize
    }

    /// The other group.
    pub fn other(self) -> Self {
        match self {
            Self::A => Self::B,
            Self::B => Self::A,
        }
    }
}

/// A content style (docs/model.md, 4.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
pub enum Style {
    /// News, explanations, facts.
    Informative,
    /// Jokes, clips, fun.
    Entertaining,
    /// Life updates for friends.
    Personal,
    /// Hostility toward the other group.
    Outrage,
    /// Bait that promises more than it gives.
    Clickbait,
    /// Abuse aimed at a person.
    Attack,
}

impl Style {
    /// Every style in index order.
    pub const ALL: [Self; STYLE_COUNT] = [
        Self::Informative,
        Self::Entertaining,
        Self::Personal,
        Self::Outrage,
        Self::Clickbait,
        Self::Attack,
    ];

    /// Position in [`Style::ALL`].
    pub fn index(self) -> usize {
        self as usize
    }

    /// Display name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Informative => "Informative",
            Self::Entertaining => "Entertaining",
            Self::Personal => "Personal",
            Self::Outrage => "Outrage",
            Self::Clickbait => "Clickbait",
            Self::Attack => "Attack",
        }
    }

    /// Lower-case key for files and columns.
    pub fn key(self) -> &'static str {
        match self {
            Self::Informative => "informative",
            Self::Entertaining => "entertaining",
            Self::Personal => "personal",
            Self::Outrage => "outrage",
            Self::Clickbait => "clickbait",
            Self::Attack => "attack",
        }
    }

    /// Outrage and Attack are hostile styles.
    pub fn is_hostile(self) -> bool {
        matches!(self, Self::Outrage | Self::Attack)
    }

    /// The style's base profile.
    pub fn profile(self) -> StyleProfile {
        BASE_PROFILES[self.index()]
    }

    /// The style's base quality at a quality gap of `gap` (docs/model.md,
    /// 4.1): `mean + gap * (base - mean)`, where `mean` is the mean base
    /// quality of the six styles. Written as `base + (gap - 1) * (base -
    /// mean)`, so gap 1 returns the profile's value exactly.
    pub fn base_quality(self, gap: f32) -> f32 {
        let base = BASE_PROFILES[self.index()].quality;
        let mean = BASE_PROFILES.iter().map(|p| p.quality).sum::<f32>() / STYLE_COUNT as f32;
        base + (gap - 1.0) * (base - mean)
    }
}

/// Base attributes of a style (docs/model.md, 4.1).
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct StyleProfile {
    /// Craft and substance.
    pub quality: f32,
    /// How exciting it is.
    pub arousal: f32,
    /// Hostility toward the author's out-group.
    pub outrage: f32,
    /// Harm to readers and targets.
    pub toxicity: f32,
    /// Truthfulness; below 0.5 is misinformation.
    pub truth: f32,
}

#[rustfmt::skip]
const BASE_PROFILES: [StyleProfile; STYLE_COUNT] = [
    StyleProfile { quality: 0.75, arousal: 0.25, outrage: 0.05, toxicity: 0.02, truth: 0.92 },
    StyleProfile { quality: 0.50, arousal: 0.55, outrage: 0.05, toxicity: 0.05, truth: 0.85 },
    StyleProfile { quality: 0.45, arousal: 0.30, outrage: 0.02, toxicity: 0.02, truth: 0.95 },
    StyleProfile { quality: 0.30, arousal: 0.85, outrage: 0.85, toxicity: 0.35, truth: 0.60 },
    StyleProfile { quality: 0.15, arousal: 0.75, outrage: 0.20, toxicity: 0.08, truth: 0.35 },
    StyleProfile { quality: 0.05, arousal: 0.80, outrage: 0.70, toxicity: 0.85, truth: 0.50 },
];

/// Toxicity above which a post counts as toxic in the truth metrics.
pub const TOXIC_THRESHOLD: f32 = 0.5;
/// Truth below which a post is misinformation.
pub const MISINFORMATION_THRESHOLD: f32 = 0.5;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WorldAssumptions;
    use crate::rng::{self, Stream};

    #[test]
    fn hostile_posts_are_more_toxic_and_mostly_public_affairs() {
        let mut rng = rng::stream_rng(1, 0, 0, Stream::Posting);
        let author = AuthorProfile::plain(Group::A, 0.5, 0.8);
        let interests = [1.0 / 6.0; TOPIC_COUNT];
        let world = WorldAssumptions::default();
        let attacks: Vec<Post> = (0..500)
            .map(|i| generate_post(&mut rng, i, 0, &author, Style::Attack, &interests, &world))
            .collect();
        let topic0 = attacks.iter().filter(|p| p.topic == 0).count() as f32 / 500.0;
        let toxic = attacks.iter().filter(|p| p.is_toxic()).count() as f32 / 500.0;
        assert!(topic0 > 0.7 && topic0 < 0.85, "{topic0}");
        assert!(toxic > 0.95, "{toxic}");
    }

    #[test]
    fn evasion_lowers_appeal_not_toxicity() {
        let plain = AuthorProfile::plain(Group::A, 0.5, 0.8);
        let veiled = AuthorProfile {
            evasion: 0.5,
            veil: 0.3,
            ..plain
        };
        let interests = [1.0 / 6.0; TOPIC_COUNT];
        let world = WorldAssumptions::default();
        for i in 0..50 {
            let mut a = rng::stream_rng(2, 0, i, Stream::Posting);
            let mut b = rng::stream_rng(2, 0, i, Stream::Posting);
            let p = generate_post(&mut a, 0, 0, &plain, Style::Attack, &interests, &world);
            let q = generate_post(&mut b, 0, 0, &veiled, Style::Attack, &interests, &world);
            assert_eq!(p.toxicity, q.toxicity);
            assert!(p.toxicity > 0.3);
            assert!((q.arousal - 0.7 * p.arousal).abs() < 1e-6);
            assert!((q.outrage - 0.7 * p.outrage).abs() < 1e-6);
            assert!(q.detectability < p.detectability);
        }
    }

    #[test]
    fn intensity_moves_hostile_posts_only() {
        let neutral = AuthorProfile::plain(Group::A, 0.5, 0.3);
        let harsh = AuthorProfile {
            intensity: 1.0,
            ..neutral
        };
        let interests = [1.0 / 6.0; TOPIC_COUNT];
        let world = WorldAssumptions::default();
        let pair = |style: Style| {
            let mut a = rng::stream_rng(4, 0, 0, Stream::Posting);
            let mut b = rng::stream_rng(4, 0, 0, Stream::Posting);
            (
                generate_post(&mut a, 0, 0, &neutral, style, &interests, &world),
                generate_post(&mut b, 0, 0, &harsh, style, &interests, &world),
            )
        };
        let (p, q) = pair(Style::Outrage);
        assert!(
            (q.toxicity - p.toxicity - 0.15).abs() < 1e-5,
            "{} {}",
            p.toxicity,
            q.toxicity
        );
        assert!(q.outrage > p.outrage);
        let (p, q) = pair(Style::Personal);
        assert_eq!(p, q);
    }

    #[test]
    fn bridging_needs_both_groups() {
        let mut rng = rng::stream_rng(1, 0, 1, Stream::Posting);
        let author = AuthorProfile::plain(Group::A, 0.5, 0.2);
        let mut post = generate_post(
            &mut rng,
            0,
            0,
            &author,
            Style::Informative,
            &[1.0 / 6.0; 6],
            &WorldAssumptions::default(),
        );
        post.likes_a = 20;
        let one_sided = post.bridging();
        post.likes_b = 20;
        assert!(post.bridging() > 0.8 && one_sided < 0.01);
    }

    #[test]
    fn the_quality_gap_scales_base_quality_around_the_mean() {
        for style in Style::ALL {
            assert_eq!(style.base_quality(1.0), style.profile().quality);
        }
        let flat: Vec<f32> = Style::ALL.iter().map(|s| s.base_quality(0.0)).collect();
        assert!(flat.iter().all(|q| (q - flat[0]).abs() < 1e-6), "{flat:?}");
        let wide = Style::Informative.base_quality(1.5) - Style::Attack.base_quality(1.5);
        let gap = Style::Informative.base_quality(1.0) - Style::Attack.base_quality(1.0);
        assert!((wide - 1.5 * gap).abs() < 1e-5);
    }

    #[test]
    fn polish_raises_the_quality_signal_not_quality() {
        let world = WorldAssumptions {
            quality_signal_noise: 0.0,
            ..WorldAssumptions::default()
        };
        let plain = AuthorProfile::plain(Group::A, 0.5, 0.2);
        let polished = AuthorProfile {
            polished: true,
            ..plain
        };
        let interests = [1.0 / 6.0; TOPIC_COUNT];
        for i in 0..50 {
            let mut a = rng::stream_rng(6, 0, i, Stream::PostDraw);
            let mut b = rng::stream_rng(6, 0, i, Stream::PostDraw);
            let p = generate_post(&mut a, 0, 0, &plain, Style::Clickbait, &interests, &world);
            let q = generate_post(
                &mut b,
                0,
                0,
                &polished,
                Style::Clickbait,
                &interests,
                &world,
            );
            assert_eq!(p.quality, q.quality);
            assert_eq!(p.quality_signal, p.quality);
            assert!(!p.polished && q.polished);
            let expected = (p.quality + world.polish_gain).min(1.0);
            assert!((q.quality_signal - expected).abs() < 1e-6);
        }
    }
}
