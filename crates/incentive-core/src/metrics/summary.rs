//! Run summaries: every metric averaged over the summary window.

use serde::Serialize;
use serde::ser::{SerializeMap, Serializer};

use super::{
    DailyMetrics, METRIC_COUNT, MetricId, nan_mean, serialize_breakdowns, serialize_values,
};
use crate::content::STYLE_COUNT;
use crate::population::ARCHETYPE_COUNT;

/// A run summary: every metric averaged over the summary window
/// (docs/model.md, section 9), skipping days without a value.
#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    /// First day of the window.
    pub first_day: u32,
    /// Days averaged.
    pub days: u32,
    pub(crate) values: Vec<f32>,
    /// Mean wellbeing of users by archetype over the window.
    pub wellbeing_by_archetype: [f32; ARCHETYPE_COUNT],
    /// Payout share by style over the window.
    pub payout_share_by_style: [f32; STYLE_COUNT],
}

impl Summary {
    /// Averages the last `window` days of `history`.
    pub fn from_history(history: &[DailyMetrics], window: usize) -> Self {
        let start = history.len().saturating_sub(window.max(1));
        let days = &history[start..];
        let values = (0..METRIC_COUNT)
            .map(|m| nan_mean(days.iter().map(|d| d.values[m])))
            .collect();
        let wellbeing_by_archetype =
            std::array::from_fn(|a| nan_mean(days.iter().map(|d| d.wellbeing_by_archetype[a])));
        let payout_share_by_style =
            std::array::from_fn(|s| nan_mean(days.iter().map(|d| d.payout_share_by_style[s])));
        Self {
            first_day: days.first().map_or(0, |d| d.day),
            days: days.len() as u32,
            values,
            wellbeing_by_archetype,
            payout_share_by_style,
        }
    }

    /// The mean of several summaries (of seeds, for example), skipping NaN.
    pub fn mean(summaries: &[Summary]) -> Option<Summary> {
        let first = summaries.first()?;
        let values = (0..METRIC_COUNT)
            .map(|m| nan_mean(summaries.iter().map(|s| s.values[m])))
            .collect();
        Some(Summary {
            first_day: first.first_day,
            days: first.days,
            values,
            wellbeing_by_archetype: std::array::from_fn(|a| {
                nan_mean(summaries.iter().map(|s| s.wellbeing_by_archetype[a]))
            }),
            payout_share_by_style: std::array::from_fn(|i| {
                nan_mean(summaries.iter().map(|s| s.payout_share_by_style[i]))
            }),
        })
    }

    /// The averaged value of a metric; NaN when no day had one.
    pub fn get(&self, id: MetricId) -> f32 {
        self.values[id.index()]
    }

    /// The averaged value of a metric, `None` when no day had one.
    pub fn get_opt(&self, id: MetricId) -> Option<f32> {
        let v = self.get(id);
        (!v.is_nan()).then_some(v)
    }
}

impl Serialize for Summary {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(METRIC_COUNT + 4))?;
        map.serialize_entry("first_day", &self.first_day)?;
        map.serialize_entry("days", &self.days)?;
        serialize_values(&mut map, &self.values)?;
        let zeros = [0.0; STYLE_COUNT];
        serialize_breakdowns(
            &mut map,
            &self.wellbeing_by_archetype,
            &self.payout_share_by_style,
            &zeros,
        )?;
        map.end()
    }
}
