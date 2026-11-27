//! Tables on stdout, CSV and JSON files.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use incentive_core::{DailyMetrics, Format, MetricId, Visibility, metric_infos};

use crate::error::CliError;

/// The headline metrics of summary tables: the indices, then the platform's
/// view, then the truth.
pub const HEADLINE: [MetricId; 28] = [
    MetricId::EngagementIndex,
    MetricId::HarmIndex,
    MetricId::HarmIndexExPolarisation,
    MetricId::DiscourseQuality,
    MetricId::BenefitIndex,
    MetricId::BenefitIndexExCreators,
    MetricId::ReportedDailyActives,
    MetricId::Impressions,
    MetricId::TimePerActive,
    MetricId::LikesPerActive,
    MetricId::SharesPerActive,
    MetricId::Revenue,
    MetricId::NetRevenue,
    MetricId::Retention30,
    MetricId::SurveySatisfaction,
    MetricId::HumanDailyActives,
    MetricId::UserShare,
    MetricId::RegrettedShare,
    MetricId::ToxicShare,
    MetricId::OutrageShare,
    MetricId::MisinformationShare,
    MetricId::HarassmentPer1000,
    MetricId::BotEngagementShare,
    MetricId::WellbeingEverUsers,
    MetricId::PolarisationSociety,
    MetricId::InformednessNonUsers,
    MetricId::CreatorsLiving,
    MetricId::PostShareOutrage,
];

/// The metrics a table shows: the requested keys, or the headline.
pub fn table_metrics(keys: &[String]) -> Result<Vec<MetricId>, CliError> {
    if keys.is_empty() {
        return Ok(HEADLINE.to_vec());
    }
    keys.iter()
        .map(|k| MetricId::from_key(k.trim()).ok_or_else(|| CliError::UnknownMetric(k.clone())))
        .collect()
}

/// The mark of a metric that moves within its summary window.
pub const MOVING_MARK: &str = "~";

/// The note under a table that marks moving metrics.
pub const MOVING_NOTE: &str = "~ moving: the metric changed by more than 10% of its mean across the summary \
    window and by more than two standard errors (mean over seeds, its standard error across them, or the \
    regression's own for one seed; \"slope\" and \"slope_se\" in the JSON), so the window averages a series \
    that is still changing";

/// [`MOVING_MARK`] when `moving`, else nothing.
pub fn moving_mark(moving: bool) -> &'static str {
    if moving { MOVING_MARK } else { "" }
}

/// "index", "observable" or "truth": the column label of a metric's view.
pub fn view_label(id: MetricId) -> &'static str {
    let info = id.info();
    match (info.family, info.visibility) {
        (incentive_core::Family::Index, _) => "index",
        (_, Visibility::Observable) => "observable",
        (_, Visibility::Truth) => "truth",
    }
}

/// A value formatted for a table.
pub fn fmt_value(id: MetricId, v: f32) -> String {
    if v.is_nan() {
        return "-".into();
    }
    match id.info().format {
        Format::Count if v.abs() >= 100.0 => format!("{v:.0}"),
        Format::Count | Format::Decimal | Format::Money => format!("{v:.3}"),
        Format::Share | Format::Score => format!("{v:.4}"),
    }
}

/// Renders rows as an aligned text table.
pub fn table(header: &[&str], rows: &[Vec<String>]) -> String {
    let mut widths: Vec<usize> = header.iter().map(|h| h.len()).collect();
    for row in rows {
        for (w, cell) in widths.iter_mut().zip(row) {
            *w = (*w).max(cell.len());
        }
    }
    let mut out = String::new();
    let line = |cells: Vec<&str>, out: &mut String| {
        let parts: Vec<String> = cells
            .iter()
            .zip(&widths)
            .enumerate()
            .map(|(i, (c, w))| {
                if i == 0 {
                    format!("{c:<w$}")
                } else {
                    format!("{c:>w$}")
                }
            })
            .collect();
        let _ = writeln!(out, "{}", parts.join("  ").trim_end());
    };
    line(header.to_vec(), &mut out);
    line(
        widths
            .iter()
            .map(|w| "-".repeat(*w))
            .collect::<Vec<_>>()
            .iter()
            .map(String::as_str)
            .collect(),
        &mut out,
    );
    for row in rows {
        line(row.iter().map(String::as_str).collect(), &mut out);
    }
    out
}

/// Writes text to standard output.
pub fn emit(text: &str) -> Result<(), CliError> {
    use std::io::Write as _;
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(text.as_bytes())
        .and_then(|()| stdout.flush())
        .map_err(CliError::Stdout)
}

/// Creates a directory and its parents.
pub fn ensure_dir(dir: &Path) -> Result<(), CliError> {
    fs::create_dir_all(dir).map_err(|source| CliError::Io {
        path: dir.to_path_buf(),
        source,
    })
}

/// Removes the daily CSVs of an earlier run, so a folder never mixes runs.
pub fn remove_daily_csvs(dir: &Path) -> Result<(), CliError> {
    let io = |source| CliError::Io {
        path: dir.to_path_buf(),
        source,
    };
    for entry in fs::read_dir(dir).map_err(io)? {
        let path = entry.map_err(io)?.path();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if name.starts_with("daily-seed") && name.ends_with(".csv") {
            fs::remove_file(&path).map_err(|source| CliError::Io {
                path: path.clone(),
                source,
            })?;
        }
    }
    Ok(())
}

/// Writes a text file.
pub fn write_file(path: &Path, text: &str) -> Result<(), CliError> {
    fs::write(path, text).map_err(|source| CliError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Writes a value as pretty JSON.
pub fn write_json(path: &Path, value: &impl serde::Serialize) -> Result<(), CliError> {
    let text = serde_json::to_string_pretty(value).map_err(|e| CliError::Json(e.to_string()))?;
    write_file(path, &text)
}

/// The daily metrics of one run as CSV: a `day` column and one per metric;
/// missing values are empty cells.
pub fn daily_csv(history: &[DailyMetrics]) -> String {
    let mut out = String::from("day");
    for info in metric_infos() {
        out.push(',');
        out.push_str(info.key);
    }
    out.push('\n');
    for day in history {
        let _ = write!(out, "{}", day.day);
        for info in metric_infos() {
            out.push(',');
            if let Some(v) = day.get_opt(info.id) {
                let _ = write!(out, "{v}");
            }
        }
        out.push('\n');
    }
    out
}

/// The repository's root directory.
fn repository_root() -> &'static Path {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    crate_dir.ancestors().nth(2).unwrap_or(crate_dir)
}

/// The default output directory: `exports/<name>` in the repository.
pub fn default_out(name: &str) -> PathBuf {
    repository_root().join("exports").join(name)
}

/// An output directory as an `--out` argument for a "Generated by" line:
/// relative to the repository root when it lies inside it, as given
/// otherwise.
pub fn out_arg(dir: &Path) -> String {
    dir.strip_prefix(repository_root())
        .unwrap_or(dir)
        .display()
        .to_string()
}

/// A file-name-safe version of a label.
pub fn slug(label: &str) -> String {
    label
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}
