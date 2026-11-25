//! The incentive lab's game app, as a library: the lab's state and logic in
//! [`lab`] and its egui views in [`ui`]. Neither touches Bevy, so the views
//! run headless in tests; the `symbios-incentive-lab` binary wraps them in a
//! thin Bevy layer for the desktop and the browser.
//!
//! ```
//! use symbios_incentive_lab::lab::{Lab, RunChoice};
//!
//! let mut lab = Lab::new(RunChoice { seed: 1, population: 200, days: 10 });
//! lab.step_once();
//! let ctx = egui::Context::default();
//! let output = ctx.run_ui(egui::RawInput::default(), |ui| {
//!     symbios_incentive_lab::ui::show(ui, &mut lab);
//! });
//! output.drop_without_applying_deltas();
//! assert_eq!(lab.sandbox.runner.day(), 1);
//! ```

pub mod lab;
pub mod screenshot;
pub mod ui;
