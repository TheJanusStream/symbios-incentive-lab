//! The lab's state and logic, free of any engine or widget: the modes with
//! their runners, the run controls, the time-sliced checks and what each
//! button does. The views in [`crate::ui`] read and change it; the Bevy layer
//! only calls [`Lab::frame`] once a frame.

pub mod atlas;
pub mod batch;
pub mod levers;
pub mod runner;
pub mod series;

use incentive_core::{
    Archetype, Config, ConfigError, DesignLevers, GoalResult, MetricId, Scenario, Summary, preset,
    presets, scenarios,
};

use atlas::AtlasState;
use batch::{ScenarioConfirm, SeedCheck};
use runner::Runner;

/// Seeds of the compare mode's multi-seed check: 9 to 16, which the
/// searches behind the Atlas never selected designs on (they search on
/// seeds 1 and 2), so an atlas design cannot pass the check by the luck of
/// the seeds it was picked on.
pub const CHECK_SEEDS: [u64; 8] = [9, 10, 11, 12, 13, 14, 15, 16];
/// Seeds of a scenario's confirmation, as `incentive scenario` runs it.
pub const CONFIRM_SEEDS: [u64; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
/// Days the compare table averages.
pub const TABLE_DAYS: usize = 30;

/// The default population: smaller in the browser, which runs slower.
pub fn default_population() -> u32 {
    if cfg!(target_arch = "wasm32") {
        1_000
    } else {
        2_000
    }
}

/// The top-level views.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    /// One platform, every lever, the inspector.
    Sandbox,
    /// Two designs side by side.
    Compare,
    /// Challenges with goals.
    Scenarios,
    /// Precomputed explorations of the design space.
    Atlas,
    /// What the lab is and how to read it.
    About,
}

impl Tab {
    /// Every tab in display order.
    pub const ALL: [Tab; 5] = [
        Tab::Sandbox,
        Tab::Compare,
        Tab::Scenarios,
        Tab::Atlas,
        Tab::About,
    ];

    /// Display name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Sandbox => "Sandbox",
            Self::Compare => "Compare",
            Self::Scenarios => "Scenarios",
            Self::Atlas => "Atlas",
            Self::About => "About",
        }
    }

    /// A tab by its name, ignoring case (for `--tab`).
    pub fn from_name(name: &str) -> Option<Tab> {
        Self::ALL
            .into_iter()
            .find(|t| t.name().eq_ignore_ascii_case(name.trim()))
    }
}

/// How fast a playing run advances.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Speed {
    /// One day per second.
    One,
    /// Five days per second.
    Five,
    /// Twenty days per second.
    Twenty,
    /// As many days as the frame budget allows.
    Max,
}

impl Speed {
    /// Every speed in display order.
    pub const ALL: [Speed; 4] = [Speed::One, Speed::Five, Speed::Twenty, Speed::Max];

    /// Days per second; `None` for as fast as the frame budget allows.
    pub fn days_per_second(self) -> Option<f32> {
        match self {
            Self::One => Some(1.0),
            Self::Five => Some(5.0),
            Self::Twenty => Some(20.0),
            Self::Max => None,
        }
    }

    /// Display name.
    pub fn label(self) -> &'static str {
        match self {
            Self::One => "1 day/s",
            Self::Five => "5 days/s",
            Self::Twenty => "20 days/s",
            Self::Max => "Max",
        }
    }
}

/// Which side of the observable/truth split the sandbox charts show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewFilter {
    /// Only what the platform can measure.
    PlatformOnly,
    /// Both, side by side.
    Both,
    /// Only the god's-eye view.
    TruthOnly,
}

/// Seed, population and run length, shared by every mode; a change applies
/// at the next restart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunChoice {
    /// Seed of every random stream.
    pub seed: u64,
    /// Individuals.
    pub population: u32,
    /// Days per run (scenarios keep their own).
    pub days: u32,
}

impl RunChoice {
    /// The choice written into a configuration.
    pub fn apply(&self, mut config: Config) -> Config {
        config.run.seed = self.seed;
        config.run.population_size = self.population;
        config.run.days = self.days;
        config
    }
}

impl Default for RunChoice {
    fn default() -> Self {
        Self {
            seed: 1,
            population: default_population(),
            days: 365,
        }
    }
}

/// A first experiment for the "Start here" box, taken from the documented
/// findings (docs/model.md, 13.3).
#[derive(Clone, Copy, Debug)]
pub struct Suggestion {
    /// What to do, in a few words.
    pub title: &'static str,
    /// What to watch for.
    pub body: &'static str,
    apply: fn(&mut DesignLevers),
}

impl Suggestion {
    /// The design with the suggestion applied.
    pub fn applied(&self, design: &DesignLevers) -> DesignLevers {
        let mut d = design.clone();
        (self.apply)(&mut d);
        d
    }
}

/// The three suggestions of the "Start here" box.
pub const SUGGESTIONS: [Suggestion; 3] = [
    Suggestion {
        title: "Stop rewarding angry replies, trust good reporters",
        body: "Set the reply weight to 0, give the platform ten times the human reviewers, and weigh each \
            report by how often its reporter's earlier reports were upheld. Over a year the harm index falls \
            by about two fifths while the engagement index holds, because toxicity drives people away; try \
            it again with churn, harassment exits and the reputation effect switched off. A strong see-less \
            penalty would backfire here: bot farms mark the other side's popular posts.",
        apply: |d| {
            d.w_reply = 0.0;
            d.review_capacity = 10.0;
            d.report_trust = 1.0;
        },
    },
    Suggestion {
        title: "Rank for quality and bridging",
        body: "Load the Quality and bridging design. The harm index falls by about four fifths, and the \
            engagement index by about a fifth. Watch the survey: the platform's own numbers say people \
            are slightly less happy than under the engagement ranking, while the truth says they are better \
            off. The survey mixes wellbeing with recent pleasure and habit, and the engagement ranking \
            serves more of both.",
        apply: |d| {
            *d = preset("quality-bridging")
                .map(|p| p.design())
                .unwrap_or_default()
        },
    },
    Suggestion {
        title: "Make the classifier stricter",
        body: "Lower the classifier threshold to 0.3. Toxic impressions fall by about nineteen twentieths \
            over a year, and authors' evasion barely dents that. But wrongful removals rise from about 4 to 50 \
            a day, and the harm index, which counts them, falls by only a quarter.",
        apply: |d| d.classifier_threshold = 0.3,
    },
];

/// The sandbox: one platform the player designs.
#[derive(Clone, Debug)]
pub struct Sandbox {
    /// The running simulation.
    pub runner: Runner,
    /// The configuration being edited: its design applies live, its world
    /// and population at the next restart.
    pub draft: Config,
    /// Configuration JSON pasted by the player.
    pub load_text: String,
    /// The outcome of the last load or copy.
    pub message: Option<String>,
}

impl Sandbox {
    fn new(draft: Config, run: RunChoice) -> Result<Self, ConfigError> {
        Ok(Self {
            runner: Runner::new(vec![("Your platform".into(), run.apply(draft.clone()))])?,
            draft,
            load_text: String::new(),
            message: None,
        })
    }

    /// The configuration a restart would run.
    pub fn planned(&self, run: RunChoice) -> Config {
        run.apply(self.draft.clone())
    }

    /// The preset whose design the draft has, if any.
    pub fn preset_index(&self) -> Option<usize> {
        presets()
            .iter()
            .position(|p| p.design() == self.draft.design)
    }
}

/// Two designs on the same seed and world, in lockstep.
#[derive(Clone, Debug)]
pub struct Compare {
    /// The two simulations, A then B.
    pub runner: Runner,
    /// The two configurations being edited; they share world and population.
    pub drafts: [Config; 2],
    /// The side the lever panel edits (0 for A, 1 for B).
    pub editing: usize,
    /// The multi-seed check, once started.
    pub check: Option<SeedCheck>,
    /// The configurations the check runs, to flag a check that no longer
    /// matches the drafts.
    pub check_configs: Option<[Config; 2]>,
    /// The metrics the overlaid charts show.
    pub metrics: Vec<MetricId>,
    /// The check's table also shows what the indices are made of.
    pub show_components: bool,
    /// The outcome of the last atlas load.
    pub message: Option<String>,
}

impl Compare {
    fn new(run: RunChoice) -> Result<Self, ConfigError> {
        let a = preset("engagement-maximiser")?.config();
        let b = preset("quality-bridging")?.config();
        let drafts = [a, b];
        Ok(Self {
            runner: Self::runner(&drafts, run)?,
            drafts,
            editing: 0,
            check: None,
            check_configs: None,
            metrics: series::DEFAULT_COMPARE.to_vec(),
            show_components: false,
            message: None,
        })
    }

    fn runner(drafts: &[Config; 2], run: RunChoice) -> Result<Runner, ConfigError> {
        Runner::new(vec![
            ("A".into(), run.apply(drafts[0].clone())),
            ("B".into(), run.apply(drafts[1].clone())),
        ])
    }

    /// Copies the edited side's world and population to the other side: the
    /// two designs always face the same world.
    pub fn share_world(&mut self) {
        let from = self.editing.min(1);
        let world = self.drafts[from].world.clone();
        let population = self.drafts[from].population.clone();
        self.drafts[1 - from].world = world;
        self.drafts[1 - from].population = population;
    }

    /// Whether the check ran another setup than the current drafts with `run`:
    /// a change to either design, the world, the population or the run length
    /// makes it stale (its seeds are its own).
    pub fn check_is_stale(&self, run: RunChoice) -> bool {
        self.check_configs.as_ref().is_some_and(|ran| {
            (0..2).any(|i| !same_setup(&ran[i], &run.apply(self.drafts[i].clone())))
        })
    }
}

/// A scenario being played: the player's design against the fixed baseline.
#[derive(Clone, Debug)]
pub struct ScenarioPlay {
    /// Position in [`scenarios`].
    pub index: usize,
    /// The player's run, then the baseline's.
    pub runner: Runner,
    /// The player's configuration; only its design may change.
    pub draft: Config,
    /// The verdict at the end of the run.
    pub results: Option<Vec<GoalResult>>,
    /// The multi-seed confirmation, once started.
    pub confirm: Option<ScenarioConfirm>,
    /// The player's configuration the confirmation runs, to flag a
    /// confirmation that no longer matches.
    pub confirm_config: Option<Config>,
    /// The spoiler question for the known solution is open.
    pub spoiler_open: bool,
    /// The known solution was loaded.
    pub solution_loaded: bool,
}

impl ScenarioPlay {
    /// The scenario.
    pub fn scenario(&self) -> &'static Scenario {
        &scenarios()[self.index.min(scenarios().len() - 1)]
    }

    /// The goals judged on what has run so far (or the final verdict).
    pub fn live_results(&self) -> Vec<GoalResult> {
        if let Some(r) = &self.results {
            return r.clone();
        }
        let summary = |i: usize| {
            let sim = &self.runner.lane(i).sim;
            Summary::from_history(sim.history(), sim.summary_days())
        };
        self.scenario().evaluate(&summary(0), &summary(1))
    }
}

/// The scenario list and the scenario being played.
#[derive(Clone, Debug, Default)]
pub struct Scenarios {
    /// The highlighted scenario.
    pub selected: usize,
    /// The scenario being played.
    pub play: Option<ScenarioPlay>,
    /// Days of a scenario run instead of the scenario's own, for quick
    /// tests and the screenshot; the game itself never sets it.
    pub days_override: Option<u32>,
}

/// The inspector panel's state.
#[derive(Clone, Debug)]
pub struct InspectorState {
    /// The panel is shown (Sandbox only).
    pub open: bool,
    /// The individual shown in detail.
    pub selected: Option<u32>,
    /// The archetype "Track another" picks from.
    pub archetype: Archetype,
    /// The outcome of the last "Track another".
    pub message: Option<String>,
}

impl Default for InspectorState {
    fn default() -> Self {
        Self {
            open: false,
            selected: None,
            archetype: Archetype::Vulnerable,
            message: None,
        }
    }
}

/// What the charts show: the sandbox's picks, and smoothing everywhere.
#[derive(Clone, Debug)]
pub struct ViewState {
    /// Platform only, both or truth only.
    pub filter: ViewFilter,
    /// The platform's metrics charted.
    pub observable: Vec<MetricId>,
    /// The true metrics charted.
    pub truth: Vec<MetricId>,
    /// The "Start here" box is shown.
    pub start_here: bool,
    /// Every daily chart and sparkline, on every tab, draws the trailing
    /// 7-day mean instead of the daily values. Tables and verdicts always use
    /// the raw summaries.
    pub smooth: bool,
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            filter: ViewFilter::Both,
            observable: series::DEFAULT_OBSERVABLE.to_vec(),
            truth: series::DEFAULT_TRUTH.to_vec(),
            start_here: true,
            smooth: true,
        }
    }
}

/// Everything the lab holds.
#[derive(Clone, Debug)]
pub struct Lab {
    /// The tab shown.
    pub tab: Tab,
    /// The active tab's run advances on its own.
    pub playing: bool,
    /// How fast it advances.
    pub speed: Speed,
    /// Seed, population and length for the next restart.
    pub run: RunChoice,
    /// The sandbox.
    pub sandbox: Sandbox,
    /// The compare mode.
    pub compare: Compare,
    /// The scenarios.
    pub scenarios: Scenarios,
    /// The inspector.
    pub inspector: InspectorState,
    /// The sandbox's chart choices.
    pub view: ViewState,
    /// The Atlas tab.
    pub atlas: AtlasState,
    /// A message for the top bar, such as a failed restart.
    pub status: Option<String>,
    due: f32,
}

impl Default for Lab {
    fn default() -> Self {
        Self::new(RunChoice::default())
    }
}

impl Lab {
    /// A lab on the given seed, population and length, paused on day 0 of
    /// the Sandbox with the default design.
    pub fn new(run: RunChoice) -> Self {
        // Every configuration here starts from validated defaults and presets
        // and a population the controls keep in range, so building it cannot
        // fail; the fallbacks only keep the app alive if it ever did.
        let sandbox = Sandbox::new(Config::default(), run)
            .or_else(|_| Sandbox::new(Config::default(), RunChoice::default()))
            .unwrap_or_else(|e| unreachable!("default configuration rejected: {e}"));
        let compare = Compare::new(run)
            .or_else(|_| Compare::new(RunChoice::default()))
            .unwrap_or_else(|e| unreachable!("preset configuration rejected: {e}"));
        Self {
            tab: Tab::Sandbox,
            playing: false,
            speed: Speed::Five,
            run,
            sandbox,
            compare,
            scenarios: Scenarios::default(),
            inspector: InspectorState::default(),
            view: ViewState::default(),
            atlas: AtlasState::default(),
            status: None,
            due: 0.0,
        }
    }

    /// The runner of the active tab; `None` on Atlas, on About and on
    /// Scenarios before a scenario is played.
    pub fn active_runner(&self) -> Option<&Runner> {
        match self.tab {
            Tab::Sandbox => Some(&self.sandbox.runner),
            Tab::Compare => Some(&self.compare.runner),
            Tab::Scenarios => self.scenarios.play.as_ref().map(|p| &p.runner),
            Tab::Atlas | Tab::About => None,
        }
    }

    fn active_runner_mut(&mut self) -> Option<&mut Runner> {
        match self.tab {
            Tab::Sandbox => Some(&mut self.sandbox.runner),
            Tab::Compare => Some(&mut self.compare.runner),
            Tab::Scenarios => self.scenarios.play.as_mut().map(|p| &mut p.runner),
            Tab::Atlas | Tab::About => None,
        }
    }

    fn active_finished(&self) -> bool {
        self.active_runner().is_none_or(Runner::is_finished)
    }

    /// Simulates one day of the active tab's run.
    pub fn step_once(&mut self) {
        if let Some(runner) = self.active_runner_mut() {
            runner.step();
        }
        self.after_step();
    }

    /// Judges a scenario once its run ends.
    fn after_step(&mut self) {
        if let Some(play) = self.scenarios.play.as_mut()
            && play.results.is_none()
            && play.runner.is_finished()
        {
            let player = play.runner.lane(0).sim.summary();
            let baseline = play.runner.lane(1).sim.summary();
            play.results = Some(play.scenario().evaluate(&player, &baseline));
        }
    }

    /// One frame: advances the active run at the chosen speed and works on
    /// the multi-seed checks. `dt` is the frame time in seconds; every loop
    /// does at least one day of what is due, then continues while
    /// `keep_going` allows (the frame's time budget).
    pub fn frame(&mut self, dt: f32, keep_going: &mut dyn FnMut() -> bool) {
        if self.playing {
            match self.speed.days_per_second() {
                None => loop {
                    self.step_once();
                    if self.active_finished() || !keep_going() {
                        break;
                    }
                },
                Some(rate) => {
                    // At most a second of backlog after a stall.
                    self.due = (self.due + rate * dt.max(0.0)).min(rate.max(1.0));
                    while self.due >= 1.0 && !self.active_finished() {
                        self.step_once();
                        self.due -= 1.0;
                        if !keep_going() {
                            break;
                        }
                    }
                }
            }
            if self.active_finished() {
                self.playing = false;
                self.due = 0.0;
            }
        }
        if let Some(check) = self.compare.check.as_mut() {
            check.work(&series::check_metrics(), keep_going);
        }
        if let Some(play) = self.scenarios.play.as_mut() {
            let scenario = play.scenario();
            if let Some(confirm) = play.confirm.as_mut() {
                confirm.work(scenario, keep_going);
            }
        }
    }

    /// Runs `days` days of the active tab at once (the screenshot's warm-up).
    pub fn run_days(&mut self, days: u32) {
        for _ in 0..days {
            if self.active_finished() {
                break;
            }
            self.step_once();
        }
    }

    /// Whether a change made since the active run started (world, population,
    /// seed or length) waits for a restart.
    pub fn needs_restart(&self) -> bool {
        match self.tab {
            Tab::Sandbox => differs(
                self.sandbox.runner.lane(0).sim.config(),
                &self.sandbox.planned(self.run),
            ),
            Tab::Compare => self.compare_needs_restart(),
            Tab::Scenarios => self.scenario_needs_restart(),
            Tab::Atlas | Tab::About => false,
        }
    }

    /// Whether the compare pair waits for a restart (see
    /// [`Lab::needs_restart`]); its multi-seed check waits too, so that the
    /// check and the live pair always share a setup.
    pub fn compare_needs_restart(&self) -> bool {
        (0..2).any(|i| {
            differs(
                self.compare.runner.lane(i).sim.config(),
                &self.run.apply(self.compare.drafts[i].clone()),
            )
        })
    }

    /// Whether the scenario being played waits for a restart; its
    /// confirmation waits too.
    pub fn scenario_needs_restart(&self) -> bool {
        self.scenarios.play.as_ref().is_some_and(|p| {
            differs(
                p.runner.lane(0).sim.config(),
                &self.scenario_config(p.index, Some(&p.draft.design)),
            )
        })
    }

    /// Whether the scenario's confirmation ran another setup than a
    /// confirmation would run now: another design or population (its seeds
    /// are its own).
    pub fn confirm_is_stale(&self) -> bool {
        self.scenarios.play.as_ref().is_some_and(|p| {
            p.confirm_config.as_ref().is_some_and(|ran| {
                !same_setup(ran, &self.scenario_config(p.index, Some(&p.draft.design)))
            })
        })
    }

    /// Whether the player changed the design after the scenario run ended, so
    /// the verdict on screen judges a design that is no longer the current one.
    pub fn verdict_is_stale(&self) -> bool {
        self.scenarios.play.as_ref().is_some_and(|p| {
            p.results.is_some() && p.draft.design != p.runner.lane(0).sim.config().design
        })
    }

    /// Restarts the active tab's run from day 0 with the current settings.
    pub fn restart(&mut self) {
        let result = match self.tab {
            Tab::Sandbox => self.restart_sandbox(),
            Tab::Compare => Compare::runner(&self.compare.drafts, self.run).map(|r| {
                self.compare.runner = r;
            }),
            Tab::Scenarios => match &self.scenarios.play {
                Some(p) => self.start_scenario_with(p.index, p.draft.design.clone()),
                None => Ok(()),
            },
            Tab::Atlas | Tab::About => Ok(()),
        };
        self.due = 0.0;
        self.status = result.err().map(|e| format!("Restart failed: {e}"));
    }

    fn restart_sandbox(&mut self) -> Result<(), ConfigError> {
        self.sandbox.runner = Runner::new(vec![(
            "Your platform".into(),
            self.sandbox.planned(self.run),
        )])?;
        let tracked = self.sandbox.runner.lane(0).sim.tracked();
        if self
            .inspector
            .selected
            .is_some_and(|id| !tracked.contains(&id))
        {
            self.inspector.selected = None;
        }
        Ok(())
    }

    /// Applies the sandbox draft's design to the running simulation from the
    /// next day on.
    pub fn apply_sandbox_design(&mut self, note: Option<String>) {
        let design = self.sandbox.draft.design.clone();
        self.status = self
            .sandbox
            .runner
            .set_design(0, design, note)
            .err()
            .map(|e| format!("Design rejected: {e}"));
    }

    /// Applies the edited compare side's design from the next day on.
    pub fn apply_compare_design(&mut self, note: Option<String>) {
        let side = self.compare.editing.min(1);
        let design = self.compare.drafts[side].design.clone();
        self.status = self
            .compare
            .runner
            .set_design(side, design, note)
            .err()
            .map(|e| format!("Design rejected: {e}"));
    }

    /// Applies the player's scenario design from the next day on.
    pub fn apply_scenario_design(&mut self, note: Option<String>) {
        if let Some(play) = self.scenarios.play.as_mut() {
            let design = play.draft.design.clone();
            self.status = play
                .runner
                .set_design(0, design, note)
                .err()
                .map(|e| format!("Design rejected: {e}"));
        }
    }

    /// Applies a "Start here" suggestion to the sandbox design.
    pub fn apply_suggestion(&mut self, index: usize) {
        if let Some(s) = SUGGESTIONS.get(index) {
            self.sandbox.draft.design = s.applied(&self.sandbox.draft.design);
            self.apply_sandbox_design(Some(format!("Start here: {}", s.title)));
        }
    }

    /// Loads a pasted configuration into the sandbox and restarts it. The
    /// configuration's seed, population and length become the run settings.
    pub fn load_sandbox_config(&mut self, text: &str) {
        match Config::from_json(text) {
            Ok(config) => {
                self.run = RunChoice {
                    seed: config.run.seed,
                    population: config.run.population_size,
                    days: config.run.days,
                };
                self.sandbox.draft = config;
                self.tab = Tab::Sandbox;
                self.restart();
                self.sandbox.message = Some(match &self.status {
                    Some(s) => s.clone(),
                    None => "Configuration loaded; the run restarted from day 0.".into(),
                });
            }
            Err(e) => self.sandbox.message = Some(format!("Not loaded: {e}")),
        }
    }

    /// Loads a design from the atlas into the sandbox, with the world
    /// assumptions and population settings of `base` (the configuration the
    /// atlas searched from), and restarts at the lab's seed, population and
    /// length, which shows the design at full size.
    /// A design outside this build's parameter ranges is refused with a
    /// status message and nothing changes.
    pub fn open_in_sandbox(&mut self, design: DesignLevers, base: &Config) {
        if let Err(e) = atlas::check_design(&design, base) {
            self.status = Some(e);
            return;
        }
        self.sandbox.draft.design = design;
        self.sandbox.draft.world = base.world.clone();
        self.sandbox.draft.population = base.population.clone();
        self.tab = Tab::Sandbox;
        self.playing = false;
        self.restart();
        self.sandbox.message = Some(match &self.status {
            Some(s) => s.clone(),
            None => format!(
                "Atlas design loaded with the world assumptions it was found under; the run \
                 restarted from day 0 on seed {} with {} people for {} days. The atlas found it \
                 at a reduced size, so expect its numbers to differ.",
                self.run.seed, self.run.population, self.run.days
            ),
        });
    }

    /// Compares a design from the atlas, as B, with the engagement
    /// ranking, as A, both with the world assumptions and population
    /// settings of `base`, and restarts the compare mode. A design outside
    /// this build's parameter ranges is refused like in
    /// [`Lab::open_in_sandbox`].
    pub fn compare_with_engagement_ranking(&mut self, design: DesignLevers, base: &Config) {
        if let Err(e) = atlas::check_design(&design, base) {
            self.status = Some(e);
            return;
        }
        for draft in &mut self.compare.drafts {
            draft.world = base.world.clone();
            draft.population = base.population.clone();
        }
        self.compare.drafts[0].design = DesignLevers::default();
        self.compare.drafts[1].design = design;
        self.compare.editing = 1;
        self.tab = Tab::Compare;
        self.playing = false;
        self.restart();
        self.compare.message = Some(match &self.status {
            Some(s) => s.clone(),
            None => format!(
                "A is the engagement ranking and B the atlas design, both with the world \
                 assumptions the atlas used; the pair restarted from day 0 on seed {} with {} \
                 people for {} days. Check on 8 seeds before trusting the difference.",
                self.run.seed, self.run.population, self.run.days
            ),
        });
    }

    /// The configuration of a scenario run: the scenario's world and length,
    /// the lab's seed and population, and `design` (the baseline's own when
    /// `None`).
    pub fn scenario_config(&self, index: usize, design: Option<&DesignLevers>) -> Config {
        let sc = &scenarios()[index.min(scenarios().len() - 1)];
        let mut config = match design {
            Some(d) => sc.player_config(d),
            None => sc.config(),
        };
        config.run.seed = self.run.seed;
        config.run.population_size = self.run.population;
        if let Some(days) = self.scenarios.days_override {
            config.run.days = days;
        }
        config
    }

    /// Starts playing a scenario from its starting design.
    pub fn start_scenario(&mut self, index: usize) {
        let design = scenarios()[index.min(scenarios().len() - 1)]
            .config()
            .design;
        self.scenarios.selected = index;
        self.status = self
            .start_scenario_with(index, design)
            .err()
            .map(|e| format!("Scenario failed to start: {e}"));
    }

    fn start_scenario_with(
        &mut self,
        index: usize,
        design: DesignLevers,
    ) -> Result<(), ConfigError> {
        let player = self.scenario_config(index, Some(&design));
        let baseline = self.scenario_config(index, None);
        let runner = Runner::new(vec![
            ("You".into(), player.clone()),
            ("Baseline".into(), baseline),
        ])?;
        let previous = self.scenarios.play.take().filter(|p| p.index == index);
        let (confirm, confirm_config, solution_loaded) = previous
            .map(|p| (p.confirm, p.confirm_config, p.solution_loaded))
            .unwrap_or_default();
        self.scenarios.play = Some(ScenarioPlay {
            index,
            runner,
            draft: player,
            results: None,
            confirm,
            confirm_config,
            spoiler_open: false,
            solution_loaded,
        });
        Ok(())
    }

    /// Leaves the scenario being played.
    pub fn stop_scenario(&mut self) {
        self.scenarios.play = None;
        self.playing = false;
    }

    /// Loads the scenario's known solution into the player's design and
    /// restarts the pair, so the solution is judged over the whole run.
    pub fn load_known_solution(&mut self) {
        let Some(play) = self.scenarios.play.as_ref() else {
            return;
        };
        let index = play.index;
        let result = play
            .scenario()
            .known_solution()
            .and_then(|design| self.start_scenario_with(index, design));
        match result {
            Ok(()) => {
                if let Some(p) = self.scenarios.play.as_mut() {
                    p.solution_loaded = true;
                }
                self.status = None;
            }
            Err(e) => self.status = Some(format!("Known solution failed: {e}")),
        }
    }

    /// Starts the scenario's confirmation on [`CONFIRM_SEEDS`]; not while a
    /// change waits for a restart.
    pub fn start_confirm(&mut self) {
        if self.scenario_needs_restart() {
            self.status = Some("Restart first: a change waits to be applied.".into());
            return;
        }
        let Some(index) = self.scenarios.play.as_ref().map(|p| p.index) else {
            return;
        };
        let Some(design) = self.scenarios.play.as_ref().map(|p| p.draft.design.clone()) else {
            return;
        };
        let player = self.scenario_config(index, Some(&design));
        let baseline = self.scenario_config(index, None);
        if let Some(play) = self.scenarios.play.as_mut() {
            play.confirm = Some(ScenarioConfirm::new(
                &player,
                &baseline,
                CONFIRM_SEEDS.to_vec(),
            ));
            play.confirm_config = Some(player);
        }
    }

    /// Starts the compare mode's check on [`CHECK_SEEDS`], at the current
    /// population and length, with the current designs; not while a change
    /// waits for a restart, so the check runs the setup on screen.
    pub fn start_seed_check(&mut self) {
        if self.compare_needs_restart() {
            self.status = Some("Restart first: a change waits to be applied.".into());
            return;
        }
        let a = self.run.apply(self.compare.drafts[0].clone());
        let b = self.run.apply(self.compare.drafts[1].clone());
        self.compare.check = Some(SeedCheck::new(&a, &b, CHECK_SEEDS.to_vec()));
        self.compare.check_configs = Some([a, b]);
    }

    /// Starts tracking an untracked individual of `archetype` in the sandbox,
    /// preferring one who is on the platform, and selects them.
    pub fn track_another(&mut self, archetype: Archetype) -> Option<u32> {
        let sim = &self.sandbox.runner.lane(0).sim;
        let tracked = sim.tracked();
        let pick = sim
            .people()
            .iter()
            .enumerate()
            .filter(|(i, p)| p.traits.archetype == archetype && !tracked.contains(&(*i as u32)))
            .min_by_key(|(_, p)| !p.state.is_user)
            .map(|(i, _)| i as u32);
        match pick {
            Some(id) if self.sandbox.runner.lane_mut(0).sim.track(id) => {
                self.inspector.selected = Some(id);
                self.inspector.message = None;
                Some(id)
            }
            _ => {
                self.inspector.message =
                    Some(format!("Every {} is tracked already.", archetype.name()));
                None
            }
        }
    }
}

/// Whether two configurations differ in anything but the design (which
/// applies live).
fn differs(running: &Config, planned: &Config) -> bool {
    running.run != planned.run
        || running.population != planned.population
        || running.world != planned.world
}

/// Whether two configurations describe the same setup apart from the seed:
/// the same design, world, population settings and run settings. Multi-seed
/// checks run their own seeds, so the seed alone never makes them stale.
fn same_setup(ran: &Config, now: &Config) -> bool {
    let mut now = now.clone();
    now.run.seed = ran.run.seed;
    ran.design == now.design
        && ran.run == now.run
        && ran.population == now.population
        && ran.world == now.world
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small() -> RunChoice {
        RunChoice {
            seed: 1,
            population: 200,
            days: 6,
        }
    }

    #[test]
    fn tabs_round_trip_by_name() {
        for tab in Tab::ALL {
            assert_eq!(Tab::from_name(tab.name()), Some(tab));
        }
        assert_eq!(Tab::from_name(" compare "), Some(Tab::Compare));
        assert_eq!(Tab::from_name("nope"), None);
    }

    #[test]
    fn the_population_defaults_by_target() {
        let expected = if cfg!(target_arch = "wasm32") {
            1_000
        } else {
            2_000
        };
        assert_eq!(RunChoice::default().population, expected);
        assert_eq!(RunChoice::default().days, 365);
    }

    #[test]
    fn paced_play_steps_by_the_clock_and_stops_at_the_end() {
        let mut lab = Lab::new(small());
        lab.playing = true;
        lab.speed = Speed::Five;
        lab.frame(0.1, &mut || true);
        assert_eq!(lab.sandbox.runner.day(), 0, "half a day is not due yet");
        lab.frame(0.1, &mut || true);
        assert_eq!(lab.sandbox.runner.day(), 1);
        lab.frame(10.0, &mut || true);
        assert_eq!(lab.sandbox.runner.day(), 6, "a second of backlog at most");
        assert!(!lab.playing, "play stops at the end");
        lab.step_once();
        assert_eq!(lab.sandbox.runner.day(), 6);
    }

    #[test]
    fn max_speed_respects_the_budget() {
        let mut lab = Lab::new(small());
        lab.playing = true;
        lab.speed = Speed::Max;
        lab.frame(0.016, &mut || false);
        assert_eq!(lab.sandbox.runner.day(), 1, "at least one day per frame");
        let mut budget = 3;
        lab.frame(0.016, &mut || {
            budget -= 1;
            budget > 0
        });
        assert_eq!(lab.sandbox.runner.day(), 4);
    }

    #[test]
    fn world_changes_wait_for_a_restart_and_designs_apply_live() {
        let mut lab = Lab::new(small());
        lab.step_once();
        assert!(!lab.needs_restart());
        lab.sandbox.draft.world.outrage_appeal = 2.0;
        assert!(lab.needs_restart());
        lab.sandbox.draft.design.w_reply = 0.0;
        lab.apply_sandbox_design(None);
        lab.step_once();
        assert_eq!(lab.sandbox.runner.lane(0).sim.config().design.w_reply, 0.0);
        assert_eq!(lab.sandbox.runner.lane(0).changes.len(), 1);
        lab.restart();
        assert!(!lab.needs_restart());
        assert_eq!(lab.sandbox.runner.day(), 0);
        assert_eq!(
            lab.sandbox.runner.lane(0).sim.config().world.outrage_appeal,
            2.0
        );
        lab.run.seed = 9;
        assert!(lab.needs_restart());
    }

    #[test]
    fn suggestions_change_the_design() {
        let mut lab = Lab::new(small());
        for (i, s) in SUGGESTIONS.iter().enumerate() {
            let before = lab.sandbox.draft.design.clone();
            assert_ne!(s.applied(&before), before, "{}", s.title);
            lab.apply_suggestion(i);
            assert!(lab.status.is_none());
        }
        assert_eq!(
            lab.sandbox.runner.lane(0).changes.len(),
            1,
            "same day, one entry"
        );
        assert_eq!(lab.sandbox.preset_index(), None);
        lab.sandbox.draft.design = presets()[2].design();
        assert_eq!(lab.sandbox.preset_index(), Some(2));
    }

    #[test]
    fn configurations_load_and_bad_ones_are_reported() {
        let mut lab = Lab::new(small());
        let mut config = lab.sandbox.planned(lab.run);
        config.run.seed = 5;
        config.design.chronological = true;
        lab.load_sandbox_config(&config.to_json_pretty());
        assert_eq!(lab.run.seed, 5);
        assert!(lab.sandbox.runner.lane(0).sim.config().design.chronological);
        assert!(
            lab.sandbox
                .message
                .as_deref()
                .is_some_and(|m| m.contains("loaded"))
        );
        lab.load_sandbox_config("{ not json");
        assert!(
            lab.sandbox
                .message
                .as_deref()
                .is_some_and(|m| m.starts_with("Not loaded"))
        );
        assert_eq!(lab.run.seed, 5);
    }

    #[test]
    fn compare_sides_share_the_world_and_check_on_seeds() {
        let mut lab = Lab::new(small());
        lab.tab = Tab::Compare;
        lab.compare.editing = 1;
        lab.compare.drafts[1].world.harassment_harm = 5.0;
        lab.compare.share_world();
        assert_eq!(lab.compare.drafts[0].world.harassment_harm, 5.0);
        assert!(lab.needs_restart());
        lab.restart();
        lab.step_once();
        assert!(lab.compare.runner.lanes().iter().all(|l| l.sim.day() == 1));
        lab.run.days = 2;
        lab.start_seed_check();
        assert!(
            lab.compare.check.is_none(),
            "no check while a restart waits"
        );
        lab.restart();
        lab.start_seed_check();
        for _ in 0..1_000 {
            if lab.compare.check.as_ref().is_none_or(|c| c.rows.is_some()) {
                break;
            }
            lab.frame(0.0, &mut || true);
        }
        let check = lab.compare.check.as_ref().expect("a check");
        assert_eq!(check.seeds, CHECK_SEEDS);
        assert_eq!(
            check.seeds.first(),
            Some(&9),
            "seeds the atlas never chose on"
        );
        let rows = check.rows.clone().unwrap_or_default();
        assert_eq!(rows.len(), series::check_metrics().len());
        assert!(rows.iter().any(|r| r.metric == MetricId::NetRevenue));
        assert_eq!(rows[0].paired.pairs, CHECK_SEEDS.len());
        assert!(!lab.compare.check_is_stale(lab.run));
        lab.run.seed += 1;
        assert!(
            !lab.compare.check_is_stale(lab.run),
            "the check's seeds are its own"
        );
        lab.compare.drafts[1].world.harassment_harm = 2.0;
        lab.compare.share_world();
        assert!(lab.compare.check_is_stale(lab.run), "a world change");
        lab.compare.drafts[1].world.harassment_harm = 5.0;
        lab.compare.share_world();
        lab.run.population += 10;
        assert!(lab.compare.check_is_stale(lab.run), "a population change");
        lab.run.population -= 10;
        assert!(!lab.compare.check_is_stale(lab.run));
        lab.compare.drafts[0].design.w_like = 0.0;
        assert!(lab.compare.check_is_stale(lab.run), "a design change");
    }

    #[test]
    fn scenarios_are_judged_at_the_end_and_confirmed_on_seeds() {
        let mut lab = Lab::new(small());
        lab.tab = Tab::Scenarios;
        lab.scenarios.days_override = Some(4);
        lab.start_scenario(0);
        assert!(lab.status.is_none());
        assert_eq!(lab.active_runner().map(Runner::days), Some(4));
        let live = lab.scenarios.play.as_ref().map(|p| p.live_results().len());
        let goals = lab
            .scenarios
            .play
            .as_ref()
            .map(|p| p.scenario().goals.len());
        assert!(goals.is_some_and(|g| g >= 2));
        assert_eq!(live, goals, "one live result per goal");
        lab.run_days(10);
        let play = lab.scenarios.play.as_ref().expect("playing");
        assert_eq!(
            play.results.as_ref().map(Vec::len),
            Some(play.scenario().goals.len())
        );
        lab.start_confirm();
        for _ in 0..1_000 {
            let running = lab
                .scenarios
                .play
                .as_ref()
                .and_then(|p| p.confirm.as_ref())
                .is_some_and(|c| c.results.is_none());
            if !running {
                break;
            }
            lab.frame(0.0, &mut || true);
        }
        assert!(!lab.confirm_is_stale() && !lab.verdict_is_stale());
        let confirm = lab.scenarios.play.as_ref().and_then(|p| p.confirm.as_ref());
        let confirm = confirm.expect("a confirmation");
        assert_eq!(confirm.seeds, CONFIRM_SEEDS);
        let confirmed = confirm.results.as_ref().expect("finished");
        assert_eq!(confirmed.verdict.goals.len(), goals.unwrap_or_default());
        assert!(
            confirmed
                .verdict
                .goals
                .iter()
                .all(|g| g.margin.n == CONFIRM_SEEDS.len())
        );
        lab.run.population += 10;
        assert!(lab.confirm_is_stale(), "a population change");
        lab.run.population -= 10;
        if let Some(play) = lab.scenarios.play.as_mut() {
            play.draft.design.w_like += 0.5;
        }
        lab.apply_scenario_design(None);
        assert!(
            lab.verdict_is_stale(),
            "the verdict is for the design that ran"
        );
        lab.load_known_solution();
        let play = lab.scenarios.play.as_ref().expect("playing");
        assert!(play.solution_loaded && play.results.is_none());
        assert_eq!(play.runner.day(), 0);
        assert!(play.confirm.as_ref().is_some_and(|c| c.results.is_some()));
        assert!(
            lab.confirm_is_stale(),
            "the solution differs from the confirmed design"
        );
        let solution = play.scenario().known_solution().expect("solution");
        assert_eq!(play.draft.design, solution);
        assert_eq!(
            play.runner.lane(1).sim.config().design,
            play.scenario().config().design
        );
        lab.stop_scenario();
        assert!(lab.active_runner().is_none());
    }

    #[test]
    fn atlas_designs_open_in_the_sandbox_and_in_compare() {
        let mut lab = Lab::new(small());
        lab.tab = Tab::Atlas;
        assert!(lab.active_runner().is_none() && !lab.needs_restart());
        lab.restart();
        assert!(lab.status.is_none());
        let data = atlas::atlas().expect("the embedded atlas parses");
        let base = &data.pareto.settings.base;
        // An out-of-range design is refused, with a message, and runs nothing.
        let bad = DesignLevers {
            predictor_quality: 1.0,
            ..DesignLevers::default()
        };
        lab.open_in_sandbox(bad.clone(), base);
        lab.compare_with_engagement_ranking(bad, base);
        assert_eq!(lab.tab, Tab::Atlas);
        assert!(
            lab.status
                .as_deref()
                .is_some_and(|m| m.contains("not loaded"))
        );
        lab.status = None;
        let design = data
            .pareto
            .front
            .iter()
            .map(|d| d.design.clone())
            .find(|d| atlas::check_design(d, base).is_ok())
            .expect("an in-range design on the front");
        lab.step_once();
        // A world the player changed gives way to the atlas's.
        lab.sandbox.draft.world.outrage_appeal = 2.5;
        lab.open_in_sandbox(design.clone(), base);
        assert_eq!(lab.tab, Tab::Sandbox);
        assert_eq!(lab.sandbox.runner.day(), 0);
        assert_eq!(lab.sandbox.runner.lane(0).sim.config().design, design);
        assert!(
            lab.sandbox
                .message
                .as_deref()
                .is_some_and(|m| m.contains("Atlas"))
        );
        assert_eq!(lab.sandbox.runner.lane(0).sim.config().world, base.world);
        lab.compare.drafts[0].world.outrage_appeal = 2.5;
        lab.compare_with_engagement_ranking(design.clone(), base);
        assert_eq!(lab.tab, Tab::Compare);
        let lanes = lab.compare.runner.lanes();
        assert_eq!(lanes[0].sim.config().design, DesignLevers::default());
        assert_eq!(lanes[1].sim.config().design, design);
        assert!(lanes.iter().all(|l| l.sim.config().world == base.world));
        assert_eq!(lab.compare.runner.day(), 0);
        assert!(lab.compare.message.is_some());
    }

    #[test]
    fn track_another_picks_an_untracked_individual() {
        let mut lab = Lab::new(small());
        lab.step_once();
        let before = lab.sandbox.runner.lane(0).sim.tracked().len();
        let id = lab
            .track_another(Archetype::Troll)
            .expect("an untracked troll");
        let sim = &lab.sandbox.runner.lane(0).sim;
        assert_eq!(sim.tracked().len(), before + 1);
        assert_eq!(
            sim.agent_view(id).map(|v| v.archetype),
            Some(Archetype::Troll)
        );
        assert_eq!(lab.inspector.selected, Some(id));
    }
}
