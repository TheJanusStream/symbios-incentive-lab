//! Line charts with design-change markers, and sparklines.
//!
//! Every chart fits its x axis to the days simulated so far (at least
//! [`MIN_SPAN_DAYS`]), labels its y axis with "nice" ticks in the metric's
//! own format, and can draw the trailing 7-day mean instead of the noisy
//! daily values. Charts with several lines carry a row of coloured labels
//! above the plot instead of a legend inside it, so nothing covers the lines.

use egui::{Color32, Id, RichText, Stroke, Ui, Vec2};
use egui_plot::{GridInput, GridMark, HLine, Line, LineStyle, Plot, PlotPoints, VLine};
use incentive_core::Format;

use crate::lab::runner::Lane;
use crate::lab::series::{self, SMOOTH_DAYS, axis_label, fmt_format, nice_step};

/// The shortest span of the x axis, in days.
pub const MIN_SPAN_DAYS: u32 = 30;

/// Width of the y axis, wide enough for labels such as "15,750" or "0.25".
const Y_AXIS_WIDTH: f32 = 46.0;

/// Target distance between two y-axis ticks, in points.
const Y_TICK_SPACING: f64 = 34.0;

/// Line width of an ordinary series.
pub const LINE_WIDTH: f32 = 1.6;

/// One line of a chart.
#[derive(Clone, Debug)]
pub struct Series {
    /// Label in the row above the plot and in the hover text.
    pub name: String,
    /// Line colour.
    pub color: Color32,
    /// Points `[day, value]`.
    pub points: Vec<[f64; 2]>,
    /// Line width in points.
    pub width: f32,
}

impl Series {
    /// A series of ordinary width.
    pub fn new(name: impl Into<String>, color: Color32, points: Vec<[f64; 2]>) -> Self {
        Self {
            name: name.into(),
            color,
            points,
            width: LINE_WIDTH,
        }
    }

    /// The same series with another line width.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }
}

/// A vertical marker at a day the design changed.
#[derive(Clone, Debug)]
pub struct Marker {
    /// The first day of the new design.
    pub day: u32,
    /// Hover text: what changed.
    pub text: String,
    /// Line colour.
    pub color: Color32,
}

/// A dashed horizontal reference line, such as a goal's threshold.
#[derive(Clone, Debug)]
pub struct Reference {
    /// Label in the row above the plot.
    pub name: String,
    /// Height.
    pub value: f64,
    /// Line colour.
    pub color: Color32,
}

/// The markers of a lane's design changes, in one colour.
pub fn markers_for(lane: &Lane, color: Color32) -> Vec<Marker> {
    lane.changes
        .iter()
        .map(|c| Marker {
            day: c.day,
            text: if lane.label.is_empty() {
                c.text()
            } else {
                format!("{}: {}", lane.label, c.text())
            },
            color,
        })
        .collect()
}

/// Everything a line chart shows.
#[derive(Clone, Debug)]
pub struct Chart<'a> {
    /// Unique id of the plot.
    pub id: Id,
    /// Title above the plot, if any.
    pub title: Option<&'a str>,
    /// Plot height in points.
    pub height: f32,
    /// Days simulated so far: the x axis spans them, and at least
    /// [`MIN_SPAN_DAYS`].
    pub day: u32,
    /// The lines, drawn in order (the last on top).
    pub series: Vec<Series>,
    /// Design-change markers.
    pub markers: &'a [Marker],
    /// Reference lines.
    pub references: Vec<Reference>,
    /// How the y-axis ticks and the hover values are written.
    pub format: Format,
    /// Draw the trailing 7-day mean of each line instead of daily values.
    pub smooth: bool,
    /// Keep zero on the y axis (shares that add up); otherwise the axis fits
    /// the lines.
    pub include_zero: bool,
}

/// The last x of the axis: the days simulated, at least [`MIN_SPAN_DAYS`].
pub fn x_span(day: u32) -> u32 {
    day.max(MIN_SPAN_DAYS)
}

/// The points a chart draws: smoothed or as they are.
pub fn prepared(points: &[[f64; 2]], smooth: bool) -> Vec<[f64; 2]> {
    if smooth {
        series::smoothed(points, SMOOTH_DAYS)
    } else {
        points.to_vec()
    }
}

/// The y-axis ticks: one step of 1, 2 or 5 times a power of ten, about
/// 34 points apart, so every tick is labelled.
pub fn y_ticks(input: GridInput) -> Vec<GridMark> {
    let (lo, hi) = input.bounds;
    if !(lo.is_finite() && hi.is_finite()) || hi <= lo || input.base_step_size <= 0.0 {
        return Vec::new();
    }
    // `base_step_size` is the value of the plot's minimum grid spacing (8
    // points), so this is the value of the target spacing.
    let step = nice_step(input.base_step_size * Y_TICK_SPACING / 8.0);
    let first = (lo / step).ceil() as i64;
    let last = (hi / step).floor() as i64;
    (first..=last.min(first + 40))
        .map(|k| GridMark {
            value: k as f64 * step,
            step_size: step,
        })
        .collect()
}

/// The row of coloured labels above a chart with several lines.
pub fn legend_row(ui: &mut Ui, series: &[Series], references: &[Reference]) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        for s in series {
            ui.label(
                RichText::new(format!("\u{25A0} {}", s.name))
                    .small()
                    .color(s.color),
            );
        }
        for r in references {
            ui.label(
                RichText::new(format!("- - {}", r.name))
                    .small()
                    .color(r.color),
            );
        }
    });
}

/// Draws a line chart. Hovering near a marker shows what changed that day.
pub fn line_chart(ui: &mut Ui, chart: Chart<'_>) {
    if let Some(title) = chart.title {
        ui.label(RichText::new(title).strong());
    }
    if chart.series.len() > 1 || !chart.references.is_empty() {
        legend_row(ui, &chart.series, &chart.references);
    }
    let format = chart.format;
    let mut plot = Plot::new(chart.id)
        .height(chart.height)
        .allow_zoom(false)
        .allow_drag(false)
        .allow_scroll(false)
        .allow_boxed_zoom(false)
        .allow_double_click_reset(false)
        .include_x(0.0)
        .include_x(f64::from(x_span(chart.day)))
        .y_axis_min_width(Y_AXIS_WIDTH)
        .y_grid_spacer(y_ticks)
        .y_axis_formatter(move |mark, _| axis_label(format, mark.value, mark.step_size))
        .x_axis_formatter(|mark, _| format!("{:.0}", mark.value))
        .x_axis_label("day")
        .label_formatter(move |hover| match hover {
            egui_plot::HoverPosition::NearDataPoint {
                plot_name,
                position,
                ..
            } => Some(format!(
                "{plot_name}\nday {:.0}: {}",
                position.x,
                fmt_format(format, position.y as f32)
            )),
            egui_plot::HoverPosition::Elsewhere { position } => {
                Some(format!("day {:.0}", position.x.max(0.0)))
            }
        });
    if chart.include_zero {
        plot = plot.include_y(0.0);
    }
    let markers = chart.markers;
    let smooth = chart.smooth;
    let response = plot.show(ui, |plot_ui| {
        for s in chart.series {
            plot_ui.line(
                Line::new(s.name, PlotPoints::new(prepared(&s.points, smooth)))
                    .color(s.color)
                    .width(s.width),
            );
        }
        for r in chart.references {
            plot_ui.hline(
                HLine::new(r.name, r.value)
                    .color(r.color)
                    .style(LineStyle::dashed_loose()),
            );
        }
        for m in markers {
            plot_ui.vline(
                VLine::new("Design change", f64::from(m.day))
                    .color(m.color)
                    .style(LineStyle::dashed_dense())
                    .width(1.2),
            );
        }
        let pointer = plot_ui.pointer_coordinate()?;
        let bounds = plot_ui.plot_bounds();
        let tolerance = (bounds.width() * 0.012).max(0.6);
        let near: Vec<&str> = markers
            .iter()
            .filter(|m| (pointer.x - f64::from(m.day)).abs() <= tolerance)
            .map(|m| m.text.as_str())
            .collect();
        (!near.is_empty()).then(|| near.join("\n\n"))
    });
    if let Some(text) = response.inner {
        response.response.on_hover_text(text);
    }
}

/// The last value a chart of `points` shows: of the 7-day mean when
/// smoothing, of the last day otherwise; NaN without points.
pub fn shown_latest(points: &[[f64; 2]], smooth: bool) -> f32 {
    prepared(points, smooth)
        .last()
        .map_or(f32::NAN, |p| p[1] as f32)
}

/// What the latest value of a chart is, for its hover text.
pub fn latest_note(smooth: bool) -> &'static str {
    if smooth {
        "Mean of the last 7 days"
    } else {
        "The latest day"
    }
}

/// A small line without axes, for headline cards; smoothed like the charts.
pub fn sparkline(ui: &mut Ui, points: &[[f64; 2]], color: Color32, size: Vec2, smooth: bool) {
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_stroke(
        rect,
        2.0,
        Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
        egui::StrokeKind::Inside,
    );
    let points = prepared(points, smooth);
    if points.len() < 2 {
        return;
    }
    let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for p in &points {
        x0 = x0.min(p[0]);
        x1 = x1.max(p[0]);
        y0 = y0.min(p[1]);
        y1 = y1.max(p[1]);
    }
    // The same span rule as the charts: at least a month across.
    x1 = x1.max(x0 + f64::from(MIN_SPAN_DAYS));
    if (y1 - y0).abs() < 1e-9 {
        y0 -= 0.5;
        y1 += 0.5;
    }
    let inner = rect.shrink(3.0);
    let span_x = (x1 - x0).max(1e-9);
    let span_y = y1 - y0;
    let line: Vec<egui::Pos2> = points
        .iter()
        .map(|p| {
            egui::pos2(
                inner.left() + ((p[0] - x0) / span_x) as f32 * inner.width(),
                inner.bottom() - ((p[1] - y0) / span_y) as f32 * inner.height(),
            )
        })
        .collect();
    painter.add(egui::Shape::line(line, Stroke::new(1.5, color)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_x_axis_spans_at_least_a_month() {
        assert_eq!(x_span(0), MIN_SPAN_DAYS);
        assert_eq!(x_span(12), MIN_SPAN_DAYS);
        assert_eq!(x_span(120), 120);
    }

    #[test]
    fn y_ticks_are_evenly_spaced_and_inside_the_bounds() {
        // 0 to 0.35 over 150 points: 8 points are about 0.019, the target
        // spacing about 0.079, so the step is 0.1.
        let marks = y_ticks(GridInput {
            bounds: (0.0, 0.35),
            base_step_size: 0.35 / 150.0 * 8.0,
        });
        let values: Vec<f64> = marks.iter().map(|m| m.value).collect();
        assert_eq!(values.len(), 4);
        assert!(values.iter().all(|v| (0.0..=0.35).contains(v)));
        assert!(marks.iter().all(|m| (m.step_size - 0.1).abs() < 1e-12));
        assert!(
            y_ticks(GridInput {
                bounds: (1.0, 1.0),
                base_step_size: 0.1,
            })
            .is_empty()
        );
    }

    #[test]
    fn the_latest_shown_value_follows_smoothing() {
        let points = [[0.0, 1.0], [1.0, 3.0]];
        assert_eq!(shown_latest(&points, false), 3.0);
        assert_eq!(shown_latest(&points, true), 2.0);
        assert!(shown_latest(&[], true).is_nan());
        assert_ne!(latest_note(true), latest_note(false));
    }
}
