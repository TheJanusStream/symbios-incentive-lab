//! The precomputed exploration atlas (docs/model.md, 13.7): a MAP-Elites map,
//! a Pareto front and two Goodhart runs, written by `incentive explore` into
//! `assets/atlas/`. The files are embedded in the binary and parsed once, so
//! the Atlas tab works natively and in the browser without file access.

use std::sync::OnceLock;

use incentive_core::{
    AtlasDesign, Config, DesignLevers, GoodhartAtlas, MapAtlas, ParamKind, ParamSide, ParetoAtlas,
    inert_levers, params,
};

use super::levers::{LeverValue, read, tree};

/// The embedded map.
const MAP_JSON: &str = include_str!("../../assets/atlas/map.json");
/// The embedded Pareto front.
const PARETO_JSON: &str = include_str!("../../assets/atlas/pareto.json");
/// The embedded Goodhart runs, one per proxy.
const GOODHART_JSON: [&str; 2] = [
    include_str!("../../assets/atlas/goodhart-engagement_index.json"),
    include_str!("../../assets/atlas/goodhart-survey_satisfaction.json"),
];

/// Every precomputed result.
#[derive(Clone, Debug, PartialEq)]
pub struct AtlasData {
    /// The map of engagement against harm.
    pub map: MapAtlas,
    /// The Pareto front.
    pub pareto: ParetoAtlas,
    /// The Goodhart runs, in proxy order.
    pub goodhart: Vec<GoodhartAtlas>,
}

impl AtlasData {
    /// Parses the three kinds of atlas file.
    pub fn parse(map: &str, pareto: &str, goodhart: &[&str]) -> Result<Self, String> {
        let map: MapAtlas = serde_json::from_str(map).map_err(|e| format!("map: {e}"))?;
        let pareto: ParetoAtlas =
            serde_json::from_str(pareto).map_err(|e| format!("Pareto front: {e}"))?;
        let goodhart = goodhart
            .iter()
            .map(|text| serde_json::from_str(text).map_err(|e| format!("Goodhart run: {e}")))
            .collect::<Result<Vec<GoodhartAtlas>, String>>()?;
        Ok(Self {
            map,
            pareto,
            goodhart,
        })
    }

    /// The design a pick points at, if it exists.
    pub fn picked(&self, pick: AtlasPick) -> Option<&AtlasDesign> {
        match pick {
            AtlasPick::Cell { x, y } => self.map.cell(x, y).map(|c| &c.elite),
            AtlasPick::Front(i) => self.pareto.front.get(i),
        }
    }

    /// The lowest and highest benefit index of the map and the front, for
    /// one colour scale shared by both views.
    pub fn benefit_range(&self) -> [f32; 2] {
        let values = self
            .map
            .cells
            .iter()
            .map(|c| c.elite.metrics.benefit_index)
            .chain(self.pareto.front.iter().map(|d| d.metrics.benefit_index))
            .filter(|v| v.is_finite());
        let (lo, hi) = values.fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(v), hi.max(v))
        });
        if lo.is_finite() && hi > lo {
            // Rounded outwards to 0.05, so the legend has round ends.
            [(lo * 20.0).floor() / 20.0, (hi * 20.0).ceil() / 20.0]
        } else {
            [0.0, 1.0]
        }
    }
}

/// The embedded atlas, parsed on first use; the error says which file
/// failed (only possible if a file was edited by hand).
pub fn atlas() -> Result<&'static AtlasData, &'static str> {
    static DATA: OnceLock<Result<AtlasData, String>> = OnceLock::new();
    DATA.get_or_init(|| AtlasData::parse(MAP_JSON, PARETO_JSON, &GOODHART_JSON))
        .as_ref()
        .map_err(String::as_str)
}

/// The three views of the Atlas tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtlasPanel {
    /// The map of engagement against harm.
    Map,
    /// The Pareto front.
    Pareto,
    /// The Goodhart runs.
    Goodhart,
}

impl AtlasPanel {
    /// Every view in display order.
    pub const ALL: [AtlasPanel; 3] = [AtlasPanel::Map, AtlasPanel::Pareto, AtlasPanel::Goodhart];

    /// Display name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Map => "The map",
            Self::Pareto => "The Pareto front",
            Self::Goodhart => "Goodhart",
        }
    }
}

/// A design picked on the map or the front.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtlasPick {
    /// A filled cell of the map.
    Cell {
        /// Column (engagement).
        x: u32,
        /// Row (harm).
        y: u32,
    },
    /// A design of the front, by position.
    Front(usize),
}

/// The Atlas tab's state.
#[derive(Clone, Debug)]
pub struct AtlasState {
    /// The view shown.
    pub panel: AtlasPanel,
    /// The picked design.
    pub picked: Option<AtlasPick>,
}

impl Default for AtlasState {
    fn default() -> Self {
        Self {
            panel: AtlasPanel::Map,
            picked: None,
        }
    }
}

/// One design lever that differs from the engagement maximiser.
#[derive(Clone, Debug, PartialEq)]
pub struct LeverDifference {
    /// The lever's display name.
    pub name: &'static str,
    /// Its value in the engagement maximiser.
    pub maximiser: String,
    /// Its value in the design.
    pub design: String,
    /// How far apart, from 0 to 1: a share of the lever's range, or 1 for a
    /// different switch or choice.
    pub distance: f32,
}

/// The design levers of `design` that differ from the engagement maximiser
/// (the default design), the most different first, at most `count`. Levers
/// that have no effect in `design` (see [`inert_levers`]) are skipped.
pub fn differences_from_maximiser(design: &DesignLevers, count: usize) -> Vec<LeverDifference> {
    let wrap = |d: &DesignLevers| {
        tree(&Config {
            design: d.clone(),
            ..Config::default()
        })
    };
    let (base, this) = (wrap(&DesignLevers::default()), wrap(design));
    let inert = inert_levers(design);
    let mut diffs: Vec<LeverDifference> = params()
        .iter()
        .filter(|p| p.side == ParamSide::Design && !inert.contains(&p.path))
        .filter_map(|p| {
            let (a, b) = (read(&base, p)?, read(&this, p)?);
            if a == b {
                return None;
            }
            let distance = match (&a, &b, p.kind) {
                (
                    LeverValue::Number(x),
                    LeverValue::Number(y),
                    ParamKind::Range { min, max, .. },
                ) => (y - x).abs() / (max - min).max(1e-9),
                (
                    LeverValue::Integer(x),
                    LeverValue::Integer(y),
                    ParamKind::Integer { min, max },
                )
                | (
                    LeverValue::Optional(Some(x)),
                    LeverValue::Optional(Some(y)),
                    ParamKind::OptionalInteger { min, max },
                ) => x.abs_diff(*y) as f32 / max.saturating_sub(min).max(1) as f32,
                _ => 1.0,
            };
            Some(LeverDifference {
                name: p.name,
                maximiser: a.display(p),
                design: b.display(p),
                distance,
            })
        })
        .collect();
    // A stable sort keeps the catalogue order among equal distances.
    diffs.sort_by(|a, b| b.distance.total_cmp(&a.distance));
    diffs.truncate(count);
    diffs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_atlas_parses_and_is_consistent() {
        let data = atlas().expect("the embedded atlas parses");
        assert!(!data.map.cells.is_empty());
        assert!(!data.pareto.front.is_empty());
        assert_eq!(data.goodhart.len(), 2);
        assert_eq!(data.goodhart[0].proxy, "engagement_index");
        assert_eq!(data.goodhart[1].proxy, "survey_satisfaction");
        for cell in &data.map.cells {
            let m = cell.elite.metrics;
            assert_eq!(
                data.map.grid.cell_of(m.engagement_index, m.harm_index),
                (cell.x, cell.y)
            );
            let config = Config {
                design: cell.elite.design.clone(),
                ..Config::default()
            };
            assert!(config.validate().is_ok());
        }
        let [lo, hi] = data.benefit_range();
        assert!(lo < hi);
        let first = &data.map.cells[0];
        assert!(
            data.picked(AtlasPick::Cell {
                x: first.x,
                y: first.y
            })
            .is_some()
        );
        assert!(data.picked(AtlasPick::Front(0)).is_some());
        assert!(data.picked(AtlasPick::Front(usize::MAX)).is_none());
        assert!(AtlasData::parse("{}", "{}", &[]).is_err());
    }

    #[test]
    fn differences_put_switches_and_large_moves_first() {
        assert!(differences_from_maximiser(&DesignLevers::default(), 5).is_empty());
        let design = DesignLevers {
            w_reply: 1.4,
            chronological: true,
            w_quality: 2.4,
            ..DesignLevers::default()
        };
        let diffs = differences_from_maximiser(&design, 5);
        let names: Vec<&str> = diffs.iter().map(|d| d.name).collect();
        assert_eq!(
            names,
            ["Chronological feed", "Quality weight", "Reply weight"]
        );
        assert_eq!(diffs[0].maximiser, "off");
        assert_eq!(diffs[0].design, "on");
        assert_eq!(differences_from_maximiser(&design, 1).len(), 1);
        let unused = DesignLevers {
            alt_feed: incentive_core::AltFeed::Bridging,
            ..DesignLevers::default()
        };
        assert!(
            differences_from_maximiser(&unused, 5).is_empty(),
            "nobody uses the alternative feed"
        );
    }
}
