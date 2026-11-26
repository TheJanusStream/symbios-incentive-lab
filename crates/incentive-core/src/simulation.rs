//! The simulation: one call to [`Simulation::step`] simulates one day
//! (docs/model.md, section 5).

use crate::bots;
use crate::config::{Config, DesignLevers};
use crate::content::Post;
use crate::economy;
use crate::error::ConfigError;
use crate::experiment;
use crate::init;
use crate::inspector::{self, AgentDay, AgentView, FeedExplanation, Inspector};
use crate::learning;
use crate::lifecycle;
use crate::measure;
use crate::metrics::{DailyMetrics, MetricId, Summary};
use crate::moderation;
use crate::person::Person;
use crate::posting;
use crate::ranking::{self, FeedCtx, FeedKind, FeedScratch, Trending};
use crate::rng::{self, Stream};
use crate::session;
use crate::society;
use crate::world::{DayCtx, DayTally, World};

/// Shortest summary window in days.
const MIN_SUMMARY_DAYS: usize = 30;

/// A running simulation. It is plain data: `Send + Sync + Clone`, so an app can
/// hold it in a resource and a compare mode can run two side by side.
#[derive(Clone, Debug)]
pub struct Simulation {
    config: Config,
    pending_design: Option<DesignLevers>,
    day: u32,
    world: World,
    history: Vec<DailyMetrics>,
    inspector: Inspector,
    scratch: FeedScratch,
}

impl Simulation {
    /// Builds the world of day 0 from a validated configuration.
    pub fn new(config: Config) -> Result<Self, ConfigError> {
        config.validate()?;
        let world = init::build_world(&config);
        let inspector = Inspector::new(&world, &config);
        Ok(Self {
            config,
            pending_design: None,
            day: 0,
            world,
            history: Vec::new(),
            inspector,
            scratch: FeedScratch::default(),
        })
    }

    /// The configuration in force (a pending design change shows from the
    /// next day on).
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Days simulated so far, which is also the index of the next day.
    pub fn day(&self) -> u32 {
        self.day
    }

    /// Whether every configured day has been simulated.
    pub fn is_finished(&self) -> bool {
        self.day >= self.config.run.days
    }

    /// Changes the design levers from the next day on.
    pub fn set_design(&mut self, levers: DesignLevers) -> Result<(), ConfigError> {
        let mut candidate = self.config.clone();
        candidate.design = levers;
        candidate.validate()?;
        self.pending_design = Some(candidate.design);
        Ok(())
    }

    /// Simulates one day and returns its metrics; `None` once finished.
    pub fn step(&mut self) -> Option<DailyMetrics> {
        if self.is_finished() {
            return None;
        }
        if let Some(design) = self.pending_design.take() {
            self.config.design = design;
        }
        let ctx = DayCtx {
            config: &self.config,
            day: self.day,
        };
        let world = &mut self.world;
        start_day(world);
        experiment::begin_day(world, &ctx);
        bots::replace_banned(world, &ctx);
        lifecycle::joining(world, &ctx);
        assign_user_controls(world, &ctx);
        session::decide_openers(world, &ctx);
        let first_new = world.posts.next_id();
        posting::write_posts(world, &ctx);
        moderation::classify_new_posts(world, &ctx, first_new);
        moderation::deliver_harassment(world, &ctx, first_new);
        session::run_sessions(world, &ctx, &mut self.scratch, &mut self.inspector);
        bots::boost(world, &ctx);
        moderation::community_notes(world, &ctx);
        moderation::human_review(world, &ctx);
        economy::run(world, &ctx);
        learning::run(world, &ctx);
        lifecycle::daily_updates(world, &ctx);
        society::run(world, &ctx);
        experiment::end_day(world, &ctx);
        let metrics = measure::measure(world, &ctx);
        world
            .truth
            .push_toxic_share(metrics.get(MetricId::ToxicShare));
        world.accounts.end_day();
        self.inspector
            .record(world, self.day, self.config.world.choice_temperature);
        self.history.push(metrics.clone());
        self.day += 1;
        Some(metrics)
    }

    /// The world, for tests that look inside.
    #[cfg(test)]
    pub(crate) fn world(&self) -> &World {
        &self.world
    }

    /// Simulates every remaining day and returns the run summary.
    pub fn run_to_end(&mut self) -> Summary {
        while self.step().is_some() {}
        self.summary()
    }

    /// Every simulated day's metrics, in order.
    pub fn history(&self) -> &[DailyMetrics] {
        &self.history
    }

    /// Days a summary averages: `summary_window` of the configured days, at
    /// least 30, at most the run.
    pub fn summary_days(&self) -> usize {
        let days = self.config.run.days as usize;
        let window = (self.config.run.summary_window * days as f32).round() as usize;
        window.max(MIN_SUMMARY_DAYS).min(days)
    }

    /// The summary of the last [`Simulation::summary_days`] simulated days.
    pub fn summary(&self) -> Summary {
        Summary::from_history(&self.history, self.summary_days())
    }

    /// Number of individuals.
    pub fn population(&self) -> usize {
        self.world.population()
    }

    /// One individual.
    pub fn person(&self, id: u32) -> Option<&Person> {
        self.world.people.get(id as usize)
    }

    /// Every individual, in index order.
    pub fn people(&self) -> &[Person] {
        &self.world.people
    }

    /// A live post; posts leave after their lifetime.
    pub fn post(&self, id: u32) -> Option<&Post> {
        self.world.posts.get(id)
    }

    /// Every live post, oldest first.
    pub fn posts(&self) -> impl Iterator<Item = &Post> {
        self.world.posts.iter()
    }

    /// Accounts following an account (individual or bot).
    pub fn followers(&self, account: u32) -> u32 {
        self.world
            .accounts
            .followers
            .get(account as usize)
            .copied()
            .unwrap_or(0)
    }

    /// The individuals the inspector tracks.
    pub fn tracked(&self) -> Vec<u32> {
        self.inspector.tracked_ids()
    }

    /// Starts tracking an individual from now on; false when unknown or
    /// already tracked.
    pub fn track(&mut self, id: u32) -> bool {
        (id as usize) < self.population() && self.inspector.track(id)
    }

    /// The daily history of a tracked individual, oldest first, at most
    /// [`crate::AGENT_HISTORY_DAYS`] days.
    pub fn agent_history(&self, id: u32) -> Option<impl Iterator<Item = &AgentDay>> {
        self.inspector.history(id).map(|h| h.iter())
    }

    /// Traits and current state of one individual.
    pub fn agent_view(&self, id: u32) -> Option<AgentView> {
        let person = self.world.people.get(id as usize)?;
        Some(AgentView {
            id,
            archetype: person.traits.archetype,
            traits: person.traits.clone(),
            state: person.state.clone(),
            followers: self.followers(id),
            style_mix: person.style_mix(self.config.world.choice_temperature),
            tracked: self.inspector.history(id).is_some(),
        })
    }

    /// The feed a tracked individual was served on their last day with a
    /// session, as it was served: one ranked list for the whole day, read by
    /// the day's sessions in turn (docs/model.md, 6.1), with every item the
    /// feed held, its score components, what the viewer truly felt about it
    /// and what they did with it (`seen` marks the items a session reached,
    /// `session` which one). `None` for an untracked individual, or before
    /// their first session since tracking began.
    pub fn served_feed(&self, id: u32) -> Option<&FeedExplanation> {
        self.inspector.served_feed(id)
    }

    /// A preview: the feed `viewer` would get on opening the app now, as
    /// long as the first session's cap, with every score component, for any
    /// user, tracked or not. It ranks the posts
    /// alive at the end of the last simulated day as the next day's session
    /// would, so they are a day older and that day's new posts do not exist
    /// yet; what a session really served is [`Simulation::served_feed`]. It
    /// uses its own random stream and changes nothing, so it never alters the
    /// run. `None` for an unknown individual or a non-user.
    pub fn explain_feed(&self, viewer: u32) -> Option<FeedExplanation> {
        let v = viewer as usize;
        if !self.world.people.get(v)?.state.is_user {
            return None;
        }
        let trending = Trending::build(&self.world);
        let mut scratch = FeedScratch::default();
        scratch.reset(self.world.posts.len());
        let mut rng = rng::stream_rng(
            self.config.run.seed,
            self.day,
            u64::from(viewer),
            Stream::Explain,
        );
        let limit = session::session_limit(&self.world, &self.config, v);
        let ctx = FeedCtx {
            world: &self.world,
            config: &self.config,
            day: self.day,
            viewer: v,
        };
        let kind = ranking::build_feed(&ctx, limit, &trending, &mut rng, &mut scratch);
        if kind == FeedKind::Chronological {
            ranking::score_for_explanation(&ctx, &mut rng, &mut scratch);
        }
        Some(FeedExplanation {
            viewer,
            day: self.day,
            served: false,
            kind,
            session_limit: limit,
            sessions: Vec::new(),
            items: inspector::explained_items(
                &self.world,
                &scratch.candidates,
                None,
                self.config.world.time_cost,
            ),
        })
    }
}

/// Clears the day's accumulators.
fn start_day(world: &mut World) {
    world.tally = DayTally::default();
    for person in &mut world.people {
        person.state.today.clear();
    }
}

/// Applies the user controls of the current design to today's users, after
/// joining: the alternative feed and the time limit go to the least impulsive
/// `round(share * users)` users, so the realised share among users equals the
/// lever (docs/model.md, 6.5 and 6.10). Non-users hold neither.
fn assign_user_controls(world: &mut World, ctx: &DayCtx) {
    let design = &ctx.config.design;
    let mut users: Vec<(u32, usize)> = world
        .people
        .iter()
        .enumerate()
        .filter(|(_, p)| p.state.is_user)
        .map(|(i, _)| (world.truth.impulsivity_rank[i], i))
        .collect();
    users.sort_unstable();
    let n = users.len() as f32;
    let alt_cut = (design.alt_feed_share * n).round() as usize;
    let limit_cut = (design.time_limit_share * n).round() as usize;
    for person in &mut world.people {
        person.state.alt_feed_user = false;
        person.state.time_limited = false;
    }
    for (order, &(_, i)) in users.iter().enumerate() {
        let state = &mut world.people[i].state;
        state.alt_feed_user = order < alt_cut;
        state.time_limited = order < limit_cut;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_controls_reach_their_share_of_users_only() {
        let mut config = Config::default();
        config.run.population_size = 500;
        config.design.alt_feed_share = 0.3;
        config.design.time_limit_share = 0.5;
        let mut world = init::build_world(&config);
        let ctx = DayCtx {
            config: &config,
            day: 0,
        };
        assign_user_controls(&mut world, &ctx);
        let users: Vec<&Person> = world.people.iter().filter(|p| p.state.is_user).collect();
        let n = users.len() as f32;
        let alt = users.iter().filter(|p| p.state.alt_feed_user).count();
        let limited = users.iter().filter(|p| p.state.time_limited).count();
        assert_eq!(alt, (0.3 * n).round() as usize);
        assert_eq!(limited, (0.5 * n).round() as usize);
        assert!(
            world
                .people
                .iter()
                .filter(|p| !p.state.is_user)
                .all(|p| !p.state.alt_feed_user && !p.state.time_limited)
        );
        // The least impulsive users adopt first.
        let max_alt = users
            .iter()
            .filter(|p| p.state.alt_feed_user)
            .map(|p| p.traits.impulsivity)
            .fold(0.0f32, f32::max);
        assert!(
            users
                .iter()
                .filter(|p| !p.state.alt_feed_user)
                .all(|p| p.traits.impulsivity >= max_alt)
        );
    }
}
