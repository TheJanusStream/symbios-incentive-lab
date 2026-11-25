//! The readable reports of `incentive explore`.

use incentive_core::{
    AtlasDesign, AtlasMetrics, DesignLevers, ExploreSettings, GoodhartAtlas, MapAtlas, MetricId,
    ParetoAtlas, metric_infos,
};

use crate::error::CliError;
use crate::report::{fmt_value, table};

/// The settings line every report starts with.
fn settings_line(s: &ExploreSettings, seconds: f32) -> String {
    format!(
        "{} designs simulated ({} runs: {} seeds each, N = {}, {} days), {} generations of {}, \
         search seed {}, world: {}; {:.0} s",
        s.evaluations,
        s.runs,
        s.seeds.len(),
        s.population,
        s.days,
        s.generations,
        s.batch,
        s.seed,
        s.world,
        seconds
    )
}

/// Short column labels of the key metrics.
const COLUMNS: [(&str, MetricId); 9] = [
    ("engage", MetricId::EngagementIndex),
    ("harm", MetricId::HarmIndex),
    ("benefit", MetricId::BenefitIndex),
    ("toxic", MetricId::ToxicShare),
    ("wellbeing", MetricId::WellbeingEverUsers),
    ("creators", MetricId::CreatorsLiving),
    ("polar", MetricId::PolarisationSociety),
    ("users", MetricId::UserShare),
    ("survey", MetricId::SurveySatisfaction),
];

fn metric_cells(m: &AtlasMetrics) -> Vec<String> {
    COLUMNS
        .iter()
        .map(|&(_, id)| fmt_value(id, m.get(id).unwrap_or(f32::NAN)))
        .collect()
}

fn header<'a>(first: &[&'a str]) -> Vec<&'a str> {
    let mut h = first.to_vec();
    h.extend(COLUMNS.iter().map(|(label, _)| *label));
    h.push("levers that differ most from the engagement maximiser");
    h
}

/// The levers of `design` furthest from the default design, as short text,
/// skipping levers that have no effect in `design`.
pub fn lever_summary(design: &DesignLevers, count: usize) -> String {
    let base = serde_json::to_value(DesignLevers::default()).unwrap_or_default();
    let tree = serde_json::to_value(design).unwrap_or_default();
    let inert = incentive_core::inert_levers(design);
    let mut diffs: Vec<(f64, String)> = incentive_core::params()
        .iter()
        .filter(|p| p.side == incentive_core::ParamSide::Design && !inert.contains(&p.path))
        .filter_map(|p| {
            let key = p.path.strip_prefix("design.")?;
            let (a, b) = (base.get(key)?, tree.get(key)?);
            if a == b {
                return None;
            }
            let distance = match (a.as_f64(), b.as_f64(), p.kind) {
                (Some(x), Some(y), incentive_core::ParamKind::Range { min, max, .. }) => {
                    (y - x).abs() / f64::from(max - min).max(1e-9)
                }
                (Some(x), Some(y), incentive_core::ParamKind::Integer { min, max })
                | (Some(x), Some(y), incentive_core::ParamKind::OptionalInteger { min, max }) => {
                    (y - x).abs() / f64::from(max.saturating_sub(min).max(1))
                }
                _ => 1.0,
            };
            let shown = match b {
                serde_json::Value::Number(n) => n
                    .as_f64()
                    .map_or_else(|| n.to_string(), |v| format!("{}", (v * 1e4).round() / 1e4)),
                serde_json::Value::Null => "none".into(),
                other => other.to_string().trim_matches('"').to_string(),
            };
            Some((distance, format!("{key} {shown}")))
        })
        .collect();
    diffs.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    diffs
        .into_iter()
        .take(count)
        .map(|(_, text)| text)
        .collect::<Vec<_>>()
        .join(", ")
}

fn design_row(first: Vec<String>, d: &AtlasDesign) -> Vec<String> {
    let mut row = first;
    row.extend(metric_cells(&d.metrics));
    row.push(lever_summary(&d.design, 4));
    row
}

/// The map as a text grid (benefit in tenths, `.` for an empty cell), then
/// the best benefit in every engagement column.
pub fn print_map(atlas: &MapAtlas, seconds: f32) -> Result<(), CliError> {
    let g = atlas.grid;
    outln!(
        "MAP-Elites: engagement index across ({} to {}), harm index up ({} to {}), {} by {} \
         cells; fitness is the benefit index",
        g.engagement[0],
        g.engagement[1],
        g.harm[0],
        g.harm[1],
        g.resolution,
        g.resolution
    );
    outln!("{}\n", settings_line(&atlas.settings, seconds));
    outln!(
        "{} of {} cells filled ({:.0}%). Each cell shows the benefit index of its elite in \
         hundredths minus 30 (so 5 is 0.35, 25 is 0.55), \".\" an empty cell:\n",
        atlas.cells.len(),
        g.resolution * g.resolution,
        atlas.coverage() * 100.0
    );
    for y in (0..g.resolution).rev() {
        let row = g.row(y);
        let mut line = format!("harm {:>4.2}-{:<4.2} |", row[0], row[1]);
        for x in 0..g.resolution {
            match atlas.cell(x, y) {
                Some(c) => {
                    let v =
                        ((c.elite.metrics.benefit_index * 100.0).round() - 30.0).clamp(0.0, 99.0);
                    line.push_str(&format!("{v:>3.0}"));
                }
                None => line.push_str("  ."),
            }
        }
        outln!("{line}");
    }
    let mut axis = String::from("engagement from      |");
    for x in 0..g.resolution {
        axis.push_str(&format!("{:>3.0}", g.column(x)[0] * 100.0));
    }
    outln!("{axis}  (hundredths)\n");
    outln!("The highest benefit found in each engagement column:\n");
    let mut rows = Vec::new();
    for x in 0..g.resolution {
        let best = atlas.cells.iter().filter(|c| c.x == x).max_by(|a, b| {
            a.elite
                .metrics
                .benefit_index
                .total_cmp(&b.elite.metrics.benefit_index)
        });
        if let Some(c) = best {
            rows.push(design_row(vec![format!("{},{}", c.x, c.y)], &c.elite));
        }
    }
    out!("{}", table(&header(&["cell"]), &rows));
    Ok(())
}

/// The front, by engagement.
pub fn print_pareto(atlas: &ParetoAtlas, seconds: f32) -> Result<(), CliError> {
    outln!(
        "NSGA-II: maximise the engagement index, minimise the harm index, maximise the benefit index"
    );
    outln!("{}\n", settings_line(&atlas.settings, seconds));
    outln!(
        "{} non-dominated designs, by engagement:\n",
        atlas.front.len()
    );
    let rows: Vec<Vec<String>> = atlas
        .front
        .iter()
        .enumerate()
        .map(|(i, d)| design_row(vec![(i + 1).to_string()], d))
        .collect();
    out!("{}", table(&header(&["#"]), &rows));
    Ok(())
}

/// Every generation's best design, then what changed from the first to the
/// last generation.
pub fn print_goodhart(atlas: &GoodhartAtlas, seconds: f32) -> Result<(), CliError> {
    let name = metric_infos()
        .iter()
        .find(|i| i.key == atlas.proxy)
        .map_or(atlas.proxy.as_str(), |i| i.name);
    outln!(
        "Goodhart: maximise {name} ({}) alone and watch the truth",
        atlas.proxy
    );
    outln!("{}\n", settings_line(&atlas.settings, seconds));
    let truth = [
        MetricId::HarmIndex,
        MetricId::BenefitIndex,
        MetricId::WellbeingEverUsers,
        MetricId::ToxicShare,
    ];
    let mut h = vec!["generation", "proxy"];
    h.extend(["harm", "benefit", "wellbeing", "toxic"]);
    h.push("levers that differ most from the engagement maximiser");
    let proxy_id = MetricId::from_key(&atlas.proxy).unwrap_or(MetricId::EngagementIndex);
    let rows: Vec<Vec<String>> = atlas
        .generations
        .iter()
        .map(|g| {
            let mut row = vec![g.generation.to_string(), fmt_value(proxy_id, g.proxy)];
            row.extend(
                truth
                    .iter()
                    .map(|&id| fmt_value(id, g.best.metrics.get(id).unwrap_or(f32::NAN))),
            );
            row.push(lever_summary(&g.best.design, 4));
            row
        })
        .collect();
    out!("{}", table(&h, &rows));
    if let (Some(first), Some(last)) = (atlas.generations.first(), atlas.generations.last()) {
        outln!(
            "\nFrom generation {} to {}: {name} {} -> {}",
            first.generation,
            last.generation,
            fmt_value(proxy_id, first.proxy),
            fmt_value(proxy_id, last.proxy)
        );
        for id in truth {
            let (a, b) = (
                first.best.metrics.get(id).unwrap_or(f32::NAN),
                last.best.metrics.get(id).unwrap_or(f32::NAN),
            );
            outln!(
                "  {:<32} {} -> {}  ({:+.4})",
                id.info().name,
                fmt_value(id, a),
                fmt_value(id, b),
                b - a
            );
        }
        outln!(
            "Harm is better lower; benefit, wellbeing are better higher; toxic impressions better \
             lower. The truth was never part of the search."
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lever_summaries_list_the_largest_changes_first() {
        assert_eq!(lever_summary(&DesignLevers::default(), 4), "");
        let d = DesignLevers {
            w_reply: 0.0,
            chronological: true,
            reshare_depth_cap: Some(2),
            ..DesignLevers::default()
        };
        let text = lever_summary(&d, 2);
        assert!(text.starts_with("chronological true"), "{text}");
        assert!(text.contains("reshare_depth_cap 2"), "{text}");
        assert!(!text.contains("w_reply"), "only two levers: {text}");
    }
}
