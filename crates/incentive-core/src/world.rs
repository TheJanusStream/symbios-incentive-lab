//! The simulated world: people, bots, accounts, posts, the platform's own
//! state and the day's running tallies.

use std::collections::VecDeque;

use crate::bots::Bot;
use crate::config::Config;
use crate::content::STYLE_COUNT;
use crate::network::Accounts;
use crate::person::Person;
use crate::posts::PostStore;
use crate::rng::{self, Stream, StreamRng};

/// The configuration and the day a phase runs on.
#[derive(Clone, Copy, Debug)]
pub(crate) struct DayCtx<'a> {
    pub config: &'a Config,
    pub day: u32,
}

impl DayCtx<'_> {
    /// The random stream of one entity for one purpose today.
    pub(crate) fn rng(&self, entity: u64, stream: Stream) -> StreamRng {
        rng::stream_rng(self.config.run.seed, self.day, entity, stream)
    }
}

/// Everything that evolves during a run.
#[derive(Clone, Debug)]
pub(crate) struct World {
    pub people: Vec<Person>,
    /// Offline ties per individual; they never change.
    pub ties: Vec<Vec<u32>>,
    pub bots: Vec<Bot>,
    pub accounts: Accounts,
    pub posts: PostStore,
    pub platform: PlatformState,
    pub truth: TruthState,
    pub tally: DayTally,
}

impl World {
    /// Number of individuals.
    pub(crate) fn population(&self) -> usize {
        self.people.len()
    }

    /// Number of current human users.
    pub(crate) fn user_count(&self) -> usize {
        self.people.iter().filter(|p| p.state.is_user).count()
    }

    /// Account id of a bot slot.
    pub(crate) fn bot_account(&self, slot: usize) -> u32 {
        (self.people.len() + slot) as u32
    }

    /// Bot slot of an account id, if the account is a bot.
    pub(crate) fn bot_slot(&self, account: u32) -> Option<usize> {
        (account as usize).checked_sub(self.people.len())
    }
}

/// The platform's running per-style averages of observed engagement, the
/// `P_style` of docs/model.md, 6.3.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct StyleRates {
    pub like: f32,
    pub reply: f32,
    pub share: f32,
    pub dwell: f32,
}

impl Default for StyleRates {
    fn default() -> Self {
        Self {
            like: 0.15,
            reply: 0.03,
            share: 0.03,
            dwell: 1.4,
        }
    }
}

/// Days the toxic reputation averages over.
pub(crate) const REPUTATION_DAYS: usize = 30;

/// State the platform keeps across days: only what it can observe.
#[derive(Clone, Debug, Default)]
pub(crate) struct PlatformState {
    pub style_rates: [StyleRates; STYLE_COUNT],
    /// How much evasion the retrained classifier sees through, in `[0, 1]`
    /// (docs/model.md, 6.7).
    pub counter_evasion: f32,
}

/// Truth-side state kept across days, which the platform cannot observe:
/// the toxicity people experienced, behind the platform's reputation, the
/// day-0 polarisation the indices measure against, and the hidden impulsivity
/// order in which people adopt user controls.
#[derive(Clone, Debug, Default)]
pub(crate) struct TruthState {
    /// Each individual's position when sorted by ascending impulsivity, a
    /// hidden trait (docs/model.md, 6.5 and 6.10).
    pub impulsivity_rank: Vec<u32>,
    /// Toxic impression shares of the latest days, oldest first.
    pub toxic_history: VecDeque<f32>,
    /// Society-wide affective polarisation before day 0.
    pub polarisation_day0: f32,
}

impl TruthState {
    /// The 30-day moving average of the toxic impression share.
    pub(crate) fn toxic_reputation(&self) -> f32 {
        if self.toxic_history.is_empty() {
            0.0
        } else {
            self.toxic_history.iter().sum::<f32>() / self.toxic_history.len() as f32
        }
    }

    /// Records today's toxic impression share.
    pub(crate) fn push_toxic_share(&mut self, share: f32) {
        if self.toxic_history.len() == REPUTATION_DAYS {
            self.toxic_history.pop_front();
        }
        self.toxic_history.push_back(share);
    }
}

/// Running totals of the current day, turned into metrics at its end.
#[derive(Clone, Debug, Default)]
pub(crate) struct DayTally {
    pub openers: u32,
    pub new_accounts: u32,
    pub departures: u32,
    pub impressions: u32,
    pub dwell: f32,
    pub likes: u32,
    pub replies: u32,
    pub shares: u32,
    pub bot_likes: u32,
    pub bot_shares: u32,
    pub reports: u32,
    pub see_less: u32,
    /// Reports and see-less marks bots filed today (included in the above).
    pub bot_reports: u32,
    pub bot_see_less: u32,
    pub auto_removals: u32,
    pub human_removals: u32,
    pub notes: u32,
    pub false_positives: u32,
    /// Toxic posts reviewers confirmed that the classifier had not flagged.
    pub confirmed_misses: u32,
    /// Summed evasion those missed posts were written with.
    pub miss_evasion: f32,
    pub harassment: u32,
    pub toxic_impressions: u32,
    pub outrage_impressions: u32,
    pub misinformation_impressions: u32,
    pub regret_items: u32,
    pub reflective_sum: f32,
    pub cross_group_likes: u32,
    pub revenue: f32,
    pub payouts: f32,
    pub payout_by_style: [f32; STYLE_COUNT],
    pub impressions_by_style: [u32; STYLE_COUNT],
    /// Human posts written today, by style (bots' posts are not counted).
    pub posts_by_style: [u32; STYLE_COUNT],
    /// Human posts written today that their authors polished.
    pub polished_posts: u32,
    /// Posts bots wrote today.
    pub bot_posts: u32,
    pub likes_by_style: [u32; STYLE_COUNT],
    pub replies_by_style: [u32; STYLE_COUNT],
    pub shares_by_style: [u32; STYLE_COUNT],
    /// Bot likes and shares today, by the style of the boosted post.
    pub bot_likes_by_style: [u32; STYLE_COUNT],
    pub bot_shares_by_style: [u32; STYLE_COUNT],
    pub dwell_by_style: [f32; STYLE_COUNT],
    pub largest_cascade: u32,
    pub top_toxic_share: f32,
    pub top_truthful_share: f32,
}
