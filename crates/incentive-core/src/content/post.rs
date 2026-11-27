//! Posts: their attributes, counters and generation (docs/model.md, 4.2).

use serde::Serialize;

use super::{Group, MISINFORMATION_THRESHOLD, Style, TOPIC_COUNT, TOXIC_THRESHOLD};
use crate::config::WorldAssumptions;
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
    /// The author's own post count before this post; with the author it
    /// keys the post's random draws (docs/model.md, section 12).
    pub ordinal: u32,
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
    /// The author's evasion it was written with, which its detectability
    /// used (net of that day's counter-evasion); 0 for a post not toxic
    /// enough to veil. The platform holds the post, so retraining may learn
    /// from it (docs/model.md, 6.7).
    pub evasion: f32,
    /// The quality classifier's noisy score (observable).
    pub quality_signal: f32,
    /// Polished by its author: a higher quality signal at a cost, with no
    /// change in quality (docs/model.md, 4.2).
    pub polished: bool,
    /// The person an Attack targets.
    pub target: Option<u32>,
    /// Human impressions.
    pub impressions: u32,
    /// Dwell time of all human impressions (observable).
    pub dwell: f32,
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
    /// Reports, one per reporting account at most.
    pub reports: u32,
    /// The accounts that reported it, in the order they did (observable).
    #[serde(skip)]
    pub reporters: Vec<u32>,
    /// See-less marks.
    pub see_less: u32,
    /// The bot accounts that marked it "see less", so a bot marks a post
    /// once at most.
    #[serde(skip)]
    pub bot_markers: Vec<u32>,
    /// Removed by moderation.
    pub removed: bool,
    /// Removed by a human reviewer, which also strikes the author.
    pub struck: bool,
    /// Demoted by the classifier.
    pub demoted: bool,
    /// Has a community note.
    pub noted: bool,
    /// Human shares when the note was attached; 0 for a post without a
    /// note. Kept for the truth metric `noted_shares_before_note`.
    #[serde(skip)]
    pub shares_at_note: u32,
    /// The toxicity classifier's score of the post, `flag` of docs/model.md,
    /// 6.7, which the platform computes when it screens the post
    /// (observable); `None` for a post it did not screen.
    #[serde(skip)]
    pub classifier_score: Option<f32>,
    /// Already seen by a human reviewer.
    pub reviewed: bool,
    /// Severity of the sanction its strike drew: 0 none, 1 a warning, 2 a
    /// suspension, 4 a ban (docs/model.md, 7.5).
    pub sanction_severity: u8,
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

    /// All replies, angry ones included.
    pub fn replies(&self) -> u32 {
        self.positive_replies + self.angry_replies
    }

    /// The bridging score: high when both groups like the post.
    pub fn bridging(&self) -> f32 {
        let (a, b) = (self.likes_a as f32, self.likes_b as f32);
        4.0 * a * b / ((a + b + 2.0) * (a + b + 2.0))
    }

    /// The entity of the post's own random draws: its author and the
    /// author's post ordinal.
    pub(crate) fn draw_key(&self) -> u64 {
        rng::post_key(self.author, self.ordinal)
    }

    /// Whether the post is misinformation.
    pub fn is_misinformation(&self) -> bool {
        self.truth < MISINFORMATION_THRESHOLD
    }

    /// Whether the post is toxic.
    pub fn is_toxic(&self) -> bool {
        self.toxicity > TOXIC_THRESHOLD
    }

    /// Files a report by `account`. An account reports a post once at most:
    /// returns false, changing nothing, when it already did.
    pub(crate) fn add_report(&mut self, account: u32) -> bool {
        if self.reporters.contains(&account) {
            return false;
        }
        self.reporters.push(account);
        self.reports += 1;
        true
    }

    /// Files a see-less mark by bot `account`, once at most per post;
    /// returns whether it was filed.
    pub(crate) fn add_bot_mark(&mut self, account: u32) -> bool {
        if self.bot_markers.contains(&account) {
            return false;
        }
        self.bot_markers.push(account);
        self.see_less += 1;
        true
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
    /// Evasion the classifier has not yet learned to see through,
    /// `max(0, evasion - counter_evasion)`.
    pub evasion: f32,
    /// The author's own evasion, which a veiled post records.
    pub written_evasion: f32,
    /// Share of a toxic post's arousal and outrage the author gives up by
    /// veiling it, `evasion_cost * evasion`.
    pub veil: f32,
    /// Learned intensity of hostile posts; [`NEUTRAL_INTENSITY`] adds nothing.
    pub intensity: f32,
    /// Whether the post being written is polished (chosen per post).
    pub polished: bool,
    /// The author's own post count before this post.
    pub ordinal: u32,
}

/// Toxicity above which a post is written in the author's evasive code.
pub(crate) const EVASION_TOXICITY: f32 = 0.3;
/// Hostile intensity of a new author and of every bot: no extra term.
pub(crate) const NEUTRAL_INTENSITY: f32 = 0.5;
/// Toxicity and outrage a hostile post gains per unit of intensity above
/// neutral: plus or minus 0.15 over the intensity range.
const INTENSITY_SPAN: f32 = 0.3;

impl AuthorProfile {
    /// An author with no evasion and neutral intensity.
    #[cfg(test)]
    pub(crate) fn plain(group: Group, skill: f32, temperament: f32) -> Self {
        Self {
            account: 0,
            is_bot: false,
            is_creator: false,
            group,
            skill,
            temperament,
            evasion: 0.0,
            written_evasion: 0.0,
            veil: 0.0,
            intensity: NEUTRAL_INTENSITY,
            polished: false,
            ordinal: 0,
        }
    }
}

/// Builds a post of `style` (docs/model.md, 4.2). The topic comes from
/// `interests`, except that hostile posts are mostly about public affairs.
/// The world sets the styles' quality gap, the quality classifier's noise
/// and what polish adds to the classifier's score.
pub(crate) fn generate_post(
    rng: &mut StreamRng,
    id: u32,
    day: u32,
    author: &AuthorProfile,
    style: Style,
    interests: &[f32; TOPIC_COUNT],
    world: &WorldAssumptions,
) -> Post {
    let base = style.profile();
    let base_quality = style.base_quality(world.style_quality_gap);
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
    let quality = (base_quality + skill_term + rng::normal(rng, 0.08)).clamp(0.0, 1.0);
    let arousal = (base.arousal + rng::normal(rng, 0.08)).clamp(0.0, 1.0);
    let push = INTENSITY_SPAN * (author.intensity - NEUTRAL_INTENSITY) * hostile;
    let outrage =
        (base.outrage + 0.2 * author.temperament * hostile + push + rng::normal(rng, 0.05))
            .clamp(0.0, 1.0);
    let toxicity =
        (base.toxicity + 0.25 * author.temperament * hostile + push + rng::normal(rng, 0.05))
            .clamp(0.0, 1.0);
    let truth = (base.truth + 0.1 * (author.skill - 0.5) + rng::normal(rng, 0.08)).clamp(0.0, 1.0);
    // An evasive author veils toxic posts: the code hides them from the
    // classifier and costs appeal, but hurts as much (docs/model.md, 4.2).
    let (detectability, appeal, evasion) = if toxicity > EVASION_TOXICITY {
        (
            (0.9 - 0.8 * author.evasion).clamp(0.0, 1.0),
            (1.0 - author.veil).clamp(0.0, 1.0),
            author.written_evasion,
        )
    } else {
        (0.9, 1.0, 0.0)
    };
    let arousal = arousal * appeal;
    let outrage = outrage * appeal;
    // Polish raises the classifier's score, not the post (docs/model.md, 6.3).
    let polish = if author.polished {
        world.polish_gain
    } else {
        0.0
    };
    let quality_signal =
        (quality + polish + rng::normal(rng, world.quality_signal_noise)).clamp(0.0, 1.0);
    Post {
        id,
        author: author.account,
        author_is_bot: author.is_bot,
        author_is_creator: author.is_creator,
        ordinal: author.ordinal,
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
        evasion,
        quality_signal,
        polished: author.polished,
        target: None,
        impressions: 0,
        dwell: 0.0,
        likes_a: 0,
        likes_b: 0,
        bot_likes: 0,
        positive_replies: 0,
        angry_replies: 0,
        shares: 0,
        bot_shares: 0,
        reports: 0,
        reporters: Vec::new(),
        see_less: 0,
        bot_markers: Vec::new(),
        removed: false,
        struck: false,
        demoted: false,
        noted: false,
        shares_at_note: 0,
        classifier_score: None,
        reviewed: false,
        sanction_severity: 0,
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
