//! The compare mode: two designs on the same seed and world in lockstep,
//! overlaid charts, a table of recent means, and the multi-seed check that
//! turns an anecdote into evidence.

use egui::{Id, RichText, Ui};
use incentive_core::{MetricId, metric_infos, presets};

use super::charts::{self, Chart, Series};
use super::{levers, palette, visibility_tag};
use crate::lab::series::{KEY_METRICS, fmt_diff, fmt_metric, metric_points, tail_mean};
use crate::lab::{CHECK_SEEDS, Lab, TABLE_DAYS};

/// Draws the compare tab.
pub fn show(ui: &mut Ui, lab: &mut Lab) {
    egui::Panel::left("compare-levers")
        .resizable(true)
        .default_size(330.0)
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("compare-levers-scroll")
                .show(ui, |ui| levers::compare_panel(ui, lab));
        });
    egui::CentralPanel::default().show(ui, |ui| {
        egui::ScrollArea::vertical()
            .id_salt("compare-centre")
            .show(ui, |ui| centre(ui, lab));
    });
}

/// "Engagement maximiser" or "custom design".
fn design_name(design: &incentive_core::DesignLevers) -> &'static str {
    levers::preset_index(design).map_or("custom design", |i| presets()[i].name)
}

fn centre(ui: &mut Ui, lab: &mut Lab) {
    let names = [
        design_name(&lab.compare.drafts[0].design),
        design_name(&lab.compare.drafts[1].design),
    ];
    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new(format!("A: {}", names[0]))
                .strong()
                .size(15.0)
                .color(palette::SIDE_A),
        );
        ui.label(RichText::new("versus").size(15.0));
        ui.label(
            RichText::new(format!("B: {}", names[1]))
                .strong()
                .size(15.0)
                .color(palette::SIDE_B),
        );
    });
    if let Some(message) = &lab.compare.message {
        ui.label(RichText::new(message).italics());
    }
    let a_config = lab.compare.runner.lane(0).sim.config();
    ui.label(format!(
        "Both run on seed {}, with the same {} people in the same world, day by day in lockstep: \
         every random draw a design does not touch is shared, so the difference you see is the \
         design's.",
        a_config.run.seed, a_config.run.population_size
    ));
    ui.label(
        RichText::new("One seed is an anecdote: the multi-seed check below is the evidence.")
            .strong(),
    );
    ui.add_space(4.0);
    overlay_charts(ui, lab);
    ui.add_space(8.0);
    recent_table(ui, lab);
    ui.add_space(8.0);
    seed_check(ui, lab);
}

fn overlay_charts(ui: &mut Ui, lab: &mut Lab) {
    let compare = &mut lab.compare;
    ui.menu_button(
        format!("Choose metrics ({})", compare.metrics.len()),
        |ui| {
            egui::ScrollArea::vertical()
                .max_height(420.0)
                .id_salt("compare-picker")
                .show(ui, |ui| {
                    for info in metric_infos() {
                        let mut on = compare.metrics.contains(&info.id);
                        if ui
                            .checkbox(&mut on, info.name)
                            .on_hover_text(info.description)
                            .changed()
                        {
                            if on {
                                compare.metrics.push(info.id);
                            } else {
                                compare.metrics.retain(|m| *m != info.id);
                            }
                        }
                    }
                });
        },
    );
    let lanes = compare.runner.lanes();
    let day = compare.runner.day();
    let smooth = lab.view.smooth;
    let mut markers = charts::markers_for(&lanes[0], palette::SIDE_A);
    markers.extend(charts::markers_for(&lanes[1], palette::SIDE_B));
    let metrics = compare.metrics.clone();
    for pair in metrics.chunks(2) {
        ui.columns(2, |cols| {
            for (col, &m) in cols.iter_mut().zip(pair) {
                let info = m.info();
                let series: Vec<Series> = lanes
                    .iter()
                    .zip([palette::SIDE_A, palette::SIDE_B])
                    .map(|(lane, color)| {
                        Series::new(
                            lane.label.clone(),
                            color,
                            metric_points(lane.sim.history(), m),
                        )
                    })
                    .collect();
                col.horizontal(|ui| {
                    ui.label(RichText::new(info.name).strong())
                        .on_hover_text(info.description);
                    visibility_tag(ui, info.visibility);
                    for s in &series {
                        ui.label(
                            RichText::new(fmt_metric(m, charts::shown_latest(&s.points, smooth)))
                                .color(s.color),
                        )
                        .on_hover_text(format!(
                            "{}: {}",
                            s.name,
                            charts::latest_note(smooth)
                        ));
                    }
                });
                charts::line_chart(
                    col,
                    Chart {
                        id: Id::new(("compare-chart", info.key)),
                        title: None,
                        height: 150.0,
                        day,
                        series,
                        markers: &markers,
                        references: Vec::new(),
                        format: info.format,
                        smooth,
                        include_zero: false,
                    },
                );
            }
        });
        ui.add_space(4.0);
    }
}

/// Last-30-day means of the key metrics, A, B and B minus A.
fn recent_table(ui: &mut Ui, lab: &Lab) {
    let lanes = lab.compare.runner.lanes();
    ui.label(RichText::new(format!("This seed, mean of the last {TABLE_DAYS} days")).strong());
    egui::Grid::new("compare-recent")
        .num_columns(5)
        .striped(true)
        .spacing([14.0, 3.0])
        .show(ui, |ui| {
            ui.label(RichText::new("Metric").strong());
            ui.label(RichText::new("View").strong());
            ui.label(RichText::new("A").strong().color(palette::SIDE_A));
            ui.label(RichText::new("B").strong().color(palette::SIDE_B));
            ui.label(RichText::new("B - A").strong());
            ui.end_row();
            for m in KEY_METRICS {
                let a = tail_mean(lanes[0].sim.history(), m, TABLE_DAYS);
                let b = tail_mean(lanes[1].sim.history(), m, TABLE_DAYS);
                ui.label(m.info().name).on_hover_text(m.info().description);
                visibility_tag(ui, m.info().visibility);
                ui.label(fmt_metric(m, a));
                ui.label(fmt_metric(m, b));
                ui.label(fmt_diff(m, b - a));
                ui.end_row();
            }
        });
}

/// "Check on 8 seeds": both designs on seeds 1 to 8, time-sliced, then the
/// means and the paired difference with its 95% interval.
fn seed_check(ui: &mut Ui, lab: &mut Lab) {
    ui.label(RichText::new(format!("Check on {} seeds", CHECK_SEEDS.len())).strong());
    ui.label(
        RichText::new(format!(
            "Runs both current designs from day 0 on seeds 1 to {} at {} people for {} days, \
             paired by seed, as `incentive compare` does. Each run is summarised over its \
             summary window: the last 30% of its days, and at least 30 days. The design changes \
             you made during the live run are not replayed: the check runs the designs as they \
             are now.",
            CHECK_SEEDS.len(),
            lab.run.population,
            lab.run.days
        ))
        .small(),
    );
    let running = lab.compare.check.as_ref().is_some_and(|c| c.rows.is_none());
    let waiting = lab.compare_needs_restart();
    ui.horizontal(|ui| {
        let label = if lab.compare.check.is_some() {
            "Check again"
        } else {
            "Check on 8 seeds"
        };
        if ui
            .add_enabled(!running && !waiting, egui::Button::new(label))
            .on_hover_text("Runs in the background, a little every frame")
            .on_disabled_hover_text(if waiting {
                "Restart first: the check runs the setup on screen, and a change waits"
            } else {
                "A check is running"
            })
            .clicked()
        {
            lab.start_seed_check();
        }
        if running && ui.button("Cancel").clicked() {
            lab.compare.check = None;
            lab.compare.check_configs = None;
        }
    });
    let stale = lab.compare.check_is_stale(lab.run);
    let Some(check) = &lab.compare.check else {
        return;
    };
    if let Some(e) = check.batch.error() {
        ui.colored_label(palette::FAIL, format!("The check stopped: {e}"));
        return;
    }
    let Some(rows) = &check.rows else {
        let (done, total) = check.batch.counts();
        ui.add(
            egui::ProgressBar::new(check.batch.progress())
                .show_percentage()
                .text(format!("{done} of {total} runs")),
        );
        return;
    };
    ui.label(
        RichText::new(format!(
            "{} paired seeds, {} people, {} days.",
            check.seeds.len(),
            check.population,
            check.days
        ))
        .small(),
    );
    if stale {
        let warn = ui.visuals().warn_fg_color;
        ui.colored_label(
            warn,
            "The designs, world, population or length changed since this check: run it again \
             to test the current setup.",
        );
    }
    egui::Grid::new("compare-check")
        .num_columns(7)
        .striped(true)
        .spacing([14.0, 3.0])
        .show(ui, |ui| {
            for (h, hover) in [
                ("Metric", ""),
                ("View", ""),
                ("A", "Mean of A over the seeds"),
                ("B", "Mean of B over the seeds"),
                ("B - A", "Mean of the paired differences"),
                (
                    "95% interval",
                    "Confidence interval of the paired difference",
                ),
                ("", "* the interval excludes zero"),
            ] {
                ui.label(RichText::new(h).strong()).on_hover_text(hover);
            }
            ui.end_row();
            for row in rows {
                let m: MetricId = row.metric;
                let d = row.paired;
                ui.label(m.info().name).on_hover_text(m.info().description);
                visibility_tag(ui, m.info().visibility);
                ui.label(RichText::new(fmt_metric(m, d.mean_a)).color(palette::SIDE_A));
                ui.label(RichText::new(fmt_metric(m, d.mean_b)).color(palette::SIDE_B));
                ui.label(fmt_diff(m, d.diff));
                ui.label(format!("{} to {}", fmt_diff(m, d.low), fmt_diff(m, d.high)));
                ui.label(if d.excludes_zero() { "*" } else { "" });
                ui.end_row();
            }
        });
    ui.label(
        RichText::new(
            "* the 95% confidence interval of the paired difference excludes zero. An interval \
             that spans zero means these seeds cannot tell the designs apart on that metric.",
        )
        .small(),
    );
    ui.label(
        RichText::new(
            "For other seeds, sizes or metrics, use the command-line runner: `incentive compare \
             --a <design> --b <design> --seeds 1..8` (designs as preset ids or Copy configuration \
             files).",
        )
        .small()
        .weak(),
    );
}
