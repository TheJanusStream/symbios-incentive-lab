//! `config --from-atlas`: a design found by `incentive explore` as a
//! configuration file, so `compare` and `factorial` can take it.

use std::path::Path;

use incentive_core::{AtlasDesign, Config, ExploreSettings, GoodhartAtlas, MapAtlas, ParetoAtlas};

use crate::error::CliError;

/// Prints design `selector` of the atlas in `file` as a configuration JSON:
/// the search's base configuration (run size, population, world) with that
/// design.
pub fn print_atlas_config(file: &Path, selector: &str) -> Result<(), CliError> {
    let text = std::fs::read_to_string(file).map_err(|source| CliError::Io {
        path: file.to_path_buf(),
        source,
    })?;
    let (config, what) = pick(&text, selector)?;
    eprintln!(
        "{what} of {}: the search's base configuration (N = {}, {} days); pass --population and --days \
         to run it at another size",
        file.display(),
        config.run.population_size,
        config.run.days
    );
    outln!("{}", config.to_json_pretty());
    Ok(())
}

/// The configuration of design `selector` of an atlas file, and what it is:
///
/// - a Pareto front: the front design's number, from 1 (as printed and in the
///   app);
/// - a map: the cell `x,y` (column, row from 0, as printed);
/// - a Goodhart search: the generation number, or `last`.
pub fn pick(text: &str, selector: &str) -> Result<(Config, String), CliError> {
    let bad = |message: String| CliError::Explore(message);
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|e| bad(format!("not an atlas file: {e}")))?;
    let mode = value
        .get("settings")
        .and_then(|s| s.get("mode"))
        .and_then(|m| m.as_str())
        .unwrap_or_default()
        .to_string();
    let parse = |e: serde_json::Error| bad(format!("not a {mode} atlas: {e}"));
    let selector = selector.trim();
    let (settings, design, what): (ExploreSettings, AtlasDesign, String) = match mode.as_str() {
        "pareto" => {
            let atlas: ParetoAtlas = serde_json::from_value(value).map_err(parse)?;
            let n: usize = selector.parse().map_err(|_| {
                bad(format!(
                    "a front design is a number from 1, not {selector:?}"
                ))
            })?;
            let count = atlas.front.len();
            let design = n
                .checked_sub(1)
                .and_then(|i| atlas.front.get(i))
                .cloned()
                .ok_or_else(|| bad(format!("the front has designs 1 to {count}, not {n}")))?;
            (atlas.settings, design, format!("front design {n}"))
        }
        "map" => {
            let atlas: MapAtlas = serde_json::from_value(value).map_err(parse)?;
            let cell = selector
                .split_once(',')
                .and_then(|(x, y)| Some((x.trim().parse().ok()?, y.trim().parse().ok()?)));
            let Some((x, y)) = cell else {
                return Err(bad(format!(
                    "a map cell is `x,y` (column, row from 0), not {selector:?}"
                )));
            };
            let design = atlas
                .cell(x, y)
                .map(|c| c.elite.clone())
                .ok_or_else(|| bad(format!("map cell {x},{y} is empty")))?;
            (atlas.settings, design, format!("map cell {x},{y}"))
        }
        "goodhart" => {
            let atlas: GoodhartAtlas = serde_json::from_value(value).map_err(parse)?;
            let generation = if selector == "last" {
                atlas.generations.last()
            } else {
                let g: u32 = selector.parse().map_err(|_| {
                    bad(format!(
                        "a Goodhart design is a generation number or `last`, not {selector:?}"
                    ))
                })?;
                atlas.generations.iter().find(|x| x.generation == g)
            }
            .ok_or_else(|| bad(format!("no generation {selector} in the file")))?;
            let what = format!("Goodhart generation {}", generation.generation);
            (atlas.settings, generation.best.clone(), what)
        }
        other => return Err(bad(format!("unknown atlas mode {other:?}"))),
    };
    let mut config = settings.base;
    config.design = design.design;
    config.validate()?;
    Ok((config, what))
}

#[cfg(test)]
mod tests {
    use super::*;
    use incentive_core::{AtlasMetrics, DesignLevers, MapCell, MapGrid, ParetoAtlas};

    fn settings(mode: &str) -> ExploreSettings {
        let mut base = Config::default();
        base.run.days = 180;
        base.run.population_size = 600;
        ExploreSettings {
            mode: mode.into(),
            seed: 1,
            seeds: vec![1, 2],
            population: 600,
            days: 180,
            generations: 1,
            batch: 1,
            initial: 1,
            mutation_rate: 0.1,
            evaluations: 2,
            runs: 4,
            world: "defaults".into(),
            command: String::new(),
            base,
            confirm_seeds: Vec::new(),
        }
    }

    fn design(w_reply: f32) -> AtlasDesign {
        let metrics = AtlasMetrics {
            engagement_index: 0.2,
            harm_index: 0.2,
            benefit_index: 0.5,
            toxic_share: 0.05,
            wellbeing_ever_users: 0.6,
            creators_living: 0.0,
            polarisation_society: 0.4,
            user_share: 0.7,
            survey_satisfaction: 0.7,
            informedness_society: 0.5,
            meaningful_per_user: 1.0,
            discourse_quality: 0.6,
        };
        AtlasDesign {
            design: DesignLevers {
                w_reply,
                ..DesignLevers::default()
            },
            metrics,
            confirmed: None,
        }
    }

    #[test]
    fn designs_are_picked_by_their_documented_selectors() {
        let front = ParetoAtlas {
            settings: settings("pareto"),
            front: vec![design(0.1), design(0.2)],
        };
        let text = serde_json::to_string(&front).unwrap_or_default();
        let (config, what) = pick(&text, "2").expect("design 2");
        assert_eq!(
            (config.design.w_reply, what.as_str()),
            (0.2, "front design 2")
        );
        assert_eq!((config.run.days, config.run.population_size), (180, 600));
        assert!(pick(&text, "0").is_err() && pick(&text, "3").is_err());
        let map = MapAtlas {
            settings: settings("map"),
            grid: MapGrid {
                resolution: 4,
                engagement: [0.0, 0.4],
                harm: [0.0, 0.4],
            },
            cells: vec![MapCell {
                x: 2,
                y: 1,
                elite: design(0.3),
            }],
        };
        let text = serde_json::to_string(&map).unwrap_or_default();
        assert_eq!(
            pick(&text, "2,1").map(|c| c.0.design.w_reply).ok(),
            Some(0.3)
        );
        assert!(pick(&text, "1,1").is_err() && pick(&text, "7").is_err());
    }
}
