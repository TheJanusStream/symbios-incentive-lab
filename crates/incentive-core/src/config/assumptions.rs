//! World assumptions: how people are assumed to behave (docs/model.md,
//! section 11).

use serde::{Deserialize, Serialize};

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
    /// Speed at which authors learn what pays.
    pub learning_rate: f32,
    /// Daily chance of copying a more successful account.
    pub imitation_rate: f32,
    /// Randomness of style choice.
    pub choice_temperature: f32,
    /// Speed at which removed authors learn to evade classifiers.
    pub evasion_rate: f32,
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
            learning_rate: 0.1,
            imitation_rate: 0.02,
            choice_temperature: 0.25,
            evasion_rate: 0.15,
            habit_gain: 0.03,
            stop_bias: -2.5,
            churn_base: 0.02,
            leave_threshold: 0.0,
            harassment_exit: 0.05,
            join_base: 0.004,
            reputation_sensitivity: 2.0,
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
        }
    }
}
