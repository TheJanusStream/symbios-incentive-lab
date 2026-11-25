//! The lab's views: plain functions over an egui [`Ui`] and the [`Lab`]. The
//! Bevy layer calls [`draw`] once a frame; the headless tests call [`show`]
//! inside `egui::Context::run_ui`.

pub mod about;
pub mod atlas;
pub mod charts;
pub mod compare;
pub mod inspector;
pub mod levers;
pub mod sandbox;
pub mod scenarios;

use egui::{Color32, Context, LayerId, RichText, Ui, UiBuilder};

use crate::lab::{Lab, Speed, Tab};

/// Colour-blind-safe colours (Okabe and Ito), brightened where needed for a
/// dark background.
pub mod palette {
    use egui::Color32;

    /// Sky blue.
    pub const SKY: Color32 = Color32::from_rgb(0x56, 0xB4, 0xE9);
    /// Orange.
    pub const ORANGE: Color32 = Color32::from_rgb(0xE6, 0x9F, 0x00);
    /// Bluish green.
    pub const GREEN: Color32 = Color32::from_rgb(0x00, 0x9E, 0x73);
    /// Yellow.
    pub const YELLOW: Color32 = Color32::from_rgb(0xF0, 0xE4, 0x42);
    /// Blue, lightened.
    pub const BLUE: Color32 = Color32::from_rgb(0x33, 0x8E, 0xD6);
    /// Vermillion.
    pub const VERMILLION: Color32 = Color32::from_rgb(0xD5, 0x5E, 0x00);
    /// Reddish purple.
    pub const PURPLE: Color32 = Color32::from_rgb(0xCC, 0x79, 0xA7);
    /// Neutral grey.
    pub const GREY: Color32 = Color32::from_rgb(0xA0, 0xA0, 0xA0);

    /// Lines of one chart, in order.
    pub const SERIES: [Color32; 8] = [SKY, ORANGE, GREEN, YELLOW, BLUE, VERMILLION, PURPLE, GREY];
    /// Design A in the compare mode, and the player ("You") in a scenario.
    pub const SIDE_A: Color32 = SKY;
    /// Design B in the compare mode.
    pub const SIDE_B: Color32 = ORANGE;
    /// The baseline in a scenario: the orange of [`SIDE_B`], muted, so the
    /// player's line drawn over it stands out.
    pub const BASELINE_MUTED: Color32 = Color32::from_rgb(0x8C, 0x6E, 0x3A);
    /// What the platform can observe.
    pub const OBSERVABLE: Color32 = SKY;
    /// What only the god's-eye view knows.
    pub const TRUTH: Color32 = YELLOW;
    /// The six styles: Informative, Entertaining, Personal, Outrage,
    /// Clickbait, Attack.
    pub const STYLES: [Color32; 6] = [SKY, GREEN, YELLOW, VERMILLION, ORANGE, PURPLE];
    /// The five archetypes: Casual, Creator, Partisan, Troll, Vulnerable.
    pub const ARCHETYPES: [Color32; 5] = [SKY, GREEN, ORANGE, VERMILLION, PURPLE];
    /// Background of something that needs the player's attention.
    pub const ATTENTION_FILL: Color32 = Color32::from_rgb(0x8A, 0x5A, 0x00);
    /// Background of a feed row that shows the Goodhart gap.
    pub const GAP_FILL: Color32 = Color32::from_rgba_premultiplied(0x6E, 0x2F, 0x00, 0xC0);
    /// A passed goal.
    pub const PASS: Color32 = GREEN;
    /// A failed goal.
    pub const FAIL: Color32 = VERMILLION;
}

/// Draws the whole lab into the context's full viewport.
pub fn draw(ctx: &Context, lab: &mut Lab) {
    let mut root = Ui::new(
        ctx.clone(),
        "symbios-incentive-lab".into(),
        UiBuilder::new()
            .layer_id(LayerId::background())
            .max_rect(ctx.viewport_rect()),
    );
    show(&mut root, lab);
}

/// Draws the whole lab into `ui`: the top bar, then the active tab.
pub fn show(ui: &mut Ui, lab: &mut Lab) {
    egui::Panel::top("top-bar").show(ui, |ui| top_bar(ui, lab));
    match lab.tab {
        Tab::Sandbox => sandbox::show(ui, lab),
        Tab::Compare => compare::show(ui, lab),
        Tab::Scenarios => scenarios::show(ui, lab),
        Tab::Atlas => atlas::show(ui, lab),
        Tab::About => {
            egui::CentralPanel::default().show(ui, about::show);
        }
    }
}

/// The top bar: tabs, run controls, day counter and messages.
pub fn top_bar(ui: &mut Ui, lab: &mut Lab) {
    ui.add_space(2.0);
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Symbios Incentive Lab").strong().size(16.0));
        ui.separator();
        for tab in Tab::ALL {
            ui.selectable_value(&mut lab.tab, tab, tab.name());
        }
        ui.separator();
        let has_run = lab.active_runner().is_some();
        let finished = lab.active_runner().is_none_or(|r| r.is_finished());
        ui.add_enabled_ui(has_run, |ui| {
            let play_label = if lab.playing { "Pause" } else { "Play" };
            if ui
                .add_enabled(!finished || lab.playing, egui::Button::new(play_label))
                .on_hover_text("Run day after day at the chosen speed")
                .clicked()
            {
                lab.playing = !lab.playing;
            }
            if ui
                .add_enabled(!finished, egui::Button::new("Step"))
                .on_hover_text("Simulate one day")
                .clicked()
            {
                lab.playing = false;
                lab.step_once();
            }
            egui::ComboBox::from_id_salt("speed")
                .selected_text(lab.speed.label())
                .width(90.0)
                .show_ui(ui, |ui| {
                    for speed in Speed::ALL {
                        ui.selectable_value(&mut lab.speed, speed, speed.label());
                    }
                })
                .response
                .on_hover_text(
                    "Days per second; Max runs as many as each frame's time budget allows",
                );
            restart_button(ui, lab);
        });
        ui.label("Seed");
        // A loaded configuration may carry a larger seed: keep it as it is.
        ui.add(
            egui::DragValue::new(&mut lab.run.seed)
                .range(0..=u64::from(u32::MAX))
                .clamp_existing_to_range(false),
        )
        .on_hover_text(
            "Seed of every random draw. The same seed and settings give the same run; a \
                 change applies at the next restart.",
        );
        ui.separator();
        match lab.active_runner() {
            Some(r) => ui.label(RichText::new(format!("Day {} of {}", r.day(), r.days())).strong()),
            None => ui.label(RichText::new("No run on this tab").weak()),
        };
        if has_run {
            ui.separator();
            ui.checkbox(&mut lab.view.smooth, "Smooth (7-day mean)")
                .on_hover_text(
                    "Draw every daily chart as the mean of the last 7 days, which hides the \
                     day-to-day noise. Tables and verdicts always use the raw values.",
                );
        }
        if lab.tab == Tab::Sandbox {
            ui.separator();
            ui.toggle_value(&mut lab.inspector.open, "Inspector")
                .on_hover_text("Follow individuals and see the feed they were served");
        }
        if let Some(status) = &lab.status {
            ui.separator();
            let warn = ui.visuals().warn_fg_color;
            ui.colored_label(warn, status);
        }
    });
    ui.add_space(2.0);
}

/// The restart button; highlighted as "Restart to apply" when a change waits
/// for it.
pub fn restart_button(ui: &mut Ui, lab: &mut Lab) {
    let pending = lab.needs_restart();
    let button = if pending {
        egui::Button::new(
            RichText::new("Restart to apply")
                .strong()
                .color(Color32::WHITE),
        )
        .fill(palette::ATTENTION_FILL)
    } else {
        egui::Button::new("Restart")
    };
    let hover = if pending {
        "World assumptions, population, length or seed changed: restart from day 0 to apply them"
    } else {
        "Start this run again from day 0"
    };
    if ui.add(button).on_hover_text(hover).clicked() {
        lab.restart();
    }
}

/// A small coloured tag: "Platform sees" or "True".
pub fn visibility_tag(ui: &mut Ui, visibility: incentive_core::Visibility) {
    let (text, color) = match visibility {
        incentive_core::Visibility::Observable => ("PLATFORM SEES", palette::OBSERVABLE),
        incentive_core::Visibility::Truth => ("TRUE", palette::TRUTH),
    };
    ui.label(RichText::new(text).small().strong().color(color))
        .on_hover_text(match visibility {
            incentive_core::Visibility::Observable => {
                "The platform can measure this, and its ranking, moderation and payouts use only such numbers"
            }
            incentive_core::Visibility::Truth => {
                "Only the simulation knows this: the platform never sees it"
            }
        });
}
