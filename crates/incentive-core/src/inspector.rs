//! The inspector: tracked individuals with a bounded daily history and the
//! last feed each was served, views of one individual, and feed explanations.

use std::collections::VecDeque;

use serde::Serialize;

use crate::config::Config;
use crate::content::{Post, STYLE_COUNT};
use crate::person::PersonState;
use crate::population::{Archetype, Traits};
use crate::ranking::{Candidate, FeedKind, ScoreComponents};
use crate::world::World;

/// Days of history kept per tracked individual.
pub const AGENT_HISTORY_DAYS: usize = 730;

/// One day in the life of a tracked individual.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AgentDay {
    /// The day.
    pub day: u32,
    /// Wellbeing at the end of the day.
    pub wellbeing: f32,
    /// Habit at the end of the day.
    pub habit: f32,
    /// Probability of choosing each style.
    pub style_mix: [f32; STYLE_COUNT],
    /// Posts written today.
    pub posts: u32,
    /// Likes received today, bots included.
    pub likes_received: u32,
    /// Harassment events today.
    pub harassment_events: u32,
    /// Creator income today.
    pub income: f32,
    /// On the platform at the end of the day.
    pub is_user: bool,
}

/// One tracked individual.
#[derive(Clone, Debug)]
struct Tracked {
    id: u32,
    history: VecDeque<AgentDay>,
    /// The feed of the last session, recorded as it was served.
    last_feed: Option<FeedExplanation>,
}

/// Tracked individuals, their histories and their last served feeds.
#[derive(Clone, Debug, Default)]
pub(crate) struct Inspector {
    tracked: Vec<Tracked>,
}

impl Inspector {
    /// Tracks `run.tracked_agents` individuals, cycling through the archetypes
    /// and taking the lowest indices of each.
    pub(crate) fn new(world: &World, config: &Config) -> Self {
        let mut by_archetype: [Vec<u32>; 5] = Default::default();
        for (i, p) in world.people.iter().enumerate() {
            by_archetype[p.traits.archetype.index()].push(i as u32);
        }
        let mut inspector = Self::default();
        let wanted = config.run.tracked_agents as usize;
        let mut round = 0;
        while inspector.tracked.len() < wanted && round < world.population() {
            for list in &by_archetype {
                if inspector.tracked.len() < wanted
                    && let Some(&id) = list.get(round)
                {
                    inspector.track(id);
                }
            }
            round += 1;
        }
        inspector
    }

    /// Starts tracking an individual; false when already tracked.
    pub(crate) fn track(&mut self, id: u32) -> bool {
        if self.is_tracked(id) {
            return false;
        }
        self.tracked.push(Tracked {
            id,
            history: VecDeque::new(),
            last_feed: None,
        });
        true
    }

    pub(crate) fn is_tracked(&self, id: u32) -> bool {
        self.tracked.iter().any(|t| t.id == id)
    }

    pub(crate) fn tracked_ids(&self) -> Vec<u32> {
        self.tracked.iter().map(|t| t.id).collect()
    }

    pub(crate) fn history(&self, id: u32) -> Option<&VecDeque<AgentDay>> {
        self.tracked.iter().find(|t| t.id == id).map(|t| &t.history)
    }

    /// The last feed a session served to a tracked individual.
    pub(crate) fn served_feed(&self, id: u32) -> Option<&FeedExplanation> {
        self.tracked
            .iter()
            .find(|t| t.id == id)
            .and_then(|t| t.last_feed.as_ref())
    }

    /// Keeps a served feed as its viewer's last; ignored for untracked viewers.
    pub(crate) fn record_feed(&mut self, feed: FeedExplanation) {
        if let Some(t) = self.tracked.iter_mut().find(|t| t.id == feed.viewer) {
            t.last_feed = Some(feed);
        }
    }

    /// Records the end of `day` for every tracked individual.
    pub(crate) fn record(&mut self, world: &World, day: u32, temperature: f32) {
        for Tracked { id, history, .. } in &mut self.tracked {
            let Some(p) = world.people.get(*id as usize) else {
                continue;
            };
            if history.len() == AGENT_HISTORY_DAYS {
                history.pop_front();
            }
            history.push_back(AgentDay {
                day,
                wellbeing: p.state.wellbeing,
                habit: p.state.habit,
                style_mix: p.style_mix(temperature),
                posts: p.state.today.posts,
                likes_received: p.state.today.likes_received,
                harassment_events: p.state.today.harassment,
                income: p.state.today.income,
                is_user: p.state.is_user,
            });
        }
    }
}

/// Everything about one individual right now.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AgentView {
    /// Individual index.
    pub id: u32,
    /// Archetype, for convenience.
    pub archetype: Archetype,
    /// Fixed traits.
    pub traits: Traits,
    /// Evolving state.
    pub state: PersonState,
    /// Accounts following this individual.
    pub followers: u32,
    /// Probability of choosing each style.
    pub style_mix: [f32; STYLE_COUNT],
    /// Whether the inspector tracks this individual.
    pub tracked: bool,
}

/// What the viewer truly feels about an item; the platform never sees these.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct TrueResponse {
    /// Impulsive appeal.
    pub appeal: f32,
    /// Reflective value `R`: what the viewer would endorse on reflection,
    /// before the time cost (docs/model.md, 7.1).
    pub reflective: f32,
    /// The time cost of seeing the item, `time_cost * D` (docs/model.md,
    /// 7.2): the viewer's wellbeing reads `reflective` minus this, and the
    /// item is worth its time when `reflective` exceeds it.
    pub time_cost: f32,
    /// True like probability.
    pub p_like: f32,
    /// True reply probability.
    pub p_reply: f32,
    /// True share probability.
    pub p_share: f32,
}

/// What a viewer did with one item of a served feed (docs/model.md, 7.1 and
/// 7.6).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ItemActions {
    /// The viewer reached the item before the session stopped.
    pub seen: bool,
    /// Liked it.
    pub liked: bool,
    /// Replied to it.
    pub replied: bool,
    /// The reply was angry.
    pub angry: bool,
    /// Shared it.
    pub shared: bool,
    /// Reported it.
    pub reported: bool,
    /// Asked to see less of it.
    pub see_less: bool,
    /// Started following its author.
    pub followed: bool,
    /// Stopped following its author.
    pub unfollowed: bool,
}

/// One item of an explained feed.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ExplainedItem {
    /// Position in the feed, from 0.
    pub rank: usize,
    /// The post with all its attributes and counters.
    pub post: Post,
    /// From a followed account (or its reshare).
    pub in_network: bool,
    /// The followed account whose reshare brought it, if any.
    pub via: Option<u32>,
    /// Reshare depth of the item.
    pub depth: u8,
    /// Every score component of docs/model.md, 6.4.
    pub components: ScoreComponents,
    /// The total score: the sum of the components, the diversity adjustment
    /// included.
    pub score: f32,
    /// What the viewer truly feels about it.
    pub truth: TrueResponse,
    /// What the viewer did with it in a served feed; `None` in a preview,
    /// which no session served.
    pub actions: Option<ItemActions>,
}

/// A feed, item by item: either served by a session or a preview.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct FeedExplanation {
    /// The viewer.
    pub viewer: u32,
    /// The day the feed is for.
    pub day: u32,
    /// Whether a session served this feed (the inspector's record of a
    /// tracked individual) rather than a preview computed on request.
    pub served: bool,
    /// Which feed the viewer gets. For a chronological feed the order is by
    /// time and the scores show what the main ranking would have given.
    pub kind: FeedKind,
    /// The session's item cap.
    pub session_limit: usize,
    /// The items in feed order.
    pub items: Vec<ExplainedItem>,
}

/// The explained items of a built feed, in order, with every post as it is
/// now. `actions` is the starting record of each item: `None` for a preview,
/// nothing done yet for a feed about to be served. `time_cost` is the world
/// assumption that prices each item's dwell.
pub(crate) fn explained_items(
    world: &World,
    candidates: &[Candidate],
    actions: Option<ItemActions>,
    time_cost: f32,
) -> Vec<ExplainedItem> {
    candidates
        .iter()
        .enumerate()
        .filter_map(|(rank, c)| {
            let post = world.posts.get(c.post)?.clone();
            let s = &c.signals;
            Some(ExplainedItem {
                rank,
                post,
                in_network: c.in_network,
                via: c.via,
                depth: c.depth,
                components: c.components,
                score: c.score,
                truth: TrueResponse {
                    appeal: s.appeal,
                    reflective: s.reflective,
                    time_cost: crate::session::time_cost(s.dwell, time_cost),
                    p_like: s.p_like,
                    p_reply: s.p_reply,
                    p_share: s.p_share,
                },
                actions,
            })
        })
        .collect()
}
