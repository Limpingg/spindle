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
    // Spindle: wrapped, as in the other dialogs; one truncated line cut the
    // sentence off at "add s..." on the PC.
    ui.add(
        egui::Label::new(
            egui::RichText::new(gettext(
                locale,
                "Listen together from anywhere. Friends join with the link and add songs from their own Spotify.",
            ))
            .font(theme::regular(13.0))
            .color(palette.secondary),
        )
        .wrap()
        .selectable(false),
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
            // Spindle: clicking the link copies it.
            let link = ui
                .add(
                    egui::Label::new(
                        egui::RichText::new(&session.join_url)
                            .font(theme::regular(14.0))
                            .color(palette.text),
                    )
                    .truncate()
                    .sense(egui::Sense::click()),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(gettext(locale, "Click to copy"));
            if link.clicked() {
                ui.ctx().copy_text(session.join_url.clone());
                app.toast(gettext(locale, "Link copied"));
            }
            ui.add_space(12.0);
            ui.vertical_centered(|ui| qr_code(ui, &session.join_url, 180.0));
            ui.add_space(4.0);
            ui.vertical_centered(|ui| {
                theme::text(
                    ui,
                    gettext(locale, "Scan with a phone camera to join"),
                    theme::regular(12.0),
                    palette.secondary,
                );
            });
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

/// Spindle: the join link as a QR code, drawn as squares on a white tile
/// with a quiet zone so phone cameras read it in light and dark themes.
fn qr_code(ui: &mut egui::Ui, text: &str, size: f32) {
    let Ok(code) = qrcode::QrCode::new(text.as_bytes()) else {
        return;
    };
    let width = code.width();
    let colors = code.to_colors();
    let quiet = 4usize;
    let cells = width + quiet * 2;
    let cell = (size / cells as f32).floor().max(1.0);
    let side = cell * cells as f32;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(side, side), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, egui::CornerRadius::same(6), egui::Color32::WHITE);
    for y in 0..width {
        for x in 0..width {
            if colors[y * width + x] == qrcode::Color::Dark {
                let min = rect.min
                    + egui::vec2((x + quiet) as f32 * cell, (y + quiet) as f32 * cell);
                painter.rect_filled(
                    egui::Rect::from_min_size(min, egui::vec2(cell, cell)),
                    0.0,
                    egui::Color32::BLACK,
                );
            }
        }
    }
}
