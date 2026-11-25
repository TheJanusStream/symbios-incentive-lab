//! The genome of a platform design: every design lever as genes in `[0, 1]`,
//! decoded through the lever's [`ParamKind`].
//!
//! - A range is interpolated between its ends and snapped to its step.
//! - An integer is interpolated and rounded.
//! - An optional integer has two genes: on at 0.5 or above, then the value.
//! - A toggle is on at 0.5 or above.
//! - A choice of `n` options splits `[0, 1]` into `n` equal buckets.
//!
//! Every genome decodes to a design that passes [`Config::validate`], so the
//! searches never waste a run on an invalid design.

use incentive_core::{Config, ConfigError, DesignLevers, ParamInfo, ParamKind, ParamSide, params};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};
use symbios_genetics::Genotype;

/// Chance that a mutated gene is redrawn uniformly instead of nudged, so
/// toggles and choices can jump and the search can leave a basin.
const RESET_CHANCE: f32 = 0.2;
/// Standard deviation of a nudge.
const NUDGE_SD: f32 = 0.12;

/// One lever's place in the genome.
#[derive(Clone, Copy, Debug)]
struct Slot {
    info: &'static ParamInfo,
    /// The field name inside `design`.
    key: &'static str,
    /// Index of the lever's first gene.
    offset: usize,
}

/// Where every design lever sits in the genome.
#[derive(Clone, Debug)]
pub struct Layout {
    slots: Vec<Slot>,
    len: usize,
}

impl Layout {
    /// The layout of every design lever of [`params`], in catalogue order.
    pub fn new() -> Self {
        let mut slots = Vec::new();
        let mut offset = 0;
        for info in params().iter().filter(|p| p.side == ParamSide::Design) {
            let key = info.path.strip_prefix("design.").unwrap_or(info.path);
            slots.push(Slot { info, key, offset });
            offset += genes_of(info.kind);
        }
        Self { slots, len: offset }
    }

    /// Genes in a genome.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.len
    }

    /// A genome of uniform random genes.
    pub fn random<R: Rng>(&self, rng: &mut R) -> Genome {
        Genome((0..self.len).map(|_| rng.random::<f32>()).collect())
    }

    /// The design a genome stands for. Genes outside `[0, 1]` are clamped.
    pub fn decode(&self, genome: &Genome) -> Result<DesignLevers, ConfigError> {
        let gene = |i: usize| genome.0.get(i).copied().unwrap_or(0.5).clamp(0.0, 1.0);
        let mut map = Map::new();
        for slot in &self.slots {
            let g = gene(slot.offset);
            let value = match slot.info.kind {
                ParamKind::Range { min, max, step } => number(snap(g, min, max, step)),
                ParamKind::Integer { min, max } => Value::from(interpolate_int(g, min, max)),
                ParamKind::OptionalInteger { min, max } => {
                    if g >= 0.5 {
                        Value::from(interpolate_int(gene(slot.offset + 1), min, max))
                    } else {
                        Value::Null
                    }
                }
                ParamKind::Toggle => Value::Bool(g >= 0.5),
                ParamKind::Choice(choices) => {
                    let n = choices.len().max(1);
                    let i = ((g * n as f32).floor() as usize).min(n - 1);
                    Value::from(choices.get(i).map_or("", |c| c.id))
                }
            };
            map.insert(slot.key.to_string(), value);
        }
        serde_json::from_value(Value::Object(map)).map_err(|e| ConfigError::Json(e.to_string()))
    }

    /// A genome that decodes to `design` (exactly, for any design whose
    /// values lie on their levers' steps, as every preset's do).
    pub fn encode(&self, design: &DesignLevers) -> Result<Genome, ConfigError> {
        let tree = serde_json::to_value(design).map_err(|e| ConfigError::Json(e.to_string()))?;
        let mut genes = vec![0.5; self.len];
        for slot in &self.slots {
            let value = tree
                .get(slot.key)
                .ok_or_else(|| ConfigError::UnknownPath(slot.info.path.to_string()))?;
            let unit = |x: f64, min: f64, max: f64| -> f32 {
                if max > min {
                    ((x - min) / (max - min)).clamp(0.0, 1.0) as f32
                } else {
                    0.5
                }
            };
            let number = || value.as_f64().unwrap_or(0.0);
            match slot.info.kind {
                ParamKind::Range { min, max, .. } => {
                    genes[slot.offset] = unit(number(), f64::from(min), f64::from(max));
                }
                ParamKind::Integer { min, max } => {
                    genes[slot.offset] = unit(number(), f64::from(min), f64::from(max));
                }
                ParamKind::OptionalInteger { min, max } => match value.as_f64() {
                    Some(x) => {
                        genes[slot.offset] = 0.75;
                        genes[slot.offset + 1] = unit(x, f64::from(min), f64::from(max));
                    }
                    None => genes[slot.offset] = 0.25,
                },
                ParamKind::Toggle => {
                    genes[slot.offset] = if value.as_bool().unwrap_or(false) {
                        0.75
                    } else {
                        0.25
                    };
                }
                ParamKind::Choice(choices) => {
                    let id = value.as_str().unwrap_or("");
                    let i = choices.iter().position(|c| c.id == id).unwrap_or(0);
                    genes[slot.offset] = (i as f32 + 0.5) / choices.len().max(1) as f32;
                }
            }
        }
        Ok(Genome(genes))
    }

    /// `design` checked inside `base`: the configuration a candidate runs.
    pub fn config(&self, base: &Config, design: DesignLevers) -> Result<Config, ConfigError> {
        let mut config = base.clone();
        config.design = design;
        config.validate()?;
        Ok(config)
    }
}

impl Default for Layout {
    fn default() -> Self {
        Self::new()
    }
}

/// Genes a lever needs.
fn genes_of(kind: ParamKind) -> usize {
    match kind {
        ParamKind::OptionalInteger { .. } => 2,
        _ => 1,
    }
}

/// `g` in `[min, max]`, snapped to a whole number of steps from `min`. The
/// value is rounded to six decimals in `f64`, so a step of 0.05 gives 0.85
/// and not 0.85000002.
fn snap(g: f32, min: f32, max: f32, step: f32) -> f64 {
    let (min, max, step) = (f64::from(min), f64::from(max), f64::from(step));
    let x = min + f64::from(g) * (max - min);
    let snapped = if step > 0.0 {
        min + ((x - min) / step).round() * step
    } else {
        x
    };
    (snapped.clamp(min, max) * 1e6).round() / 1e6
}

fn interpolate_int(g: f32, min: u32, max: u32) -> u32 {
    let x = f64::from(min) + f64::from(g) * f64::from(max.saturating_sub(min));
    (x.round() as u32).clamp(min, max)
}

fn number(x: f64) -> Value {
    // Through f32, the type of the field, so the value is the one the
    // configuration will hold.
    Number::from_f64(f64::from(x as f32)).map_or(Value::Null, Value::Number)
}

/// A design as genes in `[0, 1]`; [`Layout`] reads them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Genome(pub Vec<f32>);

impl Genotype for Genome {
    /// Each gene mutates with chance `rate`: usually a Gaussian nudge,
    /// sometimes a fresh uniform draw. A child always differs from its
    /// parent in at least one gene.
    fn mutate<R: Rng>(&mut self, rng: &mut R, rate: f32) {
        let mut changed = false;
        for g in &mut self.0 {
            if rng.random::<f32>() < rate {
                mutate_gene(g, rng);
                changed = true;
            }
        }
        if !changed && !self.0.is_empty() {
            let i = rng.random_range(0..self.0.len());
            mutate_gene(&mut self.0[i], rng);
        }
    }

    /// Uniform crossover: each gene from either parent with equal chance.
    fn crossover<R: Rng>(&self, other: &Self, rng: &mut R) -> Self {
        Genome(
            self.0
                .iter()
                .zip(&other.0)
                .map(|(&a, &b)| if rng.random::<bool>() { a } else { b })
                .collect(),
        )
    }
}

fn mutate_gene<R: Rng>(g: &mut f32, rng: &mut R) {
    if rng.random::<f32>() < RESET_CHANCE {
        *g = rng.random::<f32>();
    } else {
        *g = (*g + NUDGE_SD * gaussian(rng)).clamp(0.0, 1.0);
    }
}

/// A standard normal draw (Box-Muller), so no distribution crate is needed.
fn gaussian<R: Rng>(rng: &mut R) -> f32 {
    let u1 = rng.random::<f32>().max(f32::MIN_POSITIVE);
    let u2 = rng.random::<f32>();
    (-2.0 * u1.ln()).sqrt() * (std::f32::consts::TAU * u2).cos()
}

#[cfg(test)]
mod tests {
    use super::*;
    use incentive_core::presets;
    use rand::SeedableRng;
    use rand_pcg::Pcg64;

    #[test]
    fn every_design_lever_has_genes() {
        let layout = Layout::new();
        let levers = params()
            .iter()
            .filter(|p| p.side == ParamSide::Design)
            .count();
        // One optional integer (the reshare depth cap) takes two genes.
        assert_eq!(layout.len(), levers + 1);
    }

    #[test]
    fn presets_round_trip_through_their_genome() {
        let layout = Layout::new();
        for p in presets() {
            let design = p.design();
            let genome = layout.encode(&design).expect("encodes");
            assert!(genome.0.iter().all(|g| (0.0..=1.0).contains(g)));
            assert_eq!(layout.decode(&genome).ok(), Some(design), "{}", p.id);
        }
    }

    #[test]
    fn random_genomes_decode_to_valid_designs_that_round_trip() {
        let layout = Layout::new();
        let base = Config::default();
        let mut rng = Pcg64::seed_from_u64(7);
        for _ in 0..500 {
            let genome = layout.random(&mut rng);
            let design = layout.decode(&genome).expect("decodes");
            assert!(layout.config(&base, design.clone()).is_ok());
            let again = layout.encode(&design).and_then(|g| layout.decode(&g));
            assert_eq!(again.ok(), Some(design));
        }
        // Out-of-range genes are clamped, never invalid.
        let wild = Genome(vec![-3.0; layout.len()]);
        let design = layout.decode(&wild).expect("decodes");
        assert!(layout.config(&base, design).is_ok());
        let wild = Genome(vec![7.0; layout.len()]);
        let design = layout.decode(&wild).expect("decodes");
        assert!(layout.config(&base, design).is_ok());
    }

    #[test]
    fn kinds_decode_as_documented() {
        assert_eq!(snap(0.5, 0.0, 3.0, 0.1), 1.5);
        assert_eq!(snap(0.849, 0.0, 1.0, 0.05), 0.85);
        assert_eq!(snap(1.0, 0.2, 1.2, 0.05), 1.2);
        assert_eq!(interpolate_int(0.5, 0, 500), 250);
        assert_eq!(interpolate_int(1.0, 1, 5), 5);
        let layout = Layout::new();
        let mut genes = layout.encode(&DesignLevers::default()).expect("encodes").0;
        let cap = layout
            .slots
            .iter()
            .find(|s| s.key == "reshare_depth_cap")
            .expect("cap slot");
        genes[cap.offset] = 0.9;
        genes[cap.offset + 1] = 0.0;
        let design = layout.decode(&Genome(genes.clone())).expect("decodes");
        assert_eq!(design.reshare_depth_cap, Some(1));
        genes[cap.offset] = 0.1;
        let design = layout.decode(&Genome(genes)).expect("decodes");
        assert_eq!(design.reshare_depth_cap, None);
    }

    #[test]
    fn mutation_changes_a_child_and_crossover_mixes_parents() {
        let layout = Layout::new();
        let mut rng = Pcg64::seed_from_u64(3);
        let parent = layout.random(&mut rng);
        let mut child = parent.clone();
        child.mutate(&mut rng, 0.0);
        assert_ne!(child, parent, "at least one gene changes");
        assert!(child.0.iter().all(|g| (0.0..=1.0).contains(g)));
        let other = Genome(vec![2.0; layout.len()]);
        let mixed = parent.crossover(&other, &mut rng);
        assert!(mixed.0.contains(&2.0));
        assert!(mixed.0.iter().any(|&g| g != 2.0));
    }
}
