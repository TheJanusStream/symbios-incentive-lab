//! Phase 3: people and bots write posts (docs/model.md, 4.2 and 7.5).

use crate::bots;
use crate::config::WorldAssumptions;
use crate::content::{AuthorProfile, Group, STYLE_COUNT, Style, generate_post};
use crate::person::{Person, hostile_chances};
use crate::population::Archetype;
use crate::rng::{self, Stream, StreamRng};
use crate::world::{ContagionTally, DayCtx, World};

/// Share of each group's users, by followers, from which half of all Attack
/// targets are drawn.
const PROMINENT_SHARE: f32 = 0.05;

/// Who Attacks can target today, per group.
struct TargetPools {
    active: [Vec<u32>; 2],
    prominent: [Vec<u32>; 2],
}

impl TargetPools {
    fn build(world: &World) -> Self {
        let mut active: [Vec<u32>; 2] = [Vec::new(), Vec::new()];
        let mut users: [Vec<u32>; 2] = [Vec::new(), Vec::new()];
        for (i, person) in world.people.iter().enumerate() {
            if !person.state.is_user {
                continue;
            }
            let g = person.traits.group.index();
            users[g].push(i as u32);
            if person.state.today.opened {
                active[g].push(i as u32);
            }
        }
        let prominent = users.map(|mut list| {
            list.sort_by(|&a, &b| {
                let (fa, fb) = (
                    world.accounts.followers[a as usize],
                    world.accounts.followers[b as usize],
                );
                fb.cmp(&fa).then(a.cmp(&b))
            });
            let keep = ((list.len() as f32) * PROMINENT_SHARE).ceil() as usize;
            list.truncate(keep);
            list
        });
        Self { active, prominent }
    }

    /// An out-group target for an Attack by a member of `group`.
    fn pick(&self, rng: &mut StreamRng, group: Group) -> Option<u32> {
        let g = group.other().index();
        let pool = if rng::chance(rng, 0.5) {
            &self.prominent[g]
        } else {
            &self.active[g]
        };
        (!pool.is_empty()).then(|| pool[rng::index(rng, pool.len())])
    }
}

/// Draws a style from the choice probabilities.
fn draw_style(rng: &mut StreamRng, mix: &[f32; STYLE_COUNT]) -> Style {
    let mut target = rng::uniform(rng);
    for (style, p) in Style::ALL.iter().zip(mix) {
        if target < *p {
            return *style;
        }
        target -= p;
    }
    Style::Personal
}

/// Whether an author's previous-day feed held a hostile item, for the
/// observational exposure diagnostic (docs/model.md, 9.2).
#[derive(Clone, Copy)]
enum Exposure {
    /// No impressions the day before.
    None,
    /// Impressions, none of them hostile.
    Calm,
    /// At least one hostile impression.
    Hostile,
}

impl Exposure {
    fn of(person: &Person) -> Self {
        match (
            person.state.yesterday_items,
            person.state.yesterday_hostile_items,
        ) {
            (0, _) => Self::None,
            (_, 0) => Self::Calm,
            _ => Self::Hostile,
        }
    }

    /// Counts one human post of `style` in its arm.
    fn count_post(self, tally: &mut ContagionTally, style: Style) {
        let hostile = u32::from(style.is_hostile());
        match self {
            Self::None => {}
            Self::Calm => {
                tally.unexposed_posts += 1;
                tally.unexposed_hostile += hostile;
            }
            Self::Hostile => {
                tally.exposed_posts += 1;
                tally.exposed_hostile += hostile;
            }
        }
    }
}

/// Adds an author who posts today to the causal exposure diagnostic
/// (docs/model.md, 7.5 and 9.2): the chance of a hostile style at `e = 0`
/// and at `e = 1` from the author's own values. Measurement only.
fn count_author(tally: &mut ContagionTally, person: &Person, assume: &WorldAssumptions) {
    let (calm, exposed) = hostile_chances(
        &person.state.style_value,
        assume.choice_temperature,
        assume.exposure_contagion,
    );
    tally.authors += 1;
    tally.chance_unexposed += f64::from(calm);
    tally.chance_exposed += f64::from(exposed);
    if calm > 0.0 {
        tally.ratio_sum += f64::from(exposed / calm);
    }
}

/// Every active user who may post writes `Poisson(post_rate)` posts, then the
/// bots write theirs. The daily count has its own draw; everything about one
/// post (style, polish, topic, content noise, Attack target) comes from the
/// post's own stream, keyed on its author and the author's post ordinal, so
/// a lever that changes how many posts someone writes reshuffles no one
/// else's posts (docs/model.md, section 12).
pub(crate) fn write_posts(world: &mut World, ctx: &DayCtx) {
    let targets = TargetPools::build(world);
    let assume = &ctx.config.world;
    let temperature = assume.choice_temperature;
    for i in 0..world.population() {
        let person = &world.people[i];
        if !person.state.today.opened || !person.can_post(ctx.day) {
            continue;
        }
        let mut rng = ctx.rng(i as u64, Stream::Posting);
        let count = rng::poisson(&mut rng, person.traits.post_rate);
        let mix = person.exposed_style_mix(temperature, assume.exposure_contagion);
        let exposure = Exposure::of(person);
        if count > 0 {
            count_author(&mut world.tally.contagion, person, assume);
        }
        let p_polish = person.polish_probability(temperature);
        let author = AuthorProfile {
            account: i as u32,
            is_bot: false,
            is_creator: person.traits.archetype == Archetype::Creator,
            group: person.traits.group,
            skill: person.traits.skill,
            temperament: person.traits.temperament,
            evasion: (person.state.evasion - world.platform.counter_evasion).max(0.0),
            written_evasion: person.state.evasion,
            veil: assume.evasion_cost * person.state.evasion,
            intensity: person.state.intensity,
            polished: false,
            ordinal: person.state.posts_written,
        };
        let interests = person.traits.interests;
        for k in 0..count {
            let ordinal = author.ordinal + k;
            let mut post_rng = rng::post_rng(ctx.config.run.seed, author.account, ordinal);
            let style = draw_style(&mut post_rng, &mix);
            let polished = rng::chance(&mut post_rng, p_polish);
            let profile = AuthorProfile {
                polished,
                ordinal,
                ..author
            };
            let id = world.posts.next_id();
            let mut post = generate_post(
                &mut post_rng,
                id,
                ctx.day,
                &profile,
                style,
                &interests,
                assume,
            );
            if style == Style::Attack {
                post.target = targets.pick(&mut post_rng, author.group);
            }
            let toxicity = post.toxicity;
            exposure.count_post(&mut world.tally.contagion, style);
            world.posts.push(post);
            world.accounts.recent_posts[i].push(id);
            world.tally.posts_by_style[style.index()] += 1;
            world.tally.polished_posts += u32::from(polished);
            let state = &mut world.people[i].state;
            state.posts_written += 1;
            state.today.posts += 1;
            state.today.post_toxicity += toxicity;
            state.today.styles_written |= 1 << style.index();
        }
    }
    bots::write_posts(world, ctx);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::init;

    /// The posts of a small world on day 0 after everyone opened the app,
    /// with the post rate of individual 0 changed to `rate`.
    fn day_zero_posts(rate: f32) -> Vec<crate::content::Post> {
        let mut config = Config::default();
        config.run.population_size = 300;
        let mut world = init::build_world(&config);
        for person in &mut world.people {
            person.state.today.opened = person.state.is_user;
        }
        let first = world
            .people
            .iter()
            .position(|p| p.state.is_user)
            .unwrap_or(0);
        world.people[first].traits.post_rate = rate;
        let ctx = DayCtx {
            config: &config,
            day: 0,
        };
        write_posts(&mut world, &ctx);
        world.posts.iter().cloned().collect()
    }

    /// The posts of a small world on day 0, with every user exposed to
    /// `hostile` of 10 hostile items yesterday, at contagion `kappa`, and
    /// the day's tallies.
    fn exposed_posts(
        kappa: f32,
        hostile: u32,
    ) -> (Vec<crate::content::Post>, crate::world::DayTally) {
        let mut config = Config::default();
        config.run.population_size = 600;
        config.world.exposure_contagion = kappa;
        let mut world = init::build_world(&config);
        for person in &mut world.people {
            person.state.today.opened = person.state.is_user;
            person.state.yesterday_items = 10;
            person.state.yesterday_hostile_items = hostile;
        }
        let ctx = DayCtx {
            config: &config,
            day: 0,
        };
        write_posts(&mut world, &ctx);
        (world.posts.iter().cloned().collect(), world.tally.clone())
    }

    #[test]
    fn a_style_draw_takes_one_uniform_whatever_the_mix() {
        let calm = [0.4, 0.3, 0.3, 0.0, 0.0, 0.0];
        let hostile = [0.0, 0.0, 0.0, 0.5, 0.0, 0.5];
        for key in 0..50 {
            let mut a = rng::post_rng(1, 7, key);
            let mut b = rng::post_rng(1, 7, key);
            draw_style(&mut a, &calm);
            draw_style(&mut b, &hostile);
            assert_eq!(rng::uniform(&mut a), rng::uniform(&mut b));
        }
    }

    #[test]
    fn contagion_shifts_styles_without_new_draws() {
        let (base, _) = exposed_posts(0.0, 10);
        let (calm, _) = exposed_posts(1.0, 0);
        let (lifted, tally) = exposed_posts(1.0, 10);
        // No exposure: the same posts as without contagion.
        assert_eq!(base, calm);
        // Exposure changes styles, never how many posts anyone writes.
        let key = |p: &crate::content::Post| (p.author, p.ordinal);
        let keys = |v: &[crate::content::Post]| v.iter().map(key).collect::<Vec<_>>();
        assert_eq!(keys(&base), keys(&lifted));
        let hostile = |v: &[crate::content::Post]| {
            v.iter()
                .filter(|p| !p.author_is_bot && p.style.is_hostile())
                .count()
        };
        assert!(
            hostile(&lifted) > hostile(&base),
            "{} {}",
            hostile(&lifted),
            hostile(&base)
        );
        // A post keeps its style whenever its draw falls where both mixes
        // agree, and calm posts keep every content draw.
        let same = base
            .iter()
            .zip(&lifted)
            .filter(|(a, b)| a.style == b.style && !a.style.is_hostile())
            .all(|(a, b)| a == b);
        assert!(same);
        // The diagnostics: everyone who posted counts once, and with a fully
        // hostile feed every post falls in the exposed arm.
        let c = tally.contagion;
        let authors: std::collections::BTreeSet<u32> = lifted
            .iter()
            .filter(|p| !p.author_is_bot)
            .map(|p| p.author)
            .collect();
        assert_eq!(c.authors as usize, authors.len());
        assert!(c.chance_exposed > c.chance_unexposed);
        assert_eq!(c.unexposed_posts, 0);
        assert_eq!(
            c.exposed_posts as usize,
            lifted.iter().filter(|p| !p.author_is_bot).count()
        );
        assert_eq!(c.exposed_hostile as usize, hostile(&lifted));
    }

    #[test]
    fn one_author_posting_more_reshuffles_no_one_else() {
        let few = day_zero_posts(0.0);
        let many = day_zero_posts(20.0);
        let key = |p: &crate::content::Post| (p.author, p.ordinal);
        let mut compared = 0;
        for p in &few {
            if let Some(q) = many.iter().find(|q| key(q) == key(p)) {
                assert_eq!(
                    (p.style, p.topic, p.quality, p.toxicity),
                    (q.style, q.topic, q.quality, q.toxicity)
                );
                assert_eq!(
                    (p.quality_signal, p.polished, p.target),
                    (q.quality_signal, q.polished, q.target)
                );
                compared += 1;
            }
        }
        assert!(
            compared > 20 && compared == few.len(),
            "{compared} of {}",
            few.len()
        );
        assert!(many.len() > few.len());
    }
}
