//! The sandbox: one platform, every lever on the left, what the platform sees
//! next to what is true in the centre, and the inspector on the right.

use egui::{Color32, Id, RichText, Ui, Vec2};
use incentive_core::{
    Archetype, DailyMetrics, Family, Format, MetricId, STYLE_COUNT, Style, Visibility, metric_infos,
};

use super::charts::{self, Chart, Marker, Series};
use super::{inspector, levers, palette, visibility_tag};
use crate::lab::series::{fmt_metric, impression_points, metric_points, wellbeing_points};
use crate::lab::{Lab, SUGGESTIONS, ViewFilter};

/// Height of a small metric chart.
const CHART_HEIGHT: f32 = 110.0;

/// Draws the sandbox tab.
pub fn show(ui: &mut Ui, lab: &mut Lab) {
    egui::Panel::left("sandbox-levers")
        .resizable(true)
        .default_size(330.0)
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("sandbox-levers-scroll")
                .show(ui, |ui| levers::sandbox_panel(ui, lab));
        });
    if lab.inspector.open {
        egui::Panel::right("inspector")
            .resizable(true)
            .default_size(520.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("inspector-scroll")
                    .show(ui, |ui| inspector::show(ui, lab));
            });
    }
    egui::CentralPanel::default().show(ui, |ui| {
        egui::ScrollArea::vertical()
            .id_salt("sandbox-centre")
            .show(ui, |ui| centre(ui, lab));
    });
}

fn centre(ui: &mut Ui, lab: &mut Lab) {
    if lab.view.start_here {
        start_here(ui, lab);
        ui.add_space(6.0);
    }
    let lane = lab.sandbox.runner.lane(0);
    let history = lane.sim.history();
    let markers = charts::markers_for(lane, palette::GREY);
    let view = &mut lab.view;
    let frame = Frame {
        history,
        day: lane.sim.day(),
        markers: &markers,
        smooth: view.smooth,
    };
    headline_cards(ui, &frame);
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label("Show:");
        ui.selectable_value(&mut view.filter, ViewFilter::PlatformOnly, "Platform only");
        ui.selectable_value(&mut view.filter, ViewFilter::Both, "Both");
        ui.selectable_value(&mut view.filter, ViewFilter::TruthOnly, "Truth only");
    });
    ui.add_space(4.0);
    match view.filter {
        ViewFilter::Both => {
            ui.columns(2, |cols| {
                metric_side(
                    &mut cols[0],
                    Visibility::Observable,
                    &mut view.observable,
                    &frame,
                );
                metric_side(&mut cols[1], Visibility::Truth, &mut view.truth, &frame);
            });
        }
        ViewFilter::PlatformOnly => {
            metric_side(ui, Visibility::Observable, &mut view.observable, &frame);
        }
        ViewFilter::TruthOnly => metric_side(ui, Visibility::Truth, &mut view.truth, &frame),
    }
    ui.add_space(8.0);
    egui::CollapsingHeader::new(RichText::new("Breakdowns").strong())
        .id_salt("breakdowns")
        .default_open(true)
        .show(ui, |ui| breakdowns(ui, &frame));
}

/// What every chart of the sandbox draws from: the history, the days run,
/// the design-change markers and whether to smooth.
struct Frame<'a> {
    history: &'a [DailyMetrics],
    day: u32,
    markers: &'a [Marker],
    smooth: bool,
}

/// The "Start here" box: three experiments from the documented findings.
fn start_here(ui: &mut Ui, lab: &mut Lab) {
    let mut apply = None;
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Start here").strong().size(15.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Dismiss").clicked() {
                    lab.view.start_here = false;
                }
            });
        });
        ui.label(
            "Press Play, let a few weeks pass, then try one of these and watch the charts. The \
             dashed line marks the day you changed the design; hover it to see what changed.",
        );
        for (i, s) in SUGGESTIONS.iter().enumerate() {
            ui.horizontal_wrapped(|ui| {
                if ui.button("Try it").clicked() {
                    apply = Some(i);
                }
                ui.label(RichText::new(format!("{}. {}", i + 1, s.title)).strong());
                ui.label(s.body);
            });
        }
        ui.label(
            RichText::new(
                "One run is one possible history. Before trusting a difference, check it on \
                 several seeds in the Compare tab.",
            )
            .small()
            .weak(),
        );
    });
    if let Some(i) = apply {
        lab.apply_suggestion(i);
        if !lab.sandbox.runner.is_finished() {
            lab.playing = true;
        }
    }
}

/// The three headline cards: engagement (observable), harm and benefit
/// (truth), with their latest value (the 7-day mean when smoothing) and a
/// sparkline.
fn headline_cards(ui: &mut Ui, frame: &Frame<'_>) {
    let cards = [
        (MetricId::EngagementIndex, palette::OBSERVABLE),
        (MetricId::HarmIndex, palette::VERMILLION),
        (MetricId::BenefitIndex, palette::GREEN),
    ];
    ui.columns(3, |cols| {
        for (col, (id, color)) in cols.iter_mut().zip(cards) {
            let info = id.info();
            let points = metric_points(frame.history, id);
            egui::Frame::group(col.style()).show(col, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(info.name).strong());
                    visibility_tag(ui, info.visibility);
                });
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(fmt_metric(id, charts::shown_latest(&points, frame.smooth)))
                            .size(24.0)
                            .strong()
                            .color(color),
                    )
                    .on_hover_text(info.description);
                    ui.label(
                        RichText::new(if frame.smooth { "7-day mean" } else { "today" })
                            .small()
                            .weak(),
                    );
                });
                let width = ui.available_width().max(60.0);
                charts::sparkline(ui, &points, color, Vec2::new(width, 36.0), frame.smooth);
            });
        }
    });
}

/// The families in display order.
const FAMILIES: [Family; 9] = [
    Family::Index,
    Family::Engagement,
    Family::Harm,
    Family::Participants,
    Family::NonUsers,
    Family::Discourse,
    Family::Creators,
    Family::Content,
    Family::Platform,
];

/// Display name of a family.
pub fn family_name(family: Family) -> &'static str {
    match family {
        Family::Engagement => "Engagement",
        Family::Harm => "Harm",
        Family::Participants => "The people on it",
        Family::NonUsers => "People not on it",
        Family::Discourse => "Public discourse",
        Family::Creators => "Creators",
        Family::Content => "Content",
        Family::Platform => "Platform operations",
        Family::Index => "Indices",
    }
}

/// One side of the split: a heading, a metric picker and one small chart
/// per picked metric, grouped by family. Each chart has its own scale, so a
/// metric of about 0.3 is not flattened by one of about 20.
fn metric_side(ui: &mut Ui, visibility: Visibility, picks: &mut Vec<MetricId>, frame: &Frame<'_>) {
    let (title, blurb, color) = match visibility {
        Visibility::Observable => (
            "What the platform sees",
            "Engagement, reports and a weekly survey. Its ranking, moderation and payouts also \
             use a few signals the model hands it ready-made (see About). The survey reaches \
             only current users, and happy ones answer more often.",
            palette::OBSERVABLE,
        ),
        Visibility::Truth => (
            "What is true",
            "What only the simulation knows: the hidden quality and toxicity of posts, and \
             people's wellbeing, informedness and polarisation, on and off the platform.",
            palette::TRUTH,
        ),
    };
    ui.label(RichText::new(title).strong().size(15.0).color(color));
    ui.label(RichText::new(blurb).small());
    metric_picker(ui, visibility, picks);
    let columns = if ui.available_width() >= SMALL_CHART_MIN_WIDTH * 2.0 {
        2
    } else {
        1
    };
    for family in FAMILIES {
        let in_family: Vec<MetricId> = picks
            .iter()
            .copied()
            .filter(|m| m.info().family == family)
            .collect();
        if in_family.is_empty() {
            continue;
        }
        ui.add_space(4.0);
        ui.label(RichText::new(family_name(family)).small().strong().weak());
        for row in in_family.chunks(columns) {
            ui.columns(columns, |cols| {
                for (col, &m) in cols.iter_mut().zip(row) {
                    small_chart(col, visibility, m, color, frame);
                }
            });
        }
    }
}

/// Narrowest useful width of a small chart, in points.
const SMALL_CHART_MIN_WIDTH: f32 = 260.0;

/// One small chart: the metric's name and latest value above its own
/// auto-scaled line.
fn small_chart(
    ui: &mut Ui,
    visibility: Visibility,
    m: MetricId,
    color: Color32,
    frame: &Frame<'_>,
) {
    let info = m.info();
    let points = metric_points(frame.history, m);
    ui.horizontal(|ui| {
        ui.label(RichText::new(info.name).strong())
            .on_hover_text(info.description);
        ui.label(
            RichText::new(fmt_metric(m, charts::shown_latest(&points, frame.smooth))).color(color),
        )
        .on_hover_text(charts::latest_note(frame.smooth));
    });
    charts::line_chart(
        ui,
        Chart {
            id: Id::new(("sandbox-chart", visibility_key(visibility), info.key)),
            title: None,
            height: CHART_HEIGHT,
            day: frame.day,
            series: vec![Series::new(info.name, color, points)],
            markers: frame.markers,
            references: Vec::new(),
            format: info.format,
            smooth: frame.smooth,
            include_zero: false,
        },
    );
    ui.add_space(4.0);
}

fn visibility_key(visibility: Visibility) -> &'static str {
    match visibility {
        Visibility::Observable => "observable",
        Visibility::Truth => "truth",
    }
}

/// A button that opens the list of one side's metrics, each a checkbox.
fn metric_picker(ui: &mut Ui, visibility: Visibility, picks: &mut Vec<MetricId>) {
    ui.menu_button(format!("Choose metrics ({})", picks.len()), |ui| {
        egui::ScrollArea::vertical()
            .max_height(420.0)
            .id_salt(("picker", visibility_key(visibility)))
            .show(ui, |ui| {
                for family in FAMILIES {
                    let infos: Vec<_> = metric_infos()
                        .iter()
                        .filter(|i| i.visibility == visibility && i.family == family)
                        .filter(|i| !i.id.needs_experiment())
                        .collect();
                    if infos.is_empty() {
                        continue;
                    }
                    ui.label(RichText::new(family_name(family)).strong());
                    for info in infos {
                        let mut on = picks.contains(&info.id);
                        if ui
                            .checkbox(&mut on, info.name)
                            .on_hover_text(info.description)
                            .changed()
                        {
                            if on {
                                picks.push(info.id);
                            } else {
                                picks.retain(|m| *m != info.id);
                            }
                        }
                    }
                }
            });
    });
}

/// The breakdowns: what gets posted, what gets shown, and who is well. The
/// lines of each chart share one scale.
fn breakdowns(ui: &mut Ui, frame: &Frame<'_>) {
    ui.label(
        RichText::new(
            "Posts by style is what people write (they learn what pays); impressions by style is \
             what the feed shows; wellbeing by archetype is how current users of each kind fare.",
        )
        .small(),
    );
    let history = frame.history;
    let style_series = |points: &dyn Fn(usize) -> Vec<[f64; 2]>| -> Vec<Series> {
        (0..STYLE_COUNT)
            .map(|s| Series::new(Style::ALL[s].name(), palette::STYLES[s], points(s)))
            .collect()
    };
    let posts = style_series(&|s| metric_points(history, MetricId::post_share(Style::ALL[s])));
    let shown = style_series(&|s| impression_points(history, s));
    let wellbeing: Vec<Series> = Archetype::ALL
        .iter()
        .map(|a| {
            Series::new(
                a.name(),
                palette::ARCHETYPES[a.index()],
                wellbeing_points(history, a.index()),
            )
        })
        .collect();
    ui.columns(3, |cols| {
        for ((col, (title, series, format)), key) in cols
            .iter_mut()
            .zip([
                ("Posts by style (truth)", posts, Format::Share),
                ("Impressions by style (truth)", shown, Format::Share),
                ("Wellbeing by archetype (truth)", wellbeing, Format::Score),
            ])
            .zip(["posts", "impressions", "wellbeing"])
        {
            charts::line_chart(
                col,
                Chart {
                    id: Id::new(("breakdown", key)),
                    title: Some(title),
                    height: 180.0,
                    day: frame.day,
                    series,
                    markers: frame.markers,
                    references: Vec::new(),
                    format,
                    smooth: frame.smooth,
                    include_zero: format == Format::Share,
                },
            );
        }
    });
}
