//! `validate`: the out-of-sample validation battery (lab #31), V1 to V10, at
//! full size. Every check is judged under the integrity preset, as the
//! stylized facts are judged; the same check under the engagement ranking is
//! reported beside it as a sensitivity, never judged.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use incentive_core::{
    CheckResult, Config, DailyMetrics, DoseResult, Fact, FactRun, RawChange, Validation,
    validations,
};
use serde_json::json;

use crate::error::CliError;
use crate::facts_cmd::{RATIO_CONVENTION, interval_text, scale_text, tally, verdict};
use crate::report::{self, table};
use crate::runs;

/// One design's histories of one check, indexed `[variant][seed]`.
type Histories = Vec<Vec<Vec<DailyMetrics>>>;

/// Runs the selected checks under both designs on paired seeds, identical
/// runs once, and prints and writes the table. `set` changes the
/// configuration every arm starts from, before the arm's own change: a world
/// in which to check the findings. `only` limits the run to these ids;
/// empty means every check.
pub fn run_validate(
    seeds: u64,
    population: u32,
    days: u32,
    set: &[String],
    only: &[String],
    out: Option<PathBuf>,
) -> Result<(), CliError> {
    let selected = select(only)?;
    let run = FactRun {
        seeds: (1..=seeds.max(1)).collect(),
        population,
        days,
    };
    let mut base = Config::default();
    for assignment in set {
        base.set_assignment(assignment)?;
    }
    base.validate()?;
    let with = if set.is_empty() {
        String::new()
    } else {
        format!(", with {}", set.join(", "))
    };
    outln!(
        "Out-of-sample validation: {} paired seeds, N = {population}, {days} days{with}; judged under the \
         integrity preset, the engagement ranking beside it as a sensitivity\n",
        run.seeds.len()
    );
    // Every arm of both designs of every selected check, in one batch.
    let mut jobs: Vec<(Config, u64)> = Vec::new();
    let mut starts = Vec::new();
    for v in &selected {
        let mut pair = Vec::new();
        for fact in [v.fact, v.engagement] {
            pair.push(jobs.len());
            for &s in &run.seeds {
                for c in fact.configs_from(&base, &run, s) {
                    c.validate()?;
                    jobs.push((c, s));
                }
            }
        }
        starts.push(pair);
    }
    let (outputs, index) = runs::run_unique(&jobs)?;
    let histories = |start: usize, fact: &Fact| -> Histories {
        let k = fact.variants.len();
        (0..k)
            .map(|var| {
                (0..run.seeds.len())
                    .map(|s| outputs[index[start + s * k + var]].history.clone())
                    .collect()
            })
            .collect()
    };

    let mut rows = Vec::new();
    let mut md_rows = String::new();
    let mut doc = Vec::new();
    let mut judged = Vec::new();
    let mut doses = Vec::new();
    let mut other_lines = Vec::new();
    for (v, pair) in selected.iter().zip(&starts) {
        let hi = histories(pair[0], v.fact);
        let he = histories(pair[1], v.engagement);
        let results = v.fact.evaluate(&hi);
        let er = v.engagement.evaluate(&he);
        let raws = v.raw_changes(&hi);
        let er_raws = v.raw_changes(&he);
        let others = v.others(&hi);
        let er_others = v.others(&he);
        for (o, e) in others.iter().zip(&er_others) {
            other_lines.push(other_line(v.id, o, e));
        }
        let mut checks = Vec::new();
        for (((r, e), raw), er_raw) in results.iter().zip(&er).zip(&raws).zip(&er_raws) {
            rows.push(vec![
                v.id.to_string(),
                r.label.to_string(),
                format!("{:.4}", r.effect),
                format!("{:.4}", r.spread),
                interval_text(r, 4),
                r.bound.text(),
                verdict(r.verdict, r.known_limit),
                raw_text(raw),
                scale_text(r),
                format!("{:.4}", e.effect),
            ]);
            let _ = writeln!(
                md_rows,
                "| {} | {} | {:.4} | {:.4} | {} | {} | {} | {} | {} | {:.4} ({}) |",
                v.id,
                r.label,
                r.effect,
                r.spread,
                interval_text(r, 4),
                r.bound.text(),
                verdict(r.verdict, r.known_limit),
                raw_text(raw),
                scale_text(r),
                e.effect,
                interval_text(e, 4)
            );
            checks.push(json!({
                "label": r.label,
                "integrity": r,
                "raw": raw,
                "engagement_ranking": e,
                "engagement_ranking_raw": er_raw,
            }));
        }
        let dose = v.dose.map(|d| {
            let (achieved, er_dose) = (d.evaluate(&hi), d.evaluate(&he));
            doses.push((
                v.id, d.lever, d.value, d.target, d.achieved, achieved, er_dose,
            ));
            json!({
                "label": d.label,
                "lever": d.lever,
                "value": d.value,
                "target": d.target,
                "calibrated": d.achieved,
                "integrity": achieved,
                "engagement_ranking": er_dose,
            })
        });
        judged.extend(results.iter().cloned());
        let others: Vec<_> = others
            .iter()
            .zip(&er_others)
            .map(|(o, e)| json!({"label": o.label, "integrity": o, "engagement_ranking": e}))
            .collect();
        doc.push(json!({
            "id": v.id,
            "title": v.fact.title,
            "reference": v.fact.reference,
            "source": v.source,
            "design": v.design,
            "dose": dose,
            "checks": checks,
            "other_measures": others,
        }));
    }
    out!(
        "{}",
        table(
            &[
                "id",
                "check",
                "effect",
                "sd",
                "95% interval",
                "band",
                "verdict",
                "raw change",
                "SD used",
                "eng. ranking"
            ],
            &rows
        )
    );
    let closing = closing_text(&judged);
    outln!("\n{closing}");
    let dose_lines = dose_lines(&doses);
    for line in &dose_lines {
        outln!("{line}");
    }
    for line in &other_lines {
        outln!("{line}");
    }

    let dir = out.unwrap_or_else(|| report::default_out("validation"));
    report::ensure_dir(&dir)?;
    report::write_json(
        &dir.join("validation.json"),
        &json!({
            "run": {"seeds": run.seeds, "population": population, "days": days, "set": set, "only": only},
            "design": "integrity preset, judged; engagement ranking, a sensitivity",
            "ratio_convention": RATIO_CONVENTION,
            "checks": doc,
        }),
    )?;
    report::write_file(
        &dir.join("validation.md"),
        &markdown(
            &run,
            set,
            only,
            &dir,
            &selected,
            &md_rows,
            &closing,
            &dose_lines,
            &other_lines,
        ),
    )?;
    outln!("wrote {}", dir.display());
    Ok(())
}

/// The checks with the given ids, in battery order; every check when none
/// is given.
fn select(only: &[String]) -> Result<Vec<&'static Validation>, CliError> {
    if let Some(unknown) = only.iter().find(|id| {
        !validations()
            .iter()
            .any(|v| v.id.eq_ignore_ascii_case(id.trim()))
    }) {
        let ids: Vec<&str> = validations().iter().map(|v| v.id).collect();
        return Err(CliError::Explore(format!(
            "unknown check {unknown:?}; the checks are {}",
            ids.join(", ")
        )));
    }
    Ok(validations()
        .iter()
        .filter(|v| only.is_empty() || only.iter().any(|id| v.id.eq_ignore_ascii_case(id.trim())))
        .collect())
}

/// A raw change as text: its value with its standard deviation across
/// seeds.
fn raw_text(raw: &RawChange) -> String {
    format!("{:.4} (sd {:.4})", raw.effect, raw.spread)
}

/// The closing line: the facts' tally, with the verdicts' meaning and the
/// ratio convention.
fn closing_text(judged: &[CheckResult]) -> String {
    format!(
        "{} The verdict judges the 95% interval of the per-seed effect under the integrity preset: PASS \
         inside the band, FAIL wholly outside, UNDECIDED across it. Known limits are reported, not required. \
         The engagement ranking's effect is a sensitivity, not judged. {RATIO_CONVENTION} \
         \"SD used\" is the standard deviation an effect in SDs divides by: the participants' spread at \
         the experiment's start, the mean over the seeds.",
        tally(judged)
    )
}

/// One line per other measure of a check's quantity: reported, not judged.
fn other_line(id: &str, o: &RawChange, e: &RawChange) -> String {
    format!(
        "Other measure of {id} (not judged): {}: {:.4} (95% interval of the per-seed values {:.4} to {:.4}) \
         under integrity, {:.4} under the engagement ranking.",
        o.label, o.effect, o.interval.low, o.interval.high, e.effect
    )
}

/// One line per dose: the lever, the target, the dose calibrated and the
/// dose in these runs under both designs.
fn dose_lines(doses: &[(&str, &str, f32, f32, f32, DoseResult, DoseResult)]) -> Vec<String> {
    doses
        .iter()
        .map(|(id, lever, value, target, calibrated, achieved, er)| {
            format!(
                "Dose of {id}: {lever} {value}; target {target}, calibrated {calibrated}; here {:.4} (95% \
                 interval {:.4} to {:.4}) under integrity, {:.4} under the engagement ranking.",
                achieved.effect, achieved.interval.low, achieved.interval.high, er.effect
            )
        })
        .collect()
}

/// The Markdown report: the table, the doses and each check's source.
#[allow(clippy::too_many_arguments)]
fn markdown(
    run: &FactRun,
    set: &[String],
    only: &[String],
    dir: &Path,
    selected: &[&Validation],
    rows: &str,
    closing: &str,
    dose_lines: &[String],
    other_lines: &[String],
) -> String {
    let mut md = String::from("# Out-of-sample validation\n\n");
    let intro = format!(
        "Generated by `incentive validate --seeds {}{}{}{} --out {}`: {} paired seeds, `N = {}`, {} days. Every check \
         runs under the integrity preset and is judged there; the engagement ranking's effect and 95% \
         interval are reported beside it as a sensitivity. The raw change is the unscaled quantity behind \
         the effect, with its standard deviation across seeds.",
        run.seeds.len(),
        if run.population == 2_000 && run.days == 365 {
            String::new()
        } else {
            format!(" --population {} --days {}", run.population, run.days)
        },
        set.iter()
            .map(|s| format!(" --set {s}"))
            .collect::<String>(),
        if only.is_empty() {
            String::new()
        } else {
            format!(" --check {}", only.join(","))
        },
        report::out_arg(dir),
        run.seeds.len(),
        run.population,
        run.days
    );
    let _ = writeln!(md, "{}\n", wrap(intro.trim_end(), "", ""));
    md.push_str(
        "| id | check | effect | sd | 95% interval | band | verdict | raw change | SD used | engagement ranking |\n\
         | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n",
    );
    md.push_str(rows);
    let _ = writeln!(md, "\n{}\n", wrap(closing, "", ""));
    if !dose_lines.is_empty() {
        md.push_str("## Doses\n\n");
        for line in dose_lines {
            let _ = writeln!(md, "{}", wrap(line, "- ", "  "));
        }
        md.push('\n');
    }
    if !other_lines.is_empty() {
        md.push_str("## Other measures, not judged\n\n");
        for line in other_lines {
            let _ = writeln!(md, "{}", wrap(line, "- ", "  "));
        }
        md.push('\n');
    }
    md.push_str("## Sources and designs\n\n");
    for v in selected {
        let text = format!(
            "**{} {}** ({}). {} Design: {}",
            v.id, v.fact.title, v.fact.reference, v.source, v.design
        );
        let _ = writeln!(md, "{}", wrap(&text, "- ", "  "));
    }
    md
}

/// Words wrapped to lines of at most 120 characters, the first line after
/// `first`, the others after `rest`. A line never starts with `#` or with
/// a number followed by `.` or `)`, which Markdown could read as a heading
/// or a list item: such a word takes the previous word with it.
fn wrap(text: &str, first: &str, rest: &str) -> String {
    const WIDTH: usize = 120;
    let risky = |w: &str| {
        let digits = w.chars().take_while(char::is_ascii_digit).count();
        matches!(w, "-" | "+" | "*")
            || w.starts_with('#')
            || (digits > 0 && matches!(w[digits..].chars().next(), Some('.' | ')')))
    };
    let mut lines: Vec<Vec<&str>> = vec![Vec::new()];
    for word in text.split_whitespace() {
        let indent = if lines.len() == 1 { first } else { rest };
        let line = lines.last_mut().expect("one line at least");
        let len = indent.chars().count()
            + line.iter().map(|w| w.chars().count() + 1).sum::<usize>()
            + word.chars().count();
        if line.is_empty() || len <= WIDTH {
            line.push(word);
            continue;
        }
        let mut next = vec![word];
        if risky(word)
            && line.len() > 1
            && let Some(prev) = line.pop()
        {
            next.insert(0, prev);
        }
        lines.push(next);
    }
    lines
        .iter()
        .enumerate()
        .map(|(i, words)| format!("{}{}", if i == 0 { first } else { rest }, words.join(" ")))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prose_wraps_safely() {
        let text = "word ".repeat(30) + "53.7% to 36.2% and # not a heading " + &"x ".repeat(60);
        let wrapped = wrap(&text, "- ", "  ");
        for line in wrapped.lines() {
            assert!(line.chars().count() <= 120, "{line}");
            let body = line.trim_start_matches("- ").trim_start();
            assert!(!body.starts_with('#') && !body.starts_with("53."), "{line}");
        }
        assert_eq!(
            wrapped.split_whitespace().filter(|w| *w != "-").count(),
            text.split_whitespace().count()
        );
    }

    /// The SD an effect in SDs divides by is printed beside it, `-` for
    /// other effects, and the closing text states the ratio convention.
    #[test]
    fn the_sd_used_and_the_ratio_convention_are_printed() {
        use incentive_core::{Bound, MeanInterval, Verdict};
        let result = |scale| CheckResult {
            label: "test",
            effect: 0.1,
            spread: 0.01,
            threshold: 0.0,
            bound: Bound::Between(0.0, 1.0),
            interval: MeanInterval::of([0.09f32, 0.1, 0.11].into_iter()),
            passed: true,
            verdict: Verdict::Pass,
            known_limit: false,
            scale,
        };
        assert_eq!(scale_text(&result(Some(0.08123))), "0.0812");
        assert_eq!(scale_text(&result(None)), "-");
        let closing = closing_text(&[result(Some(0.08))]);
        assert!(closing.contains(RATIO_CONVENTION));
        assert!(closing.contains("ratio of seed means"));
        assert!(closing.contains("SD used"));
        let json = serde_json::to_value(result(Some(0.08))).unwrap_or_default();
        assert!(json.get("scale").is_some());
        let json = serde_json::to_value(result(None)).unwrap_or_default();
        assert!(json.get("scale").is_none());
    }

    #[test]
    fn checks_select_by_id() {
        let all = select(&[]).unwrap_or_default();
        assert_eq!(all.len(), 10);
        let some = select(&["v3".into(), "V10".into()]).unwrap_or_default();
        assert_eq!(some.iter().map(|v| v.id).collect::<Vec<_>>(), ["V3", "V10"]);
        assert!(select(&["V11".into()]).is_err());
    }
}
