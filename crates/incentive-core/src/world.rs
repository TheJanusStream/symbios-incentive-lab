//! The simulated world: people, bots, accounts, posts, the platform's own
//! state and the day's running tallies.

use std::collections::VecDeque;

use crate::bots::Bot;
use crate::config::{Config, FeedExperimentKind};
use crate::content::STYLE_COUNT;
use crate::experiment::{Experiment, Usage};
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
/// day-0 polarisation the indices measure against, the hidden impulsivity
/// order in which people adopt user controls, and a researcher's
/// experiment.
#[derive(Clone, Debug, Default)]
pub(crate) struct TruthState {
    /// Each individual's position when sorted by ascending impulsivity, a
    /// hidden trait (docs/model.md, 6.5 and 6.10).
    pub impulsivity_rank: Vec<u32>,
    /// Toxic impression shares of the latest days, oldest first.
    pub toxic_history: VecDeque<f32>,
    /// Society-wide affective polarisation before day 0.
    pub polarisation_day0: f32,
    /// Each individual's position when sorted by ascending partisanship, a
    /// fixed trait; only truth metrics read it (docs/model.md, 9.2).
    pub partisanship_rank: Vec<u32>,
    /// The researcher's experiment, deactivation, notifications off or a
    /// feed experiment, from its first day on (docs/model.md, 13.2).
    pub experiment: Option<Experiment>,
    /// Each individual's usage over the days before a notifications-off
    /// experiment, its participants' baseline; empty otherwise, and emptied
    /// on the experiment's first day.
    pub baseline_usage: Vec<Usage>,
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

    /// Whether individual `i` is on a deactivation experiment's break.
    pub(crate) fn on_break(&self, i: usize) -> bool {
        self.experiment.as_ref().is_some_and(|e| e.on_break(i))
    }

    /// How a feed experiment changes individual `i`'s feed, `None` for a
    /// viewer it does not treat (docs/model.md, 13.2).
    pub(crate) fn feed_treatment(&self, i: usize) -> Option<FeedExperimentKind> {
        self.experiment.as_ref().and_then(|e| e.feed_treatment(i))
    }

    /// Individual `i`'s own notification intensity under the design's
    /// `intensity`: 0 for someone whose notifications a notifications-off
    /// experiment switched off, the design's for everyone else
    /// (docs/model.md, 6.1 and 13.2).
    pub(crate) fn notification_intensity(&self, i: usize, intensity: f32) -> f32 {
        if self
            .experiment
            .as_ref()
            .is_some_and(|e| e.notifications_off(i))
        {
            0.0
        } else {
            intensity
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

/// Where a human author stands by the `partisanship` trait, ranked over the
/// whole population (docs/model.md, 9.2): in the more partisan half, the
/// most partisan tenth, the middle fifth (from 40% to 60%).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PartisanBand {
    pub half: bool,
    pub extreme: bool,
    pub middle: bool,
}

/// The partisanship band of `author`, `None` for an author outside the
/// population (a bot).
pub(crate) fn partisan_band(partisanship_rank: &[u32], author: usize) -> Option<PartisanBand> {
    let &rank = partisanship_rank.get(author)?;
    let quantile = (f64::from(rank) + 0.5) / partisanship_rank.len() as f64;
    Some(PartisanBand {
        half: quantile >= 0.5,
        extreme: quantile >= 0.9,
        middle: (0.4..0.6).contains(&quantile),
    })
}

/// Sums of the like probability's terms over one group of impressions
/// (docs/model.md, 9.2): the impressions, the terms of the logit, the
/// out-group multiplier and `P_like` itself.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct LikeTermSums {
    pub impressions: u32,
    pub bias: f64,
    pub interest: f64,
    pub arousal: f64,
    pub hostility: f64,
    pub social_proof: f64,
    pub reflective: f64,
    pub multiplier: f64,
    pub p_like: f64,
}

impl LikeTermSums {
    /// Adds one impression's terms and like probability.
    pub(crate) fn add(&mut self, t: &crate::behaviour::LikeTerms, p_like: f32) {
        self.impressions += 1;
        self.bias += f64::from(t.bias);
        self.interest += f64::from(t.interest);
        self.arousal += f64::from(t.arousal);
        self.hostility += f64::from(t.hostility);
        self.social_proof += f64::from(t.social_proof);
        self.reflective += f64::from(t.reflective);
        self.multiplier += f64::from(t.multiplier);
        self.p_like += f64::from(p_like);
    }
}

/// Today's diagnostics of how exposure shapes posting (docs/model.md, 7.5
/// and 9.2). The causal part sums, over the people who wrote at least one
/// post today, the chance that a post's style is hostile after a fully
/// hostile previous-day feed (`e = 1`) and after a feed without a hostile
/// item (`e = 0`), from each author's own style values; the observational
/// part counts human posts, and the hostile ones among them, by whether the
/// author's previous-day feed held a hostile item. Measurement only.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ContagionTally {
    pub authors: u32,
    pub chance_exposed: f64,
    pub chance_unexposed: f64,
    /// Summed per-author ratio of the two chances.
    pub ratio_sum: f64,
    pub exposed_posts: u32,
    pub exposed_hostile: u32,
    pub unexposed_posts: u32,
    pub unexposed_hostile: u32,
}

/// Running totals of the current day, turned into metrics at its end.
#[derive(Clone, Debug, Default)]
pub(crate) struct DayTally {
    pub openers: u32,
    /// Human sessions today, the first and the extra ones (docs/model.md,
    /// 6.1).
    pub sessions: u32,
    /// Of those, the sessions a notification started.
    pub notification_sessions: u32,
    /// Feeds served today, one per opener (docs/model.md, 6.2).
    pub feeds: u32,
    /// Candidates those feeds gathered, before ranking cut them to length.
    pub candidates: u32,
    pub new_accounts: u32,
    pub departures: u32,
    pub impressions: u32,
    /// Human impressions that carried ads: those of people who do not
    /// subscribe (docs/model.md, 8.3).
    pub ad_impressions: u32,
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
    /// Dwell on impressions worth less than their time, `R < time_cost * D`
    /// (docs/model.md, 9.2).
    pub low_value_dwell: f32,
    pub cross_group_likes: u32,
    pub revenue: f32,
    /// Of the revenue, what ads brought in.
    pub ad_revenue: f32,
    /// Of the revenue, what subscriptions brought in.
    pub subscription_revenue: f32,
    /// Users who pay for a subscription today.
    pub subscribers: u32,
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
    /// Summed truth of the posts people (not bots) shared today
    /// (docs/model.md, 9.2, `shared_truth`).
    pub shared_truth: f64,
    /// In-network candidates the day's feeds gathered, and of them those by
    /// the viewer's out-group (`in_network_outgroup_candidates`).
    pub in_network_candidates: u32,
    pub in_network_outgroup_candidates: u32,
    /// Impressions of in-network items, those of out-group posts, and the
    /// likes on each (`in_network_outgroup_impressions` and `_likes`).
    pub in_network_impressions: u32,
    pub in_network_outgroup_impressions: u32,
    pub in_network_likes: u32,
    pub in_network_outgroup_likes: u32,
    /// Posts ending today by human authors in the more partisan half, the
    /// most partisan tenth and the middle fifth by the `partisanship` trait,
    /// and their lifetime human impressions (`partisan_half_reach`,
    /// `extreme_reach_ratio`).
    pub partisan_half_posts: u32,
    pub partisan_half_impressions: u64,
    pub extreme_posts: u32,
    pub extreme_impressions: u64,
    pub middle_posts: u32,
    pub middle_impressions: u64,
    /// The same, over the posts ending today that were never removed or
    /// demoted (`partisan_half_reach_kept`, `extreme_reach_ratio_kept`).
    pub partisan_half_kept_posts: u32,
    pub partisan_half_kept_impressions: u64,
    pub extreme_kept_posts: u32,
    pub extreme_kept_impressions: u64,
    pub middle_kept_posts: u32,
    pub middle_kept_impressions: u64,
    /// The like probability's terms over today's human impressions, by
    /// same-group (`[0]`) and out-group (`[1]`) viewers (`like_*`).
    pub like_terms: [LikeTermSums; 2],
    /// Human shares of the noted posts ending today, all of them and those
    /// made before the note (`noted_shares_before_note`).
    pub noted_shares: u32,
    pub noted_shares_before_note: u32,
    /// Misinformation impressions today by the style of the post, bots'
    /// posts included, and those of bot posts alone (docs/model.md, 9.2).
    pub misinformation_by_style: [u32; STYLE_COUNT],
    pub misinformation_bot_impressions: u32,
    /// Posts with truth below 0.5 whose life ended today, bots' included,
    /// and of them those that got a community note (docs/model.md, 6.8).
    pub false_posts_ended: u32,
    pub noted_false_posts_ended: u32,
    /// The exposure diagnostics of today's style choices (docs/model.md,
    /// 7.5).
    pub contagion: ContagionTally,
    pub top_toxic_share: f32,
    pub top_truthful_share: f32,
}
