//! True engagement behaviour of a viewer facing a post (docs/model.md, 7.1).
//!
//! These are the probabilities people actually act with. The platform never
//! reads them directly: it sees counts and its own noisy predictions.

use crate::config::{DesignLevers, WorldAssumptions};
use crate::content::{Post, Style, TOPIC_COUNT};
use crate::population::Traits;

/// The logistic function.
pub(crate) fn sig(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

/// How the viewer relates to the post's author and the post.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Relation {
    /// Viewer and author follow each other.
    pub mutual: bool,
    /// The viewer follows the author.
    pub follows_author: bool,
    /// The post is an Attack aimed at the viewer.
    pub is_target: bool,
    /// The item is at the reshare depth cap.
    pub share_capped: bool,
}

/// Everything 7.1 derives for one viewer and one post.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Signals {
    pub same_group: bool,
    pub interest: f32,
    /// In-group appeal of hostility, `H`.
    pub hostility: f32,
    /// Provocation of the out-group, `X`.
    pub provocation: f32,
    /// Impulsive appeal, `A`.
    pub appeal: f32,
    /// Reflective value, `R`.
    pub reflective: f32,
    pub p_like: f32,
    pub p_reply: f32,
    /// Share of replies that are angry.
    pub angry_share: f32,
    pub p_share: f32,
    pub p_report: f32,
    pub p_see_less: f32,
    pub dwell: f32,
}

// Constants of 7.1. They are marked *calibrate* in the specification; values
// that differ from its text are recorded in its section 13.
const LIKE_BIAS: f32 = -3.5;
const LIKE_INTEREST: f32 = 0.6;
const LIKE_IMPULSE: f32 = 1.2;
const LIKE_REFLECTION: f32 = 1.5;
const LIKE_OUTGROUP_HOSTILITY_CUT: f32 = 0.8;
const REPLY_BIAS: f32 = -4.0;
const REPLY_INTEREST: f32 = 0.4;
const REPLY_PROVOCATION: f32 = 2.0;
const REPLY_PERSONAL: f32 = 1.5;
const REPLY_IMPULSE: f32 = 0.8;
const SHARE_BIAS: f32 = -5.5;
const SHARE_INTEREST: f32 = 0.5;
const SHARE_IMPULSE: f32 = 1.5;
const SHARE_REFLECTION: f32 = 2.0;
const REPORT_BIAS: f32 = -5.0;
const SEE_LESS_BIAS: f32 = -4.0;
/// Share of the reflective cost of outrage that hits its own group (text: 0.4).
const IN_GROUP_OUTRAGE_COST: f32 = 0.8;

/// The design and world values behaviour depends on.
#[derive(Clone, Copy, Debug)]
pub(crate) struct BehaviourEnv<'a> {
    pub design: &'a DesignLevers,
    pub world: &'a WorldAssumptions,
}

/// Interest-weighted quality, `quality * min(m, 2) / 2`.
pub(crate) fn weighted_quality(post: &Post, interest: f32) -> f32 {
    post.quality * interest.min(2.0) / 2.0
}

/// Computes the true engagement probabilities of `viewer` for `post`.
pub(crate) fn signals(viewer: &Traits, post: &Post, rel: Relation, env: BehaviourEnv) -> Signals {
    let world = env.world;
    let interest = (TOPIC_COUNT as f32 * viewer.interests[post.topic as usize]).clamp(0.0, 3.0);
    let same_group = viewer.group == post.group;
    let charge = post.outrage * (0.5 + viewer.partisanship);
    let (hostility, provocation) = if same_group {
        (charge, 0.0)
    } else {
        (0.0, charge)
    };
    let social_proof = if env.design.like_counts_visible {
        world.social_proof * (post.likes() as f32).ln_1p() / 100f32.ln()
    } else {
        0.0
    };
    let appeal =
        world.arousal_appeal * post.arousal + world.outrage_appeal * hostility + social_proof;
    let reflective = reflective_value(viewer, post, rel, interest, same_group);
    let imp = viewer.impulsivity;

    let mut p_like = sig(LIKE_BIAS
        + LIKE_INTEREST * interest
        + LIKE_IMPULSE * imp * appeal
        + LIKE_REFLECTION * (1.0 - imp) * reflective);
    if !same_group {
        p_like *= 1.0 - LIKE_OUTGROUP_HOSTILITY_CUT * post.outrage;
    }

    let personal_follow = post.style == Style::Personal && rel.follows_author;
    let reply_interest = REPLY_INTEREST * interest;
    let reply_provocation =
        REPLY_PROVOCATION * provocation * (1.0 - 0.5 * viewer.toxicity_tolerance);
    let reply_personal = if personal_follow { REPLY_PERSONAL } else { 0.0 };
    let reply_impulse = REPLY_IMPULSE * imp * appeal;
    let p_reply =
        sig(REPLY_BIAS + reply_interest + reply_provocation + reply_personal + reply_impulse);
    let positive_terms =
        reply_interest + reply_provocation + reply_personal + reply_impulse.max(0.0);
    let angry_share = if positive_terms > 0.0 {
        reply_provocation / positive_terms
    } else {
        0.0
    };

    let p_share = share_probability(viewer, post, rel, interest, hostility, env.design);
    let p_report = report_probability(viewer, post.toxicity, rel.is_target);
    let p_see_less = sig(SEE_LESS_BIAS + 3.0 * (-reflective).max(0.0));
    let dwell = 1.0 + 0.5 * post.arousal + 0.5 * weighted_quality(post, interest);
    Signals {
        same_group,
        interest,
        hostility,
        provocation,
        appeal,
        reflective,
        p_like,
        p_reply,
        angry_share,
        p_share,
        p_report,
        p_see_less,
        dwell,
    }
}

/// The chance of reporting a post of this toxicity (7.1); targets of an
/// Attack are likelier to report it.
pub(crate) fn report_probability(viewer: &Traits, toxicity: f32, is_target: bool) -> f32 {
    let felt = toxicity * (1.0 - viewer.toxicity_tolerance);
    sig(REPORT_BIAS + 4.0 * felt + if is_target { 1.5 } else { 0.0 })
}

fn reflective_value(
    viewer: &Traits,
    post: &Post,
    rel: Relation,
    interest: f32,
    same_group: bool,
) -> f32 {
    let personal_mutual = if post.style == Style::Personal && rel.mutual {
        0.3
    } else {
        0.0
    };
    let clickbait = if post.style == Style::Clickbait {
        0.3
    } else {
        0.0
    };
    let outrage_weight = if same_group {
        IN_GROUP_OUTRAGE_COST
    } else {
        1.0
    };
    0.8 * weighted_quality(post, interest) + personal_mutual + 0.2 * (post.truth - 0.5)
        - 0.8 * post.toxicity * (1.0 - viewer.toxicity_tolerance)
        - 0.4 * post.outrage * outrage_weight
        - clickbait
}

fn share_probability(
    viewer: &Traits,
    post: &Post,
    rel: Relation,
    interest: f32,
    hostility: f32,
    design: &DesignLevers,
) -> f32 {
    if rel.share_capped {
        return 0.0;
    }
    let imp = viewer.impulsivity;
    let shareable = matches!(post.style, Style::Informative | Style::Entertaining);
    let reflective = if shareable {
        SHARE_REFLECTION * (1.0 - imp) * post.quality
    } else {
        0.0
    };
    let base = sig(SHARE_BIAS
        + SHARE_INTEREST * interest
        + SHARE_IMPULSE * imp * (post.arousal + hostility)
        + reflective);
    let noted = if post.noted { 0.5 } else { 1.0 };
    base * (1.0 - design.share_friction * imp) * noted
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::content::{AuthorProfile, Group, generate_post};
    use crate::population::{Archetype, draw_traits};
    use crate::rng::{self, Stream};

    fn post_of(style: Style, group: Group) -> Post {
        let mut rng = rng::stream_rng(9, 0, style as u64, Stream::Posting);
        let author = AuthorProfile {
            account: 0,
            is_bot: false,
            is_creator: false,
            group,
            skill: 0.5,
            temperament: 0.5,
            evasion: 0.0,
        };
        generate_post(&mut rng, 0, 0, &author, style, &[1.0 / 6.0; 6])
    }

    #[test]
    fn outrage_is_liked_by_the_in_group_and_provokes_the_out_group() {
        let config = Config::default();
        let env = BehaviourEnv {
            design: &config.design,
            world: &config.world,
        };
        let mut rng = rng::stream_rng(9, 0, 99, Stream::Population);
        let mut viewer = draw_traits(&mut rng, Archetype::Partisan);
        viewer.group = Group::A;
        viewer.interests = [1.0 / 6.0; 6];
        let outrage_in = signals(
            &viewer,
            &post_of(Style::Outrage, Group::A),
            Relation::default(),
            env,
        );
        let outrage_out = signals(
            &viewer,
            &post_of(Style::Outrage, Group::B),
            Relation::default(),
            env,
        );
        let informative = signals(
            &viewer,
            &post_of(Style::Informative, Group::A),
            Relation::default(),
            env,
        );
        assert!(outrage_in.p_like > informative.p_like);
        assert!(outrage_in.reflective < informative.reflective);
        assert!(outrage_out.p_like < outrage_in.p_like);
        assert!(outrage_out.angry_share > 0.5);
    }
}
