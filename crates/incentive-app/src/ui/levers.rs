//! The lever panels: design levers and world assumptions built from
//! `params()` metadata, preset pickers, run settings and configuration
//! copy and load.

use egui::{RichText, Ui};
use incentive_core::{Config, ParamGroup, ParamInfo, ParamSide, presets};

use crate::lab::Lab;
use crate::lab::levers::{self, Control, LeverValue};

/// Draws the controls of one side's parameters, grouped, editing `config`.
/// Returns true when a value changed. With `enabled` false the controls are
/// shown but locked.
pub fn param_controls(
    ui: &mut Ui,
    config: &mut Config,
    side: ParamSide,
    enabled: bool,
    salt: &str,
) -> bool {
    let tree = levers::tree(config);
    let mut changed = false;
    ui.add_enabled_ui(enabled, |ui| {
        for (group, list) in levers::grouped(side) {
            egui::CollapsingHeader::new(group.name())
                .id_salt((salt, group.name()))
                .default_open(side == ParamSide::Design || group == ParamGroup::Appeal)
                .show(ui, |ui| {
                    egui::Grid::new((salt, group.name(), "grid"))
                        .num_columns(2)
                        .spacing([8.0, 4.0])
                        .show(ui, |ui| {
                            for info in list {
                                changed |= param_row(ui, config, &tree, info, salt);
                                ui.end_row();
                            }
                        });
                });
        }
    });
    changed
}

/// One parameter: its name with the description on hover, and its widget.
fn param_row(
    ui: &mut Ui,
    config: &mut Config,
    tree: &serde_json::Value,
    info: &'static ParamInfo,
    salt: &str,
) -> bool {
    ui.label(info.name).on_hover_text(info.description);
    let Some(value) = levers::read(tree, info) else {
        ui.label(RichText::new("unavailable").weak());
        return false;
    };
    let new_value = match (levers::control_for(info.kind), value) {
        (Control::Slider { min, max, step }, LeverValue::Number(mut x)) => {
            let r = ui.add(
                egui::Slider::new(&mut x, min..=max)
                    .step_by(f64::from(step))
                    .fixed_decimals(levers::decimals_for(step)),
            );
            r.on_hover_text(info.description)
                .changed()
                .then_some(LeverValue::Number(x))
        }
        (Control::Drag { min, max }, LeverValue::Integer(mut n)) => {
            let r = ui.add(egui::DragValue::new(&mut n).range(min..=max));
            r.on_hover_text(info.description)
                .changed()
                .then_some(LeverValue::Integer(n))
        }
        (Control::OptionalDrag { min, max }, LeverValue::Optional(current)) => {
            ui.horizontal(|ui| {
                let mut on = current.is_some();
                let mut n = current.unwrap_or(min);
                let toggled = ui
                    .checkbox(&mut on, "limit")
                    .on_hover_text(info.description)
                    .changed();
                let dragged = ui
                    .add_enabled(on, egui::DragValue::new(&mut n).range(min..=max))
                    .on_hover_text(info.description)
                    .changed();
                (toggled || dragged).then_some(LeverValue::Optional(on.then_some(n)))
            })
            .inner
        }
        (Control::Checkbox, LeverValue::Toggle(mut b)) => ui
            .checkbox(&mut b, "")
            .on_hover_text(info.description)
            .changed()
            .then_some(LeverValue::Toggle(b)),
        (Control::Combo(choices), LeverValue::Choice(id)) => {
            let selected = choices
                .iter()
                .find(|c| c.id == id)
                .map_or(id.as_str(), |c| c.name);
            let mut picked = None;
            egui::ComboBox::from_id_salt((salt, info.path))
                .selected_text(selected)
                .show_ui(ui, |ui| {
                    for c in choices {
                        if ui.selectable_label(c.id == id, c.name).clicked() && c.id != id {
                            picked = Some(LeverValue::Choice(c.id.to_string()));
                        }
                    }
                })
                .response
                .on_hover_text(info.description);
            picked
        }
        _ => {
            ui.label(RichText::new("unsupported").weak());
            None
        }
    };
    new_value.is_some_and(|v| levers::write(config, info, &v).is_ok())
}

/// A preset picker for a design; returns the picked preset's index.
pub fn preset_picker(ui: &mut Ui, salt: &str, current: Option<usize>) -> Option<usize> {
    let text = current.map_or("Custom", |i| presets()[i].name);
    let mut picked = None;
    ui.horizontal(|ui| {
        ui.label("Preset");
        egui::ComboBox::from_id_salt((salt, "preset"))
            .selected_text(text)
            .width(200.0)
            .show_ui(ui, |ui| {
                for (i, p) in presets().iter().enumerate() {
                    if ui
                        .selectable_label(current == Some(i), p.name)
                        .on_hover_text(p.description)
                        .clicked()
                    {
                        picked = Some(i);
                    }
                }
            })
            .response
            .on_hover_text(
                "A ready-made platform design. Picking one replaces every design lever, from the \
                 next day on.",
            );
    });
    picked
}

/// The preset whose design matches, if any.
pub fn preset_index(design: &incentive_core::DesignLevers) -> Option<usize> {
    presets().iter().position(|p| p.design() == *design)
}

/// Population and run length (and a note that the seed sits in the top
/// bar). With `days_fixed` the length shows as fixed.
pub fn run_settings(ui: &mut Ui, lab: &mut Lab, days_fixed: Option<u32>) {
    ui.label(RichText::new("Run").strong());
    egui::Grid::new("run-settings")
        .num_columns(2)
        .show(ui, |ui| {
            ui.label("Population")
                .on_hover_text("Individuals in the society, users and non-users");
            ui.add(
                // Every frame simulates at least one day, so the size is
                // capped where a day still fits a frame's budget in release.
                egui::DragValue::new(&mut lab.run.population)
                    .range(50..=10_000)
                    .clamp_existing_to_range(false)
                    .speed(10.0),
            )
            .on_hover_text("More people give smoother curves and slower days");
            ui.end_row();
            ui.label("Run length").on_hover_text("Days to simulate");
            match days_fixed {
                Some(days) => ui.label(format!("{days} days, fixed by the scenario")),
                None => ui.add(
                    egui::DragValue::new(&mut lab.run.days)
                        .range(1..=3_650)
                        .clamp_existing_to_range(false)
                        .suffix(" days"),
                ),
            };
            ui.end_row();
        });
    ui.label(
        RichText::new("Population, length and seed apply at the next restart.")
            .small()
            .weak(),
    );
}

/// "World assumptions", collapsed: assumptions about people, editable or
/// locked. Returns true when one changed.
pub fn world_section(ui: &mut Ui, config: &mut Config, enabled: bool, salt: &str) -> bool {
    let title = if enabled {
        "World assumptions"
    } else {
        "World assumptions (locked to the scenario)"
    };
    egui::CollapsingHeader::new(title)
        .id_salt((salt, "world"))
        .default_open(false)
        .show(ui, |ui| {
            ui.label(
                RichText::new(
                    "Assumptions about people, not choices of the platform: how strongly outrage \
                     grabs attention, how much harassment hurts, how fast authors learn. Test \
                     whether a conclusion survives a different world.",
                )
                .italics(),
            );
            if enabled {
                ui.label(
                    RichText::new("A change here applies at the next restart.")
                        .small()
                        .weak(),
                );
            }
            param_controls(ui, config, ParamSide::World, enabled, salt)
        })
        .body_returned
        .unwrap_or(false)
}

/// "Copy configuration": puts the configuration on the clipboard as JSON.
pub fn copy_config_button(ui: &mut Ui, config: &Config) -> bool {
    let clicked = ui
        .button("Copy configuration")
        .on_hover_text(
            "Copies this configuration as JSON. Save it to a file and run it on many seeds with \
             `incentive run --config file.json`.",
        )
        .clicked();
    if clicked {
        ui.ctx().copy_text(config.to_json_pretty());
    }
    clicked
}

/// The sandbox's left panel: "Your platform".
pub fn sandbox_panel(ui: &mut Ui, lab: &mut Lab) {
    ui.heading("Your platform");
    ui.label(
        RichText::new(
            "Design levers are the platform's choices. A change applies from the next day.",
        )
        .small(),
    );
    if let Some(i) = preset_picker(ui, "sandbox", lab.sandbox.preset_index()) {
        lab.sandbox.draft.design = presets()[i].design();
        lab.apply_sandbox_design(Some(format!("Preset: {}", presets()[i].name)));
    }
    ui.separator();
    if param_controls(
        ui,
        &mut lab.sandbox.draft,
        ParamSide::Design,
        true,
        "sandbox",
    ) {
        lab.apply_sandbox_design(None);
    }
    ui.separator();
    world_section(ui, &mut lab.sandbox.draft, true, "sandbox");
    ui.separator();
    run_settings(ui, lab, None);
    if lab.needs_restart() {
        super::restart_button(ui, lab);
    }
    ui.separator();
    ui.label(RichText::new("Configuration").strong());
    let planned = lab.sandbox.planned(lab.run);
    if copy_config_button(ui, &planned) {
        lab.sandbox.message = Some("Copied to the clipboard as JSON.".into());
    }
    egui::CollapsingHeader::new("Load configuration")
        .id_salt("sandbox-load")
        .show(ui, |ui| {
            ui.label(
                RichText::new(
                    "Paste configuration JSON (from Copy configuration or `incentive config`). \
                     Missing fields take their defaults; loading restarts the run.",
                )
                .small(),
            );
            ui.add(
                egui::TextEdit::multiline(&mut lab.sandbox.load_text)
                    .desired_rows(5)
                    .desired_width(f32::INFINITY)
                    .code_editor(),
            );
            if ui.button("Load").clicked() {
                let text = lab.sandbox.load_text.clone();
                lab.load_sandbox_config(&text);
            }
        });
    if let Some(message) = &lab.sandbox.message {
        ui.label(RichText::new(message).small());
    }
}

/// The compare mode's left panel: the edited design, the shared world.
pub fn compare_panel(ui: &mut Ui, lab: &mut Lab) {
    ui.heading("Two designs");
    ui.horizontal(|ui| {
        ui.label("Editing:");
        ui.selectable_value(
            &mut lab.compare.editing,
            0,
            RichText::new("A").strong().color(super::palette::SIDE_A),
        );
        ui.selectable_value(
            &mut lab.compare.editing,
            1,
            RichText::new("B").strong().color(super::palette::SIDE_B),
        );
    });
    let side = lab.compare.editing.min(1);
    let salt = if side == 0 { "compare-a" } else { "compare-b" };
    if let Some(i) = preset_picker(ui, salt, preset_index(&lab.compare.drafts[side].design)) {
        lab.compare.drafts[side].design = presets()[i].design();
        lab.apply_compare_design(Some(format!("Preset: {}", presets()[i].name)));
    }
    ui.separator();
    if param_controls(
        ui,
        &mut lab.compare.drafts[side],
        ParamSide::Design,
        true,
        salt,
    ) {
        lab.apply_compare_design(None);
    }
    ui.separator();
    ui.label(RichText::new("Both designs face the same world.").small());
    if world_section(ui, &mut lab.compare.drafts[side], true, "compare-world") {
        lab.compare.share_world();
    }
    ui.separator();
    run_settings(ui, lab, None);
    if lab.needs_restart() {
        super::restart_button(ui, lab);
    }
    ui.separator();
    let planned = lab.run.apply(lab.compare.drafts[side].clone());
    copy_config_button(ui, &planned);
}

/// The scenario's left panel: the player's design; the world is locked.
pub fn scenario_panel(ui: &mut Ui, lab: &mut Lab) {
    let Some(days) = lab.scenarios.play.as_ref().map(|p| p.runner.days()) else {
        return;
    };
    ui.heading("Your design");
    ui.label(
        RichText::new(
            "The baseline keeps the scenario's starting design. Yours starts there too; every \
             change applies from the next day.",
        )
        .small(),
    );
    let mut changed = false;
    let mut locked = None;
    if let Some(play) = lab.scenarios.play.as_mut() {
        changed = param_controls(ui, &mut play.draft, ParamSide::Design, true, "scenario");
        locked = Some(play.draft.clone());
    }
    if changed {
        lab.apply_scenario_design(None);
    }
    ui.separator();
    if let Some(mut world) = locked.clone() {
        world_section(ui, &mut world, false, "scenario-world");
    }
    ui.separator();
    run_settings(ui, lab, Some(days));
    if lab.needs_restart() {
        super::restart_button(ui, lab);
    }
    ui.separator();
    if let Some(config) = locked {
        copy_config_button(ui, &config);
    }
}
