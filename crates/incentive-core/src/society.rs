//! Phase 13: polarisation drift and spillover to non-users (docs/model.md, 8.1
//! and 8.2).

use crate::rng::{self, Stream};
use crate::session::MISBELIEF_LOSS;
use crate::world::{DayCtx, World};

/// Daily drift of affective polarisation toward its baseline.
const POLARISATION_DRIFT: f32 = 0.002;
/// Daily drift of informedness toward its baseline: forgetting, and news from
/// outside the platform. The specification gives informedness no drift;
/// without one it runs to its bounds (recorded in docs/model.md, section 13).
const INFORMEDNESS_DRIFT: f32 = 0.01;
/// Baseline informedness.
pub(crate) const INFORMEDNESS_BASELINE: f32 = 0.6;
/// Chance per credulity that a tie's misinformation share reaches a non-user.
const SPILLOVER_EXPOSURE: f32 = 0.05;

/// Runs the society phase.
pub(crate) fn run(world: &mut World, ctx: &DayCtx) {
    for person in &mut world.people {
        let s = &mut person.state;
        s.affective_polarisation +=
            POLARISATION_DRIFT * (person.traits.polarisation_baseline - s.affective_polarisation);
        s.informedness += INFORMEDNESS_DRIFT * (INFORMEDNESS_BASELINE - s.informedness);
    }
    let pulls = spillover_targets(world);
    let rate = ctx.config.world.spillover_rate;
    let credulity_scale = ctx.config.world.credulity_scale;
    for (j, pull) in pulls.into_iter().enumerate() {
        let Some(pull) = pull else { continue };
        let mut rng = ctx.rng(j as u64, Stream::Spillover);
        let person = &mut world.people[j];
        let credulity = person.traits.credulity;
        let s = &mut person.state;
        s.affective_polarisation += rate * (pull.polarisation - s.affective_polarisation);
        s.informedness += rate * (pull.informedness - s.informedness);
        for &(truth, noted) in &pull.misinformation {
            if !rng::chance(&mut rng, SPILLOVER_EXPOSURE * credulity) {
                continue;
            }
            let believe = credulity * credulity_scale * if noted { 0.3 } else { 1.0 };
            if rng::chance(&mut rng, believe) {
                s.informedness -= MISBELIEF_LOSS * (0.5 - truth);
            }
        }
        s.informedness = s.informedness.clamp(0.0, 1.0);
        s.affective_polarisation = s.affective_polarisation.clamp(0.0, 1.0);
    }
}

/// What a non-user's user ties pull it toward.
struct Pull {
    polarisation: f32,
    informedness: f32,
    misinformation: Vec<(f32, bool)>,
}

/// For each non-user with user ties, the mean attitudes of those ties and the
/// misinformation they shared today; `None` for users and the unconnected.
fn spillover_targets(world: &World) -> Vec<Option<Pull>> {
    world
        .people
        .iter()
        .enumerate()
        .map(|(j, person)| {
            if person.state.is_user {
                return None;
            }
            let mut count = 0;
            let mut pull = Pull {
                polarisation: 0.0,
                informedness: 0.0,
                misinformation: Vec::new(),
            };
            for &t in &world.ties[j] {
                let tie = &world.people[t as usize].state;
                if !tie.is_user {
                    continue;
                }
                count += 1;
                pull.polarisation += tie.affective_polarisation;
                pull.informedness += tie.informedness;
                pull.misinformation
                    .extend_from_slice(&tie.today.misinformation_shares);
            }
            (count > 0).then(|| {
                pull.polarisation /= count as f32;
                pull.informedness /= count as f32;
                pull
            })
        })
        .collect()
}
