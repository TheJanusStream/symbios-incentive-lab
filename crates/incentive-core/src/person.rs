//! The evolving state of an individual (docs/model.md, 3.3).

use serde::Serialize;

use crate::content::{NEUTRAL_INTENSITY, STYLE_COUNT};
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
    /// Activity of the last 64 days, today in the lowest bit.
    #[serde(skip)]
    pub(crate) active_bits: u64,
    /// Today's accumulators.
    #[serde(skip)]
    pub(crate) today: DayAccumulators,
}

/// What happened to one person today; cleared every day.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct DayAccumulators {
    pub opened: bool,
    pub posts: u32,
    pub items: u32,
    pub e_imp: f32,
    pub e_ref: f32,
    pub harassment: u32,
    pub likes_received: u32,
    pub mutual_replies: u32,
    pub angry_received: u32,
    pub income: f32,
    pub styles_written: u8,
    pub meaningful: u32,
    /// Misinformation this person shared today: truth and whether noted.
    pub misinformation_shares: Vec<(f32, bool)>,
    /// Posts that gave this person positive reflective value, for fan funding.
    pub fan_credits: Vec<(u32, f32)>,
}

impl DayAccumulators {
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
            active_bits: 0,
            today: DayAccumulators::default(),
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

    /// The probability of choosing each style (docs/model.md, 7.5).
    pub fn style_mix(&self, temperature: f32) -> [f32; STYLE_COUNT] {
        softmax(&self.state.style_value, temperature)
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

/// Softmax with a temperature, stable against large values.
pub(crate) fn softmax(values: &[f32; STYLE_COUNT], temperature: f32) -> [f32; STYLE_COUNT] {
    let t = temperature.max(1e-3);
    let max = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let mut out = values.map(|v| ((v - max) / t).exp());
    let total: f32 = out.iter().sum();
    out.iter_mut().for_each(|p| *p /= total);
    out
}
