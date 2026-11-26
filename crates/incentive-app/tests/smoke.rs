//! Headless smoke tests: every tab and the inspector, drawn for a few frames
//! inside `egui::Context::run_ui` on a fixed screen, with no window or GPU.

use egui::{Context, RawInput, Rect, Vec2, pos2, vec2};
use symbios_incentive_lab::lab::{Lab, RunChoice, SUGGESTIONS, Tab, ViewFilter};
use symbios_incentive_lab::ui;

const SCREEN: Vec2 = vec2(1600.0, 900.0);

fn small_lab() -> Lab {
    Lab::new(RunChoice {
        seed: 1,
        population: 200,
        days: 20,
    })
}

/// Draws `n` frames of the whole lab on a screen of `size`.
fn frames_at(ctx: &Context, lab: &mut Lab, n: usize, size: Vec2) {
    for _ in 0..n {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), size)),
            ..Default::default()
        };
        // The check is that every view lays out and paints without a panic.
        let output = ctx.run_ui(input, |ui| ui::show(ui, lab));
        // No renderer takes the font texture here.
        output.drop_without_applying_deltas();
    }
}

fn frames(lab: &mut Lab, n: usize) {
    frames_at(&Context::default(), lab, n, SCREEN);
}

#[test]
fn the_sandbox_draws_before_and_after_days_and_design_changes() {
    let mut lab = small_lab();
    frames(&mut lab, 2);
    lab.run_days(4);
    frames(&mut lab, 2);
    lab.apply_suggestion(0);
    lab.run_days(3);
    assert_eq!(lab.sandbox.runner.lane(0).changes.len(), 1);
    for filter in [
        ViewFilter::PlatformOnly,
        ViewFilter::TruthOnly,
        ViewFilter::Both,
    ] {
        lab.view.filter = filter;
        frames(&mut lab, 2);
    }
    lab.view.start_here = false;
    lab.sandbox.draft.world.outrage_appeal = 2.5;
    assert!(lab.needs_restart());
    frames(&mut lab, 2);
    assert_eq!(SUGGESTIONS.len(), 3);
}

#[test]
fn the_inspector_draws_tracked_individuals_and_their_feeds() {
    let mut lab = small_lab();
    lab.inspector.open = true;
    frames(&mut lab, 2);
    lab.run_days(5);
    let tracked = lab.sandbox.runner.lane(0).sim.tracked();
    for id in tracked {
        lab.inspector.selected = Some(id);
        frames(&mut lab, 2);
    }
    assert!(
        lab.track_another(incentive_core::Archetype::Creator)
            .is_some()
    );
    frames(&mut lab, 2);
    lab.run_days(2);
    frames(&mut lab, 3);
}

#[test]
fn the_compare_tab_draws_live_and_during_and_after_the_seed_check() {
    let mut lab = small_lab();
    lab.tab = Tab::Compare;
    frames(&mut lab, 2);
    lab.run_days(5);
    lab.compare.editing = 1;
    lab.compare.drafts[1].design.w_bridging = 2.0;
    lab.apply_compare_design(None);
    lab.run_days(2);
    frames(&mut lab, 2);
    lab.run.days = 3;
    lab.restart();
    lab.start_seed_check();
    lab.frame(0.0, &mut || false);
    frames(&mut lab, 2);
    for _ in 0..1_000 {
        if lab.compare.check.as_ref().is_none_or(|c| c.rows.is_some()) {
            break;
        }
        lab.frame(0.0, &mut || true);
    }
    assert!(lab.compare.check.as_ref().is_some_and(|c| c.rows.is_some()));
    assert_eq!(
        lab.compare.check.as_ref().map(|c| c.seeds.clone()),
        Some(symbios_incentive_lab::lab::CHECK_SEEDS.to_vec())
    );
    frames(&mut lab, 3);
    lab.compare.show_components = true;
    frames(&mut lab, 2);
    lab.compare.drafts[0].design.w_like = 0.0;
    assert!(lab.compare.check_is_stale(lab.run));
    frames(&mut lab, 2);
}

#[test]
fn the_scenarios_tab_draws_the_list_a_run_its_verdict_and_the_confirmation() {
    let mut lab = small_lab();
    lab.tab = Tab::Scenarios;
    frames(&mut lab, 2);
    lab.scenarios.days_override = Some(6);
    for index in 0..incentive_core::scenarios().len() {
        lab.scenarios.selected = index;
        frames(&mut lab, 1);
        lab.start_scenario(index);
        lab.run_days(3);
        frames(&mut lab, 2);
    }
    lab.start_scenario(0);
    lab.run_days(10);
    assert!(
        lab.scenarios
            .play
            .as_ref()
            .is_some_and(|p| p.results.is_some())
    );
    frames(&mut lab, 2);
    lab.start_confirm();
    lab.frame(0.0, &mut || false);
    frames(&mut lab, 2);
    for _ in 0..1_000 {
        let running = lab
            .scenarios
            .play
            .as_ref()
            .and_then(|p| p.confirm.as_ref())
            .is_some_and(|c| c.results.is_none());
        if !running {
            break;
        }
        lab.frame(0.0, &mut || true);
    }
    let confirmed = lab
        .scenarios
        .play
        .as_ref()
        .and_then(|p| p.confirm.as_ref())
        .and_then(|c| c.results.as_ref())
        .map(|c| c.verdict.goals.len());
    let goals = lab
        .scenarios
        .play
        .as_ref()
        .map(|p| p.scenario().goals.len());
    assert!(goals.is_some_and(|g| g >= 2));
    assert_eq!(confirmed, goals, "every goal has an interval verdict");
    frames(&mut lab, 2);
    if let Some(play) = lab.scenarios.play.as_mut() {
        play.spoiler_open = true;
    }
    frames(&mut lab, 2);
    lab.load_known_solution();
    lab.run_days(2);
    frames(&mut lab, 3);
    lab.stop_scenario();
    frames(&mut lab, 2);
}

#[test]
fn the_atlas_tab_draws_every_view_with_and_without_a_pick() {
    use symbios_incentive_lab::lab::atlas::{AtlasPanel, AtlasPick, atlas};
    let data = atlas().expect("the embedded atlas parses");
    let mut lab = small_lab();
    lab.tab = Tab::Atlas;
    for panel in AtlasPanel::ALL {
        lab.atlas.panel = panel;
        lab.atlas.picked = None;
        frames(&mut lab, 2);
    }
    let cell = &data.map.cells[0];
    lab.atlas.panel = AtlasPanel::Map;
    lab.atlas.picked = Some(AtlasPick::Cell {
        x: cell.x,
        y: cell.y,
    });
    frames(&mut lab, 2);
    // A pick from the other view shows a hint, not a panic.
    lab.atlas.panel = AtlasPanel::Pareto;
    frames(&mut lab, 2);
    lab.atlas.picked = Some(AtlasPick::Front(data.pareto.front.len() - 1));
    frames(&mut lab, 2);
    // Narrow screens stack the views.
    frames_at(&Context::default(), &mut lab, 2, vec2(700.0, 900.0));
    lab.atlas.panel = AtlasPanel::Goodhart;
    frames_at(&Context::default(), &mut lab, 2, vec2(700.0, 900.0));
    lab.open_in_sandbox(
        data.pareto.front[0].design.clone(),
        &data.pareto.settings.base,
    );
    frames(&mut lab, 2);
    lab.compare_with_engagement_ranking(cell.elite.design.clone(), &data.map.settings.base);
    frames(&mut lab, 2);
}

#[test]
fn the_about_tab_draws() {
    let mut lab = small_lab();
    lab.tab = Tab::About;
    frames(&mut lab, 3);
}

#[test]
fn every_tab_draws_on_a_small_screen_and_through_the_context_entry_point() {
    let mut lab = small_lab();
    lab.inspector.open = true;
    lab.run_days(3);
    let ctx = Context::default();
    for tab in Tab::ALL {
        lab.tab = tab;
        frames_at(&ctx, &mut lab, 2, vec2(800.0, 600.0));
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), SCREEN)),
            ..Default::default()
        };
        let output = ctx.run_ui(input, |ui| ui::draw(ui.ctx(), &mut lab));
        assert!(!output.shapes.is_empty());
        output.drop_without_applying_deltas();
    }
}

#[test]
fn every_chart_draws_with_and_without_smoothing() {
    let mut lab = small_lab();
    lab.inspector.open = true;
    lab.run_days(12);
    lab.apply_suggestion(1);
    lab.run_days(3);
    assert!(lab.view.smooth, "smoothing is on by default");
    for smooth in [true, false] {
        lab.view.smooth = smooth;
        lab.tab = Tab::Sandbox;
        // Many picked metrics on one side: two columns of small charts.
        lab.view.filter = ViewFilter::TruthOnly;
        frames(&mut lab, 2);
        lab.view.filter = ViewFilter::Both;
        frames(&mut lab, 2);
        lab.tab = Tab::Compare;
        lab.run_days(4);
        frames(&mut lab, 2);
        lab.tab = Tab::Scenarios;
        lab.scenarios.days_override = Some(8);
        lab.start_scenario(1);
        lab.run_days(5);
        frames(&mut lab, 2);
    }
}
