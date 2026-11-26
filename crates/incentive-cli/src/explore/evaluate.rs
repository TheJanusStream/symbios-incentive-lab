//! Evaluating candidate designs: each runs on every seed at the search's
//! size, and its metrics are the mean summary. Results are cached by design,
//! so a design the search proposes again (an elite carried over, a mutation
//! that snaps back to the same steps) is looked up instead of rerun.
//!
//! Determinism: a run is a pure function of its configuration, the seeds are
//! fixed and averaged in a fixed order, and the cache only ever holds the
//! value the design would get anyway. So which thread evaluates a design, or
//! whether it was cached, never changes a result.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use incentive_core::{AtlasMetrics, Config, DesignLevers, Simulation, Summary};
use rayon::prelude::*;
use symbios_genetics::Evaluator;

use super::genome::{Genome, Layout};

/// A design and its mean summary over the seeds.
#[derive(Clone, Debug)]
pub struct Evaluation {
    /// The decoded design.
    pub design: DesignLevers,
    /// The mean of the seeds' summaries.
    pub summary: Summary,
    /// Its key metrics, rounded as the atlas stores them.
    pub metrics: AtlasMetrics,
}

/// Runs candidate designs and remembers the results.
pub struct Evaluations {
    layout: Layout,
    base: Config,
    seeds: Vec<u64>,
    cache: Mutex<BTreeMap<String, Arc<Evaluation>>>,
    failure: Mutex<Option<String>>,
}

impl Evaluations {
    /// Evaluations of designs inside `base` (its run settings, population
    /// and world), each on every seed of `seeds`.
    pub fn new(layout: Layout, base: Config, seeds: Vec<u64>) -> Self {
        Self {
            layout,
            base,
            seeds,
            cache: Mutex::new(BTreeMap::new()),
            failure: Mutex::new(None),
        }
    }

    /// The genome layout.
    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    /// The distinct designs simulated so far.
    pub fn count(&self) -> usize {
        self.cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }

    /// The first failure, if any run failed (none should: every genome
    /// decodes to a valid design).
    pub fn failure(&self) -> Option<String> {
        self.failure
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// The evaluation of a genome's design, run now or looked up.
    pub fn evaluate(&self, genome: &Genome) -> Option<Arc<Evaluation>> {
        match self.try_evaluate(genome) {
            Ok(e) => Some(e),
            Err(message) => {
                let mut failure = self.failure.lock().unwrap_or_else(PoisonError::into_inner);
                failure.get_or_insert(message);
                None
            }
        }
    }

    /// The evaluation of a design given by its levers, run now or looked up.
    pub fn evaluate_design(&self, design: &DesignLevers) -> Option<Arc<Evaluation>> {
        match self.try_evaluate_design(design.clone()) {
            Ok(e) => Some(e),
            Err(message) => {
                let mut failure = self.failure.lock().unwrap_or_else(PoisonError::into_inner);
                failure.get_or_insert(message);
                None
            }
        }
    }

    fn try_evaluate(&self, genome: &Genome) -> Result<Arc<Evaluation>, String> {
        let design = self.layout.decode(genome).map_err(|e| e.to_string())?;
        self.try_evaluate_design(design)
    }

    fn try_evaluate_design(&self, design: DesignLevers) -> Result<Arc<Evaluation>, String> {
        let key = serde_json::to_string(&design).map_err(|e| e.to_string())?;
        if let Some(hit) = self
            .cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&key)
        {
            return Ok(Arc::clone(hit));
        }
        let config = self
            .layout
            .config(&self.base, design.clone())
            .map_err(|e| e.to_string())?;
        let summaries: Vec<Summary> = self
            .seeds
            .par_iter()
            .map(|&seed| {
                let mut config = config.clone();
                config.run.seed = seed;
                Simulation::new(config).map(|mut sim| sim.run_to_end())
            })
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?;
        let summary = Summary::mean(&summaries).ok_or("no seeds")?;
        let evaluation = Arc::new(Evaluation {
            metrics: AtlasMetrics::from_summary(&summary),
            design,
            summary,
        });
        // Another thread may have run the same design meanwhile; both
        // results are identical, so keeping either is the same.
        let mut cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
        Ok(Arc::clone(cache.entry(key).or_insert(evaluation)))
    }

    /// Evaluates a batch in parallel, results in input order.
    pub fn evaluate_all(&self, genomes: &[Genome]) -> Vec<Option<Arc<Evaluation>>> {
        genomes.par_iter().map(|g| self.evaluate(g)).collect()
    }
}

/// What a search optimises: from an evaluation to the library's fitness,
/// objectives and descriptor (all higher-is-better, descriptors in `[0, 1]`).
pub type Score = dyn Fn(&Evaluation) -> (f32, Vec<f32>, Vec<f32>) + Send + Sync;

/// The library's evaluator: an [`Evaluations`] cache and a score. A failed
/// run scores NaN, which every algorithm of the library sets aside.
pub struct Scored<'a> {
    /// The cache that runs designs.
    pub evaluations: &'a Evaluations,
    /// The score of an evaluation.
    pub score: &'a Score,
}

impl Evaluator<Genome> for Scored<'_> {
    fn evaluate(&self, genome: &Genome) -> (f32, Vec<f32>, Vec<f32>) {
        match self.evaluations.evaluate(genome) {
            Some(e) => (self.score)(&e),
            None => (f32::NAN, vec![f32::NAN; 3], vec![f32::NAN; 2]),
        }
    }
}
