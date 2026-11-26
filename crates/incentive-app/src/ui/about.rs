//! The About tab: what the lab is and how to read it, in plain language.

use egui::{RichText, Ui};

use super::palette;

/// The repository the lab lives in.
pub const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
/// The model specification inside the repository.
pub const MODEL_DOC: &str = "docs/model.md";

/// The six stylized facts the model is checked against, with sources
/// (docs/model.md, section 15).
pub const FACTS: [(&str, &str); 6] = [
    (
        "Engagement ranking amplifies divisive content that people do not value. Against a \
         chronological feed, the engagement ranking shows more outrage, and what it shows is \
         worth less to people on reflection.",
        "Milli et al. (2025), an audit of Twitter's ranking, PNAS Nexus",
    ),
    (
        "Social reward teaches outrage. Under engagement ranking the share of outrage posts rises \
         over time; when authors do not learn from likes, it does not.",
        "Brady et al. (2021), Science Advances",
    ),
    (
        "A chronological feed cuts time spent, and over three months barely moves polarisation. \
         The same study found more content from untrustworthy sources in chronological feeds, \
         and less incivility by tens of percent; this model gets the first direction wrong and \
         cuts incivility far more.",
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
    (
        "Leaving the platform makes people slightly happier. People who stay off a realistic \
         ranked feed for five weeks end up a little happier than those who stay on. The value \
         the model puts on time was set so that it reproduces the size of this effect.",
        "Allcott et al. (2025), quitting Facebook and Instagram before the 2020 US election, NBER",
    ),
];

/// Findings that came out of the model's calibration rather than into it
/// (docs/model.md, 13.3), offered as things to discover.
pub const DISCOVERIES: [(&str, &str); 7] = [
    (
        "Should the feed listen when people ask to see less?",
        "Ranking down authors whom many people ask to see less of sounds like letting users steer. \
         Here bot farms file those marks too, against the other side's popular posts, so a strong \
         see-less penalty raises harm instead of lowering it. Switch the farms' brigading off \
         (world assumptions) and the same penalty lowers harm. A preference anyone can file is a \
         target.",
    ),
    (
        "What does a calmer feed cost?",
        "Ranking for quality and cross-group approval cuts harm by about four fifths for about a \
         seventh of the engagement. Authors learn to polish posts so they look better to the \
         quality classifier than they are, and the more polish fools it (world assumptions), the \
         less harm the calm feed saves: at the strongest setting, less than half. Find the \
         designs in between.",
    ),
    (
        "Do notifications do people any good?",
        "They always bring engagement: at full strength, about half again as much, and they keep \
         more people on the platform. Under the engagement ranking they bring no wellbeing at \
         all. On the quality and bridging feed, where most of the time spent is worth it, the \
         same notifications make people a little happier. Now that time has a price (world \
         assumptions: value of time), more time on a feed helps only if the feed is worth it.",
    ),
    (
        "Does the engagement ranking's lead last?",
        "In the default world its lead holds over the year, but on fewer people: in the last \
         three months about 47% of society use it, against 61% for quality and bridging. Make \
         habits weaker, reputation matter more and harassment drive people away faster (world \
         assumptions: habit gain 0.01, reputation sensitivity 5, harassment exit 0.15), and the \
         lead reverses: by the end of the year the calmer design draws more engagement.",
    ),
    (
        "Does stricter moderation work?",
        "Against toxicity, yes: at a threshold of 0.3 toxic impressions fall by about nineteen \
         twentieths, and authors' evasion barely dents that. But the classifier also removes \
         about fourteen times as many innocent posts, about forty-seven a day, and the harm \
         index counts those: it falls by only a quarter.",
    ),
    (
        "Can you trust the reports?",
        "Bot farms file about three tenths of all reports and see-less marks under the \
         engagement ranking, and most of them under quality and bridging. Weighing each report \
         by how often its reporter's earlier reports were upheld does not stop the filing but \
         takes away its effect: under quality and bridging, harassment falls by more than two \
         thirds.",
    ),
    (
        "Does the survey tell the platform how people are doing?",
        "Not reliably: answers mix wellbeing with recent pleasure and habit. After three months \
         the survey ranks the engagement ranking above the chronological feed, calm by design, \
         quality and bridging and the subscription; after a year, still above the first two, \
         level with quality and bridging and below the subscription. The truth ranks it below \
         all four at both points, on the wellbeing of everyone who ever used the platform.",
    ),
];

/// What the model grants for free: designs that lean on these look better
/// than they could on a real platform (docs/model.md, section 16).
pub const FREEBIES: [&str; 5] = [
    "Time: it now costs what it could have been spent on, but at one value for everyone, set \
     so that leaving a realistic ranked feed for five weeks makes people slightly happier, as a \
     field experiment found. A notification costs only the time it brings, not the \
     interruption, and people regret about a twelfth as much of their time as surveys of real \
     users find.",
    "Money: revenue is not calibrated, and creator payouts cost nothing in any score but net \
     revenue, while creators earning a living count toward the benefit index.",
    "Review and notes: human reviewers never err, and community notes are written only on \
     posts that are false.",
    "Fans: fans tip by what posts are truly worth to them, more cleanly than any real tip jar.",
    "Prediction: most of the engagement predictor reads what each viewer truly likes; only the \
     rest learns from counts that bots can inflate.",
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
                     Its ranking, moderation and payouts use these numbers, and also signals the \
                     model hands it ready-made: an engagement predictor that partly reads what \
                     each viewer truly likes, a quality classifier that authors can fool with \
                     polish, a toxicity classifier they learn to evade, human reviewers who never \
                     err, community notes written only on false posts, and fans who tip by what \
                     posts are really worth to them.",
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
                 happier people answer more often, and answers mix wellbeing with recent \
                 pleasure and habit, so it can favour a design that leaves people worse off.",
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
                 need several seeds: use Check on 8 seeds in Compare, which runs seeds 9 to 16 \
                 that the Atlas searches never picked designs on, and Confirm on 8 seeds in \
                 Scenarios, which says pass, fail or undecided once the differences between seeds \
                 are taken into account. A ~ beside a number means it was still moving at the end \
                 of the run, so a longer or shorter run could change it. For confidence intervals \
                 on any metric, many seeds and sweeps of a lever, use the command-line runner in \
                 the repository (`incentive compare`, `incentive sweep`).",
            );

            heading(ui, "What the model gets for free");
            paragraph(
                ui,
                "Some things cost nothing here that would cost something on a real platform. \
                 Designs that lean on them look better than they could be:",
            );
            for (i, item) in FREEBIES.iter().enumerate() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(format!("{}.", i + 1)).strong());
                    ui.label(*item);
                });
                ui.add_space(2.0);
            }

            heading(ui, "What the model is checked against");
            paragraph(
                ui,
                "Six findings from research. The model was tuned until it reproduces their \
                 direction, and each is an automated test, so they show that it is consistent \
                 with these studies, not that it predicts them:",
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
