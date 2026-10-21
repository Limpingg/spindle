//! Spindle: the Jam this device hosts. Friends open the link to join and add
//! songs from their own Spotify; the music plays here.

use crate::app::App;
use crate::i18n::gettext;
use crate::model::{Action, JamState};
use crate::theme;

use super::dialogs::buttons;

pub fn dialog(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    let locale = app.locale;
    theme::text(ui, gettext(locale, "Jam"), theme::bold(20.0), palette.text);
    ui.add_space(4.0);
    theme::text(
        ui,
        gettext(locale, "Listen together from anywhere. Friends join with the link and add songs from their own Spotify."),
        theme::regular(13.0),
        palette.secondary,
    );
    ui.add_space(16.0);
    match app.jam.clone() {
        JamState::Off | JamState::Starting => {
            ui.horizontal(|ui| {
                theme::spinner(ui, 18.0, palette.accent);
                theme::text(
                    ui,
                    gettext(locale, "Starting the Jam…"),
                    theme::regular(14.0),
                    palette.text,
                );
            });
            ui.add_space(16.0);
            buttons(ui, |ui| {
                if theme::pill_button(ui, &palette, &gettext(locale, "Close"), false).clicked() {
                    app.actions.push(Action::CloseDialog);
                }
            });
        }
        JamState::Live(session) => {
            theme::text(
                ui,
                gettext(locale, "Share this link"),
                theme::medium(13.0),
                palette.secondary,
            );
            ui.add_space(4.0);
            theme::text(ui, &session.join_url, theme::regular(14.0), palette.text);
            if !session.members.is_empty() {
                ui.add_space(12.0);
                theme::text(
                    ui,
                    gettext(locale, "In the Jam"),
                    theme::medium(13.0),
                    palette.secondary,
                );
                ui.add_space(4.0);
                theme::text(
                    ui,
                    session.members.join(", "),
                    theme::regular(14.0),
                    palette.text,
                );
            }
            ui.add_space(20.0);
            buttons(ui, |ui| {
                if theme::pill_button(ui, &palette, &gettext(locale, "Copy link"), true).clicked() {
                    ui.ctx().copy_text(session.join_url.clone());
                    app.toast(gettext(locale, "Link copied"));
                }
                if theme::pill_button(ui, &palette, &gettext(locale, "End Jam"), false).clicked() {
                    app.actions.push(Action::EndJam);
                }
                if theme::pill_button(ui, &palette, &gettext(locale, "Close"), false).clicked() {
                    app.actions.push(Action::CloseDialog);
                }
            });
        }
        JamState::Ending => {
            ui.horizontal(|ui| {
                theme::spinner(ui, 18.0, palette.accent);
                theme::text(
                    ui,
                    gettext(locale, "Ending the Jam…"),
                    theme::regular(14.0),
                    palette.text,
                );
            });
        }
        JamState::Failed(message) => {
            theme::text(ui, &message, theme::regular(14.0), palette.text);
            ui.add_space(20.0);
            buttons(ui, |ui| {
                if theme::pill_button(ui, &palette, &gettext(locale, "Try again"), true).clicked() {
                    app.jam = JamState::Off;
                    app.actions.push(Action::StartJam);
                }
                if theme::pill_button(ui, &palette, &gettext(locale, "Close"), false).clicked() {
                    app.actions.push(Action::CloseDialog);
                }
            });
        }
    }
}
