//! `incentive explore`: searches of the design space with the world held
//! fixed. Every design lever is a gene ([`genome`]); every candidate runs on a
//! few seeds at a reduced size ([`evaluate`]); the searches come from
//! `symbios-genetics`:
//!
//! - `map`: MAP-Elites over the engagement index and the harm index, keeping
//!   the highest benefit index in every cell.
//! - `pareto`: NSGA-II, maximising engagement, minimising harm, maximising
//!   benefit.
//! - `goodhart`: a generational GA (the library's `SimpleGA`, with elitism)
//!   on one observable proxy alone, recording the truth of every
//!   generation's best design.
//!
//! A whole search is deterministic for a given `--seed`: the library draws
//! every random choice from one seeded generator on the calling thread, each
//! batch is evaluated in parallel but collected in input order, and runs are
//! pure functions of their configuration. The output files hold no times.

pub mod evaluate;
pub mod genome;
mod tables;

use std::path::{Path, PathBuf};
use std::time::Instant;

use incentive_core::{
    AtlasDesign, Config, ExploreSettings, GoodhartAtlas, GoodhartGeneration, MapAtlas, MapCell,
    MapGrid, MetricId, ParetoAtlas, Visibility, atlas::round4, presets,
};
use rand::SeedableRng;
use rand_pcg::Pcg64;
use symbios_genetics::Evolver;
use symbios_genetics::algorithms::map_elites::MapElites;
use symbios_genetics::algorithms::nsga2::Nsga2;
use symbios_genetics::algorithms::simple::SimpleGA;

use crate::cli::{ExploreMode, SearchOptions};
use crate::error::CliError;
use crate::report;
use evaluate::{Evaluation, Evaluations, Scored};
use genome::{Genome, Layout};

/// The map's engagement range: 0.04 to 0.36 in steps of 0.02 on a 16-cell
/// grid. The presets span 0.14 to 0.27 at full size (docs/model.md, 13.3).
pub const ENGAGEMENT_RANGE: [f32; 2] = [0.04, 0.36];
/// The map's harm range: 0 to 0.64 in steps of 0.04 on a 16-cell grid. The
/// presets span 0.07 to 0.51 at full size.
pub const HARM_RANGE: [f32; 2] = [0.0, 0.64];

/// Elites a Goodhart generation carries over unchanged.
const GOODHART_ELITISM: usize = 2;

/// Mixed into `--seed` for the initial random designs, so they come from a
/// stream of their own and not the search's.
const INITIAL_STREAM: u64 = 0x9E37_79B9_7F4A_7C15;

/// Per-mode defaults of the options that differ between modes.
struct Defaults {
    generations: u32,
    batch: u32,
    initial: u32,
}

/// Runs one explore mode.
pub fn explore(mode: &ExploreMode) -> Result<(), CliError> {
    match mode {
        ExploreMode::Map {
            search,
            grid,
            initial,
        } => map(search, *grid, *initial),
        ExploreMode::Pareto { search } => pareto(search),
        ExploreMode::Goodhart { search, proxy } => goodhart(search, proxy),
    }
}

/// What every mode shares: the base configuration, the cache and the
/// settings to record.
struct Search {
    evaluations: Evaluations,
    settings: ExploreSettings,
    started: Instant,
}

impl Search {
    fn new(
        options: &SearchOptions,
        mode: &str,
        defaults: &Defaults,
        extra_args: &str,
    ) -> Result<Self, CliError> {
        let (world, mut base) = match &options.config {
            Some(path) => (
                path.file_name()
                    .map_or_else(|| "config".into(), |n| n.to_string_lossy().into_owned()),
                read_config(path)?,
            ),
            None => ("defaults".to_string(), Config::default()),
        };
        base.set_path("run.days", f64::from(options.days))?;
        base.set_path("run.population_size", f64::from(options.population))?;
        base.run.tracked_agents = 0;
        base.design = incentive_core::DesignLevers::default();
        base.validate()?;
        if options.seeds == 0 {
            return Err(CliError::Seeds("0".into()));
        }
        if !(0.0..=1.0).contains(&options.mutation_rate) {
            return Err(CliError::Explore(
                "--mutation-rate must lie between 0 and 1".into(),
            ));
        }
        let seeds: Vec<u64> = (1..=options.seeds).collect();
        let generations = options.generations.unwrap_or(defaults.generations);
        let batch = options.batch.unwrap_or(defaults.batch).max(1);
        let mut command = format!(
            "incentive explore {mode} --seed {} --seeds {} --population {} --days {} \
             --generations {generations} --batch {batch} --mutation-rate {}",
            options.seed, options.seeds, options.population, options.days, options.mutation_rate
        );
        if let Some(path) = &options.config {
            command.push_str(&format!(" --config {}", path.display()));
        }
        command.push_str(extra_args);
        let settings = ExploreSettings {
            mode: mode.to_string(),
            seed: options.seed,
            seeds: seeds.clone(),
            population: options.population,
            days: options.days,
            generations,
            batch,
            initial: defaults.initial,
            mutation_rate: options.mutation_rate,
            evaluations: 0,
            runs: 0,
            world,
            command,
            base: base.clone(),
        };
        Ok(Self {
            evaluations: Evaluations::new(Layout::new(), base, seeds),
            settings,
            started: Instant::now(),
        })
    }

    /// The initial designs: the presets (when asked), then uniform random
    /// genomes up to `count`.
    fn initial(&self, count: usize, with_presets: bool) -> Result<Vec<Genome>, CliError> {
        let layout = self.evaluations.layout();
        let mut genomes = Vec::with_capacity(count);
        if with_presets {
            for p in presets().iter().take(count) {
                genomes.push(layout.encode(&p.design())?);
            }
        }
        let mut rng = Pcg64::seed_from_u64(self.settings.seed ^ INITIAL_STREAM);
        while genomes.len() < count {
            genomes.push(layout.random(&mut rng));
        }
        Ok(genomes)
    }

    /// Progress on standard error, so standard output stays the report.
    fn progress(&self, generation: u32, note: &str) {
        eprintln!(
            "generation {generation}/{}: {} designs simulated, {:.0} s{note}",
            self.settings.generations,
            self.evaluations.count(),
            self.started.elapsed().as_secs_f32()
        );
    }

    /// Fails if any run failed; otherwise the settings with the counts.
    fn finish(mut self) -> Result<(ExploreSettings, f32), CliError> {
        if let Some(message) = self.evaluations.failure() {
            return Err(CliError::Explore(format!("a candidate failed: {message}")));
        }
        let evaluations = self.evaluations.count() as u32;
        self.settings.evaluations = evaluations;
        self.settings.runs = evaluations * self.settings.seeds.len() as u32;
        Ok((self.settings, self.started.elapsed().as_secs_f32()))
    }

    fn design_of(&self, genome: &Genome) -> Result<AtlasDesign, CliError> {
        let e = self
            .evaluations
            .evaluate(genome)
            .ok_or_else(|| CliError::Explore("an elite could not be evaluated".into()))?;
        Ok(atlas_design(&e))
    }
}

fn atlas_design(e: &Evaluation) -> AtlasDesign {
    AtlasDesign {
        design: e.design.clone(),
        metrics: e.metrics,
    }
}

fn read_config(path: &Path) -> Result<Config, CliError> {
    let text = std::fs::read_to_string(path).map_err(|source| CliError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(Config::from_json(&text)?)
}

fn out_dir(options: &SearchOptions, name: &str) -> Result<PathBuf, CliError> {
    let dir = options
        .out
        .clone()
        .unwrap_or_else(|| report::default_out(name));
    report::ensure_dir(&dir)?;
    Ok(dir)
}

/// Writes an atlas file as compact JSON with a final newline.
fn write_atlas(path: &Path, value: &impl serde::Serialize) -> Result<(), CliError> {
    let mut text = serde_json::to_string(value).map_err(|e| CliError::Json(e.to_string()))?;
    text.push('\n');
    report::write_file(path, &text)
}

/// `explore map`: MAP-Elites over engagement and harm, benefit as fitness.
fn map(options: &SearchOptions, resolution: u32, initial: u32) -> Result<(), CliError> {
    if !(2..=64).contains(&resolution) {
        return Err(CliError::Explore("--grid must lie between 2 and 64".into()));
    }
    let defaults = Defaults {
        generations: 50,
        batch: 64,
        initial,
    };
    let search = Search::new(
        options,
        "map",
        &defaults,
        &format!(" --grid {resolution} --initial {initial}"),
    )?;
    let grid = MapGrid {
        resolution,
        engagement: ENGAGEMENT_RANGE,
        harm: HARM_RANGE,
    };
    let score = move |e: &Evaluation| {
        let m = &e.metrics;
        (
            m.benefit_index,
            vec![m.engagement_index, -m.harm_index, m.benefit_index],
            vec![
                MapGrid::normalise(m.engagement_index, grid.engagement),
                MapGrid::normalise(m.harm_index, grid.harm),
            ],
        )
    };
    let scored = Scored {
        evaluations: &search.evaluations,
        score: &score,
    };
    let mut elites: MapElites<Genome> = MapElites::new(
        resolution as usize,
        search.settings.mutation_rate,
        search.settings.batch as usize,
        search.settings.seed,
    );
    let start = search.initial(initial as usize, true)?;
    // The library seeds its archive one design at a time; evaluating the
    // initial designs here first, in parallel, makes that a cache lookup.
    search.evaluations.evaluate_all(&start);
    elites.seed_population(start, &scored);
    search.progress(0, &format!(", {} cells", elites.archive_len()));
    for g in 1..=search.settings.generations {
        elites.step(&scored);
        search.progress(g, &format!(", {} cells", elites.archive_len()));
    }
    let mut cells = Vec::new();
    for (key, elite) in elites.archive_iter() {
        let (x, y) = (key.first().copied(), key.get(1).copied());
        let (Some(x), Some(y)) = (x, y) else {
            continue;
        };
        cells.push(MapCell {
            x: x as u32,
            y: y as u32,
            elite: search.design_of(&elite.genotype)?,
        });
    }
    let (settings, seconds) = search.finish()?;
    let atlas = MapAtlas {
        settings,
        grid,
        cells,
    };
    tables::print_map(&atlas, seconds)?;
    let dir = out_dir(options, "explore-map")?;
    write_atlas(&dir.join("map.json"), &atlas)?;
    outln!("\nwrote {}", dir.join("map.json").display());
    Ok(())
}

/// `explore pareto`: NSGA-II on engagement (up), harm (down), benefit (up).
fn pareto(options: &SearchOptions) -> Result<(), CliError> {
    let defaults = Defaults {
        generations: 40,
        batch: 64,
        initial: 0,
    };
    let mut search = Search::new(options, "pareto", &defaults, "")?;
    search.settings.initial = search.settings.batch;
    let score = |e: &Evaluation| {
        let m = &e.metrics;
        (
            m.benefit_index,
            vec![m.engagement_index, -m.harm_index, m.benefit_index],
            Vec::new(),
        )
    };
    let scored = Scored {
        evaluations: &search.evaluations,
        score: &score,
    };
    let start = search.initial(search.settings.batch as usize, true)?;
    let mut nsga = Nsga2::new(start, search.settings.mutation_rate, search.settings.seed);
    for g in 1..=search.settings.generations.max(1) {
        nsga.step(&scored);
        search.progress(g, "");
    }
    let population = nsga.population().to_vec();
    let fronts = Nsga2::<Genome>::fast_non_dominated_sort(&population);
    let mut front: Vec<AtlasDesign> = Vec::new();
    for &i in fronts.first().map_or(&[][..], Vec::as_slice) {
        let design = search.design_of(&population[i].genotype)?;
        if !front.iter().any(|d| d.design == design.design) {
            front.push(design);
        }
    }
    front.sort_by(|a, b| {
        a.metrics
            .engagement_index
            .total_cmp(&b.metrics.engagement_index)
            .then(a.metrics.harm_index.total_cmp(&b.metrics.harm_index))
    });
    let (settings, seconds) = search.finish()?;
    let atlas = ParetoAtlas { settings, front };
    tables::print_pareto(&atlas, seconds)?;
    let dir = out_dir(options, "explore-pareto")?;
    write_atlas(&dir.join("pareto.json"), &atlas)?;
    outln!("\nwrote {}", dir.join("pareto.json").display());
    Ok(())
}

/// `explore goodhart`: a GA on one observable proxy; the truth is recorded,
/// never optimised.
fn goodhart(options: &SearchOptions, proxy: &str) -> Result<(), CliError> {
    let id = MetricId::from_key(proxy).ok_or_else(|| CliError::UnknownMetric(proxy.into()))?;
    if id.info().visibility != Visibility::Observable {
        return Err(CliError::Explore(format!(
            "{proxy} is a truth metric; a proxy must be one the platform can observe"
        )));
    }
    let defaults = Defaults {
        generations: 25,
        batch: 40,
        initial: 0,
    };
    let mut search = Search::new(options, "goodhart", &defaults, &format!(" --proxy {proxy}"))?;
    search.settings.initial = search.settings.batch;
    let score = move |e: &Evaluation| (round4(e.summary.get(id)), Vec::new(), Vec::new());
    let scored = Scored {
        evaluations: &search.evaluations,
        score: &score,
    };
    // Random designs only: the search starts knowing nothing, so the
    // generations show where chasing the proxy leads.
    let start = search.initial(search.settings.batch as usize, false)?;
    let mut ga = SimpleGA::new(
        start,
        search.settings.mutation_rate,
        GOODHART_ELITISM,
        search.settings.seed,
    );
    let mut generations = Vec::new();
    for g in 0..search.settings.generations.max(1) {
        ga.step(&scored);
        // After a step the first elite is the best design evaluated in this
        // generation, carried into the next one.
        let best = ga
            .population()
            .first()
            .map(|p| (p.genotype.clone(), p.fitness))
            .ok_or_else(|| CliError::Explore("empty population".into()))?;
        generations.push(GoodhartGeneration {
            generation: g,
            proxy: best.1,
            best: search.design_of(&best.0)?,
        });
        search.progress(g, &format!(", best {proxy} {:.4}", best.1));
    }
    let (settings, seconds) = search.finish()?;
    let atlas = GoodhartAtlas {
        settings,
        proxy: proxy.to_string(),
        generations,
    };
    tables::print_goodhart(&atlas, seconds)?;
    let dir = out_dir(options, &format!("explore-goodhart-{proxy}"))?;
    let file = dir.join(format!("goodhart-{proxy}.json"));
    write_atlas(&file, &atlas)?;
    outln!("\nwrote {}", file.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny(out: &Path) -> SearchOptions {
        SearchOptions {
            config: None,
            seed: 5,
            seeds: 1,
            population: 80,
            days: 12,
            generations: Some(2),
            batch: Some(6),
            mutation_rate: 0.15,
            out: Some(out.to_path_buf()),
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "incentive-explore-test-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn read(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap_or_default()
    }

    #[test]
    fn every_mode_is_deterministic_for_a_seed() {
        let a = scratch("a");
        let b = scratch("b");
        // The second search runs on a single thread: the output must not
        // depend on how rayon spreads the evaluations.
        let single = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .expect("a one-thread pool");
        for (dir, one_thread) in [(&a, false), (&b, true)] {
            let search = || {
                let options = tiny(dir);
                map(&options, 4, 8).expect("map runs");
                pareto(&options).expect("pareto runs");
                goodhart(&options, "engagement_index").expect("goodhart runs");
            };
            if one_thread {
                single.install(search);
            } else {
                search();
            }
        }
        for file in ["map.json", "pareto.json", "goodhart-engagement_index.json"] {
            let (x, y) = (read(&a.join(file)), read(&b.join(file)));
            assert!(!x.is_empty(), "{file} written");
            assert_eq!(x, y, "{file} differs between identical runs");
        }
        let atlas: MapAtlas = serde_json::from_str(&read(&a.join("map.json"))).expect("parses");
        assert!(!atlas.cells.is_empty());
        for cell in &atlas.cells {
            let m = cell.elite.metrics;
            assert_eq!(
                atlas.grid.cell_of(m.engagement_index, m.harm_index),
                (cell.x, cell.y)
            );
        }
        assert!(atlas.settings.evaluations > 0);
        assert_eq!(atlas.settings.runs, atlas.settings.evaluations);
        let front: ParetoAtlas =
            serde_json::from_str(&read(&a.join("pareto.json"))).expect("parses");
        assert!(!front.front.is_empty());
        let run: GoodhartAtlas =
            serde_json::from_str(&read(&a.join("goodhart-engagement_index.json"))).expect("parses");
        assert_eq!(run.generations.len(), 2);
        assert!(run.generations[1].proxy >= run.generations[0].proxy);
        let _ = std::fs::remove_dir_all(&a);
        let _ = std::fs::remove_dir_all(&b);
    }

    #[test]
    fn a_different_seed_changes_the_search_and_bad_options_are_refused() {
        let options = tiny(&scratch("unused"));
        let first = Search::new(
            &options,
            "map",
            &Defaults {
                generations: 1,
                batch: 1,
                initial: 4,
            },
            "",
        )
        .and_then(|s| s.initial(4, false));
        let other = SearchOptions {
            seed: 6,
            ..tiny(&scratch("unused"))
        };
        let second = Search::new(
            &other,
            "map",
            &Defaults {
                generations: 1,
                batch: 1,
                initial: 4,
            },
            "",
        )
        .and_then(|s| s.initial(4, false));
        assert_ne!(first.ok(), second.ok());
        assert!(
            goodhart(&options, "toxic_share").is_err(),
            "truth is no proxy"
        );
        assert!(goodhart(&options, "nope").is_err());
        assert!(map(&options, 1, 4).is_err());
        let bad = SearchOptions {
            seeds: 0,
            ..tiny(&scratch("unused"))
        };
        assert!(pareto(&bad).is_err());
    }
}
