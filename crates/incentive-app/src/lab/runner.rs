//! Runners: one simulation, or several stepped in lockstep on the same seed
//! and world, each with a log of the design changes made while it ran.

use incentive_core::{Config, ConfigError, DesignLevers, Simulation};

use super::levers::describe_design_change;

/// A design change during a run, drawn as a marker on every chart.
#[derive(Clone, Debug, PartialEq)]
pub struct DesignChange {
    /// The first day the new design is in force.
    pub day: u32,
    /// What changed, one lever per line, after an optional note such as the
    /// preset that was picked.
    pub lines: Vec<String>,
}

impl DesignChange {
    /// The change as hover text.
    pub fn text(&self) -> String {
        format!("From day {}:\n{}", self.day, self.lines.join("\n"))
    }
}

/// One simulation of a runner.
#[derive(Clone, Debug)]
pub struct Lane {
    /// Display name, such as "A" or "Baseline".
    pub label: String,
    /// The simulation.
    pub sim: Simulation,
    /// Design changes made while it ran, in day order, at most one per day.
    pub changes: Vec<DesignChange>,
}

/// One or more simulations that advance together, one day at a time.
#[derive(Clone, Debug)]
pub struct Runner {
    lanes: Vec<Lane>,
}

impl Runner {
    /// Builds a runner from labelled configurations; all lanes should share
    /// their run settings so they stay in lockstep.
    pub fn new(configs: Vec<(String, Config)>) -> Result<Self, ConfigError> {
        let lanes = configs
            .into_iter()
            .map(|(label, config)| {
                Ok(Lane {
                    label,
                    sim: Simulation::new(config)?,
                    changes: Vec::new(),
                })
            })
            .collect::<Result<Vec<_>, ConfigError>>()?;
        if lanes.is_empty() {
            return Err(ConfigError::Invalid("a runner needs a simulation".into()));
        }
        Ok(Self { lanes })
    }

    /// Every lane.
    pub fn lanes(&self) -> &[Lane] {
        &self.lanes
    }

    /// One lane.
    pub fn lane(&self, index: usize) -> &Lane {
        &self.lanes[index.min(self.lanes.len() - 1)]
    }

    /// One lane, mutable (tracking individuals in the inspector).
    pub fn lane_mut(&mut self, index: usize) -> &mut Lane {
        let last = self.lanes.len() - 1;
        &mut self.lanes[index.min(last)]
    }

    /// Days simulated so far by the first lane.
    pub fn day(&self) -> u32 {
        self.lanes[0].sim.day()
    }

    /// Days the run lasts.
    pub fn days(&self) -> u32 {
        self.lanes[0].sim.config().run.days
    }

    /// Whether every lane has run every day.
    pub fn is_finished(&self) -> bool {
        self.lanes.iter().all(|l| l.sim.is_finished())
    }

    /// Simulates one day in every lane that is not finished; false when
    /// nothing was left to simulate.
    pub fn step(&mut self) -> bool {
        let mut stepped = false;
        for lane in &mut self.lanes {
            stepped |= lane.sim.step().is_some();
        }
        stepped
    }

    /// Changes one lane's design from the next day on and logs the change.
    /// Several changes on the same day merge into one entry that describes
    /// the difference from the design in force; a change that is undone the
    /// same day leaves no entry.
    pub fn set_design(
        &mut self,
        index: usize,
        design: DesignLevers,
        note: Option<String>,
    ) -> Result<(), ConfigError> {
        let lane = self.lane_mut(index);
        let in_force = lane.sim.config().design.clone();
        lane.sim.set_design(design.clone())?;
        let day = lane.sim.day();
        let mut lines = describe_design_change(&in_force, &design);
        if let Some(note) = note.filter(|_| !lines.is_empty()) {
            lines.insert(0, note);
        }
        if lane.changes.last().is_some_and(|c| c.day == day) {
            lane.changes.pop();
        }
        if !lines.is_empty() {
            lane.changes.push(DesignChange { day, lines });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny(days: u32) -> Config {
        let mut config = Config::default();
        config.run.population_size = 200;
        config.run.days = days;
        config
    }

    #[test]
    fn lanes_step_in_lockstep_until_the_end() {
        let mut runner =
            Runner::new(vec![("A".into(), tiny(3)), ("B".into(), tiny(3))]).expect("valid");
        assert_eq!((runner.day(), runner.days()), (0, 3));
        while runner.step() {}
        assert!(runner.is_finished());
        assert!(runner.lanes().iter().all(|l| l.sim.history().len() == 3));
        assert!(!runner.step());
        assert!(Runner::new(Vec::new()).is_err());
    }

    #[test]
    fn design_changes_merge_per_day_and_vanish_when_undone() {
        let mut runner = Runner::new(vec![("A".into(), tiny(5))]).expect("valid");
        runner.step();
        let base = runner.lane(0).sim.config().design.clone();
        let mut design = base.clone();
        design.w_reply = 1.0;
        assert!(runner.set_design(0, design.clone(), None).is_ok());
        design.w_reply = 0.0;
        assert!(
            runner
                .set_design(0, design.clone(), Some("Note".into()))
                .is_ok()
        );
        let changes = &runner.lane(0).changes;
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].day, 1);
        assert_eq!(changes[0].lines, ["Note", "Reply weight 1.5 -> 0.0"]);
        assert!(changes[0].text().starts_with("From day 1:"));
        assert!(runner.set_design(0, base.clone(), None).is_ok());
        assert!(runner.lane(0).changes.is_empty());
        assert!(runner.set_design(0, design, None).is_ok());
        runner.step();
        assert_eq!(runner.lane(0).sim.config().design.w_reply, 0.0);
        let mut bad = base;
        bad.share_friction = 7.0;
        assert!(runner.set_design(0, bad, None).is_err());
        assert_eq!(runner.lane(0).changes.len(), 1);
    }
}
