//! The player's platform: design levers (docs/model.md, section 10).

use serde::{Deserialize, Serialize};

/// What automated moderation does with a flagged post.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModerationAction {
    /// The post disappears.
    Remove,
    /// The post stays but is ranked down by `demote_penalty`.
    Demote,
}

/// How strikes from human review escalate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Sanctions {
    /// Strikes have no consequence beyond the removal.
    None,
    /// Warning, 3-day suspension, 14-day suspension, ban.
    Graduated,
    /// A ban at the first strike.
    Strict,
}

/// How the creator payout pool is divided.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PayoutScheme {
    /// By human impressions.
    PerView,
    /// By observable likes, replies and shares, bots and angry replies included.
    PerEngagement,
    /// Engagement times the quality classifier's score.
    QualityWeighted,
    /// Each user's share of revenue goes to the authors who gave that user value.
    FanFunded,
}

/// The alternative feed some users choose.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AltFeed {
    /// Accounts followed, newest first.
    Chronological,
    /// Ranked mostly by the quality classifier.
    Quality,
    /// Ranked mostly by approval from both groups.
    Bridging,
}

/// The player's platform (docs/model.md, section 10). The default is the
/// Engagement maximiser.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DesignLevers {
    /// Ranking weight of the predicted like probability.
    pub w_like: f32,
    /// Ranking weight of the predicted reply probability.
    pub w_reply: f32,
    /// Ranking weight of the predicted share probability.
    pub w_share: f32,
    /// Ranking weight of the predicted dwell time.
    pub w_dwell: f32,
    /// Ranking weight of the quality classifier's score.
    pub w_quality: f32,
    /// Ranking weight of the bridging score.
    pub w_bridging: f32,
    /// Ranking penalty per unit of the author's see-less rate.
    pub w_see_less: f32,
    /// Ranking weight of freshness.
    pub w_recency: f32,
    /// Ranking bonus for posts from followed accounts.
    pub w_in_network: f32,
    /// Followed accounts only, newest first, no ranking.
    pub chronological: bool,
    /// Share of feed candidates recommended from outside the network.
    pub recommended_share: f32,
    /// Strength of diversity re-ranking.
    pub diversity: f32,
    /// How well the platform predicts true engagement.
    pub predictor_quality: f32,
    /// Ranking penalty of a demoted post.
    pub demote_penalty: f32,
    /// Ranking penalty of a post with a community note.
    pub noted_penalty: f32,
    /// Whether like counts are shown.
    pub like_counts_visible: bool,
    /// Deepest reshare that can still be shared; `None` means no limit.
    pub reshare_depth_cap: Option<u32>,
    /// Strength of the prompt before sharing.
    pub share_friction: f32,
    /// Whether the automated toxicity classifier runs.
    pub classifier_enabled: bool,
    /// Classifier score above which a post is actioned.
    pub classifier_threshold: f32,
    /// What the classifier does with a flagged post.
    pub moderation_action: ModerationAction,
    /// Human reviews per 1,000 users per day.
    pub review_capacity: f32,
    /// How strikes escalate.
    pub sanctions: Sanctions,
    /// Whether community notes run.
    pub community_notes: bool,
    /// Share of revenue paid to authors.
    pub payout_share: f32,
    /// How the payout pool is divided.
    pub payout_scheme: PayoutScheme,
    /// Followers an account needs before it is paid.
    pub payout_threshold_followers: u32,
    /// Chance that a block stops a harassing notification.
    pub block_effectiveness: f32,
    /// Share of users who opt into a daily time limit.
    pub time_limit_share: f32,
    /// Strength of notifications.
    pub notification_intensity: f32,
    /// Share of users on the alternative feed.
    pub alt_feed_share: f32,
    /// The alternative feed.
    pub alt_feed: AltFeed,
    /// Cost of opening an account.
    pub account_cost: f32,
    /// Share of the business funded by ads; the rest by subscriptions.
    pub ads_share: f32,
}

impl Default for DesignLevers {
    fn default() -> Self {
        Self {
            w_like: 1.0,
            w_reply: 1.5,
            w_share: 2.0,
            w_dwell: 0.5,
            w_quality: 0.0,
            w_bridging: 0.0,
            w_see_less: 0.0,
            w_recency: 0.3,
            w_in_network: 0.2,
            chronological: false,
            recommended_share: 0.4,
            diversity: 0.0,
            predictor_quality: 0.85,
            demote_penalty: 1.0,
            noted_penalty: 0.5,
            like_counts_visible: true,
            reshare_depth_cap: None,
            share_friction: 0.0,
            classifier_enabled: true,
            classifier_threshold: 0.7,
            moderation_action: ModerationAction::Remove,
            review_capacity: 1.0,
            sanctions: Sanctions::Graduated,
            community_notes: false,
            payout_share: 0.0,
            payout_scheme: PayoutScheme::PerEngagement,
            payout_threshold_followers: 0,
            block_effectiveness: 0.8,
            time_limit_share: 0.0,
            notification_intensity: 0.6,
            alt_feed_share: 0.0,
            alt_feed: AltFeed::Chronological,
            account_cost: 0.2,
            ads_share: 1.0,
        }
    }
}
