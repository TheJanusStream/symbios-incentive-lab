//! Archetypes, traits and offline ties (docs/model.md, 3.1, 3.2 and 3.4).

use serde::Serialize;

use crate::content::{Group, STYLE_COUNT, TOPIC_COUNT};
use crate::rng::{self, Stream, StreamRng};

/// One of the five kinds of people. Archetypes only set the distributions
/// traits are drawn from; behaviour then follows from the traits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
pub enum Archetype {
    /// Mostly reads, posts about everyday life.
    Casual,
    /// Posts a lot, cares about audience and income.
    Creator,
    /// Follows public affairs, strong opinions.
    Partisan,
    /// Enjoys provoking and attacking.
    Troll,
    /// Hurt most by harassment and comparison.
    Vulnerable,
}

/// Number of archetypes.
pub const ARCHETYPE_COUNT: usize = 5;

impl Archetype {
    /// Every archetype in index order.
    pub const ALL: [Self; ARCHETYPE_COUNT] = [
        Self::Casual,
        Self::Creator,
        Self::Partisan,
        Self::Troll,
        Self::Vulnerable,
    ];

    /// Position in [`Archetype::ALL`].
    pub fn index(self) -> usize {
        self as usize
    }

    /// Display name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Casual => "Casual",
            Self::Creator => "Creator",
            Self::Partisan => "Partisan",
            Self::Troll => "Troll",
            Self::Vulnerable => "Vulnerable",
        }
    }

    /// Lower-case key for files and columns.
    pub fn key(self) -> &'static str {
        match self {
            Self::Casual => "casual",
            Self::Creator => "creator",
            Self::Partisan => "partisan",
            Self::Troll => "troll",
            Self::Vulnerable => "vulnerable",
        }
    }
}

/// The archetype means of section 3.2.
struct Profile {
    activity: f32,
    post_rate: f32,
    temperament: f32,
    approval_sensitivity: f32,
    impulsivity: f32,
    toxicity_tolerance: f32,
    vulnerability: f32,
    skill: f32,
    money_weight: f32,
    credulity: f32,
    partisanship: f32,
    time_budget: u32,
    /// Informative, Entertaining, Personal, Outrage, Clickbait, Attack.
    intrinsic: [f32; STYLE_COUNT],
}

#[rustfmt::skip]
const PROFILES: [Profile; ARCHETYPE_COUNT] = [
    Profile { activity: 0.60, post_rate: 0.3, temperament: 0.20, approval_sensitivity: 0.40, impulsivity: 0.50,
        toxicity_tolerance: 0.40, vulnerability: 0.30, skill: 0.40, money_weight: 0.10, credulity: 0.40,
        partisanship: 0.30, time_budget: 20, intrinsic: [0.2, 0.4, 0.6, 0.0, -0.2, -0.6] },
    Profile { activity: 0.90, post_rate: 2.0, temperament: 0.25, approval_sensitivity: 0.75, impulsivity: 0.45,
        toxicity_tolerance: 0.50, vulnerability: 0.35, skill: 0.65, money_weight: 0.70, credulity: 0.30,
        partisanship: 0.35, time_budget: 40, intrinsic: [0.4, 0.5, 0.3, 0.0, 0.1, -0.5] },
    Profile { activity: 0.75, post_rate: 1.0, temperament: 0.45, approval_sensitivity: 0.60, impulsivity: 0.60,
        toxicity_tolerance: 0.55, vulnerability: 0.30, skill: 0.45, money_weight: 0.20, credulity: 0.55,
        partisanship: 0.85, time_budget: 30, intrinsic: [0.5, 0.2, 0.2, 0.3, 0.0, -0.2] },
    Profile { activity: 0.80, post_rate: 1.5, temperament: 0.85, approval_sensitivity: 0.70, impulsivity: 0.70,
        toxicity_tolerance: 0.90, vulnerability: 0.10, skill: 0.30, money_weight: 0.10, credulity: 0.40,
        partisanship: 0.70, time_budget: 30, intrinsic: [0.0, 0.2, 0.0, 0.5, 0.2, 0.6] },
    Profile { activity: 0.60, post_rate: 0.3, temperament: 0.15, approval_sensitivity: 0.50, impulsivity: 0.55,
        toxicity_tolerance: 0.15, vulnerability: 0.80, skill: 0.40, money_weight: 0.10, credulity: 0.45,
        partisanship: 0.30, time_budget: 15, intrinsic: [0.2, 0.3, 0.6, -0.1, -0.2, -0.8] },
];

/// Concentration of the trait Beta distributions.
const TRAIT_CONCENTRATION: f32 = 8.0;
/// Dirichlet parameter of the interest draw.
const INTEREST_ALPHA: f32 = 0.7;
/// Spread of the per-individual intrinsic style values.
const INTRINSIC_SD: f32 = 0.1;

/// Fixed traits of one individual (docs/model.md, 3.2).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Traits {
    /// The archetype the traits were drawn from.
    pub archetype: Archetype,
    /// Opinion group, by the sign of the opinion.
    pub group: Group,
    /// Base chance of opening the app on a day.
    pub activity: f32,
    /// Mean posts per active day.
    pub post_rate: f32,
    /// Propensity to incivility.
    pub temperament: f32,
    /// Weight of social reward.
    pub approval_sensitivity: f32,
    /// Weight of impulse over reflection.
    pub impulsivity: f32,
    /// How little toxicity hurts.
    pub toxicity_tolerance: f32,
    /// Harm multiplier for harassment and comparison.
    pub vulnerability: f32,
    /// Quality of what the individual writes.
    pub skill: f32,
    /// Weight of creator income.
    pub money_weight: f32,
    /// Chance of believing misinformation.
    pub credulity: f32,
    /// Magnitude of the opinion.
    pub partisanship: f32,
    /// Items per day the individual would endorse.
    pub time_budget: u32,
    /// Signed opinion: positive for group A, negative for group B.
    pub opinion: f32,
    /// Interest in each topic, summing to 1.
    pub interests: [f32; TOPIC_COUNT],
    /// Value of writing each style before any reward.
    pub intrinsic: [f32; STYLE_COUNT],
    /// Personal wellbeing baseline.
    pub wellbeing_baseline: f32,
    /// Baseline affective polarisation.
    pub polarisation_baseline: f32,
}

/// Draws an archetype from (unnormalised) shares.
pub(crate) fn draw_archetype(rng: &mut StreamRng, shares: [f32; ARCHETYPE_COUNT]) -> Archetype {
    let total: f32 = shares.iter().sum();
    let mut target = rng::uniform(rng) * total;
    for (archetype, share) in Archetype::ALL.iter().zip(shares) {
        if target < share {
            return *archetype;
        }
        target -= share;
    }
    Archetype::Casual
}

/// Draws the traits of one individual of the given archetype.
pub(crate) fn draw_traits(rng: &mut StreamRng, archetype: Archetype) -> Traits {
    let p = &PROFILES[archetype.index()];
    let mut beta = |m: f32| rng::beta_mean(rng, m, TRAIT_CONCENTRATION);
    let activity = beta(p.activity);
    let temperament = beta(p.temperament);
    let approval_sensitivity = beta(p.approval_sensitivity);
    let impulsivity = beta(p.impulsivity);
    let toxicity_tolerance = beta(p.toxicity_tolerance);
    let vulnerability = beta(p.vulnerability);
    let skill = beta(p.skill);
    let money_weight = beta(p.money_weight);
    let credulity = beta(p.credulity);
    let partisanship = beta(p.partisanship);
    let group = if rng::chance(rng, 0.5) {
        Group::A
    } else {
        Group::B
    };
    let opinion = match group {
        Group::A => partisanship,
        Group::B => -partisanship,
    };
    let interests = draw_interests(rng, archetype);
    let intrinsic = draw_intrinsic(rng, p, temperament);
    let wellbeing_baseline = (0.6 + rng::normal(rng, 0.1)).clamp(0.0, 1.0);
    Traits {
        archetype,
        group,
        activity,
        post_rate: p.post_rate,
        temperament,
        approval_sensitivity,
        impulsivity,
        toxicity_tolerance,
        vulnerability,
        skill,
        money_weight,
        credulity,
        partisanship,
        time_budget: p.time_budget,
        opinion,
        interests,
        intrinsic,
        wellbeing_baseline,
        polarisation_baseline: 0.25 + 0.4 * partisanship,
    }
}

fn draw_interests(rng: &mut StreamRng, archetype: Archetype) -> [f32; TOPIC_COUNT] {
    let mut interests = [0.0; TOPIC_COUNT];
    for weight in &mut interests {
        *weight = rng::gamma(rng, INTEREST_ALPHA).max(1e-6);
    }
    if archetype == Archetype::Partisan {
        interests[0] *= 3.0;
    }
    let total: f32 = interests.iter().sum();
    interests.iter_mut().for_each(|w| *w /= total);
    interests
}

fn draw_intrinsic(rng: &mut StreamRng, p: &Profile, temperament: f32) -> [f32; STYLE_COUNT] {
    let mut intrinsic = p.intrinsic;
    for value in &mut intrinsic {
        *value += rng::normal(rng, INTRINSIC_SD);
    }
    intrinsic[3] += 0.5 * temperament;
    intrinsic[5] += 0.6 * (temperament - 0.5);
    intrinsic
}

/// Draws the offline ties. Ties are friendships, so they are symmetric: each
/// individual draws half of `count` partners (in its own group with
/// probability `homophily`) and every tie is added both ways, which gives a
/// mean of about `count` ties per individual.
pub(crate) fn draw_ties(seed: u64, groups: &[Group], count: u32, homophily: f32) -> Vec<Vec<u32>> {
    let members: [Vec<u32>; 2] = [Group::A, Group::B].map(|g| {
        (0..groups.len() as u32)
            .filter(|&i| groups[i as usize] == g)
            .collect()
    });
    let mut ties: Vec<Vec<u32>> = vec![Vec::new(); groups.len()];
    let half = count.div_ceil(2);
    for (i, &group) in groups.iter().enumerate() {
        let mut rng = rng::stream_rng(seed, 0, i as u64, Stream::Ties);
        for other in draw_ties_of(&mut rng, i as u32, group, &members, half, homophily) {
            if !ties[i].contains(&other) {
                ties[i].push(other);
                ties[other as usize].push(i as u32);
            }
        }
    }
    ties
}

fn draw_ties_of(
    rng: &mut StreamRng,
    me: u32,
    group: Group,
    members: &[Vec<u32>; 2],
    count: u32,
    homophily: f32,
) -> Vec<u32> {
    let mut ties = Vec::with_capacity(count as usize);
    let mut attempts = 0;
    while ties.len() < count as usize && attempts < count * 20 {
        attempts += 1;
        let same = rng::chance(rng, homophily);
        let pool = &members[if same {
            group.index()
        } else {
            1 - group.index()
        }];
        if pool.is_empty() {
            continue;
        }
        let other = pool[rng::index(rng, pool.len())];
        if other != me && !ties.contains(&other) {
            ties.push(other);
        }
    }
    ties
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traits_follow_the_archetype() {
        let mut rng = rng::stream_rng(3, 0, 0, Stream::Population);
        let n = 400;
        let mean = |a: Archetype, rng: &mut StreamRng| {
            (0..n).map(|_| draw_traits(rng, a).temperament).sum::<f32>() / n as f32
        };
        let troll = mean(Archetype::Troll, &mut rng);
        let casual = mean(Archetype::Casual, &mut rng);
        assert!((troll - 0.85).abs() < 0.03, "troll {troll}");
        assert!((casual - 0.20).abs() < 0.03, "casual {casual}");
    }

    #[test]
    fn interests_sum_to_one() {
        let mut rng = rng::stream_rng(3, 0, 1, Stream::Population);
        let t = draw_traits(&mut rng, Archetype::Partisan);
        assert!((t.interests.iter().sum::<f32>() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn ties_respect_homophily() {
        let groups: Vec<Group> = (0..400)
            .map(|i| if i % 2 == 0 { Group::A } else { Group::B })
            .collect();
        let ties = draw_ties(5, &groups, 8, 0.7);
        let (mut same, mut total) = (0, 0);
        for (i, list) in ties.iter().enumerate() {
            assert!(list.iter().all(|&j| ties[j as usize].contains(&(i as u32))));
            same += list
                .iter()
                .filter(|&&j| groups[j as usize] == groups[i])
                .count();
            total += list.len();
        }
        let share = same as f32 / total as f32;
        let mean = total as f32 / ties.len() as f32;
        assert!((share - 0.7).abs() < 0.04, "share {share}");
        assert!((mean - 8.0).abs() < 0.5, "mean {mean}");
    }
}
