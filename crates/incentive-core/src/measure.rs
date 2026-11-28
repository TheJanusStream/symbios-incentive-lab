//! Phase 14: turning the day's tallies and the world's state into metrics
//! (docs/model.md, section 9).

use crate::config::Config;
use crate::content::Style;
use crate::experiment;
use crate::metrics::{DailyMetrics, MetricId as M, nan_mean};
use crate::population::{ARCHETYPE_COUNT, Archetype};
use crate::rng::{self, Stream};
use crate::world::{DayCtx, World};

/// Survey: every 7th day a 5% sample of users is asked.
const SURVEY_PERIOD: u32 = 7;
const SURVEY_SAMPLE: f32 = 0.05;
/// Everyone answers the survey a little high.
const SURVEY_BIAS: f32 = 0.1;
/// Meaningful interactions per user and day that score full marks in the
/// benefit index. The text says 2; the best presets reach about 0.06, so 2
/// would flatten that component to zero (docs/model.md, section 13).
const MEANINGFUL_BENCHMARK: f32 = 0.2;
/// Human impressions per individual, in typical sessions
/// (`base_session_items`), at which the engagement index reaches 1, as in the
/// text. With the checking habits of docs/model.md, 11, the most engaging
/// designs reach the cap (9.3 and 13.2).
const ENGAGEMENT_BENCHMARK_SESSIONS: f32 = 1.5;
/// Days of the retention measure.
const RETENTION_DAYS: u32 = 30;

/// Ratio that is 0 when the denominator is 0.
fn ratio(num: f32, den: f32) -> f32 {
    if den > 0.0 { num / den } else { 0.0 }
}

/// Computes today's metrics.
pub(crate) fn measure(world: &World, ctx: &DayCtx) -> DailyMetrics {
    let mut m = DailyMetrics::empty(ctx.day);
    let t = &world.tally;
    let active_bots = world.bots.iter().filter(|b| b.active).count() as f32;
    let users = world.user_count() as f32;
    let dau = t.openers as f32 + active_bots;
    let impressions = t.impressions as f32;
    let engagement = (t.likes + t.replies + t.shares) as f32;
    m.set(M::ReportedDailyActives, dau);
    m.set(M::ReportedAccounts, users + active_bots);
    m.set(M::NewAccounts, t.new_accounts as f32);
    m.set(M::Departures, t.departures as f32);
    m.set(M::Impressions, impressions);
    m.set(M::TimeSpent, t.dwell);
    m.set(M::Likes, t.likes as f32);
    m.set(M::Replies, t.replies as f32);
    m.set(M::Shares, t.shares as f32);
    m.set(M::ImpressionsPerActive, ratio(impressions, dau));
    m.set(M::TimePerActive, ratio(t.dwell, dau));
    m.set(M::LikesPerActive, ratio(t.likes as f32, dau));
    m.set(M::RepliesPerActive, ratio(t.replies as f32, dau));
    m.set(M::SharesPerActive, ratio(t.shares as f32, dau));
    // Each active bot counts as one session, the first and only one.
    let sessions = t.sessions as f32 + active_bots;
    m.set(M::SessionsPerActive, ratio(sessions, dau));
    m.set(
        M::NotificationSessionShare,
        ratio(t.notification_sessions as f32, sessions),
    );
    m.set(
        M::CandidatesPerFeed,
        ratio(t.candidates as f32, t.feeds as f32),
    );
    m.set(M::Reports, t.reports as f32);
    m.set(M::SeeLess, t.see_less as f32);
    m.set(M::AutoRemovals, t.auto_removals as f32);
    m.set(M::HumanRemovals, t.human_removals as f32);
    m.set(M::Notes, t.notes as f32);
    m.set(M::Revenue, t.revenue);
    m.set(M::AdRevenue, t.ad_revenue);
    m.set(M::SubscriptionRevenue, t.subscription_revenue);
    m.set(M::SubscriberShare, ratio(t.subscribers as f32, dau));
    m.set(M::Payouts, t.payouts);
    m.set(M::NetRevenue, t.revenue - t.payouts);
    m.set(M::Retention30, retention(world, ctx.day));
    m.set(M::SurveySatisfaction, survey(world, ctx));
    m.set(M::LargestCascade, t.largest_cascade as f32);
    m.set(M::HumanDailyActives, t.openers as f32);
    m.set(M::UserShare, ratio(users, world.population() as f32));
    m.set(M::RegrettedShare, ratio(t.regret_items as f32, impressions));
    m.set(
        M::ToxicShare,
        ratio(t.toxic_impressions as f32, impressions),
    );
    m.set(
        M::OutrageShare,
        ratio(t.outrage_impressions as f32, impressions),
    );
    m.set(
        M::MisinformationShare,
        ratio(t.misinformation_impressions as f32, impressions),
    );
    m.set(
        M::HarassmentPer1000,
        ratio(1000.0 * t.harassment as f32, users),
    );
    m.set(M::FalsePositives, t.false_positives as f32);
    m.set(
        M::BotEngagementShare,
        ratio((t.bot_likes + t.bot_shares) as f32, engagement),
    );
    m.set(
        M::BotFlagShare,
        ratio(
            (t.bot_reports + t.bot_see_less) as f32,
            (t.reports + t.see_less) as f32,
        ),
    );
    m.set(
        M::ReflectivePerImpression,
        ratio(t.reflective_sum, impressions),
    );
    m.set(M::LowValueTime, ratio(t.low_value_dwell, t.dwell));
    m.set(M::DeactivationEffect, experiment::effect(world, ctx));
    m.set(M::BreakChange, experiment::break_change(world, ctx));
    let [time, sessions, wellbeing] = experiment::notifications_off_changes(world, ctx);
    m.set(M::NotificationsOffTime, time);
    m.set(M::NotificationsOffSessions, sessions);
    m.set(M::NotificationsOffWellbeing, wellbeing);
    let [time, sessions, wellbeing] = experiment::notifications_off_sds(world, ctx);
    m.set(M::NotificationsOffTimeSd, time);
    m.set(M::NotificationsOffSessionsSd, sessions);
    m.set(M::NotificationsOffWellbeingSd, wellbeing);
    let [
        polarisation,
        informedness,
        polarisation_raw,
        informedness_raw,
    ] = experiment::break_society_changes(world, ctx);
    m.set(M::BreakPolarisation, polarisation);
    m.set(M::BreakInformedness, informedness);
    m.set(M::BreakPolarisationRaw, polarisation_raw);
    m.set(M::BreakInformednessRaw, informedness_raw);
    m.set(M::PostBreakDwell, experiment::post_break_dwell(world, ctx));
    let [wellbeing, polarisation, informedness] = experiment::break_sds(world, ctx);
    m.set(M::BreakWellbeingSd, wellbeing);
    m.set(M::BreakPolarisationSd, polarisation);
    m.set(M::BreakInformednessSd, informedness);
    let feed = experiment::feed_measures(world, ctx);
    for (id, value) in FEED_EXPERIMENT_METRICS.iter().zip(feed) {
        m.set(*id, value);
    }
    diagnostics(world, &mut m);
    m.set(M::TopToxicShare, t.top_toxic_share);
    m.set(M::TopTruthfulShare, t.top_truthful_share);
    m.set(
        M::CrossGroupLikes,
        ratio(t.cross_group_likes as f32, impressions),
    );
    let harmful: f32 = Style::ALL
        .iter()
        .filter(|s| is_harmful(**s))
        .map(|s| t.payout_by_style[s.index()])
        .sum();
    // On a day without payouts the payout shares do not exist: NaN, which
    // summaries and scenario goals skip, as for the survey.
    let payout_share = |amount: f32| {
        if t.payouts > 0.0 {
            amount / t.payouts
        } else {
            f32::NAN
        }
    };
    m.set(M::HarmfulPayoutShare, payout_share(harmful));
    // The content mix counts human posts only; bots' posts are counted apart.
    let posts: u32 = t.posts_by_style.iter().sum();
    m.set(
        M::PolishedShare,
        ratio(t.polished_posts as f32, posts as f32),
    );
    m.set(M::BotPosts, t.bot_posts as f32);
    for style in Style::ALL {
        m.set(
            M::post_share(style),
            ratio(t.posts_by_style[style.index()] as f32, posts as f32),
        );
        m.payout_share_by_style[style.index()] = payout_share(t.payout_by_style[style.index()]);
        m.impression_share_by_style[style.index()] =
            ratio(t.impressions_by_style[style.index()] as f32, impressions);
    }
    people_metrics(world, &mut m);
    creator_metrics(world, ctx.config, &mut m);
    indices(world, ctx.config, &mut m);
    m
}

/// The feed experiment's metrics, in the order of
/// [`experiment::feed_measures`].
const FEED_EXPERIMENT_METRICS: [M; experiment::FEED_MEASURES] = [
    M::FeedExperimentDwell,
    M::FeedExperimentEngagement,
    M::FeedExperimentToxicShare,
    M::FeedExperimentMisinformationShare,
    M::FeedExperimentSameGroupShare,
    M::FeedExperimentPostToxicity,
    M::FeedExperimentSharedTruth,
    M::FeedExperimentPolarisation,
    M::FeedExperimentInformedness,
    M::FeedExperimentWellbeing,
    M::FeedExperimentFlaggedShare,
    M::FeedExperimentPolarisationRaw,
    M::FeedExperimentInformednessRaw,
    M::FeedExperimentPolarisationSd,
    M::FeedExperimentInformednessSd,
    M::FeedExperimentWellbeingSd,
    M::FeedExperimentPartisanHalfImpressions,
    M::FeedExperimentExtremeImpressions,
    M::FeedExperimentMiddleImpressions,
    M::FeedExperimentImpressions,
    M::FeedExperimentExtremeMiddleRatio,
    M::FeedExperimentPartisanHalfImpressionsKept,
    M::FeedExperimentExtremeImpressionsKept,
    M::FeedExperimentMiddleImpressionsKept,
    M::FeedExperimentImpressionsKept,
    M::FeedExperimentExtremeMiddleRatioKept,
];

/// A ratio that does not exist when its denominator is 0: NaN, which
/// summaries skip, rather than the 0 of [`ratio`].
fn ratio_or_nan(num: f64, den: f64) -> f32 {
    if den > 0.0 {
        (num / den) as f32
    } else {
        f32::NAN
    }
}

/// The diagnostics of docs/model.md, 9.2, from the day's tallies: reach by
/// partisanship among the posts ending today, with and without the posts
/// removed or demoted, the out-group shares of the in-network stages, the
/// shares made before a note, the share rate of Outrage against the
/// non-hostile styles, the truth of what people shared, the day's counts
/// behind the ratios (for ratios pooled over a window), and the like
/// probability's terms by viewer group. Measurement only.
fn diagnostics(world: &World, m: &mut DailyMetrics) {
    let t = &world.tally;
    let reach = |impressions: u64, posts: u32| ratio_or_nan(impressions as f64, f64::from(posts));
    m.set(
        M::PartisanHalfReach,
        reach(t.partisan_half_impressions, t.partisan_half_posts),
    );
    let extreme = reach(t.extreme_impressions, t.extreme_posts);
    let middle = reach(t.middle_impressions, t.middle_posts);
    m.set(
        M::ExtremeReachRatio,
        ratio_or_nan(f64::from(extreme), f64::from(middle)),
    );
    m.set(
        M::PartisanHalfReachKept,
        reach(t.partisan_half_kept_impressions, t.partisan_half_kept_posts),
    );
    let extreme_kept = reach(t.extreme_kept_impressions, t.extreme_kept_posts);
    let middle_kept = reach(t.middle_kept_impressions, t.middle_kept_posts);
    m.set(
        M::ExtremeReachRatioKept,
        ratio_or_nan(f64::from(extreme_kept), f64::from(middle_kept)),
    );
    let share = |part: u32, whole: u32| ratio_or_nan(f64::from(part), f64::from(whole));
    let candidates = share(t.in_network_outgroup_candidates, t.in_network_candidates);
    let impressions = share(t.in_network_outgroup_impressions, t.in_network_impressions);
    let likes = share(t.in_network_outgroup_likes, t.in_network_likes);
    m.set(M::InNetworkOutgroupCandidates, candidates);
    m.set(M::InNetworkOutgroupImpressions, impressions);
    m.set(M::InNetworkOutgroupLikes, likes);
    m.set(
        M::OutgroupRankingRatio,
        ratio_or_nan(f64::from(impressions), f64::from(candidates)),
    );
    m.set(
        M::OutgroupLikeRatio,
        ratio_or_nan(f64::from(likes), f64::from(impressions)),
    );
    m.set(
        M::NotedSharesBeforeNote,
        share(t.noted_shares_before_note, t.noted_shares),
    );
    let rate = |styles: &[Style]| {
        let shares: u32 = styles.iter().map(|s| t.shares_by_style[s.index()]).sum();
        let seen: u32 = styles
            .iter()
            .map(|s| t.impressions_by_style[s.index()])
            .sum();
        share(shares, seen)
    };
    let calm_styles = [Style::Informative, Style::Entertaining, Style::Personal];
    let calm = rate(&calm_styles);
    m.set(
        M::OutrageShareRatio,
        ratio_or_nan(f64::from(rate(&[Style::Outrage])), f64::from(calm)),
    );
    let count = |id: M, n: u32| (id, n as f32);
    let calm_shares: u32 = calm_styles
        .iter()
        .map(|s| t.shares_by_style[s.index()])
        .sum();
    let calm_seen: u32 = calm_styles
        .iter()
        .map(|s| t.impressions_by_style[s.index()])
        .sum();
    let outrage = Style::Outrage.index();
    for (id, value) in [
        count(M::InNetworkCandidateCount, t.in_network_candidates),
        count(
            M::InNetworkOutgroupCandidateCount,
            t.in_network_outgroup_candidates,
        ),
        count(M::InNetworkImpressionCount, t.in_network_impressions),
        count(
            M::InNetworkOutgroupImpressionCount,
            t.in_network_outgroup_impressions,
        ),
        count(M::InNetworkLikeCount, t.in_network_likes),
        count(M::InNetworkOutgroupLikeCount, t.in_network_outgroup_likes),
        count(M::OutrageStyleShareCount, t.shares_by_style[outrage]),
        count(
            M::OutrageStyleImpressionCount,
            t.impressions_by_style[outrage],
        ),
        count(M::CalmStyleShareCount, calm_shares),
        count(M::CalmStyleImpressionCount, calm_seen),
        count(M::NotedShareCount, t.noted_shares),
        count(M::NotedShareBeforeNoteCount, t.noted_shares_before_note),
        count(M::MisinformationCountBots, t.misinformation_bot_impressions),
        count(M::ToxicCountBots, t.toxic_bot_impressions),
        count(M::FalsePostsEnded, t.false_posts_ended),
        count(M::NotedFalsePostsEnded, t.noted_false_posts_ended),
        count(M::ContagionAuthors, t.contagion.authors),
        count(M::ExposedPostCount, t.contagion.exposed_posts),
        count(M::ExposedHostilePostCount, t.contagion.exposed_hostile),
        count(M::UnexposedPostCount, t.contagion.unexposed_posts),
        count(M::UnexposedHostilePostCount, t.contagion.unexposed_hostile),
    ] {
        m.set(id, value);
    }
    for style in Style::ALL {
        m.set(
            M::misinformation_count(style),
            t.misinformation_by_style[style.index()] as f32,
        );
        m.set(
            M::toxic_count(style),
            t.toxic_by_style[style.index()] as f32,
        );
    }
    // The sums have no value on a day without authors, their subject.
    let authors = t.contagion.authors > 0;
    let sum = |v: f64| if authors { v as f32 } else { f32::NAN };
    m.set(M::ContagionChanceExposed, sum(t.contagion.chance_exposed));
    m.set(
        M::ContagionChanceUnexposed,
        sum(t.contagion.chance_unexposed),
    );
    m.set(M::ContagionRatioSum, sum(t.contagion.ratio_sum));
    like_terms(world, m);
    m.set(
        M::SharedTruth,
        ratio_or_nan(t.shared_truth, f64::from(t.shares - t.bot_shares)),
    );
}

/// The like probability's terms (docs/model.md, 7.1 and 9.2): the means of
/// each logit term, the out-group multiplier and `P_like` over today's
/// impressions, by same-group and out-group viewers. NaN for a group
/// without impressions.
fn like_terms(world: &World, m: &mut DailyMetrics) {
    let [same, out] = &world.tally.like_terms;
    let mean = |sum: f64, n: u32| ratio_or_nan(sum, f64::from(n));
    for (sums, ids) in [
        (
            same,
            [
                M::LikeBiasSame,
                M::LikeInterestSame,
                M::LikeArousalSame,
                M::LikeHostilitySame,
                M::LikeSocialProofSame,
                M::LikeReflectiveSame,
                M::LikeProbabilitySame,
            ],
        ),
        (
            out,
            [
                M::LikeBiasOut,
                M::LikeInterestOut,
                M::LikeArousalOut,
                M::LikeHostilityOut,
                M::LikeSocialProofOut,
                M::LikeReflectiveOut,
                M::LikeProbabilityOut,
            ],
        ),
    ] {
        let n = sums.impressions;
        let values = [
            sums.bias,
            sums.interest,
            sums.arousal,
            sums.hostility,
            sums.social_proof,
            sums.reflective,
            sums.p_like,
        ];
        for (id, sum) in ids.into_iter().zip(values) {
            m.set(id, mean(sum, n));
        }
    }
    m.set(
        M::LikeOutgroupMultiplier,
        mean(out.multiplier, out.impressions),
    );
}

/// Outrage, Clickbait and Attack: the styles scenario goals call harmful.
pub(crate) fn is_harmful(style: Style) -> bool {
    matches!(style, Style::Outrage | Style::Clickbait | Style::Attack)
}

fn retention(world: &World, day: u32) -> f32 {
    if day < RETENTION_DAYS {
        return f32::NAN;
    }
    let then = 1u64 << RETENTION_DAYS;
    let (mut base, mut kept) = (0u32, 0u32);
    for p in &world.people {
        if p.state.active_bits & then != 0 {
            base += 1;
            kept += (p.state.active_bits & 1) as u32;
        }
    }
    if base == 0 {
        f32::NAN
    } else {
        kept as f32 / base as f32
    }
}

/// The weekly survey (docs/model.md, 9.1): happier users answer more often,
/// everyone answers a little high, and people who left are never asked. An
/// answer mixes wellbeing with recent impulsive pleasure and habit; answers
/// collected right after a session weigh pleasure twice.
fn survey(world: &World, ctx: &DayCtx) -> f32 {
    if ctx.day % SURVEY_PERIOD != SURVEY_PERIOD - 1 {
        return f32::NAN;
    }
    let assume = &ctx.config.world;
    let after_share = ctx.config.design.survey_after_session;
    let mut answers = Vec::new();
    for (i, p) in world.people.iter().enumerate() {
        if !p.state.is_user {
            continue;
        }
        let mut rng = ctx.rng(i as u64, Stream::Survey);
        let wb = p.state.wellbeing;
        if rng::chance(&mut rng, SURVEY_SAMPLE) && rng::chance(&mut rng, 0.3 * (0.5 + wb)) {
            let after_session = rng::chance(&mut rng, after_share);
            let noise = rng::normal(&mut rng, 0.1);
            let felt = survey_answer(
                wb,
                p.state.pleasure,
                p.state.habit,
                assume.survey_impulsive_weight,
                assume.survey_habit_weight,
                after_session,
            );
            answers.push((felt + noise).clamp(0.0, 1.0));
        }
    }
    nan_mean(answers.into_iter())
}

/// A survey answer before noise and clipping:
/// `(1 - a - b) * wellbeing + a * pleasure + b * habit + 0.1`, where the
/// pleasure weight `a` doubles right after a session, at most to `1 - b`.
fn survey_answer(
    wellbeing: f32,
    pleasure: f32,
    habit: f32,
    impulsive_weight: f32,
    habit_weight: f32,
    after_session: bool,
) -> f32 {
    let b = habit_weight.clamp(0.0, 1.0);
    let a = if after_session {
        2.0 * impulsive_weight
    } else {
        impulsive_weight
    }
    .clamp(0.0, 1.0 - b);
    (1.0 - a - b) * wellbeing + a * pleasure + b * habit + SURVEY_BIAS
}

fn people_metrics(world: &World, m: &mut DailyMetrics) {
    let mut by_archetype = [(0.0f32, 0u32); ARCHETYPE_COUNT];
    let (mut wb_ever, mut n_ever) = (0.0, 0u32);
    let (mut vulnerable_ever, mut vulnerable_left) = (0u32, 0u32);
    let mut groups = [[0.0f32; 3]; 2];
    let mut meaningful = 0u32;
    for p in &world.people {
        let s = &p.state;
        let g = usize::from(!s.is_user);
        groups[g][0] += s.affective_polarisation;
        groups[g][1] += s.informedness;
        groups[g][2] += 1.0;
        if s.ever_user {
            wb_ever += s.wellbeing;
            n_ever += 1;
        }
        if p.traits.archetype == Archetype::Vulnerable && s.ever_user {
            vulnerable_ever += 1;
            vulnerable_left += u32::from(!s.is_user);
        }
        if s.is_user {
            let entry = &mut by_archetype[p.traits.archetype.index()];
            entry.0 += s.wellbeing;
            entry.1 += 1;
            meaningful += s.today.meaningful;
        }
    }
    let users: u32 = by_archetype.iter().map(|e| e.1).sum();
    let wb_users: f32 = by_archetype.iter().map(|e| e.0).sum();
    for (a, (sum, n)) in by_archetype.iter().enumerate() {
        m.wellbeing_by_archetype[a] = if *n > 0 { sum / *n as f32 } else { f32::NAN };
    }
    m.set(M::WellbeingUsers, ratio(wb_users, users as f32));
    m.set(M::WellbeingEverUsers, ratio(wb_ever, n_ever as f32));
    m.set(M::MeaningfulPerUser, ratio(meaningful as f32, users as f32));
    m.set(
        M::VulnerableLeftShare,
        ratio(vulnerable_left as f32, vulnerable_ever as f32),
    );
    m.set(M::PolarisationUsers, ratio(groups[0][0], groups[0][2]));
    m.set(M::InformednessUsers, ratio(groups[0][1], groups[0][2]));
    m.set(M::PolarisationNonUsers, ratio(groups[1][0], groups[1][2]));
    m.set(M::InformednessNonUsers, ratio(groups[1][1], groups[1][2]));
    let everyone = groups[0][2] + groups[1][2];
    m.set(
        M::PolarisationSociety,
        ratio(groups[0][0] + groups[1][0], everyone),
    );
    m.set(
        M::InformednessSociety,
        ratio(groups[0][1] + groups[1][1], everyone),
    );
}

fn creator_metrics(world: &World, config: &Config, m: &mut DailyMetrics) {
    let mut incomes: Vec<f32> = world
        .people
        .iter()
        .filter(|p| p.traits.archetype == Archetype::Creator && p.state.ever_user)
        .map(|p| p.state.income_ema)
        .collect();
    incomes.sort_by(f32::total_cmp);
    let living = incomes
        .iter()
        .filter(|&&x| x >= config.world.living_income)
        .count();
    m.set(
        M::CreatorsLiving,
        ratio(living as f32, incomes.len() as f32),
    );
    m.set(
        M::CreatorMedianIncome,
        incomes.get(incomes.len() / 2).copied().unwrap_or(0.0),
    );
    m.set(M::CreatorIncomeGini, gini(&incomes));
}

/// Gini coefficient of sorted, non-negative values.
fn gini(sorted: &[f32]) -> f32 {
    let n = sorted.len() as f32;
    let total: f32 = sorted.iter().sum();
    if sorted.is_empty() || total <= 0.0 {
        return 0.0;
    }
    let weighted: f32 = sorted
        .iter()
        .enumerate()
        .map(|(i, x)| (2.0 * (i as f32 + 1.0) - n - 1.0) * x)
        .sum();
    weighted / (n * total)
}

/// The composite indices of docs/model.md, 9.3.
fn indices(world: &World, config: &Config, m: &mut DailyMetrics) {
    let n = world.population() as f32;
    let benchmark = ENGAGEMENT_BENCHMARK_SESSIONS * n * config.world.base_session_items;
    m.set(
        M::EngagementIndex,
        (m.get(M::Impressions) / benchmark).min(1.0),
    );
    let polarisation_growth =
        ((m.get(M::PolarisationSociety) - world.truth.polarisation_day0) / 0.2).clamp(0.0, 1.0);
    let wrongful = ratio(1000.0 * m.get(M::FalsePositives), world.user_count() as f32);
    let harm = harm_components(
        m.get(M::ToxicShare),
        m.get(M::HarassmentPer1000),
        m.get(M::MisinformationShare),
        polarisation_growth,
        wrongful,
    );
    m.set(M::HarmIndex, mean_of(&harm));
    m.set(
        M::HarmIndexExPolarisation,
        mean_without(&harm, HARM_POLARISATION),
    );
    let discourse = [
        1.0 - m.get(M::TopToxicShare),
        m.get(M::TopTruthfulShare),
        (m.get(M::CrossGroupLikes) / 0.1).min(1.0),
        1.0 - polarisation_growth,
    ];
    let discourse_quality = discourse.iter().sum::<f32>() / discourse.len() as f32;
    m.set(M::DiscourseQuality, discourse_quality);
    let benefit = [
        m.get(M::WellbeingEverUsers),
        m.get(M::InformednessSociety),
        (m.get(M::MeaningfulPerUser) / MEANINGFUL_BENCHMARK).min(1.0),
        m.get(M::CreatorsLiving),
        discourse_quality,
    ];
    m.set(M::BenefitIndex, mean_of(&benefit));
    m.set(
        M::BenefitIndexExCreators,
        mean_without(&benefit, BENEFIT_CREATORS),
    );
}

/// Position of the polarisation component in [`harm_components`].
const HARM_POLARISATION: usize = 3;
/// Position of the creators-living component in the benefit index.
const BENEFIT_CREATORS: usize = 3;

/// The plain mean of an index's components.
fn mean_of(components: &[f32]) -> f32 {
    components.iter().sum::<f32>() / components.len() as f32
}

/// The mean of an index's components without the one at `skip`.
fn mean_without(components: &[f32], skip: usize) -> f32 {
    let kept: f32 = components
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != skip)
        .map(|(_, v)| v)
        .sum();
    kept / (components.len() - 1) as f32
}

/// The five capped components of the harm index (docs/model.md, 9.3), the
/// polarisation component at [`HARM_POLARISATION`]. Wrongful sanctions are
/// removals and demotions of harmless posts per 1,000 users, scaled like
/// harassment.
fn harm_components(
    toxic_share: f32,
    harassment_per_1000: f32,
    misinformation_share: f32,
    polarisation_growth: f32,
    wrongful_per_1000: f32,
) -> [f32; 5] {
    [
        (toxic_share / 0.2).min(1.0),
        (harassment_per_1000 / 20.0).min(1.0),
        (misinformation_share / 0.3).min(1.0),
        polarisation_growth,
        (wrongful_per_1000 / 20.0).min(1.0),
    ]
}

/// The harm index: the mean of its five components.
#[cfg(test)]
fn harm_index(
    toxic_share: f32,
    harassment_per_1000: f32,
    misinformation_share: f32,
    polarisation_growth: f32,
    wrongful_per_1000: f32,
) -> f32 {
    mean_of(&harm_components(
        toxic_share,
        harassment_per_1000,
        misinformation_share,
        polarisation_growth,
        wrongful_per_1000,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_value_time_is_a_share_of_dwell() {
        let mut config = Config::default();
        config.run.population_size = 100;
        let mut world = crate::init::build_world(&config);
        let ctx = DayCtx {
            config: &config,
            day: 3,
        };
        assert_eq!(measure(&world, &ctx).get(M::LowValueTime), 0.0, "no time");
        world.tally.dwell = 8.0;
        world.tally.low_value_dwell = 2.0;
        assert_eq!(measure(&world, &ctx).get(M::LowValueTime), 0.25);
    }

    #[test]
    fn sessions_per_active_counts_each_bot_once_and_notifications_apart() {
        let mut config = Config::default();
        config.run.population_size = 200;
        let mut world = crate::init::build_world(&config);
        let ctx = DayCtx {
            config: &config,
            day: 3,
        };
        let bots = world.bots.iter().filter(|b| b.active).count() as f32;
        world.tally.openers = 20;
        world.tally.sessions = 50;
        world.tally.notification_sessions = 10;
        let m = measure(&world, &ctx);
        assert_eq!(m.get(M::SessionsPerActive), (50.0 + bots) / (20.0 + bots));
        assert_eq!(m.get(M::NotificationSessionShare), 10.0 / (50.0 + bots));
        // One session per opener: one session per active, none from notifications.
        world.tally.sessions = 20;
        world.tally.notification_sessions = 0;
        let m = measure(&world, &ctx);
        assert_eq!(m.get(M::SessionsPerActive), 1.0);
        assert_eq!(m.get(M::NotificationSessionShare), 0.0);
    }

    #[test]
    fn wrongful_sanctions_raise_the_harm_index() {
        let base = harm_index(0.05, 2.0, 0.1, 0.1, 0.0);
        let wrongful = harm_index(0.05, 2.0, 0.1, 0.1, 10.0);
        assert!((wrongful - base - 0.1).abs() < 1e-6, "{base} {wrongful}");
        assert!((harm_index(0.0, 0.0, 0.0, 0.0, 100.0) - 0.2).abs() < 1e-6);
    }

    #[test]
    fn the_indices_without_one_component_drop_it_from_the_mean() {
        let harm = harm_components(0.1, 10.0, 0.15, 0.4, 0.0);
        // Components 0.5, 0.5, 0.5, 0.4, 0: the full mean and the mean of four.
        assert!((mean_of(&harm) - 0.38).abs() < 1e-6);
        assert!((mean_without(&harm, HARM_POLARISATION) - 0.375).abs() < 1e-6);
        let benefit = [0.6, 0.5, 0.2, 0.4, 0.9];
        assert!((mean_without(&benefit, BENEFIT_CREATORS) - 0.55).abs() < 1e-6);
    }

    #[test]
    fn gini_of_equal_and_concentrated_incomes() {
        assert!(gini(&[1.0, 1.0, 1.0, 1.0]).abs() < 1e-6);
        assert!((gini(&[0.0, 0.0, 0.0, 4.0]) - 0.75).abs() < 1e-6);
    }

    #[test]
    fn a_survey_answer_mixes_wellbeing_pleasure_and_habit() {
        let plain = survey_answer(0.6, 0.9, 0.5, 0.3, 0.1, false);
        assert!((plain - (0.6 * 0.6 + 0.3 * 0.9 + 0.1 * 0.5 + 0.1)).abs() < 1e-6);
        let after = survey_answer(0.6, 0.9, 0.5, 0.3, 0.1, true);
        assert!((after - (0.3 * 0.6 + 0.6 * 0.9 + 0.1 * 0.5 + 0.1)).abs() < 1e-6);
        // The doubled weight never pushes the weights past 1.
        let capped = survey_answer(0.6, 0.9, 0.5, 0.5, 0.3, true);
        assert!((capped - (0.7 * 0.9 + 0.3 * 0.5 + 0.1)).abs() < 1e-6);
        assert!((survey_answer(0.6, 0.9, 0.5, 0.0, 0.0, true) - 0.7).abs() < 1e-6);
    }

    #[test]
    fn the_diagnostics_follow_their_definitions() {
        let mut config = Config::default();
        config.run.population_size = 100;
        let mut world = crate::init::build_world(&config);
        let ctx = DayCtx {
            config: &config,
            day: 3,
        };
        // Nothing to describe: no value, not 0; the day's counts are 0.
        let empty = measure(&world, &ctx);
        for id in M::all().filter(|id| id.diagnostic()) {
            let value = empty.get(id);
            if id.info().format == crate::metrics::Format::Count {
                assert_eq!(value, 0.0, "{}", id.info().key);
            } else {
                assert!(value.is_nan(), "{}: {value}", id.info().key);
            }
        }
        let t = &mut world.tally;
        t.partisan_half_posts = 4;
        t.partisan_half_impressions = 50;
        t.extreme_posts = 2;
        t.extreme_impressions = 30;
        t.middle_posts = 5;
        t.middle_impressions = 25;
        t.partisan_half_kept_posts = 2;
        t.partisan_half_kept_impressions = 30;
        t.extreme_kept_posts = 1;
        t.extreme_kept_impressions = 20;
        t.middle_kept_posts = 4;
        t.middle_kept_impressions = 16;
        let terms = crate::behaviour::LikeTerms {
            bias: -3.5,
            interest: 0.6,
            arousal: 0.2,
            hostility: 0.3,
            social_proof: 0.1,
            reflective: -0.4,
            multiplier: 1.0,
        };
        t.like_terms[0].add(&terms, 0.1);
        t.like_terms[0].add(
            &crate::behaviour::LikeTerms {
                interest: 0.0,
                hostility: 0.1,
                ..terms
            },
            0.3,
        );
        t.like_terms[1].add(
            &crate::behaviour::LikeTerms {
                hostility: 0.0,
                multiplier: 0.6,
                ..terms
            },
            0.05,
        );
        t.in_network_candidates = 200;
        t.in_network_outgroup_candidates = 50;
        t.in_network_impressions = 40;
        t.in_network_outgroup_impressions = 8;
        t.in_network_likes = 10;
        t.in_network_outgroup_likes = 1;
        t.noted_shares = 20;
        t.noted_shares_before_note = 17;
        t.impressions_by_style = [100, 100, 200, 50, 10, 5];
        t.shares_by_style = [2, 4, 0, 6, 9, 9];
        t.shares = 40;
        t.bot_shares = 10;
        t.shared_truth = 15.0;
        let m = measure(&world, &ctx);
        let close = |id: M, want: f32| {
            let got = m.get(id);
            assert!(
                (got - want).abs() < 1e-6,
                "{}: {got} against {want}",
                id.info().key
            );
        };
        close(M::PartisanHalfReach, 12.5);
        close(M::ExtremeReachRatio, 15.0 / 5.0);
        close(M::InNetworkOutgroupCandidates, 0.25);
        close(M::InNetworkOutgroupImpressions, 0.2);
        close(M::InNetworkOutgroupLikes, 0.1);
        close(M::OutgroupRankingRatio, 0.2 / 0.25);
        close(M::OutgroupLikeRatio, 0.1 / 0.2);
        close(M::NotedSharesBeforeNote, 0.85);
        // Outrage 6 shares in 50 impressions against 6 in 400.
        close(M::OutrageShareRatio, (6.0 / 50.0) / (6.0 / 400.0));
        // Bots' shares carry no truth of a person's choice: 30 human shares.
        close(M::SharedTruth, 0.5);
        close(M::PartisanHalfReachKept, 15.0);
        close(M::ExtremeReachRatioKept, 20.0 / 4.0);
        close(M::InNetworkCandidateCount, 200.0);
        close(M::InNetworkOutgroupCandidateCount, 50.0);
        close(M::InNetworkImpressionCount, 40.0);
        close(M::InNetworkOutgroupImpressionCount, 8.0);
        close(M::InNetworkLikeCount, 10.0);
        close(M::InNetworkOutgroupLikeCount, 1.0);
        close(M::OutrageStyleShareCount, 6.0);
        close(M::OutrageStyleImpressionCount, 50.0);
        close(M::CalmStyleShareCount, 6.0);
        close(M::CalmStyleImpressionCount, 400.0);
        close(M::NotedShareCount, 20.0);
        close(M::NotedShareBeforeNoteCount, 17.0);
        // Same-group means over two impressions, out-group over one.
        close(M::LikeBiasSame, -3.5);
        close(M::LikeInterestSame, 0.3);
        close(M::LikeArousalSame, 0.2);
        close(M::LikeHostilitySame, 0.2);
        close(M::LikeSocialProofSame, 0.1);
        close(M::LikeReflectiveSame, -0.4);
        close(M::LikeProbabilitySame, 0.2);
        close(M::LikeHostilityOut, 0.0);
        close(M::LikeInterestOut, 0.6);
        close(M::LikeOutgroupMultiplier, 0.6);
        close(M::LikeProbabilityOut, 0.05);
    }
}
