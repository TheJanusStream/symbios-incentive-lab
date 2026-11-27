//! The Atlas tab: precomputed explorations of the design space. A map of
//! where designs land on engagement against harm, coloured by benefit; the
//! Pareto front of the three; and two Goodhart runs, where a search chased
//! one observable number and the truth was recorded alongside.

use egui::{Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2, pos2, vec2};
use egui_plot::{Line, MarkerShape, Plot, PlotPoint, PlotPoints, Points};
use incentive_core::{
    AtlasDesign, AtlasMetrics, ExploreSettings, Format, GoodhartAtlas, MetricId, Visibility,
};

use super::charts;
use super::{palette, visibility_tag};
use crate::lab::Lab;
use crate::lab::atlas::{
    AtlasData, AtlasPanel, AtlasPick, atlas, differences_from_engagement_ranking,
};
use crate::lab::series::{axis_label, fmt_metric};

/// Levers listed in a hover.
const HOVER_LEVERS: usize = 5;
/// Levers listed for the picked design.
const PICKED_LEVERS: usize = 10;
/// Width reserved left of the map for the harm ticks.
const MAP_LEFT: f32 = 46.0;
/// Height reserved below the map for the engagement ticks and title.
const MAP_BOTTOM: f32 = 40.0;
/// Width reserved right of the map for the colour legend.
const MAP_LEGEND: f32 = 92.0;

/// What a button asked for, applied after drawing.
enum Action {
    Sandbox(AtlasDesign),
    Compare(AtlasDesign),
}

/// Draws the Atlas tab.
pub fn show(ui: &mut Ui, lab: &mut Lab) {
    egui::CentralPanel::default().show(ui, |ui| {
        egui::ScrollArea::vertical()
            .id_salt("atlas-scroll")
            .show(ui, |ui| body(ui, lab));
    });
}

fn body(ui: &mut Ui, lab: &mut Lab) {
    ui.heading("Atlas of designs");
    let data = match atlas() {
        Ok(data) => data,
        Err(e) => {
            ui.colored_label(palette::FAIL, format!("The atlas could not be read: {e}"));
            return;
        }
    };
    how_to_read(ui);
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        for panel in AtlasPanel::ALL {
            ui.selectable_value(
                &mut lab.atlas.panel,
                panel,
                RichText::new(panel.name()).strong(),
            );
        }
    });
    ui.separator();
    let action = match lab.atlas.panel {
        AtlasPanel::Map => {
            settings_box(ui, &data.map.settings);
            map_view(ui, lab, data)
        }
        AtlasPanel::Pareto => {
            settings_box(ui, &data.pareto.settings);
            pareto_view(ui, lab, data)
        }
        AtlasPanel::Goodhart => {
            goodhart_view(ui, data);
            None
        }
    };
    // A design runs with the world assumptions and population settings it
    // was found under.
    let base = match lab.atlas.panel {
        AtlasPanel::Pareto => &data.pareto.settings.base,
        AtlasPanel::Map | AtlasPanel::Goodhart => &data.map.settings.base,
    };
    match action {
        Some(Action::Sandbox(d)) => lab.open_in_sandbox(d.design, base),
        Some(Action::Compare(d)) => lab.compare_with_engagement_ranking(d.design, base),
        None => {}
    }
}

fn how_to_read(ui: &mut Ui) {
    ui.label(RichText::new("How to read this").strong());
    ui.label(
        "A computer tried thousands of platform designs, each in a simulated society, and kept \
         the most interesting ones. The map places every design by how much attention it keeps \
         (the engagement index, which the platform can measure) and how much harm it does (the \
         harm index, which it cannot). Each square holds the design that did the most good \
         there: its colour is the benefit index, from people's wellbeing to creators earning a \
         living. Squares that stay empty are places no design reached. The Pareto front lists \
         the designs that no other design beats on all three at once. Goodhart shows what \
         happens when a search chases one number the platform can see.",
    );
    ui.label(
        RichText::new(
            "It is a map of a toy model, not a forecast: it shows where designs succeed or fail \
             in this model, and under its default assumptions about people. The designs were \
             searched in the default world only; across other plausible worlds the model's main \
             findings keep their direction, though their sizes move. At those defaults \
             the model charges for the time people spend but not for the interruption a \
             notification makes, and every design on the front keeps notifications at 0.95 or \
             more: on feeds worth their time the visits they bring raise wellbeing a little, \
             while on feeds that waste much of the time spent they leave it unchanged or lower \
             it, and on some designs of either kind they raise harm. Creator payouts cost nothing \
             in any score but net revenue, and every design on \
             the front pays creators, so designs that push payouts look better here than they \
             would be: open one and set them back to check (About lists what the model gets for \
             free).",
        )
        .small()
        .weak(),
    );
}

/// The settings a result was found with, and the reminder to open a design
/// at full size.
fn settings_box(ui: &mut Ui, s: &ExploreSettings) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Found at reduced size:").strong());
            ui.label(
                RichText::new(format!(
                    "{} people, {} days, {} seeds per design",
                    s.population,
                    s.days,
                    s.seeds.len()
                ))
                .strong()
                .color(palette::ORANGE),
            );
            ui.label(format!(
                "- {} designs evaluated ({} runs), {} generations of {}, search seed {}, {} world \
                 assumptions.",
                thousands(s.evaluations),
                thousands(s.runs),
                s.generations,
                s.batch,
                s.seed,
                if s.world == "defaults" {
                    "default".to_string()
                } else {
                    s.world.clone()
                }
            ));
        });
        ui.label(
            RichText::new(
                "Small, short runs on two seeds are noisy and can differ from a full run. Open a \
                 design in the Sandbox to see it at full size, or compare it with the engagement \
                 ranking and check it on 8 fresh seeds.",
            )
            .small(),
        );
        ui.label(
            RichText::new(format!("Reproduce: {}", s.command))
                .small()
                .weak()
                .monospace(),
        );
    });
}

fn thousands(n: u32) -> String {
    fmt_metric(MetricId::Impressions, n as f32)
}

/// The sequential colour of a benefit value: viridis, from dark purple
/// (low) to yellow (high), readable for colour-blind viewers.
pub fn benefit_color(value: f32, range: [f32; 2]) -> Color32 {
    const STOPS: [[f32; 3]; 5] = [
        [68.0, 1.0, 84.0],
        [59.0, 82.0, 139.0],
        [33.0, 145.0, 140.0],
        [94.0, 201.0, 98.0],
        [253.0, 231.0, 37.0],
    ];
    let t = if range[1] > range[0] {
        ((value - range[0]) / (range[1] - range[0])).clamp(0.0, 1.0)
    } else {
        0.5
    };
    let scaled = t * (STOPS.len() - 1) as f32;
    let i = (scaled.floor() as usize).min(STOPS.len() - 2);
    let f = scaled - i as f32;
    let c = |k: usize| STOPS[i][k] + (STOPS[i + 1][k] - STOPS[i][k]) * f;
    Color32::from_rgb(c(0).round() as u8, c(1).round() as u8, c(2).round() as u8)
}

/// The map on the left, the picked design on the right (stacked on a
/// narrow screen).
fn map_view(ui: &mut Ui, lab: &mut Lab, data: &'static AtlasData) -> Option<Action> {
    let mut action = None;
    if ui.available_width() >= 960.0 {
        ui.columns(2, |cols| {
            map_grid(&mut cols[0], lab, data);
            action = picked_details(&mut cols[1], lab, data);
        });
    } else {
        map_grid(ui, lab, data);
        ui.add_space(6.0);
        action = picked_details(ui, lab, data);
    }
    action
}

/// The painted map: one square per cell, coloured by its elite's benefit.
fn map_grid(ui: &mut Ui, lab: &mut Lab, data: &AtlasData) {
    let map = &data.map;
    let grid = map.grid;
    let n = grid.resolution.max(1);
    let range = data.benefit_range();
    let side = (ui.available_width() - MAP_LEFT - MAP_LEGEND).clamp(160.0, 520.0);
    let (rect, response) = ui.allocate_exact_size(
        vec2(MAP_LEFT + side + MAP_LEGEND, side + MAP_BOTTOM + 18.0),
        Sense::click(),
    );
    let painter = ui.painter_at(rect);
    let visuals = ui.visuals();
    let text_color = visuals.text_color();
    let weak = visuals.weak_text_color();
    let font = FontId::proportional(11.0);
    let area = Rect::from_min_size(
        pos2(rect.left() + MAP_LEFT, rect.top() + 18.0),
        vec2(side, side),
    );
    let cell = side / n as f32;
    let cell_rect = |x: u32, y: u32| {
        Rect::from_min_size(
            pos2(
                area.left() + x as f32 * cell,
                area.bottom() - (y + 1) as f32 * cell,
            ),
            vec2(cell, cell),
        )
    };
    painter.text(
        pos2(area.left(), rect.top()),
        Align2::LEFT_TOP,
        "Harm index (true), up",
        font.clone(),
        text_color,
    );
    painter.rect_filled(area, 0.0, visuals.extreme_bg_color);
    let grid_stroke = Stroke::new(0.5, visuals.widgets.noninteractive.bg_stroke.color);
    for i in 0..=n {
        let x = area.left() + i as f32 * cell;
        let y = area.bottom() - i as f32 * cell;
        painter.line_segment([pos2(x, area.top()), pos2(x, area.bottom())], grid_stroke);
        painter.line_segment([pos2(area.left(), y), pos2(area.right(), y)], grid_stroke);
    }
    for c in &map.cells {
        let color = benefit_color(c.elite.metrics.benefit_index, range);
        painter.rect_filled(cell_rect(c.x, c.y).shrink(0.5), 1.0, color);
    }
    // Ticks every quarter of the grid.
    let every = (n / 4).max(1);
    for i in (0..=n).step_by(every as usize) {
        let x = area.left() + i as f32 * cell;
        let value = if i == n {
            grid.engagement[1]
        } else {
            grid.column(i)[0]
        };
        painter.text(
            pos2(x, area.bottom() + 3.0),
            Align2::CENTER_TOP,
            format!("{value:.2}"),
            font.clone(),
            weak,
        );
        let y = area.bottom() - i as f32 * cell;
        let value = if i == n { grid.harm[1] } else { grid.row(i)[0] };
        painter.text(
            pos2(area.left() - 4.0, y),
            Align2::RIGHT_CENTER,
            format!("{value:.2}"),
            font.clone(),
            weak,
        );
    }
    painter.text(
        pos2(area.center().x, area.bottom() + 20.0),
        Align2::CENTER_TOP,
        "Engagement index (the platform sees it), across",
        font.clone(),
        text_color,
    );
    legend(&painter, area, range, &font, text_color, weak);
    let hovered = response.hover_pos().and_then(|p| cell_at(area, n, p));
    if let Some(AtlasPick::Cell { x, y }) = lab.atlas.picked {
        painter.rect_stroke(
            cell_rect(x, y),
            1.0,
            Stroke::new(2.5, Color32::WHITE),
            egui::StrokeKind::Inside,
        );
    }
    if let Some((x, y)) = hovered {
        let filled = map.cell(x, y);
        painter.rect_stroke(
            cell_rect(x, y),
            1.0,
            Stroke::new(1.5, text_color),
            egui::StrokeKind::Inside,
        );
        let clicked = response.clicked();
        match filled {
            Some(c) => {
                let elite = c.elite.clone();
                let (cx, cy) = (grid.column(x), grid.row(y));
                response.on_hover_ui_at_pointer(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "Engagement {:.2} to {:.2}, harm {:.2} to {:.2}",
                            cx[0], cx[1], cy[0], cy[1]
                        ))
                        .strong(),
                    );
                    design_summary(ui, &elite, &map.settings, HOVER_LEVERS);
                    ui.label(RichText::new("Click to pick this design.").small().weak());
                });
                if clicked {
                    lab.atlas.picked = Some(AtlasPick::Cell { x, y });
                }
            }
            None => {
                response.on_hover_text("No design reached this square.");
            }
        }
    }
}

/// The colour legend right of the map.
fn legend(
    painter: &egui::Painter,
    area: Rect,
    range: [f32; 2],
    font: &FontId,
    text: Color32,
    weak: Color32,
) {
    let bar = Rect::from_min_size(
        pos2(area.right() + 14.0, area.top() + 18.0),
        vec2(14.0, area.height() - 36.0),
    );
    painter.text(
        pos2(bar.left(), area.top()),
        Align2::LEFT_TOP,
        "Benefit",
        font.clone(),
        text,
    );
    let steps = 48;
    for i in 0..steps {
        let t0 = i as f32 / steps as f32;
        let t1 = (i + 1) as f32 / steps as f32;
        let value = range[0] + (range[1] - range[0]) * (t0 + t1) / 2.0;
        let r = Rect::from_min_max(
            pos2(bar.left(), bar.bottom() - t1 * bar.height()),
            pos2(bar.right(), bar.bottom() - t0 * bar.height() + 0.5),
        );
        painter.rect_filled(r, 0.0, benefit_color(value, range));
    }
    for k in 0..=4 {
        let t = k as f32 / 4.0;
        let value = range[0] + (range[1] - range[0]) * t;
        painter.text(
            pos2(bar.right() + 4.0, bar.bottom() - t * bar.height()),
            Align2::LEFT_CENTER,
            format!("{value:.2}"),
            font.clone(),
            weak,
        );
    }
    painter.text(
        pos2(bar.left(), bar.bottom() + 4.0),
        Align2::LEFT_TOP,
        "index",
        font.clone(),
        weak,
    );
}

/// The headline metrics, shown with their fresh-seed values when the atlas
/// has them.
const HEADLINE: [MetricId; 3] = [
    MetricId::EngagementIndex,
    MetricId::HarmIndex,
    MetricId::BenefitIndex,
];
/// The other metrics of a design's card, after the benefit index's parts.
const OTHERS: [MetricId; 4] = [
    MetricId::ToxicShare,
    MetricId::PolarisationSociety,
    MetricId::UserShare,
    MetricId::SurveySatisfaction,
];

/// A design's key metrics (with the fresh seeds' engagement, harm and
/// benefit when the atlas re-ran it), the five parts of its benefit index,
/// and the levers furthest from the engagement ranking.
fn design_summary(ui: &mut Ui, d: &AtlasDesign, settings: &ExploreSettings, levers: usize) {
    let confirmed = d.confirmed.filter(|_| !settings.confirm_seeds.is_empty());
    let value = |m: &AtlasMetrics, id: MetricId| fmt_metric(id, m.get(id).unwrap_or(f32::NAN));
    let columns = if confirmed.is_some() { 4 } else { 3 };
    let mut missing_parts = false;
    egui::Grid::new(ui.next_auto_id())
        .num_columns(columns)
        .spacing([10.0, 2.0])
        .show(ui, |ui| {
            ui.label("");
            ui.label(RichText::new("Search").small().strong());
            if confirmed.is_some() {
                ui.label(RichText::new("Fresh seeds").small().strong());
            }
            ui.label("");
            ui.end_row();
            let row = |ui: &mut Ui, id: MetricId, indent: bool, fresh: bool| {
                let info = id.info();
                ui.horizontal(|ui| {
                    if indent {
                        ui.add_space(12.0);
                    }
                    ui.label(info.name);
                });
                ui.label(RichText::new(value(&d.metrics, id)).strong());
                if let Some(c) = &confirmed {
                    if fresh {
                        ui.label(RichText::new(value(c, id)).strong().color(palette::GREEN));
                    } else {
                        ui.label("");
                    }
                }
                visibility_tag(ui, info.visibility);
                ui.end_row();
            };
            for id in HEADLINE {
                row(ui, id, false, true);
            }
            ui.label(
                RichText::new("Parts of the benefit index")
                    .small()
                    .italics(),
            );
            ui.end_row();
            for id in AtlasMetrics::BENEFIT_COMPONENTS {
                missing_parts |= d.metrics.get(id).is_none_or(f32::is_nan);
                row(ui, id, true, false);
            }
            for id in OTHERS {
                row(ui, id, false, false);
            }
        });
    if let Some(note) = fresh_note(settings, confirmed.is_some()) {
        ui.label(RichText::new(note).small().weak());
    }
    if missing_parts {
        ui.label(
            RichText::new("This atlas was made before every part was recorded: \"-\" marks those.")
                .small()
                .weak(),
        );
    }
    let diffs = differences_from_engagement_ranking(&d.design, levers);
    if diffs.is_empty() {
        ui.label("The same levers as the engagement ranking.");
        return;
    }
    ui.label(RichText::new("Levers that differ most from the engagement ranking").strong());
    for diff in diffs {
        ui.label(format!(
            "{}: {} -> {}",
            diff.name, diff.baseline, diff.design
        ));
    }
}

/// What the two columns of a design's card mean: the search's seeds, and
/// the fresh seeds it was re-run on, if any.
pub fn fresh_note(settings: &ExploreSettings, confirmed: bool) -> Option<String> {
    let search = super::compare::seed_range(&settings.seeds);
    if confirmed {
        Some(format!(
            "Search: the seeds the design was picked on ({search}). Fresh seeds: re-run on seeds \
             {} that played no part in picking it; trust these more.",
            super::compare::seed_range(&settings.confirm_seeds)
        ))
    } else if settings.seeds.is_empty() {
        None
    } else {
        Some(format!(
            "Search: the seeds the design was picked on ({search}); this atlas has no re-run on \
             fresh seeds."
        ))
    }
}

/// The picked design with its two actions.
fn picked_details(ui: &mut Ui, lab: &Lab, data: &AtlasData) -> Option<Action> {
    let Some(pick) = lab.atlas.picked else {
        ui.label(RichText::new("Picked design").strong());
        ui.label("Hover over a square or a point to see its design; click it to pick it.");
        return None;
    };
    let Some(design) = data.picked(pick) else {
        ui.label("Pick a design on this view.");
        return None;
    };
    let title = match pick {
        AtlasPick::Cell { x, y } => {
            let (cx, cy) = (data.map.grid.column(x), data.map.grid.row(y));
            format!(
                "Picked: the square of engagement {:.2} to {:.2} and harm {:.2} to {:.2}",
                cx[0], cx[1], cy[0], cy[1]
            )
        }
        AtlasPick::Front(i) => format!(
            "Picked: design {} of {} on the front",
            i + 1,
            data.pareto.front.len()
        ),
    };
    ui.label(RichText::new(title).strong());
    let mut action = None;
    ui.horizontal_wrapped(|ui| {
        if ui
            .button("Open in Sandbox")
            .on_hover_text("Load this design into the Sandbox and restart it at full size")
            .clicked()
        {
            action = Some(Action::Sandbox(design.clone()));
        }
        if ui
            .button("Compare with the engagement ranking")
            .on_hover_text("Load the engagement ranking as A and this design as B in Compare")
            .clicked()
        {
            action = Some(Action::Compare(design.clone()));
        }
    });
    let settings = match pick {
        AtlasPick::Cell { .. } => &data.map.settings,
        AtlasPick::Front(_) => &data.pareto.settings,
    };
    design_summary(ui, design, settings, PICKED_LEVERS);
    action
}

/// The front as a scatter of engagement against harm, coloured by benefit,
/// with the picked design beside it.
fn pareto_view(ui: &mut Ui, lab: &mut Lab, data: &'static AtlasData) -> Option<Action> {
    let mut action = None;
    if ui.available_width() >= 960.0 {
        ui.columns(2, |cols| {
            pareto_plot(&mut cols[0], lab, data);
            action = picked_details(&mut cols[1], lab, data);
        });
    } else {
        pareto_plot(ui, lab, data);
        ui.add_space(6.0);
        action = picked_details(ui, lab, data);
    }
    action
}

fn pareto_plot(ui: &mut Ui, lab: &mut Lab, data: &AtlasData) {
    let front = &data.pareto.front;
    let range = data.benefit_range();
    ui.label(
        RichText::new(format!(
            "{} designs that no other design found beats on engagement, harm and benefit at \
             once. Colour is the benefit index, on the map's scale ({:.2} to {:.2}).",
            front.len(),
            range[0],
            range[1]
        ))
        .small(),
    );
    let picked = match lab.atlas.picked {
        Some(AtlasPick::Front(i)) => Some(i),
        _ => None,
    };
    let height = (ui.available_width() * 0.8).clamp(220.0, 460.0);
    let response = Plot::new("atlas-pareto")
        .height(height)
        .allow_zoom(false)
        .allow_drag(false)
        .allow_scroll(false)
        .allow_boxed_zoom(false)
        .allow_double_click_reset(false)
        .include_y(0.0)
        .x_axis_label("Engagement index (the platform sees it)")
        .y_axis_label("Harm index (true)")
        .y_axis_min_width(46.0)
        .x_axis_formatter(|m, _| axis_label(Format::Score, m.value, m.step_size))
        .y_axis_formatter(|m, _| axis_label(Format::Score, m.value, m.step_size))
        .label_formatter(|hover| {
            let p = hover_position(hover);
            Some(format!("engagement {:.3}\nharm {:.3}", p.x, p.y))
        })
        .show(ui, |plot_ui| {
            for (i, d) in front.iter().enumerate() {
                let m = d.metrics;
                let point = [f64::from(m.engagement_index), f64::from(m.harm_index)];
                if picked == Some(i) {
                    plot_ui.points(
                        Points::new("Picked", PlotPoints::new(vec![point]))
                            .shape(MarkerShape::Circle)
                            .filled(false)
                            .radius(9.0)
                            .color(Color32::WHITE),
                    );
                }
                plot_ui.points(
                    Points::new(format!("Design {}", i + 1), PlotPoints::new(vec![point]))
                        .shape(MarkerShape::Circle)
                        .filled(true)
                        .radius(5.0)
                        .color(benefit_color(m.benefit_index, range)),
                );
            }
        });
    let transform = response.transform;
    let nearest = response.response.hover_pos().and_then(|p| {
        front
            .iter()
            .enumerate()
            .map(|(i, d)| {
                let at = transform.position_from_point(&PlotPoint::new(
                    f64::from(d.metrics.engagement_index),
                    f64::from(d.metrics.harm_index),
                ));
                (i, at.distance(p))
            })
            .filter(|(_, dist)| *dist <= 12.0)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    });
    if let Some(i) = nearest {
        if response.response.clicked() {
            lab.atlas.picked = Some(AtlasPick::Front(i));
        }
        let design = front[i].clone();
        response.response.on_hover_ui_at_pointer(|ui| {
            ui.label(RichText::new(format!("Design {} of {}", i + 1, front.len())).strong());
            design_summary(ui, &design, &data.pareto.settings, HOVER_LEVERS);
            ui.label(RichText::new("Click to pick this design.").small().weak());
        });
    }
}

/// The Goodhart runs: for each proxy, the proxy and the truth over the
/// generations as small charts.
fn goodhart_view(ui: &mut Ui, data: &AtlasData) {
    ui.label(
        "How to read these: the first chart in each row is the number the search was told to \
         raise, which the platform can measure; the others are true outcomes the search never \
         saw. If they get worse while the first one climbs, chasing the number is doing harm: \
         Goodhart's law. The survey run is the subtler one: a survey answer mixes the person's \
         wellbeing with recent pleasure and habit, so a search can raise the answers partly \
         through pleasure and habit; here it also raised wellbeing, while the toxic share more \
         than tripled and the harm index rose.",
    );
    ui.add_space(4.0);
    for run in &data.goodhart {
        goodhart_run(ui, run);
        ui.add_space(10.0);
    }
}

fn goodhart_run(ui: &mut Ui, run: &GoodhartAtlas) {
    let proxy = MetricId::from_key(&run.proxy).unwrap_or(MetricId::EngagementIndex);
    ui.label(
        RichText::new(format!(
            "Maximising {} alone",
            proxy.info().name.to_lowercase()
        ))
        .strong()
        .size(15.0),
    );
    settings_box(ui, &run.settings);
    let metrics = [
        (MetricId::HarmIndex, palette::VERMILLION),
        (MetricId::BenefitIndex, palette::GREEN),
        (MetricId::WellbeingEverUsers, palette::GREEN),
        (MetricId::ToxicShare, palette::VERMILLION),
    ];
    if let (Some(first), Some(last)) = (run.generations.first(), run.generations.last()) {
        let mut parts = vec![format!(
            "{} {} -> {}",
            proxy.info().name,
            fmt_metric(proxy, first.proxy),
            fmt_metric(proxy, last.proxy)
        )];
        for (id, _) in metrics {
            let (a, b) = (
                first.best.metrics.get(id).unwrap_or(f32::NAN),
                last.best.metrics.get(id).unwrap_or(f32::NAN),
            );
            parts.push(format!(
                "{} {} -> {}",
                id.info().name.to_lowercase(),
                fmt_metric(id, a),
                fmt_metric(id, b)
            ));
        }
        ui.label(
            RichText::new(format!(
                "Best design of generation {} against generation {}: {}.",
                first.generation,
                last.generation,
                parts.join(", ")
            ))
            .small(),
        );
    }
    let mut charts_spec: Vec<(MetricId, Color32, Vec<[f64; 2]>)> = vec![(
        proxy,
        palette::OBSERVABLE,
        run.generations
            .iter()
            .map(|g| [f64::from(g.generation), f64::from(g.proxy)])
            .collect(),
    )];
    for (id, color) in metrics {
        charts_spec.push((
            id,
            color,
            run.generations
                .iter()
                .filter_map(|g| {
                    let v = g.best.metrics.get(id)?;
                    v.is_finite()
                        .then_some([f64::from(g.generation), f64::from(v)])
                })
                .collect(),
        ));
    }
    let columns = if ui.available_width() >= 1100.0 {
        5
    } else if ui.available_width() >= 640.0 {
        3
    } else {
        1
    };
    for row in charts_spec.chunks(columns) {
        ui.columns(columns, |cols| {
            for (col, (id, color, points)) in cols.iter_mut().zip(row) {
                generation_chart(col, &run.proxy, *id, *color, points);
            }
        });
    }
}

/// One small chart over the generations.
fn generation_chart(ui: &mut Ui, proxy: &str, id: MetricId, color: Color32, points: &[[f64; 2]]) {
    let info = id.info();
    ui.horizontal(|ui| {
        ui.label(RichText::new(info.name).strong())
            .on_hover_text(info.description);
        visibility_tag(ui, info.visibility);
    });
    let role = if info.visibility == Visibility::Observable && info.key == proxy {
        "what the search raised"
    } else {
        "never seen by the search"
    };
    ui.label(RichText::new(role).small().weak());
    let format = info.format;
    Plot::new(("atlas-goodhart", proxy, info.key))
        .height(120.0)
        .allow_zoom(false)
        .allow_drag(false)
        .allow_scroll(false)
        .allow_boxed_zoom(false)
        .allow_double_click_reset(false)
        .y_axis_min_width(46.0)
        .y_grid_spacer(charts::y_ticks)
        .y_axis_formatter(move |m, _| axis_label(format, m.value, m.step_size))
        .x_axis_formatter(|m, _| format!("{:.0}", m.value))
        .x_axis_label("generation")
        .label_formatter(move |hover| {
            let p = hover_position(hover);
            Some(format!(
                "generation {:.0}: {}",
                p.x,
                crate::lab::series::fmt_format(format, p.y as f32)
            ))
        })
        .show(ui, |plot_ui| {
            plot_ui.line(
                Line::new(info.name, PlotPoints::new(points.to_vec()))
                    .color(color)
                    .width(charts::LINE_WIDTH + 0.4),
            );
        });
}

/// The plot position of a hover, near a point or not.
fn hover_position(hover: &egui_plot::HoverPosition<'_>) -> PlotPoint {
    match hover {
        egui_plot::HoverPosition::NearDataPoint { position, .. }
        | egui_plot::HoverPosition::Elsewhere { position } => *position,
    }
}

/// The map cell under a screen position, row 0 at the bottom; `None`
/// outside the map.
pub fn cell_at(area: Rect, resolution: u32, pos: Pos2) -> Option<(u32, u32)> {
    if !area.contains(pos) || resolution == 0 {
        return None;
    }
    let cell: Vec2 = area.size() / resolution as f32;
    let x = (((pos.x - area.left()) / cell.x).floor() as u32).min(resolution - 1);
    let y = (((area.bottom() - pos.y) / cell.y).floor() as u32).min(resolution - 1);
    Some((x, y))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_benefit_scale_runs_from_dark_to_light() {
        let range = [0.3, 0.8];
        let low = benefit_color(0.3, range);
        let high = benefit_color(0.8, range);
        assert_eq!(low, Color32::from_rgb(68, 1, 84));
        assert_eq!(high, Color32::from_rgb(253, 231, 37));
        let luminance = |c: Color32| u32::from(c.r()) + u32::from(c.g()) + u32::from(c.b());
        let mut last = 0;
        for k in 0..=10 {
            let c = benefit_color(0.3 + 0.05 * k as f32, range);
            assert!(luminance(c) >= last, "the scale brightens");
            last = luminance(c);
        }
        assert_eq!(benefit_color(9.0, range), high, "clamped");
    }

    #[test]
    fn the_card_says_which_seeds_each_column_used() {
        let data = atlas().expect("the embedded atlas parses");
        let mut settings = data.map.settings.clone();
        settings.seeds = vec![1, 2];
        settings.confirm_seeds = (9..=16).collect();
        let note = fresh_note(&settings, true).unwrap_or_default();
        assert!(
            note.contains("(1 and 2)") && note.contains("9 to 16"),
            "{note}"
        );
        let note = fresh_note(&settings, false).unwrap_or_default();
        assert!(note.contains("no re-run"), "{note}");
        settings.seeds.clear();
        assert_eq!(fresh_note(&settings, false), None);
    }

    #[test]
    fn the_card_draws_with_fresh_seeds_and_missing_parts() {
        let data = atlas().expect("the embedded atlas parses");
        let mut settings = data.map.settings.clone();
        settings.confirm_seeds = (9..=16).collect();
        let mut design = data.map.cells[0].elite.clone();
        design.metrics.informedness_society = f32::NAN;
        design.confirmed = Some(AtlasMetrics {
            harm_index: design.metrics.harm_index + 0.02,
            ..design.metrics
        });
        let ctx = egui::Context::default();
        for (d, s) in [(&design, &settings), (&design, &data.map.settings)] {
            let output = ctx.run_ui(egui::RawInput::default(), |ui| {
                design_summary(ui, d, s, HOVER_LEVERS);
            });
            assert!(!output.shapes.is_empty());
            output.drop_without_applying_deltas();
        }
    }

    #[test]
    fn screen_positions_map_to_cells_with_y_up() {
        let area = Rect::from_min_size(pos2(0.0, 0.0), vec2(160.0, 160.0));
        assert_eq!(cell_at(area, 16, pos2(1.0, 159.0)), Some((0, 0)));
        assert_eq!(cell_at(area, 16, pos2(159.0, 1.0)), Some((15, 15)));
        assert_eq!(cell_at(area, 16, pos2(25.0, 155.0)), Some((2, 0)));
        assert_eq!(cell_at(area, 16, pos2(25.0, 149.0)), Some((2, 1)));
        assert_eq!(cell_at(area, 16, pos2(-1.0, 5.0)), None);
    }
}
