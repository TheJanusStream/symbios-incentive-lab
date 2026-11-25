//! The native `--screenshot` flag's plan: which tab, how many days, where
//! the PNG goes. The binary captures the window; this module only reads the
//! command line and prepares the lab, so it is tested without a window.

use std::path::PathBuf;

use crate::lab::{Lab, RunChoice, Tab};

/// Days run before the capture when `--days` is not given.
pub const DEFAULT_DAYS: u32 = 120;

/// How the screenshot flag is used.
pub const USAGE: &str = "usage: symbios-incentive-lab [--screenshot <path.png> [--tab <name>] [--days <n>]]\n\
    \n  --screenshot <path>  run, save one PNG of the window to <path> and exit\
    \n  --tab <name>         sandbox, inspector (the sandbox with the inspector open), compare,\
    \n                       scenarios (the first scenario, playing), atlas or about; default\
    \n                       sandbox\
    \n  --days <n>           days to simulate on seed 1 before the capture; default 120";

/// A screenshot to take.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    /// Where the PNG goes.
    pub path: PathBuf,
    /// The tab shown.
    pub tab: Tab,
    /// The sandbox's inspector is open.
    pub inspector: bool,
    /// Days simulated before the capture.
    pub days: u32,
}

impl Plan {
    /// Reads the command line (without the program name): `None` without
    /// `--screenshot`, an error for anything it does not know.
    pub fn from_args(args: impl Iterator<Item = String>) -> Result<Option<Plan>, String> {
        let mut path = None;
        let mut tab = Tab::Sandbox;
        let mut inspector = false;
        let mut days = DEFAULT_DAYS;
        let mut args = args;
        while let Some(arg) = args.next() {
            let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
            match arg.as_str() {
                "--screenshot" => path = Some(PathBuf::from(value("--screenshot")?)),
                "--tab" => {
                    let name = value("--tab")?;
                    inspector = name.trim().eq_ignore_ascii_case("inspector");
                    tab = if inspector {
                        Tab::Sandbox
                    } else {
                        Tab::from_name(&name).ok_or_else(|| format!("unknown tab {name}"))?
                    };
                }
                "--days" => {
                    let text = value("--days")?;
                    days = text
                        .trim()
                        .parse()
                        .map_err(|_| format!("--days takes a whole number, not {text}"))?;
                }
                other => return Err(format!("unknown argument {other}")),
            }
        }
        match path {
            Some(path) => Ok(Some(Plan {
                path,
                tab,
                inspector,
                days,
            })),
            None if tab != Tab::Sandbox || inspector || days != DEFAULT_DAYS => {
                Err("--tab and --days need --screenshot".into())
            }
            None => Ok(None),
        }
    }

    /// The lab to capture: seed 1, the tab shown, the days already run,
    /// paused.
    pub fn lab(&self) -> Lab {
        let mut lab = Lab::new(RunChoice {
            seed: 1,
            ..RunChoice::default()
        });
        self.prepare(&mut lab);
        lab
    }

    /// Shows the tab in `lab` and runs the days.
    pub fn prepare(&self, lab: &mut Lab) {
        lab.tab = self.tab;
        lab.inspector.open = self.inspector;
        lab.view.start_here = false;
        if self.tab == Tab::Scenarios {
            lab.start_scenario(0);
        }
        lab.run_days(self.days);
        lab.playing = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Option<Plan>, String> {
        Plan::from_args(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn arguments_parse() {
        assert_eq!(parse(&[]), Ok(None));
        let plan = parse(&["--screenshot", "a.png", "--tab", "compare", "--days", "30"]);
        assert_eq!(
            plan,
            Ok(Some(Plan {
                path: "a.png".into(),
                tab: Tab::Compare,
                inspector: false,
                days: 30
            }))
        );
        let inspector = parse(&["--screenshot", "b.png", "--tab", "Inspector"]);
        assert!(inspector.is_ok_and(|p| p.is_some_and(|p| p.inspector && p.tab == Tab::Sandbox)));
        let atlas = parse(&["--screenshot", "c.png", "--tab", "atlas"]);
        assert!(atlas.is_ok_and(|p| p.is_some_and(|p| p.tab == Tab::Atlas)));
        assert!(parse(&["--tab", "about"]).is_err());
        assert!(parse(&["--screenshot"]).is_err());
        assert!(parse(&["--screenshot", "a.png", "--days", "x"]).is_err());
        assert!(parse(&["--screenshot", "a.png", "--tab", "nope"]).is_err());
        assert!(parse(&["--wat"]).is_err());
    }

    #[test]
    fn a_plan_prepares_the_lab() {
        let plan = Plan {
            path: "x.png".into(),
            tab: Tab::Scenarios,
            inspector: false,
            days: 3,
        };
        let mut lab = Lab::new(RunChoice {
            seed: 1,
            population: 200,
            days: 10,
        });
        lab.scenarios.days_override = Some(5);
        plan.prepare(&mut lab);
        assert_eq!(lab.tab, Tab::Scenarios);
        assert_eq!(lab.active_runner().map(|r| r.day()), Some(3));
        assert!(!lab.playing && !lab.view.start_here);
    }
}
