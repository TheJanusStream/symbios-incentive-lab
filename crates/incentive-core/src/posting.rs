//! Phase 3: people and bots write posts (docs/model.md, 4.2 and 7.5).

use crate::bots;
use crate::content::{AuthorProfile, Group, STYLE_COUNT, Style, generate_post};
use crate::population::Archetype;
use crate::rng::{self, Stream, StreamRng};
use crate::world::{DayCtx, World};

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

/// Every active user who may post writes `Poisson(post_rate)` posts, then the
/// bots write theirs.
pub(crate) fn write_posts(world: &mut World, ctx: &DayCtx) {
    let targets = TargetPools::build(world);
    let temperature = ctx.config.world.choice_temperature;
    for i in 0..world.population() {
        let person = &world.people[i];
        if !person.state.today.opened || !person.can_post(ctx.day) {
            continue;
        }
        let mut rng = ctx.rng(i as u64, Stream::Posting);
        let count = rng::poisson(&mut rng, person.traits.post_rate);
        let mix = person.style_mix(temperature);
        let author = AuthorProfile {
            account: i as u32,
            is_bot: false,
            is_creator: person.traits.archetype == Archetype::Creator,
            group: person.traits.group,
            skill: person.traits.skill,
            temperament: person.traits.temperament,
            evasion: person.state.evasion,
        };
        let interests = person.traits.interests;
        for _ in 0..count {
            let style = draw_style(&mut rng, &mix);
            let id = world.posts.next_id();
            let mut post = generate_post(&mut rng, id, ctx.day, &author, style, &interests);
            if style == Style::Attack {
                post.target = targets.pick(&mut rng, author.group);
            }
            world.posts.push(post);
            world.accounts.recent_posts[i].push(id);
            world.tally.posts_by_style[style.index()] += 1;
            let today = &mut world.people[i].state.today;
            today.posts += 1;
            today.styles_written |= 1 << style.index();
        }
    }
    bots::write_posts(world, ctx);
}
