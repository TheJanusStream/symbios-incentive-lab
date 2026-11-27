//! The evolving state of an individual (docs/model.md, 3.3).

use serde::Serialize;

use crate::content::{NEUTRAL_INTENSITY, STYLE_COUNT, Style};
use crate::population::Traits;
use crate::society::INFORMEDNESS_BASELINE;

/// Most accounts one person blocks; the oldest block is dropped beyond it.
pub(crate) const BLOCK_LIMIT: usize = 32;
/// Most accounts one person follows. The specification sets no limit; without
/// one, follows from liked recommendations grow without bound over long runs.
pub(crate) const FOLLOW_LIMIT: usize = 150;

/// The evolving state of one individual.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PersonState {
    /// Currently on the platform.
    pub is_user: bool,
    /// Has ever been on the platform.
    pub ever_user: bool,
    /// Day of the latest joining.
    pub joined_day: Option<u32>,
    /// Day of the latest departure.
    pub left_day: Option<u32>,
    /// Wellbeing in `[0, 1]`.
    pub wellbeing: f32,
    /// Habit strength in `[0, 1]`.
    pub habit: f32,
    /// Moving average of daily stay utility.
    pub stay_ema: f32,
    /// Dislike of the other group, in `[0, 1]`.
    pub affective_polarisation: f32,
    /// How well informed, in `[0, 1]`.
    pub informedness: f32,
    /// Learned value of writing each style.
    pub style_value: [f32; STYLE_COUNT],
    /// Learned advantage of a plain post (index 0) and of a polished post
    /// (index 1) over the style's value (docs/model.md, 7.5).
    pub polish_value: [f32; 2],
    /// Posts written over the run; the next post's ordinal.
    pub posts_written: u32,
    /// Recent impulsive pleasure in `[0, 1]`: a smoothed, squashed daily
    /// impulsive experience (docs/model.md, 7.3).
    pub pleasure: f32,
    /// Skill at writing toxicity classifiers miss.
    pub evasion: f32,
    /// Learned intensity of hostile posts in `[0, 1]`; 0.5 adds nothing.
    pub intensity: f32,
    /// Accounts followed, sorted ascending.
    pub follows: Vec<u32>,
    /// Accounts blocked, oldest first.
    pub blocked: Vec<u32>,
    /// Strikes from human review on the current account.
    pub strikes: u32,
    /// First day the person may post again.
    pub suspended_until: u32,
    /// Banned and not yet back.
    pub banned: bool,
    /// First day a banned person may open a new account.
    pub reentry_day: u32,
    /// 30-day moving average of daily creator income.
    pub income_ema: f32,
    /// Creator income over the whole run.
    pub income_total: f32,
    /// Moving average of the reward of the person's posts.
    pub reward_ema: f32,
    /// Pays for a subscription.
    pub subscriber: bool,
    /// Typical likes on the person's own posts, for social comparison.
    pub typical_likes: f32,
    /// Uses the alternative feed.
    pub alt_feed_user: bool,
    /// Has a daily time limit.
    pub time_limited: bool,
    /// Reflective utility summed since the last subscription decision.
    #[serde(skip)]
    pub(crate) eref_window: f32,
    /// Ad annoyance felt since the last subscription decision.
    #[serde(skip)]
    pub(crate) annoyance_window: f32,
    /// Items seen since the last subscription decision.
    #[serde(skip)]
    pub(crate) items_window: u32,
    /// Activity of the last 64 days, today in the lowest bit.
    #[serde(skip)]
    pub(crate) active_bits: u64,
    /// Today's accumulators.
    #[serde(skip)]
    pub(crate) today: DayAccumulators,
    /// Impressions of the previous day, and of them those of hostile posts
    /// (Outrage or Attack), kept when the day's accumulators are cleared:
    /// the exposure of docs/model.md, 7.5.
    #[serde(skip)]
    pub(crate) yesterday_items: u32,
    #[serde(skip)]
    pub(crate) yesterday_hostile_items: u32,
}

/// What happened to one person today; cleared every day.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct DayAccumulators {
    pub opened: bool,
    pub posts: u32,
    pub items: u32,
    /// Items seen with ads today, while not subscribing (docs/model.md, 8.3).
    pub ad_items: u32,
    /// Ad annoyance felt today, included in `e_ref` (docs/model.md, 7.2).
    pub ad_annoyance: f32,
    /// Today's dwell, the time on the platform (docs/model.md, 7.1).
    pub dwell: f32,
    pub e_imp: f32,
    /// Reflective utility before the time cost (docs/model.md, 7.2):
    /// subscriptions read it.
    pub e_ref: f32,
    /// The time cost of today's impressions, `time_cost * D` summed
    /// (docs/model.md, 7.2), kept apart from `e_ref`.
    pub time_cost: f32,
    /// The interruption cost of today's notification-driven extra sessions
    /// (docs/model.md, 7.2), kept apart like the time cost.
    pub interruption_cost: f32,
    /// Habitual extra sessions today, after the first (docs/model.md, 6.1).
    pub habitual_sessions: u32,
    /// Notification-driven extra sessions today, after the first.
    pub notified_sessions: u32,
    pub harassment: u32,
    pub likes_received: u32,
    pub mutual_replies: u32,
    pub angry_received: u32,
    pub income: f32,
    pub styles_written: u8,
    pub meaningful: u32,
    /// Engagement actions today: likes, replies and shares given.
    pub actions: u32,
    /// Of today's impressions, the toxic ones (toxicity above 0.5).
    pub toxic_items: u32,
    /// Of today's impressions, the misinformation (truth below 0.5).
    pub misinformation_items: u32,
    /// Of today's impressions, those of hostile posts (Outrage or Attack).
    pub hostile_items: u32,
    /// The time of day of today's sessions, in hours from 0 to 24, drawn
    /// only with community notes on (docs/model.md, 6.8); 0 otherwise.
    pub session_hour: f32,
    /// Of today's impressions, those of posts by the person's own group.
    pub same_group_items: u32,
    /// Of today's impressions, those of posts whose classifier score exceeds
    /// a hide-flagged feed experiment's threshold; counted only in a run that
    /// sets one up, its placebo included, as that experiment's dose.
    pub flagged_items: u32,
    /// Of today's impressions, those of posts by human authors in the more
    /// partisan half, the most partisan tenth and the middle fifth by the
    /// partisanship trait (docs/model.md, 9.2); truth-side counts for a feed
    /// experiment's reach measures.
    pub partisan_half_items: u32,
    pub extreme_items: u32,
    pub middle_items: u32,
    /// Summed toxicity of the posts the person wrote today.
    pub post_toxicity: f32,
    /// Shares the person made today.
    pub shares: u32,
    /// Summed truth of the posts the person shared today.
    pub shared_truth: f32,
    /// Misinformation this person shared today: truth and whether noted.
    pub misinformation_shares: Vec<(f32, bool)>,
    /// Posts that gave this person positive reflective value, for fan funding.
    pub fan_credits: Vec<(u32, f32)>,
}

impl DayAccumulators {
    /// Reflective utility after the time cost and the interruption cost,
    /// what wellbeing and stay utility read (docs/model.md, 7.3).
    pub(crate) fn net_reflective(&self) -> f32 {
        self.e_ref - self.time_cost - self.interruption_cost
    }

    pub(crate) fn clear(&mut self) {
        let mut shares = std::mem::take(&mut self.misinformation_shares);
        let mut credits = std::mem::take(&mut self.fan_credits);
        shares.clear();
        credits.clear();
        *self = Self {
            misinformation_shares: shares,
            fan_credits: credits,
            ..Self::default()
        };
    }
}

/// One individual: fixed traits and evolving state.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Person {
    /// Fixed traits.
    pub traits: Traits,
    /// Evolving state.
    pub state: PersonState,
}

impl Person {
    /// A non-user at the start of a run.
    pub(crate) fn new(traits: Traits) -> Self {
        let state = PersonState {
            is_user: false,
            ever_user: false,
            joined_day: None,
            left_day: None,
            wellbeing: traits.wellbeing_baseline,
            habit: 0.1,
            stay_ema: 0.3,
            affective_polarisation: traits.polarisation_baseline,
            informedness: INFORMEDNESS_BASELINE,
            style_value: traits.intrinsic,
            polish_value: [0.0; 2],
            posts_written: 0,
            pleasure: 0.0,
            evasion: 0.0,
            intensity: NEUTRAL_INTENSITY,
            follows: Vec::new(),
            blocked: Vec::new(),
            strikes: 0,
            suspended_until: 0,
            banned: false,
            reentry_day: 0,
            income_ema: 0.0,
            income_total: 0.0,
            reward_ema: 0.0,
            subscriber: false,
            typical_likes: 2.0,
            alt_feed_user: false,
            time_limited: false,
            eref_window: 0.0,
            annoyance_window: 0.0,
            items_window: 0,
            active_bits: 0,
            today: DayAccumulators::default(),
            yesterday_items: 0,
            yesterday_hostile_items: 0,
        };
        Self { traits, state }
    }

    /// Whether this person follows `account`.
    pub fn follows(&self, account: u32) -> bool {
        self.state.follows.binary_search(&account).is_ok()
    }

    /// Whether this person blocked `account`.
    pub fn has_blocked(&self, account: u32) -> bool {
        self.state.blocked.contains(&account)
    }

    /// Whether this person may post today.
    pub(crate) fn can_post(&self, day: u32) -> bool {
        self.state.is_user && day >= self.state.suspended_until
    }

    /// Adds a follow; false when already followed or at the limit.
    pub(crate) fn add_follow(&mut self, account: u32) -> bool {
        if self.state.follows.len() >= FOLLOW_LIMIT {
            return false;
        }
        match self.state.follows.binary_search(&account) {
            Ok(_) => false,
            Err(pos) => {
                self.state.follows.insert(pos, account);
                true
            }
        }
    }

    /// Removes a follow; false when it did not exist.
    pub(crate) fn remove_follow(&mut self, account: u32) -> bool {
        match self.state.follows.binary_search(&account) {
            Ok(pos) => {
                self.state.follows.remove(pos);
                true
            }
            Err(_) => false,
        }
    }

    /// Blocks an account, dropping the oldest block at the limit.
    pub(crate) fn block(&mut self, account: u32) {
        if self.has_blocked(account) {
            return;
        }
        if self.state.blocked.len() >= BLOCK_LIMIT {
            self.state.blocked.remove(0);
        }
        self.state.blocked.push(account);
    }

    /// The probability of choosing each style (docs/model.md, 7.5), without
    /// the exposure term.
    pub fn style_mix(&self, temperature: f32) -> [f32; STYLE_COUNT] {
        softmax(&self.state.style_value, temperature)
    }

    /// The hostile share of the person's impressions on the previous day,
    /// `e` of docs/model.md, 7.5; 0 when they had none.
    pub fn hostile_exposure(&self) -> f32 {
        let items = self.state.yesterday_items;
        if items == 0 {
            0.0
        } else {
            self.state.yesterday_hostile_items as f32 / items as f32
        }
    }

    /// The probability of choosing each style with the exposure term of
    /// docs/model.md, 7.5: `contagion * e` added to the values of the hostile
    /// styles. Without contagion or exposure it is [`Person::style_mix`]
    /// exactly.
    pub fn exposed_style_mix(&self, temperature: f32, contagion: f32) -> [f32; STYLE_COUNT] {
        let lift = contagion * self.hostile_exposure();
        if lift == 0.0 {
            return self.style_mix(temperature);
        }
        softmax(&lift_hostile(&self.state.style_value, lift), temperature)
    }

    /// The probability of polishing a post (docs/model.md, 7.5): a logistic
    /// choice between the learned values of polished and plain posts at the
    /// polish temperature, kept at least `POLISH_EXPLORATION` away from 0
    /// and 1 so authors keep finding out whether polish pays.
    pub fn polish_probability(&self, choice_temperature: f32) -> f32 {
        let [plain, polished] = self.state.polish_value;
        let t = polish_temperature(choice_temperature);
        let p = 1.0 / (1.0 + (-(polished - plain) / t).exp());
        POLISH_EXPLORATION + (1.0 - 2.0 * POLISH_EXPLORATION) * p
    }
}

/// The temperature of the polish choice as a share of the style-choice
/// temperature (docs/model.md, 7.5). Polish changes a post's reward by a few
/// hundredths, against tenths between styles, so a choice as loose as the
/// style choice would never tell polish that pays from polish that does not.
pub const POLISH_TEMPERATURE_SHARE: f32 = 0.1;

/// The learning rate of the polish values as a share of `learning_rate`
/// (docs/model.md, 7.5): what polish adds to a reward is small against the
/// spread of rewards between posts, so authors judge it over many posts.
pub const POLISH_LEARNING_SHARE: f32 = 0.15;

/// The temperature of the polish choice.
pub fn polish_temperature(choice_temperature: f32) -> f32 {
    (POLISH_TEMPERATURE_SHARE * choice_temperature).max(1e-3)
}

/// How far, in polish temperatures, the learned value of a polished post
/// starts below that of a plain one (docs/model.md, 7.5).
pub const POLISH_PRIOR_TEMPERATURES: f32 = 3.0;

/// Least chance of either polishing or not polishing a post.
pub const POLISH_EXPLORATION: f32 = 0.05;

/// Style values with `lift` added to the hostile styles' (docs/model.md,
/// 7.5).
pub(crate) fn lift_hostile(values: &[f32; STYLE_COUNT], lift: f32) -> [f32; STYLE_COUNT] {
    let mut out = *values;
    for style in Style::ALL {
        if style.is_hostile() {
            out[style.index()] += lift;
        }
    }
    out
}

/// The chance that a post's style is hostile (Outrage or Attack) after a
/// previous-day feed without a hostile item (`e = 0`) and after a fully
/// hostile one (`e = 1`), from the style choice of docs/model.md, 7.5 with
/// the given values: the two arms of the causal exposure diagnostic.
pub(crate) fn hostile_chances(
    values: &[f32; STYLE_COUNT],
    temperature: f32,
    contagion: f32,
) -> (f32, f32) {
    let hostile = |mix: [f32; STYLE_COUNT]| -> f32 {
        Style::ALL
            .iter()
            .filter(|s| s.is_hostile())
            .map(|s| mix[s.index()])
            .sum()
    };
    let calm = hostile(softmax(values, temperature));
    let exposed = hostile(softmax(&lift_hostile(values, contagion), temperature));
    (calm, exposed)
}

/// Softmax with a temperature, stable against large values.
pub(crate) fn softmax(values: &[f32; STYLE_COUNT], temperature: f32) -> [f32; STYLE_COUNT] {
    let t = temperature.max(1e-3);
    let max = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let mut out = values.map(|v| ((v - max) / t).exp());
    let total: f32 = out.iter().sum();
    out.iter_mut().for_each(|p| *p /= total);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::init;

    /// A user with the given style values who saw `items` items yesterday,
    /// `hostile` of them Outrage or Attack posts.
    fn person(values: [f32; STYLE_COUNT], items: u32, hostile: u32) -> Person {
        let mut config = Config::default();
        config.run.population_size = 50;
        let world = init::build_world(&config);
        let mut p = world.people[0].clone();
        p.state.style_value = values;
        p.state.yesterday_items = items;
        p.state.yesterday_hostile_items = hostile;
        p
    }

    const VALUES: [f32; STYLE_COUNT] = [0.3, 0.2, 0.25, 0.1, 0.0, -0.2];

    #[test]
    fn the_exposure_term_lifts_the_hostile_styles_by_contagion_times_exposure() {
        let p = person(VALUES, 10, 4);
        assert!((p.hostile_exposure() - 0.4).abs() < 1e-6);
        // By hand: kappa 0.5 and e 0.4 add 0.2 to Outrage and Attack, at
        // temperature 0.25.
        let lifted: [f32; STYLE_COUNT] = [0.3, 0.2, 0.25, 0.3, 0.0, 0.0];
        let weights: Vec<f64> = lifted
            .iter()
            .map(|v| (f64::from(*v) / 0.25).exp())
            .collect();
        let total: f64 = weights.iter().sum();
        let mix = p.exposed_style_mix(0.25, 0.5);
        for (got, w) in mix.iter().zip(&weights) {
            assert!(
                (f64::from(*got) - w / total).abs() < 1e-6,
                "{got} {}",
                w / total
            );
        }
        assert!(mix[Style::Outrage.index()] > p.style_mix(0.25)[Style::Outrage.index()]);
    }

    #[test]
    fn without_contagion_or_exposure_the_style_mix_is_unchanged() {
        let p = person(VALUES, 10, 4);
        assert_eq!(p.exposed_style_mix(0.25, 0.0), p.style_mix(0.25));
        let calm = person(VALUES, 10, 0);
        assert_eq!(calm.exposed_style_mix(0.25, 0.7), calm.style_mix(0.25));
        let away = person(VALUES, 0, 0);
        assert_eq!(away.hostile_exposure(), 0.0);
        assert_eq!(away.exposed_style_mix(0.25, 0.7), away.style_mix(0.25));
    }

    #[test]
    fn the_causal_diagnostic_compares_a_hostile_with_a_calm_feed() {
        let (calm, exposed) = hostile_chances(&VALUES, 0.25, 0.0);
        assert_eq!(calm, exposed);
        let (calm, exposed) = hostile_chances(&VALUES, 0.25, 0.28);
        let w = |v: f32| f64::from(v / 0.25).exp();
        let hostile = w(0.1) + w(-0.2);
        let other = w(0.3) + w(0.2) + w(0.25) + w(0.0);
        let k = f64::from(0.28f32 / 0.25).exp();
        assert!((f64::from(calm) - hostile / (hostile + other)).abs() < 1e-6);
        let lifted = hostile * k / (hostile * k + other);
        assert!((f64::from(exposed) - lifted).abs() < 1e-6);
        // The full-exposure arm is the mix of a person whose whole feed was
        // hostile yesterday.
        let p = person(VALUES, 7, 7);
        let mix = p.exposed_style_mix(0.25, 0.28);
        let from_mix = mix[Style::Outrage.index()] + mix[Style::Attack.index()];
        assert!((from_mix - exposed).abs() < 1e-6);
    }
}
