//! Ready-made platform designs (docs/model.md, 14.1).

use crate::config::{Config, DesignLevers, ModerationAction, PayoutScheme, Sanctions};
use crate::error::ConfigError;

/// A named platform design.
#[derive(Clone, Copy, Debug)]
pub struct Preset {
    /// Stable kebab-case id.
    pub id: &'static str,
    /// Display name.
    pub name: &'static str,
    /// One plain-language paragraph for the public.
    pub description: &'static str,
    apply: fn(&mut DesignLevers),
}

impl Preset {
    /// The preset's design levers.
    pub fn design(&self) -> DesignLevers {
        let mut design = DesignLevers::default();
        (self.apply)(&mut design);
        design
    }

    /// The default configuration with this preset's design.
    pub fn config(&self) -> Config {
        Config {
            design: self.design(),
            ..Config::default()
        }
    }

    /// Replaces the design of `config` with this preset's, keeping its run,
    /// population and world.
    pub fn apply_to(&self, config: &mut Config) {
        config.design = self.design();
    }
}

/// Every preset, in display order; the first is the default design.
pub fn presets() -> &'static [Preset] {
    &PRESETS
}

/// A preset by id.
pub fn preset(id: &str) -> Result<&'static Preset, ConfigError> {
    PRESETS
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| ConfigError::UnknownPreset(id.to_string()))
}

fn quality_bridging(d: &mut DesignLevers) {
    d.w_like = 0.5;
    d.w_reply = 0.3;
    d.w_share = 0.5;
    d.w_quality = 1.0;
    d.w_bridging = 1.0;
    d.w_see_less = 1.0;
    d.diversity = 0.3;
    d.community_notes = true;
}

/// The engagement ranking with integrity measures (docs/model.md, 14.1):
/// demotion of borderline toxicity, a quality signal, see-less feedback and
/// demoted community notes. The stylized facts compare a chronological feed
/// with it (section 15).
pub(crate) fn integrity_ranking(d: &mut DesignLevers) {
    d.moderation_action = ModerationAction::Demote;
    d.classifier_threshold = 0.6;
    d.demote_penalty = 1.5;
    d.w_quality = 0.5;
    d.w_see_less = 1.0;
    d.community_notes = true;
    d.noted_penalty = 1.0;
}

/// The most engaging design the searches found (docs/model.md, 13.7 and
/// 14.1): front design 64 of the Pareto atlas, every lever as found, those
/// without effect in it included (the classifier's threshold of 1.05 flags
/// nothing, so the demotion and notes penalties never apply). `incentive
/// explore` leaves it out of its initial designs, so the atlas it comes from
/// stays reproducible.
fn max_engagement(d: &mut DesignLevers) {
    d.w_like = 2.9;
    d.w_share = 1.7;
    d.w_dwell = 3.0;
    d.w_in_network = 1.0;
    d.recommended_share = 0.0;
    d.diversity = 0.2;
    d.predictor_quality = 0.9;
    d.demote_penalty = 0.4;
    d.noted_penalty = 2.4;
    d.share_friction = 0.35;
    d.classifier_threshold = 1.05;
    d.review_capacity = 10.0;
    d.report_trust = 0.65;
    d.sanctions = Sanctions::None;
    d.payout_share = 0.2;
    d.payout_threshold_followers = 86;
    d.time_limit_share = 0.1;
    d.notification_intensity = 1.0;
    d.account_cost = 0.15;
    d.ads_share = 0.35;
    d.survey_after_session = 0.95;
}

const PRESETS: [Preset; 8] = [
    Preset {
        id: "engagement-maximiser",
        name: "Engagement ranking",
        description: "The feed shows whatever it predicts people will like, reply to, share and linger on, \
            mixing in trending posts from strangers. An automated filter removes the worst toxicity and human \
            moderators handle some reports. It is the default design and the yardstick every other design is \
            compared with, but it does not maximise engagement: the searches found a design with about 80% more \
            (the most engaging design found).",
        apply: |_| {},
    },
    Preset {
        id: "chronological",
        name: "Chronological",
        description: "No ranking at all: people see the posts of the accounts they follow, newest first, and \
            nothing from strangers. Moderation stays as in the engagement ranking. Studies of switching \
            Facebook and Instagram feeds to chronological found people spent less time on them.",
        apply: |d| d.chronological = true,
    },
    Preset {
        id: "quality-bridging",
        name: "Quality and bridging",
        description: "The feed still listens to likes, but much less, and adds a quality classifier, a bonus \
            for posts liked across the political divide, a penalty for authors people ask to see less of, and \
            some variety. Community notes add context to misleading posts. It is the design many researchers \
            propose as an alternative to pure engagement ranking.",
        apply: quality_bridging,
    },
    Preset {
        id: "calm-by-design",
        name: "Calm by design",
        description: "The engagement ranking stays, but the platform slows down: a prompt before sharing, \
            reshares that stop after two hops, hidden like counts, gentle notifications, a daily time limit \
            for those who want it, a stricter toxicity filter and three times the human moderators.",
        apply: |d| {
            d.share_friction = 0.5;
            d.reshare_depth_cap = Some(2);
            d.like_counts_visible = false;
            d.notification_intensity = 0.2;
            d.time_limit_share = 0.3;
            d.classifier_threshold = 0.5;
            d.review_capacity = 3.0;
        },
    },
    Preset {
        id: "subscription",
        name: "Subscription",
        description: "No ads: people pay a subscription when the platform is worth it to them, so revenue \
            follows the value people get rather than the time they spend. The feed uses the quality and \
            bridging ranking, and 30% of revenue goes to the authors whose posts subscribers valued.",
        apply: |d| {
            quality_bridging(d);
            d.ads_share = 0.0;
            d.payout_share = 0.3;
            d.payout_scheme = PayoutScheme::FanFunded;
        },
    },
    Preset {
        id: "engagement-paid-creators",
        name: "Engagement-paid creators",
        description: "The engagement ranking, plus a creator fund: 30% of ad revenue is paid to authors in \
            proportion to the likes, replies and shares their posts collect, angry replies and bot boosts \
            included. Creator programmes on several platforms pay this way.",
        apply: |d| {
            d.payout_share = 0.3;
            d.payout_scheme = PayoutScheme::PerEngagement;
        },
    },
    Preset {
        id: "integrity-ranking",
        name: "Engagement ranking with integrity",
        description: "The engagement ranking with the integrity measures large ranked feeds add to it. The \
            toxicity filter no longer removes posts: it pushes borderline and worse posts down the feed, \
            starting a little below the line where the plain ranking removes them. A quality classifier \
            pushes clickbait and shoddy posts down, the feed listens to people who ask to see less of an \
            author, and community notes add context to misleading posts, which sink in the feed too. It is \
            the realistic ranked feed to set a chronological feed against.",
        apply: integrity_ranking,
    },
    Preset {
        id: "max-engagement",
        name: "Most engaging design found",
        description: "The design with the highest engagement that the lab's design-space searches found. It weighs \
            likes and time spent far above the engagement ranking while keeping its reply and share weights, shows \
            only posts and reshares from accounts people follow, pushes \
            notifications at full strength, and has no working toxicity filter: ten times the human moderators \
            act on reports, weighted by each reporter's record, but strikes carry no sanctions. It pays 20% of \
            revenue to creators and takes most of its revenue from subscriptions. It comes from the searches, \
            not from any real platform, so it shows what maximising engagement looks like in this model and \
            what it costs: about 80% more engagement than the engagement ranking, with about 30% more harm, \
            twice the toxic share, more polarisation and lower wellbeing.",
        apply: max_engagement,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_validate_and_ids_are_unique() {
        for (i, p) in presets().iter().enumerate() {
            assert!(p.config().validate().is_ok(), "{}", p.id);
            assert!(presets()[i + 1..].iter().all(|q| q.id != p.id));
        }
        assert_eq!(presets()[0].design(), DesignLevers::default());
        assert!(preset("nope").is_err());
    }
}
