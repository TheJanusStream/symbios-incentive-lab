//! Styles, topics, groups and posts (docs/model.md, section 4).

use serde::Serialize;

mod post;

pub use post::Post;
pub(crate) use post::{AuthorProfile, generate_post};

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
    use crate::rng::{self, Stream};

    #[test]
    fn hostile_posts_are_more_toxic_and_mostly_public_affairs() {
        let mut rng = rng::stream_rng(1, 0, 0, Stream::Posting);
        let author = AuthorProfile {
            account: 0,
            is_bot: false,
            is_creator: false,
            group: Group::A,
            skill: 0.5,
            temperament: 0.8,
            evasion: 0.0,
        };
        let interests = [1.0 / 6.0; TOPIC_COUNT];
        let attacks: Vec<Post> = (0..500)
            .map(|i| generate_post(&mut rng, i, 0, &author, Style::Attack, &interests))
            .collect();
        let topic0 = attacks.iter().filter(|p| p.topic == 0).count() as f32 / 500.0;
        let toxic = attacks.iter().filter(|p| p.is_toxic()).count() as f32 / 500.0;
        assert!(topic0 > 0.7 && topic0 < 0.85, "{topic0}");
        assert!(toxic > 0.95, "{toxic}");
    }

    #[test]
    fn bridging_needs_both_groups() {
        let mut rng = rng::stream_rng(1, 0, 1, Stream::Posting);
        let author = AuthorProfile {
            account: 0,
            is_bot: false,
            is_creator: false,
            group: Group::A,
            skill: 0.5,
            temperament: 0.2,
            evasion: 0.0,
        };
        let mut post = generate_post(&mut rng, 0, 0, &author, Style::Informative, &[1.0 / 6.0; 6]);
        post.likes_a = 20;
        let one_sided = post.bridging();
        post.likes_b = 20;
        assert!(post.bridging() > 0.8 && one_sided < 0.01);
    }
}
