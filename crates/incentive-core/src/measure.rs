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
}
