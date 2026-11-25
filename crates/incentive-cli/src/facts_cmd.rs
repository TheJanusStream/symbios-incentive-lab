//! `facts`: the stylized-fact checks of docs/model.md, section 15, at full size.

use std::path::PathBuf;

use incentive_core::{Config, FactRun, facts};
use serde_json::json;

use crate::error::CliError;
use crate::report::{self, table};
use crate::runs;

/// Runs every fact's variants on paired seeds and prints the effect sizes.
pub fn run_facts(
    seeds: u64,
    population: u32,
    days: u32,
    out: Option<PathBuf>,
) -> Result<(), CliError> {
    let run = FactRun {
        seeds: (1..=seeds.max(1)).collect(),
        population,
        days,
    };
    outln!(
        "Stylized facts: {} paired seeds, N = {population}, {days} days\n",
        run.seeds.len()
    );
    let mut rows = Vec::new();
    let mut doc = Vec::new();
    let mut all = true;
    for fact in facts() {
        let jobs: Vec<(Config, u64)> = run
            .seeds
            .iter()
            .flat_map(|&s| fact.configs(&run, s).into_iter().map(move |c| (c, s)))
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
            all &= r.passed;
            rows.push(vec![
                fact.id.to_string(),
                r.label.to_string(),
                format!("{:.4}", r.effect),
                format!("{:.4}", r.spread),
                format!("{:.4}", r.threshold),
                if r.passed { "PASS" } else { "FAIL" }.to_string(),
            ]);
        }
        doc.push(json!({"id": fact.id, "title": fact.title, "reference": fact.reference, "checks": results}));
    }
    out!(
        "{}",
        table(&["fact", "check", "effect", "sd", "threshold", ""], &rows)
    );
    outln!(
        "\n{}",
        if all {
            "All facts hold."
        } else {
            "Some facts do not hold."
        }
    );
    let dir = out.unwrap_or_else(|| report::default_out("facts"));
    report::ensure_dir(&dir)?;
    report::write_json(
        &dir.join("facts.json"),
        &json!({"run": {"seeds": run.seeds, "population": population, "days": days}, "facts": doc}),
    )?;
    outln!("wrote {}", dir.display());
    Ok(())
}
