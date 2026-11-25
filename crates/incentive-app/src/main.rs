//! `symbios-incentive-lab`: the windowed lab for the desktop and the browser.
//! A thin Bevy layer around the library: one resource holding the [`Lab`], a
//! system that advances it within a per-frame time budget, and one system
//! that draws its egui views.

use bevy::platform::time::Instant;
use bevy::prelude::*;
use bevy::window::WindowResolution;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass};
use core::time::Duration;
use symbios_incentive_lab::lab::Lab;

/// Simulation time per frame. Each loop runs at least one due day, then
/// stops once the budget is spent, so the window stays responsive even when
/// one simulated day takes longer than a frame.
const FRAME_BUDGET: Duration = Duration::from_millis(10);

/// The lab, as a Bevy resource.
#[derive(Resource)]
struct LabState(Lab);

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    let plan = match screenshot::Plan::from_args(std::env::args().skip(1)) {
        Ok(plan) => plan,
        Err(message) => {
            eprintln!("error: {message}\n\n{}", screenshot::USAGE);
            std::process::exit(2);
        }
    };
    #[cfg(not(target_arch = "wasm32"))]
    let lab = plan
        .as_ref()
        .map_or_else(Lab::default, screenshot::Plan::lab);
    #[cfg(target_arch = "wasm32")]
    let lab = Lab::default();

    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.08, 0.08, 0.09)))
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Symbios Incentive Lab".into(),
                    resolution: WindowResolution::new(1600, 900),
                    ..default()
                }),
                ..default()
            }),
            EguiPlugin::default(),
        ))
        .insert_resource(LabState(lab))
        .add_systems(Startup, spawn_camera)
        .add_systems(Update, step_lab)
        .add_systems(EguiPrimaryContextPass, draw_lab);

    #[cfg(not(target_arch = "wasm32"))]
    if let Some(plan) = plan {
        screenshot::install(plan, &mut app);
    }

    app.run();
}

/// egui draws on the first camera.
fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

/// Advances the lab within the frame budget.
fn step_lab(time: Res<Time>, mut lab: ResMut<LabState>) {
    let start = Instant::now();
    lab.0
        .frame(time.delta_secs(), &mut || start.elapsed() < FRAME_BUDGET);
}

/// Draws the lab's views.
fn draw_lab(mut contexts: EguiContexts, mut lab: ResMut<LabState>) -> Result {
    let ctx = contexts.ctx_mut()?;
    symbios_incentive_lab::ui::draw(ctx, &mut lab.0);
    Ok(())
}

/// The capture of `--screenshot` (the plan itself lives in the library's
/// `screenshot` module). Native only: a browser has no file to write to.
#[cfg(not(target_arch = "wasm32"))]
mod screenshot {
    use std::path::PathBuf;

    use bevy::prelude::*;
    use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
    pub use symbios_incentive_lab::screenshot::{Plan, USAGE};

    /// Frames to wait before capturing, so the window is up and egui has
    /// laid out every panel.
    const WARM_UP_FRAMES: u32 = 30;

    /// The capture still to take.
    #[derive(Resource)]
    struct Pending {
        path: PathBuf,
        frames: u32,
        taken: bool,
    }

    /// Adds the capture of `plan` to the app.
    pub fn install(plan: Plan, app: &mut App) {
        app.insert_resource(Pending {
            path: plan.path,
            frames: 0,
            taken: false,
        })
        .add_systems(Update, capture);
    }

    /// Captures the window after the warm-up; the app exits once the PNG is
    /// written.
    fn capture(mut commands: Commands, mut pending: ResMut<Pending>) {
        if pending.taken {
            return;
        }
        pending.frames += 1;
        if pending.frames < WARM_UP_FRAMES {
            return;
        }
        pending.taken = true;
        let path = pending.path.clone();
        let mut save = save_to_disk(path.clone());
        commands.spawn(Screenshot::primary_window()).observe(
            move |captured: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                save(captured);
                info!("screenshot saved to {}", path.display());
                exit.write(AppExit::Success);
            },
        );
    }
}
