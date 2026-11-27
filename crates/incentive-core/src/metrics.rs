//! The metric catalogue, daily metrics and run summaries (docs/model.md,
//! section 9).
//!
//! Missing values are `f32::NAN`: survey satisfaction exists only on survey
//! days, 30-day retention only from day 30, the experiments' measures (the
//! deactivation experiment's, the notifications-off experiment's and the
//! feed experiment's) only on the last day of a run with that experiment,
//! and a diagnostic only on a day that has its subject. [`DailyMetrics::get_opt`]
//! turns them into `None`, summaries average only the days that have a value,
//! and JSON writes them as `null`.

use serde::Serialize;
use serde::ser::{SerializeMap, Serializer};

use crate::content::{STYLE_COUNT, Style};
use crate::population::{ARCHETYPE_COUNT, Archetype};

mod summary;

pub use summary::{
    MOVING_STANDARD_ERRORS, MOVING_THRESHOLD, Summary, is_moving_over, is_moving_slope,
};

/// Whether the platform can measure a metric or only the god's-eye view has it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Visibility {
    /// The platform can measure it.
    Observable,
    /// Only the simulation knows it.
    Truth,
}

/// What a metric is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Family {
    /// How much people use the platform.
    Engagement,
    /// Toxicity, harassment, misinformation, bots.
    Harm,
    /// The people on the platform.
    Participants,
    /// People who do not use the platform.
    NonUsers,
    /// The public conversation.
    Discourse,
    /// Creators' livelihoods.
    Creators,
    /// What gets written.
    Content,
    /// The platform's own operations.
    Platform,
    /// The composite indices.
    Index,
}

/// How to show a metric's value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Format {
    /// A whole count per day.
    Count,
    /// A share in `[0, 1]`, best shown as a percentage.
    Share,
    /// A score in `[0, 1]`.
    Score,
    /// A plain number with decimals.
    Decimal,
    /// Money per day.
    Money,
}

macro_rules! metrics {
    ($( $id:ident, $key:literal, $name:literal, $format:ident, $family:ident, $vis:ident, $desc:literal; )*) => {
        /// Every metric of docs/model.md, section 9.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
        pub enum MetricId { $( #[doc = $desc] $id, )* }

        const INFOS: &[MetricInfo] = &[ $( MetricInfo {
            id: MetricId::$id, key: $key, name: $name, format: Format::$format,
            family: Family::$family, visibility: Visibility::$vis, description: $desc,
        }, )* ];
    };
}

#[rustfmt::skip]
metrics! {
    ReportedDailyActives, "reported_dau", "Reported daily actives", Count, Engagement, Observable, "Accounts that were active today, bots included.";
    ReportedAccounts, "reported_accounts", "Reported accounts", Count, Platform, Observable, "Accounts on the platform, bots included.";
    NewAccounts, "new_accounts", "New accounts", Count, Platform, Observable, "Accounts opened today: joiners, returners and new bots.";
    Departures, "departures", "Departures", Count, Platform, Observable, "Accounts that left or were banned today.";
    Impressions, "impressions", "Impressions", Count, Engagement, Observable, "Items people saw today.";
    TimeSpent, "time_spent", "Time spent", Decimal, Engagement, Observable, "Total dwell time today.";
    Likes, "likes", "Likes", Count, Engagement, Observable, "Likes today, bots included.";
    Replies, "replies", "Replies", Count, Engagement, Observable, "Replies today, angry ones included.";
    Shares, "shares", "Shares", Count, Engagement, Observable, "Shares today, bots included.";
    ImpressionsPerActive, "impressions_per_active", "Impressions per active", Decimal, Engagement, Observable, "Impressions per reported daily active.";
    TimePerActive, "time_per_active", "Time per active", Decimal, Engagement, Observable, "Dwell time per reported daily active.";
    LikesPerActive, "likes_per_active", "Likes per active", Decimal, Engagement, Observable, "Likes per reported daily active.";
    RepliesPerActive, "replies_per_active", "Replies per active", Decimal, Engagement, Observable, "Replies per reported daily active.";
    SharesPerActive, "shares_per_active", "Shares per active", Decimal, Engagement, Observable, "Shares per reported daily active.";
    SessionsPerActive, "sessions_per_active", "Sessions per active", Decimal, Engagement, Observable, "App sessions per reported daily active; each active bot counts as one session.";
    NotificationSessionShare, "notification_session_share", "Sessions from notifications", Share, Engagement, Observable, "Share of today's sessions that a notification started.";
    CandidatesPerFeed, "candidates_per_feed", "Candidates per feed", Decimal, Platform, Observable, "Mean candidates gathered for each feed served today, before ranking cuts the feed to its length (6.2).";
    Reports, "reports", "Reports", Count, Platform, Observable, "Posts reported today.";
    SeeLess, "see_less", "See-less marks", Count, Platform, Observable, "\"Show me less\" marks today.";
    AutoRemovals, "auto_removals", "Automated removals", Count, Platform, Observable, "Posts the classifier removed today.";
    HumanRemovals, "human_removals", "Human removals", Count, Platform, Observable, "Posts human reviewers removed today.";
    Notes, "notes", "Community notes", Count, Platform, Observable, "Community notes that took effect today: shown on a post for the first time, or due by the day's end on a post with at least 50 impressions.";
    Revenue, "revenue", "Revenue", Money, Platform, Observable, "Ad and subscription revenue today.";
    AdRevenue, "ad_revenue", "Ad revenue", Money, Platform, Observable, "Revenue from the ads people saw today.";
    SubscriptionRevenue, "subscription_revenue", "Subscription revenue", Money, Platform, Observable, "Revenue from subscriptions today.";
    SubscriberShare, "subscriber_share", "Subscribers per active", Share, Platform, Observable, "Subscribers per reported daily active.";
    Payouts, "payouts", "Creator payouts", Money, Creators, Observable, "Money paid to authors today.";
    NetRevenue, "net_revenue", "Net revenue", Money, Platform, Observable, "Revenue minus creator payouts today.";
    Retention30, "retention_30", "30-day retention", Share, Engagement, Observable, "Share of people active 30 days ago who are active today.";
    SurveySatisfaction, "survey_satisfaction", "Survey satisfaction", Score, Participants, Observable, "Weekly survey of users; biased toward the happy and blind to those who left.";
    LargestCascade, "largest_cascade", "Largest cascade", Count, Content, Truth, "Most human shares of one post among the posts ending today.";
    HumanDailyActives, "human_dau", "Human daily actives", Count, Engagement, Truth, "People who opened the app today.";
    UserShare, "user_share", "Users in society", Share, Engagement, Truth, "Share of all individuals who are users.";
    RegrettedShare, "regretted_share", "Regretted time", Share, Engagement, Truth, "Share of items beyond what people would endorse spending.";
    ToxicShare, "toxic_share", "Toxic impressions", Share, Harm, Truth, "Share of impressions with toxicity above 0.5.";
    OutrageShare, "outrage_share", "Outrage impressions", Share, Harm, Truth, "Share of impressions with outrage above 0.5.";
    MisinformationShare, "misinformation_share", "Misinformation impressions", Share, Harm, Truth, "Share of impressions with truth below 0.5.";
    HarassmentPer1000, "harassment_per_1000", "Harassment per 1,000 users", Decimal, Harm, Truth, "Harassment events per 1,000 users today.";
    FalsePositives, "false_positives", "False-positive actions", Count, Harm, Truth, "Posts the classifier removed or demoted although they were not toxic.";
    BotEngagementShare, "bot_engagement_share", "Bot engagement", Share, Harm, Truth, "Share of reported engagement that came from bots.";
    BotFlagShare, "bot_flag_share", "Bot reports and marks", Share, Harm, Truth, "Share of reports and see-less marks that brigading bots filed.";
    ReflectivePerImpression, "reflective_per_impression", "Value per impression", Decimal, Participants, Truth, "Mean reflective value of what people were shown.";
    LowValueTime, "low_value_time", "Time not worth it", Share, Participants, Truth, "Share of time spent on items worth less, on reflection, than the time they took.";
    WellbeingUsers, "wellbeing_users", "Wellbeing of users", Score, Participants, Truth, "Mean wellbeing of current users.";
    WellbeingEverUsers, "wellbeing_ever_users", "Wellbeing of all who used it", Score, Participants, Truth, "Mean wellbeing of current and former users, free of survivorship bias.";
    MeaningfulPerUser, "meaningful_per_user", "Meaningful interactions", Decimal, Participants, Truth, "Likes and friendly replies between mutual follows on personal posts, per user.";
    VulnerableLeftShare, "vulnerable_left_share", "Vulnerable who left", Share, Participants, Truth, "Share of vulnerable people who used the platform and left.";
    DeactivationEffect, "deactivation_effect", "Deactivation effect", Decimal, Participants, Truth, "Wellbeing gain of a deactivation experiment's break group over the users who stayed, in standard deviations; only on the last day of a run that sets one up.";
    BreakChange, "break_change", "Break group's change", Decimal, Participants, Truth, "Wellbeing change of a deactivation experiment's break group, in standard deviations; compared between a run and its placebo, it is the break's effect; only on the last day of a run that sets one up.";
    NotificationsOffTime, "notifications_off_time", "Notifications-off time change", Decimal, Participants, Truth, "Change in daily time on the platform of a notifications-off experiment's treated group, from the week before, in standard deviations; compared between a run and its placebo, it is the effect of switching notifications off; only on the last day of a run that sets one up.";
    NotificationsOffSessions, "notifications_off_sessions", "Notifications-off sessions change", Decimal, Participants, Truth, "Change in sessions a day of a notifications-off experiment's treated group, from the week before, in standard deviations; compared between a run and its placebo, it is the effect of switching notifications off; only on the last day of a run that sets one up.";
    NotificationsOffWellbeing, "notifications_off_wellbeing", "Notifications-off wellbeing change", Decimal, Participants, Truth, "Wellbeing change of a notifications-off experiment's treated group, in standard deviations; compared between a run and its placebo, it is the effect of switching notifications off; only on the last day of a run that sets one up.";
    PolarisationUsers, "polarisation_users", "Polarisation of users", Score, Participants, Truth, "Mean affective polarisation of users.";
    InformednessUsers, "informedness_users", "Informedness of users", Score, Participants, Truth, "Mean informedness of users.";
    PolarisationNonUsers, "polarisation_non_users", "Polarisation of non-users", Score, NonUsers, Truth, "Mean affective polarisation of non-users.";
    InformednessNonUsers, "informedness_non_users", "Informedness of non-users", Score, NonUsers, Truth, "Mean informedness of non-users.";
    TopToxicShare, "top_toxic_share", "Toxic top posts", Share, Discourse, Truth, "Toxic share of the day's top 5% most-viewed posts.";
    TopTruthfulShare, "top_truthful_share", "Truthful top posts", Share, Discourse, Truth, "Truthful share of the day's top 5% most-viewed posts.";
    CrossGroupLikes, "cross_group_likes", "Cross-group likes", Decimal, Discourse, Truth, "Likes across the group line per impression.";
    PolarisationSociety, "polarisation_society", "Polarisation of society", Score, Discourse, Truth, "Mean affective polarisation of everyone.";
    InformednessSociety, "informedness_society", "Informedness of society", Score, Discourse, Truth, "Mean informedness of everyone.";
    CreatorsLiving, "creators_living", "Creators earning a living", Share, Creators, Truth, "Share of Creators who ever used the platform whose income reaches a living.";
    CreatorMedianIncome, "creator_median_income", "Median creator income", Money, Creators, Truth, "Median 30-day average daily income of those Creators.";
    CreatorIncomeGini, "creator_income_gini", "Creator income inequality", Score, Creators, Truth, "Gini coefficient of those Creators' incomes.";
    HarmfulPayoutShare, "harmful_payout_share", "Payouts to harmful styles", Share, Creators, Truth, "Share of payouts going to Outrage, Clickbait and Attack posts.";
    PostShareInformative, "posts_informative", "Informative posts", Share, Content, Truth, "Share of today's human posts that are Informative.";
    PostShareEntertaining, "posts_entertaining", "Entertaining posts", Share, Content, Truth, "Share of today's human posts that are Entertaining.";
    PostSharePersonal, "posts_personal", "Personal posts", Share, Content, Truth, "Share of today's human posts that are Personal.";
    PostShareOutrage, "posts_outrage", "Outrage posts", Share, Content, Truth, "Share of today's human posts that are Outrage.";
    PostShareClickbait, "posts_clickbait", "Clickbait posts", Share, Content, Truth, "Share of today's human posts that are Clickbait.";
    PostShareAttack, "posts_attack", "Attack posts", Share, Content, Truth, "Share of today's human posts that are Attacks.";
    PolishedShare, "polished_share", "Polished posts", Share, Content, Truth, "Share of today's human posts polished to score higher with the quality classifier.";
    BotPosts, "bot_posts", "Bot posts", Count, Content, Truth, "Posts bots wrote today; the content mix counts human posts only.";
    EngagementIndex, "engagement_index", "Engagement index", Score, Index, Observable, "Human impressions against a full-attention benchmark.";
    HarmIndex, "harm_index", "Harm index", Score, Index, Truth, "Mean of toxicity, harassment, misinformation, polarisation growth and wrongful sanctions, each scaled to [0, 1].";
    HarmIndexExPolarisation, "harm_index_ex_polarisation", "Harm index without polarisation", Score, Index, Truth, "The harm index without its polarisation component: the mean of toxicity, harassment, misinformation and wrongful sanctions.";
    DiscourseQuality, "discourse_quality", "Discourse quality", Score, Index, Truth, "Mean of civil and truthful top posts, cross-group approval and stable polarisation.";
    BenefitIndex, "benefit_index", "Benefit index", Score, Index, Truth, "Mean of wellbeing, informedness, meaningful interactions, creator livelihoods and discourse quality.";
    BenefitIndexExCreators, "benefit_index_ex_creators", "Benefit index without creators", Score, Index, Truth, "The benefit index without its creators-living component: the mean of wellbeing, informedness, meaningful interactions and discourse quality.";
    FeedExperimentDwell, "feed_experiment_dwell", "Feed experiment: time per day", Decimal, Participants, Truth, "Mean daily time on the platform of a feed experiment's treated group over its days, leavers counted with none; compared between a run and its placebo; only on the last day of a run that sets one up.";
    FeedExperimentEngagement, "feed_experiment_engagement", "Feed experiment: actions per day", Decimal, Participants, Truth, "Mean daily likes, replies and shares given by a feed experiment's treated group over its days, leavers counted with none; only on the last day of a run that sets one up.";
    FeedExperimentToxicShare, "feed_experiment_toxic_share", "Feed experiment: toxic impressions", Share, Participants, Truth, "Toxic share of a feed experiment's treated group's impressions over its days; only on the last day of a run that sets one up.";
    FeedExperimentMisinformationShare, "feed_experiment_misinformation_share", "Feed experiment: misinformation impressions", Share, Participants, Truth, "Misinformation share of a feed experiment's treated group's impressions over its days; only on the last day of a run that sets one up.";
    FeedExperimentSameGroupShare, "feed_experiment_same_group_share", "Feed experiment: same-group impressions", Share, Participants, Truth, "Share of a feed experiment's treated group's impressions that were posts by their own group, over its days; only on the last day of a run that sets one up.";
    FeedExperimentPostToxicity, "feed_experiment_post_toxicity", "Feed experiment: toxicity of own posts", Score, Participants, Truth, "Mean toxicity of the posts a feed experiment's treated group wrote over its days; only on the last day of a run that sets one up.";
    FeedExperimentSharedTruth, "feed_experiment_shared_truth", "Feed experiment: truth of shares", Score, Participants, Truth, "Mean truth of the posts a feed experiment's treated group shared over its days; only on the last day of a run that sets one up.";
    FeedExperimentPolarisation, "feed_experiment_polarisation", "Feed experiment: polarisation change", Decimal, Participants, Truth, "Change in affective polarisation of a feed experiment's treated group over its days, in standard deviations of the participants at its start; only on the last day of a run that sets one up.";
    FeedExperimentInformedness, "feed_experiment_informedness", "Feed experiment: informedness change", Decimal, Participants, Truth, "Change in informedness of a feed experiment's treated group over its days, in standard deviations of the participants at its start; only on the last day of a run that sets one up.";
    FeedExperimentWellbeing, "feed_experiment_wellbeing", "Feed experiment: wellbeing change", Decimal, Participants, Truth, "Change in wellbeing of a feed experiment's treated group over its days, in standard deviations of the participants at its start; only on the last day of a run that sets one up.";
    FeedExperimentFlaggedShare, "feed_experiment_flagged_share", "Feed experiment: flagged impressions", Share, Participants, Truth, "Share of a hide-flagged feed experiment's treated group's impressions whose classifier score exceeds its threshold, over its days: the dose, read in the placebo run; only on the last day of a run that sets one up.";
    FeedExperimentPolarisationRaw, "feed_experiment_polarisation_raw", "Feed experiment: polarisation change, unscaled", Decimal, Participants, Truth, "Mean change in affective polarisation of a feed experiment's treated group over its days, in its own units; only on the last day of a run that sets one up.";
    FeedExperimentInformednessRaw, "feed_experiment_informedness_raw", "Feed experiment: informedness change, unscaled", Decimal, Participants, Truth, "Mean change in informedness of a feed experiment's treated group over its days, in its own units; only on the last day of a run that sets one up.";
    BreakPolarisation, "break_polarisation", "Break group's polarisation change", Decimal, Participants, Truth, "Change in affective polarisation of a deactivation experiment's break group over the break, in standard deviations of the participants at its start; only on the last day of a run that sets one up.";
    BreakInformedness, "break_informedness", "Break group's informedness change", Decimal, Participants, Truth, "Change in informedness of a deactivation experiment's break group over the break, in standard deviations of the participants at its start; only on the last day of a run that sets one up.";
    BreakPolarisationRaw, "break_polarisation_raw", "Break group's polarisation change, unscaled", Decimal, Participants, Truth, "Change in affective polarisation of a deactivation experiment's break group over the break, in its own units; only on the last day of a run that sets one up.";
    BreakInformednessRaw, "break_informedness_raw", "Break group's informedness change, unscaled", Decimal, Participants, Truth, "Change in informedness of a deactivation experiment's break group over the break, in its own units; only on the last day of a run that sets one up.";
    PostBreakDwell, "post_break_dwell", "Break group's time after the break", Decimal, Participants, Truth, "Mean daily time on the platform of a deactivation experiment's break group from the break's end to the run's end, leavers counted with none; only on the last day of a run whose break ends before it.";
    PartisanHalfReach, "partisan_half_reach", "Reach of the more partisan half", Decimal, Discourse, Truth, "Mean lifetime human impressions of the posts ending today by authors in the more partisan half by the partisanship trait; a diagnostic.";
    ExtremeReachRatio, "extreme_reach_ratio", "Reach of the most partisan", Decimal, Discourse, Truth, "Mean reach of the posts ending today by the most partisan tenth of authors over that of the middle fifth; a diagnostic.";
    InNetworkOutgroupCandidates, "in_network_outgroup_candidates", "Out-group in-network candidates", Share, Discourse, Truth, "Share of the in-network candidates gathered for today's feeds that are posts by the viewer's out-group; a diagnostic.";
    InNetworkOutgroupImpressions, "in_network_outgroup_impressions", "Out-group in-network impressions", Share, Discourse, Truth, "Share of today's in-network impressions that are posts by the viewer's out-group; a diagnostic.";
    InNetworkOutgroupLikes, "in_network_outgroup_likes", "Out-group in-network likes", Share, Discourse, Truth, "Share of today's likes on in-network impressions that went to posts by the viewer's out-group; a diagnostic.";
    OutgroupRankingRatio, "outgroup_ranking_ratio", "Out-group exposure after ranking", Decimal, Discourse, Truth, "The out-group share of in-network impressions over that of in-network candidates; a diagnostic.";
    OutgroupLikeRatio, "outgroup_like_ratio", "Out-group likes after exposure", Decimal, Discourse, Truth, "The out-group share of likes on in-network impressions over that of in-network impressions; a diagnostic.";
    NotedSharesBeforeNote, "noted_shares_before_note", "Shares before the note", Share, Harm, Truth, "Share of the human shares of noted posts ending today that were made before the note; a diagnostic.";
    OutrageShareRatio, "outrage_share_ratio", "Outrage share rate ratio", Decimal, Content, Truth, "Human shares per impression of Outrage posts over those of Informative, Entertaining and Personal posts, today; a diagnostic.";
    SharedTruth, "shared_truth", "Truth of shares", Score, Harm, Truth, "Mean truth of the posts people shared today; a diagnostic.";
    BreakWellbeingSd, "break_wellbeing_sd", "Break group: wellbeing SD", Decimal, Participants, Truth, "Standard deviation of all participants' wellbeing at the start of a deactivation experiment, the scale of its wellbeing changes; only on the last day of a run that sets one up.";
    BreakPolarisationSd, "break_polarisation_sd", "Break group: polarisation SD", Decimal, Participants, Truth, "Standard deviation of all participants' affective polarisation at the start of a deactivation experiment, the scale of its polarisation change; only on the last day of a run that sets one up.";
    BreakInformednessSd, "break_informedness_sd", "Break group: informedness SD", Decimal, Participants, Truth, "Standard deviation of all participants' informedness at the start of a deactivation experiment, the scale of its informedness change; only on the last day of a run that sets one up.";
    NotificationsOffTimeSd, "notifications_off_time_sd", "Notifications-off: time SD", Decimal, Participants, Truth, "Standard deviation of the participants' baseline daily time on the platform, the scale of a notifications-off experiment's time change; only on the last day of a run that sets one up.";
    NotificationsOffSessionsSd, "notifications_off_sessions_sd", "Notifications-off: sessions SD", Decimal, Participants, Truth, "Standard deviation of the participants' baseline sessions a day, the scale of a notifications-off experiment's sessions change; only on the last day of a run that sets one up.";
    NotificationsOffWellbeingSd, "notifications_off_wellbeing_sd", "Notifications-off: wellbeing SD", Decimal, Participants, Truth, "Standard deviation of the participants' wellbeing at the start of a notifications-off experiment, the scale of its wellbeing change; only on the last day of a run that sets one up.";
    FeedExperimentPolarisationSd, "feed_experiment_polarisation_sd", "Feed experiment: polarisation SD", Decimal, Participants, Truth, "Standard deviation of all participants' affective polarisation at the start of a feed experiment, the scale of its polarisation change; only on the last day of a run that sets one up.";
    FeedExperimentInformednessSd, "feed_experiment_informedness_sd", "Feed experiment: informedness SD", Decimal, Participants, Truth, "Standard deviation of all participants' informedness at the start of a feed experiment, the scale of its informedness change; only on the last day of a run that sets one up.";
    FeedExperimentWellbeingSd, "feed_experiment_wellbeing_sd", "Feed experiment: wellbeing SD", Decimal, Participants, Truth, "Standard deviation of all participants' wellbeing at the start of a feed experiment, the scale of its wellbeing change; only on the last day of a run that sets one up.";
    FeedExperimentPartisanHalfImpressions, "feed_experiment_partisan_half_impressions", "Feed experiment: partisan half's impressions", Decimal, Participants, Truth, "Impressions per treated viewer and day of posts by authors in the more partisan half by the partisanship trait, pooled over a feed experiment's treated group and days. Only on the last day of a run that sets one up.";
    FeedExperimentExtremeImpressions, "feed_experiment_extreme_impressions", "Feed experiment: most partisan tenth's impressions", Decimal, Participants, Truth, "Impressions per treated viewer and day of posts by the most partisan tenth of authors, pooled over a feed experiment's treated group and days. Only on the last day of a run that sets one up.";
    FeedExperimentMiddleImpressions, "feed_experiment_middle_impressions", "Feed experiment: middle fifth's impressions", Decimal, Participants, Truth, "Impressions per treated viewer and day of posts by the middle fifth of authors by partisanship (40% to 60%), pooled over a feed experiment's treated group and days. Only on the last day of a run that sets one up.";
    FeedExperimentImpressions, "feed_experiment_impressions", "Feed experiment: impressions", Decimal, Participants, Truth, "Impressions per treated viewer and day, pooled over a feed experiment's treated group and days. Only on the last day of a run that sets one up.";
    FeedExperimentExtremeMiddleRatio, "feed_experiment_extreme_middle_ratio", "Feed experiment: most partisan over middle", Decimal, Participants, Truth, "A feed experiment's treated group's impressions of posts by the most partisan tenth of authors over those of the middle fifth, pooled over its days. Only on the last day of a run that sets one up.";
    FeedExperimentPartisanHalfImpressionsKept, "feed_experiment_partisan_half_impressions_kept", "Feed experiment: partisan half's impressions, kept", Decimal, Participants, Truth, "Impressions per treated viewer and day of posts by authors in the more partisan half by the partisanship trait, pooled over a feed experiment's treated group and days. Posts removed or demoted by the end of their life are left out. Only on the last day of a run that sets one up.";
    FeedExperimentExtremeImpressionsKept, "feed_experiment_extreme_impressions_kept", "Feed experiment: most partisan tenth's impressions, kept", Decimal, Participants, Truth, "Impressions per treated viewer and day of posts by the most partisan tenth of authors, pooled over a feed experiment's treated group and days. Posts removed or demoted by the end of their life are left out. Only on the last day of a run that sets one up.";
    FeedExperimentMiddleImpressionsKept, "feed_experiment_middle_impressions_kept", "Feed experiment: middle fifth's impressions, kept", Decimal, Participants, Truth, "Impressions per treated viewer and day of posts by the middle fifth of authors by partisanship (40% to 60%), pooled over a feed experiment's treated group and days. Posts removed or demoted by the end of their life are left out. Only on the last day of a run that sets one up.";
    FeedExperimentImpressionsKept, "feed_experiment_impressions_kept", "Feed experiment: impressions, kept", Decimal, Participants, Truth, "Impressions per treated viewer and day, pooled over a feed experiment's treated group and days. Posts removed or demoted by the end of their life are left out. Only on the last day of a run that sets one up.";
    FeedExperimentExtremeMiddleRatioKept, "feed_experiment_extreme_middle_ratio_kept", "Feed experiment: most partisan over middle, kept", Decimal, Participants, Truth, "A feed experiment's treated group's impressions of posts by the most partisan tenth of authors over those of the middle fifth, pooled over its days. Posts removed or demoted by the end of their life are left out. Only on the last day of a run that sets one up.";
    PartisanHalfReachKept, "partisan_half_reach_kept", "Reach of the more partisan half, kept", Decimal, Discourse, Truth, "Mean lifetime human impressions of the posts ending today by authors in the more partisan half, over the posts never removed or demoted; a diagnostic.";
    ExtremeReachRatioKept, "extreme_reach_ratio_kept", "Reach of the most partisan, kept", Decimal, Discourse, Truth, "Mean reach of the posts ending today by the most partisan tenth of authors over that of the middle fifth, over the posts never removed or demoted; a diagnostic.";
    InNetworkCandidateCount, "in_network_candidate_count", "In-network candidates", Count, Discourse, Truth, "In-network candidates gathered for today's feeds; a count for pooled diagnostics.";
    InNetworkOutgroupCandidateCount, "in_network_outgroup_candidate_count", "Out-group in-network candidates (count)", Count, Discourse, Truth, "Of those, the posts by the viewer's out-group; a count for pooled diagnostics.";
    InNetworkImpressionCount, "in_network_impression_count", "In-network impressions", Count, Discourse, Truth, "Today's impressions of in-network items; a count for pooled diagnostics.";
    InNetworkOutgroupImpressionCount, "in_network_outgroup_impression_count", "Out-group in-network impressions (count)", Count, Discourse, Truth, "Of those, the impressions of posts by the viewer's out-group; a count for pooled diagnostics.";
    InNetworkLikeCount, "in_network_like_count", "Likes on in-network impressions", Count, Discourse, Truth, "Today's likes on in-network impressions; a count for pooled diagnostics.";
    InNetworkOutgroupLikeCount, "in_network_outgroup_like_count", "Out-group in-network likes (count)", Count, Discourse, Truth, "Of those, the likes on posts by the viewer's out-group; a count for pooled diagnostics.";
    OutrageStyleShareCount, "outrage_style_share_count", "Human shares of Outrage posts", Count, Content, Truth, "Human shares of Outrage posts today; a count for pooled diagnostics.";
    OutrageStyleImpressionCount, "outrage_style_impression_count", "Impressions of Outrage posts", Count, Content, Truth, "Impressions of Outrage posts today; a count for pooled diagnostics.";
    CalmStyleShareCount, "calm_style_share_count", "Human shares of calm posts", Count, Content, Truth, "Human shares of Informative, Entertaining and Personal posts today; a count for pooled diagnostics.";
    CalmStyleImpressionCount, "calm_style_impression_count", "Impressions of calm posts", Count, Content, Truth, "Impressions of Informative, Entertaining and Personal posts today; a count for pooled diagnostics.";
    LikeBiasSame, "like_bias_same", "Like logit: bias, same-group", Decimal, Discourse, Truth, "Mean bias of the like probability's logit (7.1) over today's impressions by same-group viewers; a diagnostic.";
    LikeInterestSame, "like_interest_same", "Like logit: interest term, same-group", Decimal, Discourse, Truth, "Mean interest term of the like probability's logit (7.1) over today's impressions by same-group viewers; a diagnostic.";
    LikeArousalSame, "like_arousal_same", "Like logit: arousal term, same-group", Decimal, Discourse, Truth, "Mean arousal term of the like probability's logit (7.1) over today's impressions by same-group viewers; a diagnostic.";
    LikeHostilitySame, "like_hostility_same", "Like logit: in-group hostility term, same-group", Decimal, Discourse, Truth, "Mean in-group hostility term of the like probability's logit (7.1) over today's impressions by same-group viewers; a diagnostic.";
    LikeSocialProofSame, "like_social_proof_same", "Like logit: social-proof term, same-group", Decimal, Discourse, Truth, "Mean social-proof term of the like probability's logit (7.1) over today's impressions by same-group viewers; a diagnostic.";
    LikeReflectiveSame, "like_reflective_same", "Like logit: reflective term, same-group", Decimal, Discourse, Truth, "Mean reflective term of the like probability's logit (7.1) over today's impressions by same-group viewers; a diagnostic.";
    LikeBiasOut, "like_bias_out", "Like logit: bias, out-group", Decimal, Discourse, Truth, "Mean bias of the like probability's logit (7.1) over today's impressions by out-group viewers; a diagnostic.";
    LikeInterestOut, "like_interest_out", "Like logit: interest term, out-group", Decimal, Discourse, Truth, "Mean interest term of the like probability's logit (7.1) over today's impressions by out-group viewers; a diagnostic.";
    LikeArousalOut, "like_arousal_out", "Like logit: arousal term, out-group", Decimal, Discourse, Truth, "Mean arousal term of the like probability's logit (7.1) over today's impressions by out-group viewers; a diagnostic.";
    LikeHostilityOut, "like_hostility_out", "Like logit: in-group hostility term, out-group", Decimal, Discourse, Truth, "Mean in-group hostility term of the like probability's logit (7.1) over today's impressions by out-group viewers; a diagnostic.";
    LikeSocialProofOut, "like_social_proof_out", "Like logit: social-proof term, out-group", Decimal, Discourse, Truth, "Mean social-proof term of the like probability's logit (7.1) over today's impressions by out-group viewers; a diagnostic.";
    LikeReflectiveOut, "like_reflective_out", "Like logit: reflective term, out-group", Decimal, Discourse, Truth, "Mean reflective term of the like probability's logit (7.1) over today's impressions by out-group viewers; a diagnostic.";
    LikeOutgroupMultiplier, "like_outgroup_multiplier", "Like: out-group multiplier", Decimal, Discourse, Truth, "Mean out-group multiplier of the like probability, 1 - outgroup_like_cut * outrage (7.1), over today's impressions by out-group viewers; a diagnostic.";
    LikeProbabilitySame, "like_probability_same", "Like probability, same-group", Score, Discourse, Truth, "Mean like probability over today's impressions by same-group viewers; a diagnostic.";
    LikeProbabilityOut, "like_probability_out", "Like probability, out-group", Score, Discourse, Truth, "Mean like probability over today's impressions by out-group viewers; a diagnostic.";
    MisinformationCountInformative, "misinformation_count_informative", "Misinformation impressions: Informative", Count, Harm, Truth, "Impressions today of Informative posts with truth below 0.5; a count for pooled diagnostics.";
    MisinformationCountEntertaining, "misinformation_count_entertaining", "Misinformation impressions: Entertaining", Count, Harm, Truth, "Impressions today of Entertaining posts with truth below 0.5; a count for pooled diagnostics.";
    MisinformationCountPersonal, "misinformation_count_personal", "Misinformation impressions: Personal", Count, Harm, Truth, "Impressions today of Personal posts with truth below 0.5; a count for pooled diagnostics.";
    MisinformationCountOutrage, "misinformation_count_outrage", "Misinformation impressions: Outrage", Count, Harm, Truth, "Impressions today of Outrage posts with truth below 0.5, bots' included; a count for pooled diagnostics.";
    MisinformationCountClickbait, "misinformation_count_clickbait", "Misinformation impressions: Clickbait", Count, Harm, Truth, "Impressions today of Clickbait posts with truth below 0.5, bots' included; a count for pooled diagnostics.";
    MisinformationCountAttack, "misinformation_count_attack", "Misinformation impressions: Attack", Count, Harm, Truth, "Impressions today of Attack posts with truth below 0.5; a count for pooled diagnostics.";
    MisinformationCountBots, "misinformation_count_bots", "Misinformation impressions: bot posts", Count, Harm, Truth, "Impressions today of bot posts with truth below 0.5, of either style; a count for pooled diagnostics.";
    FalsePostsEnded, "false_posts_ended", "False posts ending", Count, Harm, Truth, "Posts with truth below 0.5 whose life ended today, bots' included; a count for pooled diagnostics.";
    NotedFalsePostsEnded, "noted_false_posts_ended", "Noted false posts ending", Count, Harm, Truth, "Of the false posts whose life ended today, those that got a community note; a count for pooled diagnostics.";
    ContagionAuthors, "contagion_authors", "Contagion: authors", Count, Content, Truth, "People who wrote at least one post today, the base of the causal exposure diagnostic (7.5); a count for pooled diagnostics.";
    ContagionChanceExposed, "contagion_chance_exposed", "Contagion: hostile chance after a hostile feed", Decimal, Content, Truth, "Summed over today's authors, the chance that a post's style is hostile after a fully hostile previous-day feed, from each author's style values (7.5); a sum for pooled diagnostics.";
    ContagionChanceUnexposed, "contagion_chance_unexposed", "Contagion: hostile chance after a calm feed", Decimal, Content, Truth, "Summed over today's authors, the chance that a post's style is hostile after a previous-day feed without a hostile item, from each author's style values (7.5); a sum for pooled diagnostics.";
    ContagionRatioSum, "contagion_ratio_sum", "Contagion: summed per-author risk ratio", Decimal, Content, Truth, "Summed over today's authors, each author's ratio of the two chances above; a sum for pooled diagnostics.";
    ExposedPostCount, "exposed_post_count", "Posts after a feed with hostile items", Count, Content, Truth, "Human posts today whose authors' previous-day feed held at least one Outrage or Attack post; a count for pooled diagnostics.";
    ExposedHostilePostCount, "exposed_hostile_post_count", "Hostile posts after a feed with hostile items", Count, Content, Truth, "Of those, the Outrage and Attack posts; a count for pooled diagnostics.";
    UnexposedPostCount, "unexposed_post_count", "Posts after a feed without hostile items", Count, Content, Truth, "Human posts today whose authors' previous-day feed held items but no Outrage or Attack post; a count for pooled diagnostics.";
    UnexposedHostilePostCount, "unexposed_hostile_post_count", "Hostile posts after a feed without hostile items", Count, Content, Truth, "Of those, the Outrage and Attack posts; a count for pooled diagnostics.";
}

/// Number of metrics.
pub const METRIC_COUNT: usize = INFOS.len();

impl Format {
    /// The smallest scale a metric's change across a window is measured
    /// against (see [`Summary::slope`]): 1 for counts, 0.01 for everything
    /// else, so a metric whose mean is near zero is judged by its absolute
    /// change.
    pub fn scale_floor(self) -> f32 {
        match self {
            Self::Count => 1.0,
            Self::Share | Self::Score | Self::Decimal | Self::Money => 0.01,
        }
    }
}

/// Metadata of one metric.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct MetricInfo {
    /// The metric.
    pub id: MetricId,
    /// Stable snake-case key for files and columns.
    pub key: &'static str,
    /// Display name.
    pub name: &'static str,
    /// How to show the value.
    pub format: Format,
    /// What it is about.
    pub family: Family,
    /// Observable or truth.
    pub visibility: Visibility,
    /// One plain-language sentence.
    pub description: &'static str,
}

impl MetricId {
    /// Every metric in catalogue order.
    pub fn all() -> impl Iterator<Item = MetricId> {
        INFOS.iter().map(|i| i.id)
    }

    /// Position in the catalogue.
    pub fn index(self) -> usize {
        self as usize
    }

    /// The metric's metadata.
    pub fn info(self) -> &'static MetricInfo {
        &INFOS[self.index()]
    }

    /// Looks a metric up by its key.
    pub fn from_key(key: &str) -> Option<MetricId> {
        INFOS.iter().find(|i| i.key == key).map(|i| i.id)
    }

    /// Whether the metric exists only in a run that sets up an experiment
    /// (the deactivation experiment's effect, its break group's changes and
    /// time after the break, the notifications-off experiment's three
    /// changes and the feed experiment's measures): it has no value in an
    /// ordinary run, so metric pickers and default tables leave it out.
    pub fn needs_experiment(self) -> bool {
        matches!(
            self,
            MetricId::DeactivationEffect
                | MetricId::BreakChange
                | MetricId::BreakPolarisation
                | MetricId::BreakInformedness
                | MetricId::BreakPolarisationRaw
                | MetricId::BreakInformednessRaw
                | MetricId::PostBreakDwell
                | MetricId::NotificationsOffTime
                | MetricId::NotificationsOffSessions
                | MetricId::NotificationsOffWellbeing
                | MetricId::FeedExperimentDwell
                | MetricId::FeedExperimentEngagement
                | MetricId::FeedExperimentToxicShare
                | MetricId::FeedExperimentMisinformationShare
                | MetricId::FeedExperimentSameGroupShare
                | MetricId::FeedExperimentPostToxicity
                | MetricId::FeedExperimentSharedTruth
                | MetricId::FeedExperimentPolarisation
                | MetricId::FeedExperimentInformedness
                | MetricId::FeedExperimentWellbeing
                | MetricId::FeedExperimentFlaggedShare
                | MetricId::FeedExperimentPolarisationRaw
                | MetricId::FeedExperimentInformednessRaw
                | MetricId::BreakWellbeingSd
                | MetricId::BreakPolarisationSd
                | MetricId::BreakInformednessSd
                | MetricId::NotificationsOffTimeSd
                | MetricId::NotificationsOffSessionsSd
                | MetricId::NotificationsOffWellbeingSd
                | MetricId::FeedExperimentPolarisationSd
                | MetricId::FeedExperimentInformednessSd
                | MetricId::FeedExperimentWellbeingSd
                | MetricId::FeedExperimentPartisanHalfImpressions
                | MetricId::FeedExperimentExtremeImpressions
                | MetricId::FeedExperimentMiddleImpressions
                | MetricId::FeedExperimentImpressions
                | MetricId::FeedExperimentExtremeMiddleRatio
                | MetricId::FeedExperimentPartisanHalfImpressionsKept
                | MetricId::FeedExperimentExtremeImpressionsKept
                | MetricId::FeedExperimentMiddleImpressionsKept
                | MetricId::FeedExperimentImpressionsKept
                | MetricId::FeedExperimentExtremeMiddleRatioKept
        )
    }

    /// Whether the metric is a diagnostic: a descriptive truth measure kept
    /// for checking the model against field findings (docs/model.md, 9.2),
    /// not a goal or a harm. It exists in every run, though it has no value
    /// on a day without its subject (no posts ending, no notes), so metric
    /// pickers and default tables leave it out.
    pub fn diagnostic(self) -> bool {
        matches!(
            self,
            MetricId::PartisanHalfReach
                | MetricId::ExtremeReachRatio
                | MetricId::InNetworkOutgroupCandidates
                | MetricId::InNetworkOutgroupImpressions
                | MetricId::InNetworkOutgroupLikes
                | MetricId::OutgroupRankingRatio
                | MetricId::OutgroupLikeRatio
                | MetricId::NotedSharesBeforeNote
                | MetricId::OutrageShareRatio
                | MetricId::SharedTruth
                | MetricId::PartisanHalfReachKept
                | MetricId::ExtremeReachRatioKept
                | MetricId::InNetworkCandidateCount
                | MetricId::InNetworkOutgroupCandidateCount
                | MetricId::InNetworkImpressionCount
                | MetricId::InNetworkOutgroupImpressionCount
                | MetricId::InNetworkLikeCount
                | MetricId::InNetworkOutgroupLikeCount
                | MetricId::OutrageStyleShareCount
                | MetricId::OutrageStyleImpressionCount
                | MetricId::CalmStyleShareCount
                | MetricId::CalmStyleImpressionCount
                | MetricId::LikeBiasSame
                | MetricId::LikeInterestSame
                | MetricId::LikeArousalSame
                | MetricId::LikeHostilitySame
                | MetricId::LikeSocialProofSame
                | MetricId::LikeReflectiveSame
                | MetricId::LikeBiasOut
                | MetricId::LikeInterestOut
                | MetricId::LikeArousalOut
                | MetricId::LikeHostilityOut
                | MetricId::LikeSocialProofOut
                | MetricId::LikeReflectiveOut
                | MetricId::LikeOutgroupMultiplier
                | MetricId::LikeProbabilitySame
                | MetricId::LikeProbabilityOut
                | MetricId::MisinformationCountInformative
                | MetricId::MisinformationCountEntertaining
                | MetricId::MisinformationCountPersonal
                | MetricId::MisinformationCountOutrage
                | MetricId::MisinformationCountClickbait
                | MetricId::MisinformationCountAttack
                | MetricId::MisinformationCountBots
                | MetricId::FalsePostsEnded
                | MetricId::NotedFalsePostsEnded
                | MetricId::ContagionAuthors
                | MetricId::ContagionChanceExposed
                | MetricId::ContagionChanceUnexposed
                | MetricId::ContagionRatioSum
                | MetricId::ExposedPostCount
                | MetricId::ExposedHostilePostCount
                | MetricId::UnexposedPostCount
                | MetricId::UnexposedHostilePostCount
        )
    }

    /// Whether metric pickers and default tables leave the metric out: it
    /// needs an experiment or it is a diagnostic.
    pub fn hidden_by_default(self) -> bool {
        self.needs_experiment() || self.diagnostic()
    }

    /// The post-share metric of a style.
    pub fn post_share(style: Style) -> MetricId {
        const BY_STYLE: [MetricId; STYLE_COUNT] = [
            MetricId::PostShareInformative,
            MetricId::PostShareEntertaining,
            MetricId::PostSharePersonal,
            MetricId::PostShareOutrage,
            MetricId::PostShareClickbait,
            MetricId::PostShareAttack,
        ];
        BY_STYLE[style.index()]
    }

    /// The diagnostic count of a style's misinformation impressions.
    pub fn misinformation_count(style: Style) -> MetricId {
        const BY_STYLE: [MetricId; STYLE_COUNT] = [
            MetricId::MisinformationCountInformative,
            MetricId::MisinformationCountEntertaining,
            MetricId::MisinformationCountPersonal,
            MetricId::MisinformationCountOutrage,
            MetricId::MisinformationCountClickbait,
            MetricId::MisinformationCountAttack,
        ];
        BY_STYLE[style.index()]
    }
}

/// The metadata of every metric.
pub fn metric_infos() -> &'static [MetricInfo] {
    INFOS
}

/// One day's metrics.
#[derive(Clone, Debug, PartialEq)]
pub struct DailyMetrics {
    /// The day the metrics describe.
    pub day: u32,
    pub(crate) values: Vec<f32>,
    /// Mean wellbeing of current users by archetype (NaN without users).
    pub wellbeing_by_archetype: [f32; ARCHETYPE_COUNT],
    /// Share of today's payouts by style.
    pub payout_share_by_style: [f32; STYLE_COUNT],
    /// Share of today's impressions by style.
    pub impression_share_by_style: [f32; STYLE_COUNT],
}

impl DailyMetrics {
    pub(crate) fn empty(day: u32) -> Self {
        Self {
            day,
            values: vec![f32::NAN; METRIC_COUNT],
            wellbeing_by_archetype: [f32::NAN; ARCHETYPE_COUNT],
            payout_share_by_style: [0.0; STYLE_COUNT],
            impression_share_by_style: [0.0; STYLE_COUNT],
        }
    }

    /// The value of a metric; NaN when it has none today.
    pub fn get(&self, id: MetricId) -> f32 {
        self.values[id.index()]
    }

    /// The value of a metric, `None` when it has none today.
    pub fn get_opt(&self, id: MetricId) -> Option<f32> {
        let v = self.get(id);
        (!v.is_nan()).then_some(v)
    }

    pub(crate) fn set(&mut self, id: MetricId, value: f32) {
        self.values[id.index()] = value;
    }
}

impl Serialize for DailyMetrics {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(METRIC_COUNT + 4))?;
        map.serialize_entry("day", &self.day)?;
        serialize_values(&mut map, &self.values)?;
        serialize_breakdowns(
            &mut map,
            &self.wellbeing_by_archetype,
            &self.payout_share_by_style,
            &self.impression_share_by_style,
        )?;
        map.end()
    }
}

pub(super) fn serialize_values<M: SerializeMap>(
    map: &mut M,
    values: &[f32],
) -> Result<(), M::Error> {
    for (info, v) in INFOS.iter().zip(values) {
        map.serialize_entry(info.key, &finite(*v))?;
    }
    Ok(())
}

pub(super) fn serialize_breakdowns<M: SerializeMap>(
    map: &mut M,
    wellbeing: &[f32; ARCHETYPE_COUNT],
    payouts: &[f32; STYLE_COUNT],
    impressions: &[f32; STYLE_COUNT],
) -> Result<(), M::Error> {
    let by_archetype: Vec<(&str, Option<f32>)> = Archetype::ALL
        .iter()
        .map(|a| (a.key(), finite(wellbeing[a.index()])))
        .collect();
    let by_style = |values: &[f32; STYLE_COUNT]| -> Vec<(&str, Option<f32>)> {
        Style::ALL
            .iter()
            .map(|s| (s.key(), finite(values[s.index()])))
            .collect()
    };
    map.serialize_entry("wellbeing_by_archetype", &by_archetype)?;
    map.serialize_entry("payout_share_by_style", &by_style(payouts))?;
    map.serialize_entry("impression_share_by_style", &by_style(impressions))
}

fn finite(v: f32) -> Option<f32> {
    v.is_finite().then_some(v)
}

/// The mean of the non-NaN values, NaN when there are none.
pub(crate) fn nan_mean(values: impl Iterator<Item = f32>) -> f32 {
    let (sum, n) = values
        .filter(|v| !v.is_nan())
        .fold((0.0f64, 0u32), |(s, n), v| (s + f64::from(v), n + 1));
    if n == 0 {
        f32::NAN
    } else {
        (sum / f64::from(n)) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogue_is_consistent() {
        for (i, info) in INFOS.iter().enumerate() {
            assert_eq!(info.id.index(), i);
            assert_eq!(MetricId::from_key(info.key), Some(info.id));
        }
    }

    #[test]
    fn summaries_skip_missing_values() {
        let mut a = DailyMetrics::empty(0);
        let mut b = DailyMetrics::empty(1);
        a.set(MetricId::SurveySatisfaction, 0.5);
        a.set(MetricId::Likes, 2.0);
        b.set(MetricId::Likes, 4.0);
        let s = Summary::from_history(&[a, b], 30);
        assert_eq!(s.get(MetricId::Likes), 3.0);
        assert_eq!(s.get(MetricId::SurveySatisfaction), 0.5);
        assert!(s.get_opt(MetricId::Notes).is_none());
    }
}
