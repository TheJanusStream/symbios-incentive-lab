//! The About tab: what the lab is and how to read it, in plain language.

use egui::{RichText, Ui};

use super::palette;

/// The repository the lab lives in.
pub const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
/// The model specification inside the repository.
pub const MODEL_DOC: &str = "docs/model.md";

/// The five stylized facts the model is checked against, with sources
/// (docs/model.md, section 15).
pub const FACTS: [(&str, &str); 5] = [
    (
        "Engagement ranking amplifies divisive content that people do not value. Against a \
         chronological feed, the engagement maximiser shows more outrage, and what it shows is \
         worth less to people on reflection.",
        "Milli et al. (2025), an audit of Twitter's ranking, PNAS Nexus",
    ),
    (
        "Social reward teaches outrage. Under engagement ranking the share of outrage posts rises \
         over time; when authors do not learn from likes, it does not.",
        "Brady et al. (2021), Science Advances",
    ),
    (
        "A chronological feed cuts time spent, and over three months barely moves polarisation.",
        "Guess et al. (2023), Facebook and Instagram in the 2020 US election, Science",
    ),
    (
        "Forwarding limits slow viral spread: capping reshares shrinks the largest cascades.",
        "de Freitas Melo et al. (2019), WhatsApp's forwarding limits",
    ),
    (
        "Harassment drives its targets away: more trolls, more vulnerable people leave.",
        "Pew Research Center (2017), Online Harassment 2017",
    ),
];

/// Findings that came out of the model's calibration rather than into it
/// (docs/model.md, 13.3), offered as things to discover.
pub const DISCOVERIES: [(&str, &str); 6] = [
    (
        "Is the angry-reply bonus worth it?",
        "Ranking up posts people reply to, angry replies included, looks like an engagement win. \
         Over a year it is not even that: without it engagement holds and toxicity falls by more \
         than half, because the bile drives people away.",
    ),
    (
        "What does a calmer feed cost?",
        "Ranking for quality and cross-group approval cuts harm by about four fifths for about a \
         third of the engagement. Find the designs in between.",
    ),
    (
        "Does the engagement maximiser's lead last?",
        "In the default world its lead grows as habits form. Make habits weaker and reputation \
         matter more (world assumptions), and the lead peaks mid-year and then shrinks.",
    ),
    (
        "Does stricter moderation work?",
        "A stricter toxicity classifier barely lowers what people see over a year: authors whose \
         posts are removed learn to write toxicity the classifier misses.",
    ),
    (
        "Should toxic posts be ranked down instead of removed?",
        "Ranking them down hides them from the very people who would report them, so strict \
         enforcement with heavy down-ranking catches fewer trolls than strict enforcement alone.",
    ),
    (
        "Can bots game a bridging feed?",
        "Coordinated bots of both groups boost the same posts, which then look liked across the \
         divide. Put everyone on the bridging alternative feed and misinformation rises while \
         toxicity falls. Adding a quality signal resists it.",
    ),
];

fn paragraph(ui: &mut Ui, text: &str) {
    ui.label(text);
    ui.add_space(4.0);
}

fn heading(ui: &mut Ui, text: &str) {
    ui.add_space(8.0);
    ui.label(RichText::new(text).strong().size(17.0));
    ui.add_space(2.0);
}

/// Draws the About tab.
pub fn show(ui: &mut Ui) {
    egui::ScrollArea::vertical()
        .id_salt("about")
        .show(ui, |ui| {
            ui.set_max_width(820.0);
            ui.heading("About the Symbios Incentive Lab");
            paragraph(
                ui,
                "This lab simulates one social platform and the society around it. A thousand or \
                 two simulated people read ranked feeds, post in six styles (informative, \
                 entertaining, personal, outrage, clickbait and attacks), like, reply, share and \
                 report, get harassed, learn which kinds of posts pay off, and join or leave. You \
                 design the platform and watch what happens to engagement, to harm, and to the \
                 people on it and off it.",
            );
            paragraph(
                ui,
                "The question it explores: can a platform that maximises engagement also keep \
                 toxic outcomes low and benefits high, and where does that break?",
            );

            heading(ui, "How to read it");
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new("Platform sees")
                        .strong()
                        .color(palette::OBSERVABLE),
                );
                ui.label(
                    "marks what the platform can measure: engagement, reports, a weekly survey. \
                     Its ranking, moderation and payouts can use only these numbers.",
                );
            });
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("True").strong().color(palette::TRUTH));
                ui.label(
                    "marks what only the simulation knows: how good, toxic or truthful each \
                     post really is, and how people's wellbeing, informedness and polarisation \
                     change, including people who left or never joined.",
                );
            });
            paragraph(
                ui,
                "The gap between the two is the point. When a measure becomes a target it stops \
                 being a good measure (Goodhart's law): the platform's numbers can climb while \
                 the truth turns. The survey is a good example. It only reaches current users, \
                 and happier people answer more often, so it looks better than the wellbeing of \
                 everyone who ever used the platform.",
            );
            paragraph(
                ui,
                "Design levers are the platform's choices: ranking weights, moderation, creator \
                 payouts, user controls, the business model. World assumptions are beliefs about \
                 people: how strongly outrage grabs attention, how much harassment hurts, how \
                 fast authors learn. They are not choices, and nobody knows their true values. \
                 Change them to see whether a conclusion survives a different world.",
            );
            paragraph(
                ui,
                "In the Sandbox, try designs and follow individuals in the Inspector, down to the \
                 feed they were served. In Compare, run two designs side by side on the same \
                 world. In Scenarios, meet goals against a baseline.",
            );

            heading(ui, "How much to trust it");
            paragraph(
                ui,
                "This is a toy model for building intuition, not a forecast of any real \
                 platform. Its people are simple, its effect sizes are starting values grounded \
                 in a handful of studies, and it has no advertisers, regulators or competitors.",
            );
            paragraph(
                ui,
                "One run is one possible history; a different seed gives a different one. Claims \
                 need several seeds: use Check on 8 seeds in Compare and Confirm on 4 seeds in \
                 Scenarios. For confidence intervals on any metric, many seeds and sweeps of a \
                 lever, use the command-line runner in the repository (`incentive compare`, \
                 `incentive sweep`).",
            );

            heading(ui, "What the model is checked against");
            paragraph(
                ui,
                "Five findings from research, each an automated test of the model:",
            );
            for (i, (fact, source)) in FACTS.iter().enumerate() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(format!("{}.", i + 1)).strong());
                    ui.label(*fact);
                    ui.label(RichText::new(*source).italics().weak());
                });
                ui.add_space(2.0);
            }

            heading(ui, "Things to discover");
            paragraph(
                ui,
                "These came out of building the model rather than going into it. Try to find \
                 them yourself, then check them on several seeds:",
            );
            for (question, finding) in DISCOVERIES {
                egui::CollapsingHeader::new(RichText::new(question).strong())
                    .id_salt(("discover", question))
                    .default_open(false)
                    .show(ui, |ui| {
                        ui.label(finding);
                    });
            }

            heading(ui, "The source");
            paragraph(
                ui,
                "The code and the full model specification, with every formula, the calibration \
                 and the sources, are open:",
            );
            link_row(ui, "Repository", REPOSITORY);
            link_row(ui, "Model", &format!("{MODEL_DOC} in the repository"));
            ui.add_space(12.0);
        });
}

/// A label and a selectable address with a copy button.
fn link_row(ui: &mut Ui, label: &str, text: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(label).strong());
        ui.add(egui::Label::new(RichText::new(text).monospace()).selectable(true));
        if ui.small_button("Copy").clicked() {
            ui.ctx().copy_text(text.to_string());
        }
    });
}
