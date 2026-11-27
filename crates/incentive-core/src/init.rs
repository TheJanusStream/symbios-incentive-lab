//! Building the world on day 0 (docs/model.md, 3.1 to 3.5).

use crate::bots;
use crate::config::Config;
use crate::network::{self, Accounts, FollowTarget};
use crate::person::{POLISH_PRIOR_TEMPERATURES, Person, polish_temperature};
use crate::population::{Traits, draw_archetype, draw_ties, draw_traits};
use crate::posts::PostStore;
use crate::rng::{self, Stream};
use crate::world::{DayTally, PlatformState, StyleRates, TruthState, World};

/// Builds the population, ties, bots, initial users and their follows.
pub(crate) fn build_world(config: &Config) -> World {
    let seed = config.run.seed;
    let n = config.run.population_size as usize;
    let shares = config.population.archetype_shares.as_array();
    let people: Vec<Person> = (0..n)
        .map(|i| {
            let mut race = rng::stream_rng(seed, 0, i as u64, Stream::Archetype);
            let archetype = draw_archetype(&mut race, shares);
            let mut rng = rng::stream_rng(seed, 0, i as u64, Stream::Population);
            let mut person = Person::new(draw_traits(&mut rng, archetype));
            person.state.polish_value = initial_polish_values(config.world.choice_temperature);
            person
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
        ..PlatformState::default()
    };
    let truth = TruthState {
        impulsivity_rank: ranks(&people, |t| t.impulsivity),
        partisanship_rank: ranks(&people, |t| t.partisanship),
        ..TruthState::default()
    };
    let mut world = World {
        people,
        ties,
        bots,
        accounts,
        posts: PostStore::default(),
        platform,
        truth,
        tally: DayTally::default(),
    };
    seed_users(&mut world, config);
    let n_f = n as f32;
    world.truth.polarisation_day0 = world
        .people
        .iter()
        .map(|p| p.state.affective_polarisation)
        .sum::<f32>()
        / n_f;
    world
}

/// The learned values of plain and polished posts on day 0 (docs/model.md,
/// 7.5): polish starts [`POLISH_PRIOR_TEMPERATURES`] polish temperatures
/// below plain, so few authors polish until they find that it pays.
pub(crate) fn initial_polish_values(choice_temperature: f32) -> [f32; 2] {
    [
        0.0,
        -POLISH_PRIOR_TEMPERATURES * polish_temperature(choice_temperature),
    ]
}

/// Each individual's position when sorted by ascending value of a trait,
/// ties to the lower index. User controls are adopted in the order of
/// impulsivity (docs/model.md, 6.5 and 6.10); truth metrics read the order of
/// partisanship (9.2).
fn ranks(people: &[Person], of: impl Fn(&Traits) -> f32) -> Vec<u32> {
    let mut order: Vec<usize> = (0..people.len()).collect();
    order.sort_by(|&a, &b| {
        of(&people[a].traits)
            .total_cmp(&of(&people[b].traits))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_impulsivity_order_is_truth_side() {
        let mut config = Config::default();
        config.run.population_size = 300;
        let world = build_world(&config);
        let ranks = &world.truth.impulsivity_rank;
        assert_eq!(ranks.len(), world.population());
        let mut by_rank = vec![usize::MAX; ranks.len()];
        for (i, &r) in ranks.iter().enumerate() {
            by_rank[r as usize] = i;
        }
        assert!(by_rank.iter().all(|&i| i != usize::MAX));
        for pair in by_rank.windows(2) {
            let (a, b) = (&world.people[pair[0]].traits, &world.people[pair[1]].traits);
            assert!(a.impulsivity <= b.impulsivity);
        }
    }
}
