//! The scenarios: challenges with goals, each played as the player's design
//! against a fixed baseline on the same seed and world.

use egui::{Id, RichText, Ui};
use incentive_core::{Comparison, GoalResult, Scenario, Summary, scenarios};

use super::charts::{self, Chart, Reference, Series};
use super::{levers, palette, visibility_tag};
use crate::lab::series::{fmt_metric, metric_points};
use crate::lab::{CONFIRM_SEEDS, Lab, ScenarioPlay};

/// Draws the scenarios tab.
pub fn show(ui: &mut Ui, lab: &mut Lab) {
    if lab.scenarios.play.is_some() {
        egui::Panel::left("scenario-levers")
            .resizable(true)
            .default_size(330.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("scenario-levers-scroll")
                    .show(ui, |ui| levers::scenario_panel(ui, lab));
            });
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("scenario-centre")
                .show(ui, |ui| playing(ui, lab));
        });
    } else {
        egui::Panel::left("scenario-list")
            .resizable(true)
            .default_size(280.0)
            .show(ui, |ui| scenario_list(ui, lab));
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("scenario-brief")
                .show(ui, |ui| briefing(ui, lab));
        });
    }
}

fn scenario_list(ui: &mut Ui, lab: &mut Lab) {
    ui.heading("Scenarios");
    ui.label(
        RichText::new(
            "Each scenario is a platform in trouble and goals to meet. Your design runs against \
             a baseline that keeps the starting design, on the same seed and world.",
        )
        .small(),
    );
    ui.add_space(4.0);
    for (i, sc) in scenarios().iter().enumerate() {
        if ui
            .selectable_label(
                lab.scenarios.selected == i,
                RichText::new(sc.title).strong(),
            )
            .on_hover_text(sc.briefing)
            .clicked()
        {
            lab.scenarios.selected = i;
        }
    }
}

fn days_of(sc: &Scenario) -> u32 {
    sc.config().run.days
}

fn comparison_text(c: Comparison) -> String {
    match c {
        Comparison::AtLeastTimesBaseline(f) => {
            format!("at least {:.0}% of the baseline", f * 100.0)
        }
        Comparison::AtMostTimesBaseline(f) => format!("at most {:.0}% of the baseline", f * 100.0),
        Comparison::MoreThanBaseline => "more than the baseline".into(),
        Comparison::LessThanBaseline => "less than the baseline".into(),
        Comparison::AtLeastBaselinePlus(d) => format!("at least the baseline plus {d}"),
        Comparison::Below(v) => format!("below {v}"),
    }
}

fn briefing(ui: &mut Ui, lab: &mut Lab) {
    let index = lab.scenarios.selected.min(scenarios().len() - 1);
    let sc = &scenarios()[index];
    ui.heading(sc.title);
    ui.label(sc.briefing);
    ui.add_space(4.0);
    ui.label(RichText::new("Goals").strong());
    for g in sc.goals {
        ui.horizontal_wrapped(|ui| {
            ui.label("-");
            ui.label(g.description);
            visibility_tag(ui, g.metric.info().visibility);
            ui.label(
                RichText::new(format!(
                    "({}: {})",
                    g.metric.info().name,
                    comparison_text(g.comparison)
                ))
                .small()
                .weak(),
            );
        });
    }
    ui.add_space(4.0);
    ui.label(format!(
        "{} days, {} people, seed {}. Goals are judged on the mean of the last 30% of days. The \
         world assumptions are the scenario's and stay locked.",
        days_of(sc),
        lab.run.population,
        lab.run.seed
    ));
    ui.add_space(6.0);
    if ui
        .button(RichText::new("Play").strong().size(15.0))
        .on_hover_text("Start your design and the baseline from day 0")
        .clicked()
    {
        lab.start_scenario(index);
        lab.playing = true;
    }
}

fn playing(ui: &mut Ui, lab: &mut Lab) {
    let Some(play) = lab.scenarios.play.as_ref() else {
        return;
    };
    let sc = play.scenario();
    ui.horizontal(|ui| {
        ui.heading(sc.title);
        if ui.button("Back to the list").clicked() {
            lab.stop_scenario();
        }
    });
    let Some(play) = lab.scenarios.play.as_ref() else {
        return;
    };
    ui.label(sc.briefing);
    ui.add_space(4.0);
    let finished = play.results.is_some();
    let results = play.live_results();
    ui.label(
        RichText::new(if finished {
            "The verdict on this seed".to_string()
        } else {
            format!(
                "So far (day {} of {}): judged on the last {} days run, as the verdict at the \
                 end will be",
                play.runner.day(),
                play.runner.days(),
                (play.runner.day() as usize).min(play.runner.lane(0).sim.summary_days())
            )
        })
        .strong(),
    );
    goal_table(ui, "scenario-goals", &results);
    if finished {
        let passed = results.iter().all(|r| r.passed);
        let (text, color) = if passed {
            ("Every goal met on this seed.", palette::PASS)
        } else {
            ("Not every goal is met on this seed.", palette::FAIL)
        };
        ui.label(RichText::new(text).strong().color(color));
        if lab.verdict_is_stale() {
            let warn = ui.visuals().warn_fg_color;
            ui.colored_label(
                warn,
                "Your design changed after this run ended: restart to judge the new one.",
            );
        }
        ui.label(
            RichText::new(
                "One seed is one possible history: confirm on several seeds before calling it.",
            )
            .small(),
        );
    }
    ui.add_space(6.0);
    goal_charts(ui, play, lab.view.smooth);
    ui.add_space(6.0);
    confirm(ui, lab);
    ui.add_space(6.0);
    known_solution(ui, lab);
}

fn goal_table(ui: &mut Ui, id: &str, results: &[GoalResult]) {
    egui::Grid::new(id)
        .num_columns(5)
        .striped(true)
        .spacing([14.0, 3.0])
        .show(ui, |ui| {
            ui.label(RichText::new("Goal").strong());
            ui.label(RichText::new("You").strong().color(palette::SIDE_A));
            ui.label(
                RichText::new("Baseline")
                    .strong()
                    .color(palette::BASELINE_MUTED),
            );
            ui.label(RichText::new("Threshold").strong());
            ui.label(RichText::new("").strong());
            ui.end_row();
            for r in results {
                ui.label(r.description);
                ui.label(fmt_metric(r.metric, r.player));
                ui.label(fmt_metric(r.metric, r.baseline));
                ui.label(fmt_metric(r.metric, r.threshold));
                if r.passed {
                    ui.label(RichText::new("PASS").strong().color(palette::PASS));
                } else {
                    ui.label(RichText::new("FAIL").strong().color(palette::FAIL));
                }
                ui.end_row();
            }
        });
}

/// One chart per goal metric: you against the baseline, with the threshold
/// the baseline's run so far implies. "You" is drawn last and stronger, so
/// it stays visible while the two designs are equal.
fn goal_charts(ui: &mut Ui, play: &ScenarioPlay, smooth: bool) {
    let sc = play.scenario();
    let lanes = play.runner.lanes();
    let day = play.runner.day();
    let markers = charts::markers_for(&lanes[0], palette::SIDE_A);
    let baseline_summary = {
        let sim = &lanes[1].sim;
        Summary::from_history(sim.history(), sim.summary_days())
    };
    ui.columns(sc.goals.len().max(1), |cols| {
        for (col, g) in cols.iter_mut().zip(sc.goals) {
            let info = g.metric.info();
            col.horizontal(|ui| {
                ui.label(RichText::new(info.name).strong())
                    .on_hover_text(info.description);
                visibility_tag(ui, info.visibility);
            });
            let threshold = g.comparison.threshold(baseline_summary.get(g.metric));
            let references = if threshold.is_finite() {
                vec![Reference {
                    name: "Threshold".into(),
                    value: f64::from(threshold),
                    color: palette::GREY,
                }]
            } else {
                Vec::new()
            };
            let series = vec![
                Series::new(
                    lanes[1].label.clone(),
                    palette::BASELINE_MUTED,
                    metric_points(lanes[1].sim.history(), g.metric),
                )
                .width(1.2),
                Series::new(
                    lanes[0].label.clone(),
                    palette::SIDE_A,
                    metric_points(lanes[0].sim.history(), g.metric),
                )
                .width(2.4),
            ];
            charts::line_chart(
                col,
                Chart {
                    id: Id::new(("scenario-chart", sc.id, info.key)),
                    title: None,
                    height: 190.0,
                    day,
                    series,
                    markers: &markers,
                    references,
                    format: info.format,
                    smooth,
                    include_zero: false,
                },
            );
        }
    });
}

/// "Confirm on 4 seeds": the current design and the baseline on seeds 1 to
/// 4, judged on the mean summaries.
fn confirm(ui: &mut Ui, lab: &mut Lab) {
    ui.label(RichText::new(format!("Confirm on {} seeds", CONFIRM_SEEDS.len())).strong());
    ui.label(
        RichText::new(format!(
            "Runs your current design and the baseline from day 0 on seeds 1 to {}, then judges \
             the goals on the mean of each side's summaries, as `incentive scenario` does.",
            CONFIRM_SEEDS.len()
        ))
        .small(),
    );
    let Some(play) = lab.scenarios.play.as_ref() else {
        return;
    };
    let running = play.confirm.as_ref().is_some_and(|c| c.results.is_none());
    let stale = lab.confirm_is_stale();
    let waiting = lab.scenario_needs_restart();
    let mut start = false;
    let mut cancel = false;
    ui.horizontal(|ui| {
        start = ui
            .add_enabled(
                !running && !waiting,
                egui::Button::new("Confirm on 4 seeds"),
            )
            .on_hover_text("Runs in the background, a little every frame")
            .on_disabled_hover_text(if waiting {
                "Restart first: the confirmation runs the setup on screen, and a change waits"
            } else {
                "A confirmation is running"
            })
            .clicked();
        cancel = running && ui.button("Cancel").clicked();
    });
    if let Some(c) = &play.confirm {
        if let Some(e) = c.batch.error() {
            ui.colored_label(palette::FAIL, format!("The confirmation stopped: {e}"));
        } else if let Some(results) = &c.results {
            if let Some(ran) = &play.confirm_config {
                ui.label(
                    RichText::new(format!(
                        "{} seeds, {} people, {} days.",
                        c.seeds.len(),
                        ran.run.population_size,
                        ran.run.days
                    ))
                    .small(),
                );
            }
            if stale {
                let warn = ui.visuals().warn_fg_color;
                ui.colored_label(
                    warn,
                    "Your design or the population changed since: confirm again to test them.",
                );
            }
            goal_table(ui, "scenario-confirm", results);
            let passed = results.iter().all(|r| r.passed);
            let (text, color) = if passed {
                (
                    "Confirmed: every goal is met on the mean of 4 seeds.",
                    palette::PASS,
                )
            } else {
                ("Not confirmed on the mean of 4 seeds.", palette::FAIL)
            };
            ui.label(RichText::new(text).strong().color(color));
        } else {
            let (done, total) = c.batch.counts();
            ui.add(
                egui::ProgressBar::new(c.batch.progress())
                    .show_percentage()
                    .text(format!("{done} of {total} runs")),
            );
        }
    }
    if start {
        lab.start_confirm();
    }
    if cancel && let Some(p) = lab.scenarios.play.as_mut() {
        p.confirm = None;
        p.confirm_config = None;
    }
}

/// "Show a known solution", behind a spoiler question.
fn known_solution(ui: &mut Ui, lab: &mut Lab) {
    let Some(play) = lab.scenarios.play.as_mut() else {
        return;
    };
    let mut load = false;
    if play.solution_loaded {
        let changes: Vec<String> = play
            .scenario()
            .known_solution_changes()
            .iter()
            .map(|(path, value)| format!("{path} = {value}"))
            .collect();
        ui.label(
            RichText::new(format!(
                "Known solution loaded and the run restarted from day 0: {}.",
                changes.join(", ")
            ))
            .small(),
        );
    }
    if play.spoiler_open {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.label(
                "This loads a design known to meet every goal into yours and restarts the run. \
                 Finding your own is the game. Show it anyway?",
            );
            ui.horizontal(|ui| {
                if ui.button("Yes, show the solution").clicked() {
                    load = true;
                    play.spoiler_open = false;
                }
                if ui.button("No, keep trying").clicked() {
                    play.spoiler_open = false;
                }
            });
        });
    } else if ui
        .button("Show a known solution")
        .on_hover_text("A spoiler: asks before it shows anything")
        .clicked()
    {
        play.spoiler_open = true;
    }
    if load {
        lab.load_known_solution();
        lab.playing = true;
    }
}
