//! The inspector: the tracked individuals, one individual's traits, state,
//! learned style mix and history, and the feed they were last served, item by
//! item, with what the platform predicted next to what was true.

use egui::{Id, RichText, Ui};
use incentive_core::{
    AgentView, Archetype, ExplainedItem, FeedExplanation, FeedKind, Format, STYLE_COUNT,
    Simulation, Style, TOPICS,
};

use super::charts::{self, Chart, Series};
use super::palette;
use crate::lab::Lab;
use crate::lab::series::{high_score_cut, is_goodhart_gap, predicted_engagement};

/// Draws the inspector into the right panel.
pub fn show(ui: &mut Ui, lab: &mut Lab) {
    ui.heading("Inspector");
    ui.label(
        RichText::new(
            "Individuals followed since the run began, two of each kind, plus any you add. \
             Watching never changes the run.",
        )
        .small(),
    );
    let tracked = lab.sandbox.runner.lane(0).sim.tracked();
    if lab
        .inspector
        .selected
        .is_none_or(|id| !tracked.contains(&id))
    {
        lab.inspector.selected = tracked.first().copied();
    }
    tracked_list(ui, lab, &tracked);
    track_another(ui, lab);
    ui.separator();
    let Some(id) = lab.inspector.selected else {
        ui.label("Nobody is tracked.");
        return;
    };
    let sim = &lab.sandbox.runner.lane(0).sim;
    let Some(view) = sim.agent_view(id) else {
        ui.label("This individual does not exist in the current run.");
        return;
    };
    details(ui, sim, &view, lab.view.smooth);
}

/// The tracked individuals, each selectable.
fn tracked_list(ui: &mut Ui, lab: &mut Lab, tracked: &[u32]) {
    let sim = &lab.sandbox.runner.lane(0).sim;
    let mut select = None;
    egui::Grid::new("tracked-list")
        .num_columns(4)
        .striped(true)
        .show(ui, |ui| {
            ui.label(RichText::new("Who").strong());
            ui.label(RichText::new("Kind").strong());
            ui.label(RichText::new("Where").strong());
            ui.label(RichText::new("Wellbeing").strong());
            ui.end_row();
            for &id in tracked {
                let Some(person) = sim.person(id) else {
                    continue;
                };
                let archetype = person.traits.archetype;
                let selected = lab.inspector.selected == Some(id);
                if ui.selectable_label(selected, format!("#{id}")).clicked() {
                    select = Some(id);
                }
                ui.colored_label(palette::ARCHETYPES[archetype.index()], archetype.name());
                if person.state.is_user {
                    ui.label("on the platform");
                } else {
                    ui.label(RichText::new("off the platform").weak());
                }
                ui.label(format!("{:.2}", person.state.wellbeing));
                ui.end_row();
            }
        });
    if select.is_some() {
        lab.inspector.selected = select;
    }
}

/// "Track another": picks an untracked individual of a chosen archetype.
fn track_another(ui: &mut Ui, lab: &mut Lab) {
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("track-archetype")
            .selected_text(lab.inspector.archetype.name())
            .show_ui(ui, |ui| {
                for a in Archetype::ALL {
                    ui.selectable_value(&mut lab.inspector.archetype, a, a.name());
                }
            });
        if ui
            .button("Track another")
            .on_hover_text("Follow one more individual of this kind from today on")
            .clicked()
        {
            let archetype = lab.inspector.archetype;
            lab.track_another(archetype);
        }
    });
    if let Some(m) = &lab.inspector.message {
        ui.label(RichText::new(m).small());
    }
}

fn details(ui: &mut Ui, sim: &Simulation, view: &AgentView, smooth: bool) {
    let a = view.archetype;
    ui.label(
        RichText::new(format!("#{} - {}", view.id, a.name()))
            .size(15.0)
            .strong()
            .color(palette::ARCHETYPES[a.index()]),
    );
    egui::CollapsingHeader::new("Traits and state")
        .id_salt("inspector-traits")
        .default_open(true)
        .show(ui, |ui| traits_and_state(ui, view));
    ui.label(RichText::new("What they have learned to post").strong())
        .on_hover_text(
            "The chance of choosing each style for the next post, learned from the reward of \
             past posts: likes, followers, replies and money",
        );
    style_bars(ui, &view.style_mix);
    ui.add_space(4.0);
    history_chart(ui, sim, view.id, smooth);
    ui.add_space(4.0);
    match sim.served_feed(view.id) {
        Some(feed) => served_feed(ui, feed),
        None => {
            ui.label(RichText::new("Last served feed").strong());
            ui.label(
                RichText::new(
                    "No session yet since tracking began: this individual has not opened the app.",
                )
                .weak(),
            );
        }
    }
}

fn traits_and_state(ui: &mut Ui, view: &AgentView) {
    let t = &view.traits;
    let s = &view.state;
    let opt_day = |d: Option<u32>| d.map_or_else(|| "-".to_string(), |d| format!("day {d}"));
    let traits: [(&str, String, &str); 13] = [
        (
            "Opinion group",
            format!("{:?}", t.group),
            "Which side of the divide",
        ),
        (
            "Activity",
            format!("{:.2}", t.activity),
            "Base chance of opening the app on a day",
        ),
        (
            "Posts per day",
            format!("{:.2}", t.post_rate),
            "Mean posts per active day",
        ),
        (
            "Temperament",
            format!("{:.2}", t.temperament),
            "Propensity to incivility",
        ),
        (
            "Approval sensitivity",
            format!("{:.2}", t.approval_sensitivity),
            "Weight of likes and followers",
        ),
        (
            "Impulsivity",
            format!("{:.2}", t.impulsivity),
            "Weight of impulse over reflection",
        ),
        (
            "Toxicity tolerance",
            format!("{:.2}", t.toxicity_tolerance),
            "How little toxicity hurts",
        ),
        (
            "Vulnerability",
            format!("{:.2}", t.vulnerability),
            "Harm multiplier for harassment and comparison",
        ),
        (
            "Skill",
            format!("{:.2}", t.skill),
            "Quality of what they write",
        ),
        (
            "Money weight",
            format!("{:.2}", t.money_weight),
            "Weight of creator income",
        ),
        (
            "Credulity",
            format!("{:.2}", t.credulity),
            "Chance of believing misinformation",
        ),
        (
            "Partisanship",
            format!("{:.2}", t.partisanship),
            "Strength of their opinion",
        ),
        (
            "Time budget",
            format!("{} items", t.time_budget),
            "Items a day they would endorse",
        ),
    ];
    let state: [(&str, String, &str); 13] = [
        (
            "On the platform",
            if s.is_user { "yes" } else { "no" }.to_string(),
            "A current user",
        ),
        ("Joined", opt_day(s.joined_day), "Latest joining"),
        ("Left", opt_day(s.left_day), "Latest departure"),
        (
            "Wellbeing",
            format!("{:.3}", s.wellbeing),
            "Truth: in [0, 1]",
        ),
        (
            "Habit",
            format!("{:.3}", s.habit),
            "Truth: the pull to open the app",
        ),
        (
            "Polarisation",
            format!("{:.3}", s.affective_polarisation),
            "Truth: dislike of the other group",
        ),
        (
            "Informedness",
            format!("{:.3}", s.informedness),
            "Truth: how well informed",
        ),
        (
            "Followers",
            view.followers.to_string(),
            "Accounts following them",
        ),
        (
            "Following",
            s.follows.len().to_string(),
            "Accounts they follow",
        ),
        (
            "Blocked",
            s.blocked.len().to_string(),
            "Accounts they blocked",
        ),
        (
            "Strikes",
            format!("{}{}", s.strikes, if s.banned { ", banned" } else { "" }),
            "Strikes from human review on this account",
        ),
        (
            "Evasion",
            format!("{:.3}", s.evasion),
            "Skill at writing toxicity the classifier misses",
        ),
        (
            "Income (30 days)",
            format!("{:.4}", s.income_ema),
            "Average daily creator income",
        ),
    ];
    ui.columns(2, |cols| {
        for (col, (title, rows)) in cols
            .iter_mut()
            .zip([("Traits (fixed)", &traits), ("State (now)", &state)])
        {
            col.label(RichText::new(title).strong());
            egui::Grid::new(("agent-grid", title))
                .num_columns(2)
                .striped(true)
                .show(col, |ui| {
                    for (name, value, hover) in rows.iter() {
                        ui.label(*name).on_hover_text(*hover);
                        ui.label(value);
                        ui.end_row();
                    }
                });
        }
    });
}

/// The learned style mix as horizontal bars, each percentage beside its bar
/// so it reads on any bar colour.
fn style_bars(ui: &mut Ui, mix: &[f32; STYLE_COUNT]) {
    egui::Grid::new("style-bars").num_columns(3).show(ui, |ui| {
        for (s, p) in Style::ALL.iter().zip(mix) {
            ui.label(s.name());
            ui.add(
                egui::ProgressBar::new(*p)
                    .desired_width(200.0)
                    .fill(palette::STYLES[s.index()]),
            );
            ui.label(format!("{:.0}%", p * 100.0));
            ui.end_row();
        }
    });
}

/// Wellbeing and habit, day by day.
fn history_chart(ui: &mut Ui, sim: &Simulation, id: u32, smooth: bool) {
    let (mut wellbeing, mut habit) = (Vec::new(), Vec::new());
    if let Some(days) = sim.agent_history(id) {
        for d in days {
            wellbeing.push([f64::from(d.day), f64::from(d.wellbeing)]);
            habit.push([f64::from(d.day), f64::from(d.habit)]);
        }
    }
    charts::line_chart(
        ui,
        Chart {
            id: Id::new(("agent-history", id)),
            title: Some("Wellbeing and habit (truth)"),
            height: 140.0,
            day: sim.day(),
            series: vec![
                Series::new("Wellbeing", palette::GREEN, wellbeing),
                Series::new("Habit", palette::ORANGE, habit),
            ],
            markers: &[],
            references: Vec::new(),
            format: Format::Score,
            smooth,
            include_zero: false,
        },
    );
}

fn feed_kind_name(kind: FeedKind) -> &'static str {
    match kind {
        FeedKind::Ranked => "ranked by the design's weights",
        FeedKind::Chronological => "chronological (scores show what ranking would have given)",
        FeedKind::Quality => "the quality-ranked alternative feed",
        FeedKind::Bridging => "the bridging-ranked alternative feed",
    }
}

/// The last served feed as a table.
fn served_feed(ui: &mut Ui, feed: &FeedExplanation) {
    let seen = feed
        .items
        .iter()
        .filter(|i| i.actions.is_some_and(|a| a.seen))
        .count();
    ui.label(RichText::new(format!("Last served feed, day {}", feed.day)).strong());
    ui.label(
        RichText::new(format!(
            "{}; {} items, {} seen before the session ended (cap {}).",
            feed_kind_name(feed.kind),
            feed.items.len(),
            seen,
            feed.session_limit
        ))
        .small(),
    );
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Highlighted:").small().strong());
        ui.label(
            RichText::new(
                "the platform scored it high, yet the viewer would rather not have seen it \
                 (negative reflective value). That is Goodhart's law in one line.",
            )
            .small(),
        );
    });
    ui.label(
        RichText::new("Dimmed: the viewer stopped scrolling before reaching it.")
            .small()
            .weak(),
    );
    let cut = high_score_cut(feed);
    let gaps: Vec<bool> = feed.items.iter().map(|i| is_goodhart_gap(i, cut)).collect();
    let header_rows = 1;
    egui::ScrollArea::horizontal()
        .id_salt("feed-scroll")
        .show(ui, |ui| {
            egui::Grid::new("served-feed")
                .num_columns(12)
                .spacing([10.0, 3.0])
                .with_row_color(move |row, _style| {
                    row.checked_sub(header_rows)
                        .and_then(|i| gaps.get(i).copied())
                        .filter(|gap| *gap)
                        .map(|_| palette::GAP_FILL)
                })
                .show(ui, |ui| {
                    feed_header(ui);
                    for item in &feed.items {
                        feed_row(ui, item);
                    }
                });
        });
}

fn feed_header(ui: &mut Ui) {
    let columns: [(&str, &str); 12] = [
        ("#", "Position in the feed"),
        ("Style", "The post's style"),
        ("Topic", "The post's topic"),
        (
            "Source",
            "Followed account, a followed account's reshare, or a recommendation from a stranger",
        ),
        ("Score", "The ranking score; expand for its components"),
        (
            "Predicted",
            "Observable: the platform's predicted engagement (weighted like, reply, share and dwell terms)",
        ),
        (
            "Appeal",
            "Truth: how much the viewer is drawn to it on impulse",
        ),
        (
            "Reflective",
            "Truth: how much the viewer would endorse having seen it",
        ),
        ("Quality", "Truth: the post's hidden craft and substance"),
        ("Toxicity", "Truth: the post's hidden harm"),
        (
            "Truthful",
            "Truth: the post's truthfulness (below 0.5 is misinformation)",
        ),
        ("Viewer did", "What the viewer did with it"),
    ];
    for (name, hover) in columns {
        ui.label(RichText::new(name).strong()).on_hover_text(hover);
    }
    ui.end_row();
}

fn feed_row(ui: &mut Ui, item: &ExplainedItem) {
    let seen = item.actions.is_some_and(|a| a.seen);
    let weak = ui.visuals().weak_text_color();
    let text = |s: String| -> RichText {
        let t = RichText::new(s);
        if seen { t } else { t.color(weak) }
    };
    let post = &item.post;
    ui.label(text((item.rank + 1).to_string()));
    let style_color = if seen {
        palette::STYLES[post.style.index()]
    } else {
        weak
    };
    ui.label(RichText::new(post.style.name()).color(style_color));
    ui.label(text(
        TOPICS[usize::from(post.topic).min(TOPICS.len() - 1)].to_string(),
    ));
    let author = if post.author_is_bot {
        "a bot".to_string()
    } else {
        format!("#{}", post.author)
    };
    let source = match (item.via, item.in_network) {
        (Some(via), _) => format!("reshared by #{via}, from {author}"),
        (None, true) => format!("followed: {author}"),
        (None, false) => format!("recommended: {author}"),
    };
    ui.label(text(source));
    egui::CollapsingHeader::new(text(format!("{:.2}", item.score)))
        .id_salt(("score", post.id, item.rank))
        .default_open(false)
        .show(ui, |ui| score_components(ui, item));
    ui.label(text(format!("{:.2}", predicted_engagement(item))));
    ui.label(text(format!("{:.2}", item.truth.appeal)));
    let reflective = item.truth.reflective;
    let reflective_text = RichText::new(format!("{reflective:+.2}"));
    ui.label(if !seen {
        reflective_text.color(weak)
    } else if reflective < 0.0 {
        reflective_text.color(palette::VERMILLION)
    } else {
        reflective_text
    });
    ui.label(text(format!("{:.2}", post.quality)));
    ui.label(text(format!("{:.2}", post.toxicity)));
    ui.label(text(format!("{:.2}", post.truth)));
    ui.label(text(actions_text(item)));
    ui.end_row();
}

fn score_components(ui: &mut Ui, item: &ExplainedItem) {
    let c = &item.components;
    let parts = [
        ("like", c.like),
        ("reply", c.reply),
        ("share", c.share),
        ("dwell", c.dwell),
        ("quality", c.quality),
        ("bridging", c.bridging),
        ("recency", c.recency),
        ("in-network", c.in_network),
        ("see-less", c.see_less),
        ("demoted", c.demoted),
        ("noted", c.noted),
        ("diversity", c.diversity),
    ];
    for (name, value) in parts.iter().filter(|(_, v)| v.abs() > 1e-6) {
        let label = RichText::new(format!("{name} {value:+.3}")).small();
        ui.label(if *value < 0.0 {
            label.color(palette::VERMILLION)
        } else {
            label
        });
    }
}

/// What the viewer did with an item, in words.
pub fn actions_text(item: &ExplainedItem) -> String {
    let Some(a) = item.actions else {
        return "preview".into();
    };
    if !a.seen {
        return "never reached".into();
    }
    let mut did = Vec::new();
    if a.liked {
        did.push("liked");
    }
    if a.replied {
        did.push(if a.angry {
            "replied angrily"
        } else {
            "replied"
        });
    }
    if a.shared {
        did.push("shared");
    }
    if a.reported {
        did.push("reported");
    }
    if a.see_less {
        did.push("asked for less");
    }
    if a.followed {
        did.push("followed the author");
    }
    if a.unfollowed {
        did.push("unfollowed the author");
    }
    if did.is_empty() {
        "scrolled past".into()
    } else {
        did.join(", ")
    }
}
