//! Building the world on day 0 (docs/model.md, 3.1 to 3.5).

use crate::bots;
use crate::config::Config;
use crate::network::{self, Accounts, FollowTarget};
use crate::person::Person;
use crate::population::{draw_archetype, draw_ties, draw_traits};
use crate::posts::PostStore;
use crate::rng::{self, Stream};
use crate::world::{DayTally, PlatformState, StyleRates, World};

/// Builds the population, ties, bots, initial users and their follows.
pub(crate) fn build_world(config: &Config) -> World {
    let seed = config.run.seed;
    let n = config.run.population_size as usize;
    let shares = config.population.archetype_shares.as_array();
    let people: Vec<Person> = (0..n)
        .map(|i| {
            let mut rng = rng::stream_rng(seed, 0, i as u64, Stream::Population);
            let archetype = draw_archetype(&mut rng, shares);
            Person::new(draw_traits(&mut rng, archetype))
        })
        .collect();
    let groups: Vec<_> = people.iter().map(|p| p.traits.group).collect();
    let ties = draw_ties(
        seed,
        &groups,
        config.run.offline_ties,
        config.world.homophily,
    );
    let bots = bots::create(config, n);
    let accounts = Accounts::new(n + bots.len());
    let platform = PlatformState {
        style_rates: [StyleRates::default(); 6],
        impulsivity_rank: impulsivity_ranks(&people),
        ..PlatformState::default()
    };
    let mut world = World {
        people,
        ties,
        bots,
        accounts,
        posts: PostStore::default(),
        platform,
        tally: DayTally::default(),
    };
    seed_users(&mut world, config);
    let n_f = n as f32;
    world.platform.polarisation_day0 = world
        .people
        .iter()
        .map(|p| p.state.affective_polarisation)
        .sum::<f32>()
        / n_f;
    world
}

/// Each individual's position when sorted by ascending impulsivity; user
/// controls are adopted in this order (docs/model.md, 6.5 and 6.10).
fn impulsivity_ranks(people: &[Person]) -> Vec<u32> {
    let mut order: Vec<usize> = (0..people.len()).collect();
    order.sort_by(|&a, &b| {
        people[a]
            .traits
            .impulsivity
            .total_cmp(&people[b].traits.impulsivity)
            .then(a.cmp(&b))
    });
    let mut ranks = vec![0; people.len()];
    for (rank, &i) in order.iter().enumerate() {
        ranks[i] = rank as u32;
    }
    ranks
}

/// Picks exactly `round(initial_user_share * N)` initial users at random, then
/// gives each its follows in ascending index.
fn seed_users(world: &mut World, config: &Config) {
    let n = world.population();
    let wanted = ((config.run.initial_user_share * n as f32).round() as usize).min(n);
    let mut order: Vec<usize> = (0..n).collect();
    let mut rng = rng::stream_rng(config.run.seed, 0, u64::MAX, Stream::Population);
    for k in 0..wanted {
        let pick = k + rng::index(&mut rng, n - k);
        order.swap(k, pick);
    }
    let mut users = order[..wanted].to_vec();
    users.sort_unstable();
    for &i in &users {
        let s = &mut world.people[i].state;
        s.is_user = true;
        s.ever_user = true;
        s.joined_day = Some(0);
    }
    let target = FollowTarget::Total(config.run.initial_follows as usize);
    for &i in &users {
        let mut rng = rng::stream_rng(config.run.seed, 0, i as u64, Stream::Follows);
        network::attach_follows(world, i, target, config.world.homophily, &mut rng);
    }
}
