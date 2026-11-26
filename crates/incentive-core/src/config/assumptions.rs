//! World assumptions: how people are assumed to behave (docs/model.md,
//! section 11).

use serde::{Deserialize, Serialize};

/// How bot farms run their accounts (docs/model.md, 3.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BotStrategy {
    /// Every bot boosts random posts from one shared pool of bot posts and
    /// Clickbait posts, whatever its group.
    SharedPool,
    /// Each bot boosts only posts of its own group.
    OwnSide,
    /// Bots aim at what the design rewards: they split their boosts between
    /// own-side engagement and cross-group coordination by the ranking's
    /// weights, polish when the design reads the quality signal, and brigade
    /// the other group's popular posts with reports and see-less marks.
    Adaptive,
}

/// How people are assumed to behave (docs/model.md, section 11). Values that
/// differ from the specification's text are recorded in its section 13.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WorldAssumptions {
    /// Pull of in-group hostility on impulsive attention.
    pub outrage_appeal: f32,
    /// Pull of arousing content on impulsive attention.
    pub arousal_appeal: f32,
    /// Pull of visible like counts.
    pub social_proof: f32,
    /// Harm of one harassing notification.
    pub harassment_harm: f32,
    /// Cost of seeing others do better.
    pub comparison_cost: f32,
    /// Cost of each item beyond the time a person would endorse.
    pub regret_cost: f32,
    /// Reflective hurt of having a harmless post removed or demoted, felt
    /// like harassment (more for vulnerable authors).
    pub wrongful_sanction_cost: f32,
    /// Speed at which authors learn what pays.
    pub learning_rate: f32,
    /// Daily chance of copying a more successful account.
    pub imitation_rate: f32,
    /// Randomness of style choice.
    pub choice_temperature: f32,
    /// Speed at which authors learn to evade classifiers after an automated
    /// removal or demotion.
    pub evasion_rate: f32,
    /// Daily share of everyone's evasion that fades.
    pub evasion_decay: f32,
    /// Share of a toxic post's arousal and outrage that full evasion gives up.
    pub evasion_cost: f32,
    /// Speed at which the classifier retrains on toxic posts it missed.
    pub classifier_retraining: f32,
    /// Speed at which authors learn how hostile their hostile posts can be.
    pub intensity_rate: f32,
    /// Extra learning penalty per unit of sanction severity.
    pub deterrence: f32,
    /// Speed at which engaging sessions build habit.
    pub habit_gain: f32,
    /// Baseline tendency to end a session after an item.
    pub stop_bias: f32,
    /// Base daily churn of unhappy users.
    pub churn_base: f32,
    /// Stay utility below which users start to leave.
    pub leave_threshold: f32,
    /// Extra churn per harassment event, scaled by vulnerability.
    pub harassment_exit: f32,
    /// Base daily chance that a non-user joins.
    pub join_base: f32,
    /// How strongly a toxic reputation deters joining.
    pub reputation_sensitivity: f32,
    /// How strongly the account cost deters people from joining and returning.
    pub account_cost_deterrence: f32,
    /// Share of ties and follows within one's own group.
    pub homophily: f32,
    /// Polarisation per unit of in-group hostility seen.
    pub polarisation_rate: f32,
    /// Depolarisation per liked out-group post, times its quality.
    pub contact_rate: f32,
    /// Daily pull of non-users toward their user ties' attitudes.
    pub spillover_rate: f32,
    /// Multiplier on everyone's credulity.
    pub credulity_scale: f32,
    /// Ad revenue per human impression.
    pub ad_rate: f32,
    /// Monthly subscription price.
    pub subscription_price: f32,
    /// Mean daily reflective utility at which half of the willing subscribe.
    pub subscription_hurdle: f32,
    /// Reflective cost of ads per impression.
    pub ad_annoyance: f32,
    /// Daily income that counts as a living.
    pub living_income: f32,
    /// Typical items in a session.
    pub base_session_items: f32,
    /// Share of bot accounts farms keep at the highest account cost, by
    /// buying verified accounts.
    pub bot_farm_floor: f32,
    /// Standard deviation of the quality classifier's noise.
    pub quality_signal_noise: f32,
    /// How much polishing a post raises its quality signal without raising
    /// its quality.
    pub polish_gain: f32,
    /// Effort a polished post costs its author, in the units of the post's
    /// learning reward.
    pub polish_cost: f32,
    /// Weight of interest-weighted quality in reflective value.
    pub reflective_quality_weight: f32,
    /// Share of the reflective cost of outrage that its author's own group
    /// feels; the out-group feels all of it.
    pub in_group_outrage_cost: f32,
    /// Reflective cost of a Clickbait post.
    pub clickbait_penalty: f32,
    /// Spread of the styles' base quality around their mean: 1 keeps the
    /// profiles of 4.1, 0 gives every style the same base quality.
    pub style_quality_gap: f32,
    /// Weight of recent impulsive pleasure in a survey answer.
    pub survey_impulsive_weight: f32,
    /// Weight of habit in a survey answer.
    pub survey_habit_weight: f32,
    /// How bot farms run their accounts.
    pub bot_strategy: BotStrategy,
    /// Reports and see-less marks each adaptive bot files per day against
    /// the other group's popular posts, in units of a base count.
    pub bot_brigading: f32,
    /// Polarisation a strongly partisan viewer gains from seeing a calm
    /// out-group post, as a share of `contact_rate`.
    pub polarisation_backfire: f32,
}

impl Default for WorldAssumptions {
    fn default() -> Self {
        Self {
            outrage_appeal: 1.2,
            arousal_appeal: 1.0,
            social_proof: 0.3,
            harassment_harm: 3.0,
            comparison_cost: 0.1,
            regret_cost: 0.05,
            wrongful_sanction_cost: 0.5,
            learning_rate: 0.1,
            imitation_rate: 0.02,
            choice_temperature: 0.25,
            evasion_rate: 0.15,
            evasion_decay: 0.01,
            evasion_cost: 0.3,
            classifier_retraining: 0.1,
            intensity_rate: 0.01,
            deterrence: 0.5,
            habit_gain: 0.03,
            stop_bias: -2.5,
            churn_base: 0.02,
            leave_threshold: 0.0,
            harassment_exit: 0.05,
            join_base: 0.004,
            reputation_sensitivity: 2.0,
            account_cost_deterrence: 0.5,
            homophily: 0.7,
            polarisation_rate: 0.00015,
            contact_rate: 0.001,
            spillover_rate: 0.01,
            credulity_scale: 1.0,
            ad_rate: 0.001,
            subscription_price: 5.0,
            subscription_hurdle: 0.5,
            ad_annoyance: 0.02,
            living_income: 0.025,
            base_session_items: 20.0,
            bot_farm_floor: 0.2,
            quality_signal_noise: 0.15,
            polish_gain: 0.3,
            polish_cost: 0.03,
            reflective_quality_weight: 0.8,
            in_group_outrage_cost: 0.8,
            clickbait_penalty: 0.3,
            style_quality_gap: 1.0,
            survey_impulsive_weight: 0.3,
            survey_habit_weight: 0.1,
            bot_strategy: BotStrategy::Adaptive,
            bot_brigading: 1.0,
            polarisation_backfire: 0.0,
        }
    }
}
