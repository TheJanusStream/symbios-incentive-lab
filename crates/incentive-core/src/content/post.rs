//! Posts: their attributes, counters and generation (docs/model.md, 4.2).

use serde::Serialize;

use super::{Group, MISINFORMATION_THRESHOLD, Style, TOPIC_COUNT, TOXIC_THRESHOLD};
use crate::rng::{self, StreamRng};

/// A post and its counters. Counters include bot engagement where the
/// platform would count it; the `bot_*` fields let truth metrics remove it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Post {
    /// Unique, increasing id.
    pub id: u32,
    /// Author account (individual index, or a bot account above the population).
    pub author: u32,
    /// Whether a bot wrote it.
    pub author_is_bot: bool,
    /// Whether the author is a Creator, for social comparison.
    pub author_is_creator: bool,
    /// Day written.
    pub day: u32,
    /// Style.
    pub style: Style,
    /// Topic index into [`TOPICS`](super::TOPICS).
    pub topic: u8,
    /// The author's group.
    pub group: Group,
    /// Craft and substance.
    pub quality: f32,
    /// How exciting it is.
    pub arousal: f32,
    /// Hostility toward the author's out-group.
    pub outrage: f32,
    /// Harm to readers and targets.
    pub toxicity: f32,
    /// Truthfulness.
    pub truth: f32,
    /// How visible its toxicity is to classifiers.
    pub detectability: f32,
    /// The quality classifier's noisy score (observable).
    pub quality_signal: f32,
    /// The person an Attack targets.
    pub target: Option<u32>,
    /// Human impressions.
    pub impressions: u32,
    /// Likes from group A (bots included).
    pub likes_a: u32,
    /// Likes from group B (bots included).
    pub likes_b: u32,
    /// Likes from bots.
    pub bot_likes: u32,
    /// Friendly replies.
    pub positive_replies: u32,
    /// Angry replies.
    pub angry_replies: u32,
    /// Shares (bots included).
    pub shares: u32,
    /// Shares by bots.
    pub bot_shares: u32,
    /// Reports.
    pub reports: u32,
    /// See-less marks.
    pub see_less: u32,
    /// Removed by moderation.
    pub removed: bool,
    /// Removed by a human reviewer, which also strikes the author.
    pub struck: bool,
    /// Demoted by the classifier.
    pub demoted: bool,
    /// Has a community note.
    pub noted: bool,
    /// Already seen by a human reviewer.
    pub reviewed: bool,
    /// Followers the author gained from it.
    pub followers_gained: u32,
    /// Creator payout it earned.
    pub payout: f32,
    /// Human impressions today.
    pub impressions_today: u32,
    /// Observable likes, replies and shares today.
    pub engagement_today: u32,
    /// Likes today, bots included.
    pub likes_today: u32,
    /// Friendly replies today from mutual follows on a Personal post.
    pub mutual_replies_today: u32,
    /// Angry replies today.
    pub angry_today: u32,
    /// Human shares today.
    pub human_shares_today: u32,
}

impl Post {
    /// All likes, bots included.
    pub fn likes(&self) -> u32 {
        self.likes_a + self.likes_b
    }

    /// Likes from people.
    pub fn human_likes(&self) -> u32 {
        self.likes().saturating_sub(self.bot_likes)
    }

    /// Shares by people.
    pub fn human_shares(&self) -> u32 {
        self.shares.saturating_sub(self.bot_shares)
    }

    /// The platform's engagement count: likes, replies and shares, bots and
    /// angry replies included.
    pub fn observed_engagement(&self) -> u32 {
        self.likes() + self.positive_replies + self.angry_replies + self.shares
    }

    /// The bridging score: high when both groups like the post.
    pub fn bridging(&self) -> f32 {
        let (a, b) = (self.likes_a as f32, self.likes_b as f32);
        4.0 * a * b / ((a + b + 2.0) * (a + b + 2.0))
    }

    /// Whether the post is misinformation.
    pub fn is_misinformation(&self) -> bool {
        self.truth < MISINFORMATION_THRESHOLD
    }

    /// Whether the post is toxic.
    pub fn is_toxic(&self) -> bool {
        self.toxicity > TOXIC_THRESHOLD
    }

    /// Clears the per-day counters.
    pub(crate) fn reset_today(&mut self) {
        self.impressions_today = 0;
        self.engagement_today = 0;
        self.likes_today = 0;
        self.mutual_replies_today = 0;
        self.angry_today = 0;
        self.human_shares_today = 0;
    }
}

/// What a post generator needs to know about its author.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AuthorProfile {
    pub account: u32,
    pub is_bot: bool,
    pub is_creator: bool,
    pub group: Group,
    pub skill: f32,
    pub temperament: f32,
    pub evasion: f32,
}

/// Builds a post of `style` (docs/model.md, 4.2). The topic comes from
/// `interests`, except that hostile posts are mostly about public affairs.
pub(crate) fn generate_post(
    rng: &mut StreamRng,
    id: u32,
    day: u32,
    author: &AuthorProfile,
    style: Style,
    interests: &[f32; TOPIC_COUNT],
) -> Post {
    let base = style.profile();
    let hostile = if style.is_hostile() { 1.0 } else { 0.0 };
    let topic = if style.is_hostile() && rng::chance(rng, 0.7) {
        0
    } else {
        draw_topic(rng, interests)
    };
    let skill_term = if style == Style::Attack {
        0.0
    } else {
        0.4 * (author.skill - 0.5)
    };
    let quality = (base.quality + skill_term + rng::normal(rng, 0.08)).clamp(0.0, 1.0);
    let arousal = (base.arousal + rng::normal(rng, 0.08)).clamp(0.0, 1.0);
    let outrage = (base.outrage + 0.2 * author.temperament * hostile + rng::normal(rng, 0.05))
        .clamp(0.0, 1.0);
    let toxicity = (base.toxicity + 0.25 * author.temperament * hostile + rng::normal(rng, 0.05))
        .clamp(0.0, 1.0);
    let truth = (base.truth + 0.1 * (author.skill - 0.5) + rng::normal(rng, 0.08)).clamp(0.0, 1.0);
    let detectability = if toxicity > 0.3 {
        (0.9 - 0.8 * author.evasion).clamp(0.0, 1.0)
    } else {
        0.9
    };
    let quality_signal = (quality + rng::normal(rng, 0.15)).clamp(0.0, 1.0);
    Post {
        id,
        author: author.account,
        author_is_bot: author.is_bot,
        author_is_creator: author.is_creator,
        day,
        style,
        topic,
        group: author.group,
        quality,
        arousal,
        outrage,
        toxicity,
        truth,
        detectability,
        quality_signal,
        target: None,
        impressions: 0,
        likes_a: 0,
        likes_b: 0,
        bot_likes: 0,
        positive_replies: 0,
        angry_replies: 0,
        shares: 0,
        bot_shares: 0,
        reports: 0,
        see_less: 0,
        removed: false,
        struck: false,
        demoted: false,
        noted: false,
        reviewed: false,
        followers_gained: 0,
        payout: 0.0,
        impressions_today: 0,
        engagement_today: 0,
        likes_today: 0,
        mutual_replies_today: 0,
        angry_today: 0,
        human_shares_today: 0,
    }
}

fn draw_topic(rng: &mut StreamRng, interests: &[f32; TOPIC_COUNT]) -> u8 {
    let mut target = rng::uniform(rng);
    for (topic, weight) in interests.iter().enumerate() {
        if target < *weight {
            return topic as u8;
        }
        target -= weight;
    }
    (TOPIC_COUNT - 1) as u8
}
