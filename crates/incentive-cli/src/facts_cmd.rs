//! `facts`: the stylized-fact checks of docs/model.md, section 15, at full size.

use std::path::PathBuf;

use incentive_core::{CheckResult, Config, Fact, FactRun, Verdict, facts};
use serde_json::json;

use crate::error::CliError;
use crate::report::{self, table};
use crate::runs;

/// Runs every fact's variants on paired seeds and prints the effect sizes,
/// each with the 95% interval of its per-seed effect and the verdict on it.
/// `set` changes the configuration every arm starts from, before the arm's
/// own change: a world in which to check the facts. `only` limits the run to
/// the facts with these ids; empty means every fact.
pub fn run_facts(
    seeds: u64,
    population: u32,
    days: u32,
    set: &[String],
    only: &[String],
    out: Option<PathBuf>,
) -> Result<(), CliError> {
    let selected = select_facts(only)?;
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
    outln!(
        "Stylized facts: {} paired seeds, N = {population}, {days} days{}\n",
        run.seeds.len(),
        if set.is_empty() {
            String::new()
        } else {
            format!(", with {}", set.join(", "))
        }
    );
    let mut rows = Vec::new();
    let mut doc = Vec::new();
    let mut all_results = Vec::new();
    for fact in selected {
        let jobs: Vec<(Config, u64)> = run
            .seeds
            .iter()
            .flat_map(|&s| {
                fact.configs_from(&base, &run, s)
                    .into_iter()
                    .map(move |c| (c, s))
            })
            .collect();
        let outputs = runs::run_many(&jobs)?;
        let k = fact.variants.len();
        let histories: Vec<Vec<_>> = (0..k)
            .map(|v| {
                outputs
                    .iter()
                    .skip(v)
                    .step_by(k)
                    .map(|o| o.history.clone())
                    .collect()
            })
            .collect();
        let results = fact.evaluate(&histories);
        for r in &results {
            rows.push(vec![
                fact.id.to_string(),
                r.label.to_string(),
                format!("{:.4}", r.effect),
                format!("{:.4}", r.spread),
                interval_text(r, 4),
                r.bound.text(),
                means_text(r.passed).to_string(),
                verdict(r.verdict, r.known_limit),
                scale_text(r),
            ]);
        }
        all_results.extend(results.iter().cloned());
        doc.push(json!({"id": fact.id, "title": fact.title, "reference": fact.reference, "checks": results}));
    }
    out!(
        "{}",
        table(
            &[
                "fact",
                "check",
                "effect",
                "sd",
                "95% interval",
                "bound",
                "means",
                "verdict",
                "SD used"
            ],
            &rows
        )
    );
    outln!(
        "\n{}\n\nThe verdict judges the 95% interval of the per-seed effect: PASS inside the bound, FAIL \
         wholly outside, UNDECIDED across it; \"means\" judges the seed means alone. Known limits are \
         reported, not required (docs/model.md, section 16). {RATIO_CONVENTION} \"SD used\" is the \
         standard deviation an effect in SDs divides by: the participants' spread at the experiment's \
         start, the mean over the seeds.",
        tally(&all_results)
    );
    let dir = out.unwrap_or_else(|| report::default_out("facts"));
    report::ensure_dir(&dir)?;
    report::write_json(
        &dir.join("facts.json"),
        &json!({"run": {"seeds": run.seeds, "population": population, "days": days, "set": set, "only": only}, "facts": doc}),
    )?;
    outln!("wrote {}", dir.display());
    Ok(())
}

/// The facts with the given ids, in catalogue order; every fact when none
/// is given.
fn select_facts(only: &[String]) -> Result<Vec<&'static Fact>, CliError> {
    if let Some(unknown) = only
        .iter()
        .find(|id| !facts().iter().any(|f| f.id == id.trim()))
    {
        return Err(CliError::UnknownFact(unknown.clone()));
    }
    Ok(facts()
        .iter()
        .filter(|f| only.is_empty() || only.iter().any(|id| id.trim() == f.id))
        .collect())
}

/// A check's interval verdict as text: `PASS`, `FAIL` or `UNDECIDED`, with a
/// mark for a check the model is known to fail (docs/model.md, section 16).
pub fn verdict(verdict: Verdict, known_limit: bool) -> String {
    if known_limit {
        format!("{} (known limit)", verdict.label())
    } else {
        verdict.label().to_string()
    }
}

/// How every check computes a ratio effect, printed with the results.
pub const RATIO_CONVENTION: &str = "Ratio convention: every ratio, relative change and elasticity \
    effect is a ratio of seed means (the mean of one arm over the mean of the other, on paired seeds); its \
    95% interval and its verdict are of the per-seed values, so the interval's centre, the mean of the \
    per-seed ratios, can differ from the effect.";

/// The standard deviation an effect in SDs was scaled by, as text, or `-`
/// for every other effect.
pub fn scale_text(r: &CheckResult) -> String {
    r.scale.map_or_else(|| "-".into(), |sd| format!("{sd:.4}"))
}

/// Whether the seed means lie within the bound, as text.
pub fn means_text(passed: bool) -> &'static str {
    if passed { "inside" } else { "outside" }
}

/// The 95% interval of a check's per-seed effect, `low to high` with
/// `digits` decimals, or `-` without one.
pub fn interval_text(r: &CheckResult, digits: usize) -> String {
    let i = r.interval;
    if i.low.is_nan() || i.high.is_nan() {
        "-".into()
    } else {
        format!("{:.digits$} to {:.digits$}", i.low, i.high)
    }
}

/// The closing line: how the required checks fare on their intervals and on
/// the seed means, and how many known limits fail.
pub fn tally(results: &[CheckResult]) -> String {
    let required: Vec<&CheckResult> = results.iter().filter(|r| !r.known_limit).collect();
    let count = |v: Verdict| required.iter().filter(|r| r.verdict == v).count();
    let (pass, fail, undecided) = (
        count(Verdict::Pass),
        count(Verdict::Fail),
        count(Verdict::Undecided),
    );
    let on_means = required.iter().filter(|r| r.passed).count();
    let limit_count = |v: Verdict| {
        results
            .iter()
            .filter(|r| r.known_limit && r.verdict == v)
            .count()
    };
    let mut limits_text = match limit_count(Verdict::Fail) {
        1 => "1 known limit fails".to_string(),
        n => format!("{n} known limits fail"),
    };
    for (v, word) in [(Verdict::Undecided, "undecided"), (Verdict::Pass, "pass")] {
        let n = limit_count(v);
        if n > 0 {
            limits_text.push_str(&format!(", {n} {word}"));
        }
    }
    if pass == required.len() {
        format!(
            "All {} required checks hold; {limits_text}.",
            required.len()
        )
    } else {
        format!(
            "Required checks: {pass} of {} hold, {undecided} undecided, {fail} fail on their 95% intervals \
             ({on_means} hold on the seed means); {limits_text}.",
            required.len()
        )
    }
}
