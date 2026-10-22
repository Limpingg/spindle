//! The Spotify Connect device picker.

use egui::{Align, CornerRadius, Layout, Rect, Sense, pos2, vec2};
use fastframe_audio::DeviceInfo;

use crate::api::models::Device;
use crate::app::App;
use crate::i18n::gettext;
use crate::model::Action;
use crate::theme::{self, Icon};

pub const BUTTON_RECT_ID: &str = "devices-button-rect";

enum OutputRow {
    Device(Device),
    Receiver(crate::zeroconf::Receiver),
}

impl OutputRow {
    /// Spotify's transfer identity, used to keep the active output first.
    fn identity(&self) -> Option<&str> {
        match self {
            Self::Device(device) => device.id.as_deref(),
            Self::Receiver(receiver) => receiver.device_id.as_deref(),
        }
    }

    /// The display label used only after selection history ties.
    fn name(&self) -> &str {
        match self {
            Self::Device(device) => &device.name,
            Self::Receiver(receiver) => &receiver.name,
        }
    }

    /// A stable UI identity even while nearby rows are inserted or reordered.
    fn widget_key(&self) -> String {
        match self {
            Self::Device(device) => format!("connect:{:?}:{}", device.id, device.name),
            Self::Receiver(receiver) => format!(
                "receiver:{}:{}:{}",
                receiver.cast, receiver.address, receiver.port
            ),
        }
    }

    /// Cast preference identity is distinct from its name-derived transfer ID.
    fn preference_key(&self) -> Option<String> {
        match self {
            Self::Device(device) => device.id.clone(),
            Self::Receiver(receiver) => receiver.preference_key(),
        }
    }
}

/// Active output first, then account-scoped frequency, recency and stable ties.
fn rank_outputs(
    rows: &mut [OutputRow],
    history: &crate::device_history::DeviceHistory,
    account: Option<&str>,
    active: Option<&str>,
) {
    rows.sort_by_cached_key(|row| {
        (
            row.identity().is_none_or(|id| Some(id) != active),
            history.rank(account, row.preference_key().as_deref()),
            row.name().to_lowercase(),
            row.widget_key(),
        )
    });
}

fn receiver_is_listed(receiver: &crate::zeroconf::Receiver, devices: &[Device]) -> bool {
    devices
        .iter()
        .any(|device| match receiver.device_id.as_deref() {
            Some(id) => device.id.as_deref() == Some(id),
            None => device.name == receiver.name,
        })
}

fn name_devices(devices: &mut [Device], receivers: &[crate::zeroconf::Receiver]) {
    for device in devices {
        if device.id.is_some()
            && let Some(receiver) = receivers
                .iter()
                .find(|receiver| receiver.device_id == device.id)
        {
            device.name.clone_from(&receiver.name);
        }
    }
}

pub fn device_icon(kind: &str) -> Icon {
    match kind.to_ascii_lowercase().as_str() {
        "computer" => Icon::Laptop,
        "smartphone" => Icon::Smartphone,
        "tablet" => Icon::Tablet,
        "tv" => Icon::Tv,
        "game_console" => Icon::Gamepad,
        "automobile" => Icon::Car,
        "cast_video" | "cast_audio" | "castaudio" | "castvideo" => Icon::Cast,
        "smartwatch" => Icon::Watch,
        "avr" | "stb" | "audio_dongle" => Icon::Monitor,
        _ => Icon::Speaker,
    }
}

/// A row offering to authorize local playback on this computer.
fn enable_playback_row(app: &mut App, ui: &mut egui::Ui) {
    use egui::{Rect, Sense, Vec2, pos2, vec2};
    let palette = app.palette;
    let authorizing = matches!(
        app.local_playback,
        crate::backend::LocalPlayback::Authorizing
            | crate::backend::LocalPlayback::Connecting
            | crate::backend::LocalPlayback::WaitingForNetwork
    );
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 52.0), Sense::click());
    if response.hovered() && !authorizing {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(6), palette.surface_hover);
    }
    let icon_rect =
        Rect::from_center_size(pos2(rect.left() + 24.0, rect.center().y), Vec2::splat(22.0));
    Icon::Laptop
        .image(palette.text, 22.0)
        .paint_at(ui, icon_rect);
    let painter = ui.painter().with_clip_rect(rect);
    painter.text(
        pos2(rect.left() + 48.0, rect.center().y - 9.0),
        egui::Align2::LEFT_CENTER,
        // Translators: {name} is the name this computer uses in Spotify Connect.
        gettext(app.locale, "{name} (this computer)").replace("{name}", &app.settings.device_name),
        theme::medium(14.0),
        palette.text,
    );
    painter.text(
        pos2(rect.left() + 48.0, rect.center().y + 10.0),
        egui::Align2::LEFT_CENTER,
        if authorizing {
            gettext(app.locale, "Setting up…")
        } else {
            gettext(app.locale, "Set up playback here")
        },
        theme::regular(12.0),
        palette.accent,
    );
    if authorizing {
        let mut spin = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(Rect::from_center_size(
                    pos2(rect.right() - 18.0, rect.center().y),
                    Vec2::splat(20.0),
                ))
                .layout(egui::Layout::centered_and_justified(
                    egui::Direction::LeftToRight,
                )),
        );
        theme::spinner(&mut spin, 16.0, palette.accent);
    }
    if response.clicked() && !authorizing {
        app.actions.push(Action::EnablePlayback);
    }
    ui.painter().hline(
        rect.x_range().shrink(6.0),
        rect.bottom(),
        egui::Stroke::new(1.0, palette.outline),
    );
    ui.add_space(4.0);
}

/// A receiver announced on the local network but not yet in the account.
/// Choosing it hands over the account, after which it behaves like any other
/// Spotify Connect device.
fn receiver_row(app: &mut App, ui: &mut egui::Ui, receiver: &crate::zeroconf::Receiver) {
    use egui::{Rect, Sense, Vec2, pos2, vec2};
    let palette = app.palette;
    let activating = app.activating_receiver.as_deref() == Some(receiver.name.as_str());
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 52.0), Sense::click());
    if response.hovered() && !activating {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(6), palette.surface_hover);
    }
    let icon_rect =
        Rect::from_center_size(pos2(rect.left() + 24.0, rect.center().y), Vec2::splat(22.0));
    let icon = if receiver.cast {
        Icon::Cast
    } else {
        Icon::Speaker
    };
    icon.image(palette.text, 22.0).paint_at(ui, icon_rect);
    let painter = ui.painter().with_clip_rect(rect);
    crate::bidi::paint_line(
        &painter,
        rect.left() + 48.0,
        rect.right() - 12.0,
        rect.center().y - 9.0,
        &receiver.name,
        theme::medium(14.0),
        palette.text,
    );
    painter.text(
        pos2(rect.left() + 48.0, rect.center().y + 10.0),
        egui::Align2::LEFT_CENTER,
        if activating {
            gettext(app.locale, "Connecting…")
        } else {
            gettext(app.locale, "On your network, click to connect")
        },
        theme::regular(12.0),
        palette.secondary,
    );
    if activating {
        let mut spin = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(Rect::from_center_size(
                    pos2(rect.right() - 18.0, rect.center().y),
                    Vec2::splat(20.0),
                ))
                .layout(egui::Layout::centered_and_justified(
                    egui::Direction::LeftToRight,
                )),
        );
        theme::spinner(&mut spin, 16.0, palette.accent);
    }
    if response.clicked() && !activating {
        app.actions
            .push(Action::ActivateReceiver(Box::new(receiver.clone())));
    }
}

/// The name to show for the saved output: the listed device whose id it
/// is, or whose name an older build saved; otherwise what was saved.
fn saved_output_name<'a>(saved: &'a str, devices: &'a [DeviceInfo]) -> &'a str {
    devices
        .iter()
        .find(|device| device.id.as_str() == saved)
        .or_else(|| devices.iter().find(|device| device.name == saved))
        .map_or(saved, |device| device.name.as_str())
}

/// Where this computer plays: the system's default output or a device,
/// shown by name and saved by its id in `audio_device` (#963). Choosing one
/// restarts local playback on it. Listed only when asked, so the popup
/// opens at once; a saved device lists them once, for its name.
/// This computer's sound output as a row that opens the list.
fn output_picker(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    let locale = app.locale;
    let open_id = ui.make_persistent_id("audio-output-open");
    let mut open = ui.data(|data| data.get_temp::<bool>(open_id).unwrap_or(false));
    super::widgets::menu_separator(ui, &palette);
    let system_default = gettext(locale, "System default").into_owned();
    let label = gettext(locale, "Audio output");
    let current = app.settings.audio_device.clone();
    if current.is_some() && matches!(app.output_devices, crate::model::Loadable::NotLoaded) {
        app.actions.push(Action::ListOutputDevices);
    }
    let listed = app.output_devices.get().map_or(&[][..], Vec::as_slice);
    let current_name = current
        .as_deref()
        .map(|saved| saved_output_name(saved, listed).to_string());
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 44.0), Sense::click());
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label.as_ref()));
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(6), palette.surface_hover);
    }
    Icon::Volume2.image(palette.secondary, 18.0).paint_at(
        ui,
        Rect::from_center_size(
            pos2(rect.left() + 24.0, rect.center().y),
            egui::Vec2::splat(18.0),
        ),
    );
    let painter = ui.painter().with_clip_rect(rect);
    painter.text(
        pos2(rect.left() + 48.0, rect.center().y - 8.0),
        egui::Align2::LEFT_CENTER,
        label.as_ref(),
        theme::medium(13.0),
        palette.text,
    );
    crate::bidi::paint_line(
        &painter,
        rect.left() + 48.0,
        rect.right() - 36.0,
        rect.center().y + 9.0,
        current_name.as_deref().unwrap_or(&system_default),
        theme::regular(12.0),
        palette.secondary,
    );
    if open {
        Icon::ChevronUp
    } else {
        Icon::ChevronDown
    }
    .image(palette.secondary, 16.0)
    .paint_at(
        ui,
        Rect::from_center_size(
            pos2(rect.right() - 18.0, rect.center().y),
            egui::Vec2::splat(16.0),
        ),
    );
    if response.clicked() {
        open = !open;
        if open {
            app.actions.push(Action::ListOutputDevices);
        }
    }
    ui.data_mut(|data| data.insert_temp(open_id, open));
    if !open {
        return;
    }
    output_choices(app, ui);
}

/// Spindle: this computer's sound outputs as a list to pick from.
fn output_choices(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    let locale = app.locale;
    let system_default = gettext(locale, "System default").into_owned();
    let current = app.settings.audio_device.clone();
    match &app.output_devices {
        crate::model::Loadable::Loaded(devices) => {
            // The saved device's row: by its id, or by the name an older
            // build saved.
            let chosen = current.as_deref().and_then(|saved| {
                devices
                    .iter()
                    .position(|device| device.id.as_str() == saved)
                    .or_else(|| devices.iter().position(|device| device.name == saved))
            });
            let mut choices: Vec<(Option<String>, String, bool)> =
                vec![(None, system_default.clone(), current.is_none())];
            choices.extend(devices.iter().enumerate().map(|(index, device)| {
                (
                    Some(device.id.as_str().to_string()),
                    device.name.clone(),
                    chosen == Some(index),
                )
            }));
            // A saved device that is unplugged stays listed, so the choice
            // shows and can be changed.
            if let Some(saved) = current.as_ref().filter(|_| chosen.is_none()) {
                choices.push((Some(saved.clone()), saved.clone(), true));
            }
            // As tall as its rows, up to six; the popup grows upwards from
            // the button, so the list asks for its height rather than taking
            // what last frame's popup left below it.
            let height = (choices.len() as f32 * 34.0).min(204.0);
            egui::ScrollArea::vertical()
                .id_salt("audio-outputs")
                .max_height(height)
                .min_scrolled_height(height)
                .show(ui, |ui| {
                    for (choice, name, selected) in choices {
                        if super::widgets::menu_item(
                            ui,
                            &palette,
                            Some(if selected { Icon::Check } else { Icon::Speaker }),
                            &name,
                        ) && !selected
                        {
                            app.actions.push(Action::SetAudioDevice(choice));
                        }
                    }
                });
        }
        crate::model::Loadable::Failed(_) => {
            theme::subtle(
                ui,
                &palette,
                &gettext(locale, "Couldn't list this computer's audio outputs."),
            );
        }
        _ => {
            theme::subtle(ui, &palette, &gettext(locale, "Finding audio outputs…"));
        }
    }
}

/// Every output the Connect panel offers, ranked, with this computer's id
/// and the active output's id.
fn output_rows(app: &App) -> (Vec<OutputRow>, Option<String>, Option<String>) {
    let local_id = app.local_device_id.clone();
    let mut devices: Vec<Device> = app.devices.clone();
    if app.local_ready
        && let Some(local_id) = &local_id
        && !devices
            .iter()
            .any(|device| device.id.as_deref() == Some(local_id.as_str()))
    {
        devices.insert(
            0,
            Device {
                id: Some(local_id.clone()),
                // Named without its suffix; the row below adds it.
                name: app.settings.device_name.clone(),
                is_active: app.local.is_active(),
                is_restricted: false,
                volume_percent: Some(crate::app::volume_to_percent(app.local.volume)),
                supports_volume: Some(true),
                kind: "computer".into(),
            },
        );
    }
    let active_id = match app.target() {
        crate::app::Target::Local => local_id.clone(),
        crate::app::Target::Remote(id) => id,
    };
    // Spotify leaves a Sonos out of its list; it is offered from
    // the network instead and driven over Connect.
    for receiver in &app.receivers {
        let listed = devices
            .iter()
            .any(|device| device.id.is_some() && device.id == receiver.device_id);
        if !listed
            && let Some(row) =
                crate::sonos::device_row(receiver, receiver.device_id == active_id)
        {
            devices.push(row);
        }
    }
    name_devices(&mut devices, &app.receivers);
    let mut seen = std::collections::HashSet::new();
    devices
        .retain(|device| device.id.as_ref().is_none_or(|id| seen.insert(id.clone())));

    // Receivers on the network that Spotify has not listed yet.
    // They are real speakers the user can see in the official
    // client, so offering them here is the whole point.
    let waiting: Vec<crate::zeroconf::Receiver> = app
        .receivers
        .iter()
        .filter(|receiver| !receiver_is_listed(receiver, &devices))
        .cloned()
        .collect();

    let mut rows: Vec<_> = devices
        .into_iter()
        .map(OutputRow::Device)
        .chain(waiting.into_iter().map(OutputRow::Receiver))
        .collect();
    rank_outputs(
        &mut rows,
        &app.device_history,
        app.user_id(),
        active_id.as_deref(),
    );

    (rows, local_id, active_id)
}

/// One Spotify Connect device in the list.
fn device_row(
    app: &mut App,
    ui: &mut egui::Ui,
    device: &Device,
    local_id: &Option<String>,
    active_id: &Option<String>,
) {
    let palette = app.palette;
    let locale = app.locale;
    let is_local = device.id.is_some() && device.id == *local_id;
    let active = device.id.is_some() && device.id == *active_id;
    let name = if is_local {
        // Translators: {name} is the name this computer uses in Spotify Connect.
        gettext(locale, "{name} (this computer)")
            .replace("{name}", &device.name)
    } else {
        device.name.clone()
    };
    let (rect, response) = ui.allocate_exact_size(
        vec2(ui.available_width(), 52.0),
        Sense::click(),
    );
    if response.hovered() {
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(6),
            palette.surface_hover,
        );
    }
    let color = if active { palette.accent } else { palette.text };
    let icon_rect = Rect::from_center_size(
        pos2(rect.left() + 24.0, rect.center().y),
        egui::Vec2::splat(22.0),
    );
    device_icon(&device.kind)
        .image(color, 22.0)
        .paint_at(ui, icon_rect);
    let painter = ui.painter().with_clip_rect(rect);
    crate::bidi::paint_line(
        &painter,
        rect.left() + 48.0,
        rect.right() - 12.0,
        rect.center().y - 9.0,
        &name,
        theme::medium(14.0),
        color,
    );
    let status = if active {
        gettext(locale, "Listening on this device").into_owned()
    } else if device.is_restricted {
        gettext(locale, "Restricted").into_owned()
    } else if is_local {
        gettext(locale, "Play here").into_owned()
    } else {
        device.kind.replace('_', " ")
    };
    painter.text(
        pos2(rect.left() + 48.0, rect.center().y + 10.0),
        egui::Align2::LEFT_CENTER,
        status,
        theme::regular(12.0),
        if active {
            palette.accent
        } else {
            palette.secondary
        },
    );
    if active {
        let dot = pos2(rect.right() - 16.0, rect.center().y);
        ui.painter().circle_filled(dot, 4.0, palette.accent);
    }
    if response.clicked()
        && !active
        && let Some(id) = &device.id
    {
        app.actions.push(Action::Transfer(id.clone()));
    }
}

/// Spindle: the Connect panel on the right, as in Spotify: the device
/// playing now with its Jam and this computer's sound outputs, the other
/// devices, and Spotify's help pages.
pub fn side_panel(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    let locale = app.locale;
    let fit = super::yielding_panel(
        ui.ctx(),
        "connect-panel",
        theme::SIDE_PANEL_MIN_WIDTH..=560.0,
        app.settings.queue_width,
        ui.available_width() - super::topbar::least_width(ui.ctx()),
    );
    let panel = egui::Panel::right("connect-panel")
        .resizable(true)
        .default_size(app.settings.queue_width)
        .size_range(fit.range.clone())
        .show_separator_line(false)
        .frame(
            egui::Frame::new()
                .fill(palette.panel)
                .inner_margin(egui::Margin::symmetric(12, 12)),
        );
    let response = panel.show(ui, |ui| {
        let window_controls =
            super::window_controls_reservation(ui.ctx(), true, false, ui.available_width());
        ui.add_space(window_controls.queue_top);
        let mut close = false;
        egui::Sides::new().shrink_left().show(
            ui,
            |ui| {
                ui.add_space(4.0);
                theme::text(ui, gettext(locale, "Connect"), theme::bold(16.0), palette.text);
            },
            |ui| {
                close = theme::icon_button(
                    ui,
                    Icon::X,
                    18.0,
                    palette.secondary,
                    palette.text,
                    &gettext(locale, "Close"),
                )
                .clicked();
                if app.devices_loading {
                    theme::spinner(ui, 16.0, palette.accent);
                } else if theme::icon_button(
                    ui,
                    Icon::Refresh,
                    15.0,
                    palette.secondary,
                    palette.text,
                    &gettext(locale, "Refresh"),
                )
                .clicked()
                {
                    app.actions.push(Action::RefreshDevices);
                }
            },
        );
        if close {
            app.actions.push(Action::ToggleDevicesPopup);
        }
        ui.add_space(12.0);
        let (rows, local_id, active_id) = output_rows(app);
        let links_height = 2.0 * 44.0 + 12.0;
        let list_height = (ui.available_height() - links_height).max(120.0);
        egui::ScrollArea::vertical()
            .id_salt("connect-device-list")
            .auto_shrink([false, true])
            .max_height(list_height)
            .show(ui, |ui| {
                current_card(app, ui, &rows, &local_id, &active_id);
                ui.add_space(8.0);
                if !app.local_ready {
                    enable_playback_row(app, ui);
                }
                let others: Vec<&OutputRow> = rows
                    .iter()
                    .filter(|row| {
                        active_id.is_none() || row.identity() != active_id.as_deref()
                    })
                    .collect();
                if others.is_empty() && app.local_ready {
                    ui.add_space(8.0);
                    theme::subtle(
                        ui,
                        &palette,
                        &gettext(
                            locale,
                            "No other devices found. Open Spotify on another device, then refresh.",
                        ),
                    );
                }
                let local_active = active_id.is_none() || active_id == local_id;
                for row in others {
                    ui.push_id(row.widget_key(), |ui| match row {
                        OutputRow::Device(device) => {
                            device_row(app, ui, device, &local_id, &active_id)
                        }
                        OutputRow::Receiver(receiver) => receiver_row(app, ui, receiver),
                    });
                }
                // While another device plays, this computer's outputs stay
                // a row away instead of behind the card's arrow.
                if app.local_ready && !local_active {
                    output_picker(app, ui);
                }
            });
        ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
            help_link(
                app,
                ui,
                &gettext(locale, "What can I connect to?"),
                "https://connect.spotify.com/",
            );
            help_link(
                app,
                ui,
                &gettext(locale, "Don't see your device?"),
                "https://support.spotify.com/us/article/spotify-connect/",
            );
        });
    });
    let width = response.response.rect.width();
    if (width - app.settings.queue_width).abs() > 1.0
        && super::panel_width_chosen(ui.ctx(), "connect-panel", &fit)
    {
        app.settings.queue_width = width;
        app.actions.push(Action::SettingsChanged);
    }
}

/// The device playing now, in green, with the Jam under it and, for this
/// computer, its sound outputs behind the arrow.
fn current_card(
    app: &mut App,
    ui: &mut egui::Ui,
    rows: &[OutputRow],
    local_id: &Option<String>,
    active_id: &Option<String>,
) {
    let palette = app.palette;
    let locale = app.locale;
    let active = rows.iter().find_map(|row| match row {
        OutputRow::Device(device)
            if active_id.is_some() && device.id.as_deref() == active_id.as_deref() =>
        {
            Some(device.clone())
        }
        _ => None,
    });
    let is_local = active
        .as_ref()
        .map_or(app.local_ready, |device| device.id.is_some() && device.id == *local_id);
    let (name, kind) = match &active {
        Some(device) if !is_local => (device.name.clone(), device.kind.clone()),
        _ => (app.settings.device_name.clone(), "computer".to_string()),
    };
    let open_id = ui.make_persistent_id("connect-outputs-open");
    let mut open = ui.data(|data| data.get_temp::<bool>(open_id).unwrap_or(false));
    egui::Frame::new()
        .fill(palette.surface)
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            // Under this computer's name: where its sound goes now.
            let output = (is_local && app.local_ready).then(|| {
                let listed = app.output_devices.get().map_or(&[][..], Vec::as_slice);
                app.settings.audio_device.as_deref().map_or_else(
                    || gettext(locale, "System default").into_owned(),
                    |saved| saved_output_name(saved, listed).to_string(),
                )
            });
            if app.settings.audio_device.is_some()
                && matches!(app.output_devices, crate::model::Loadable::NotLoaded)
            {
                app.actions.push(Action::ListOutputDevices);
            }
            let height = if output.is_some() { 48.0 } else { 40.0 };
            let (rect, response) =
                ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
            device_icon(&kind)
                .image(palette.accent, 22.0)
                .paint_at(
                    ui,
                    Rect::from_center_size(
                        pos2(rect.left() + 12.0, rect.center().y),
                        egui::Vec2::splat(22.0),
                    ),
                );
            let painter = ui.painter().with_clip_rect(rect);
            crate::bidi::paint_line(
                &painter,
                rect.left() + 36.0,
                rect.right() - 32.0,
                rect.center().y - if output.is_some() { 9.0 } else { 0.0 },
                &name,
                theme::medium(16.0),
                palette.accent,
            );
            if let Some(output) = &output {
                crate::bidi::paint_line(
                    &painter,
                    rect.left() + 36.0,
                    rect.right() - 32.0,
                    rect.center().y + 11.0,
                    output,
                    theme::regular(12.0),
                    palette.secondary,
                );
            }
            if is_local && app.local_ready {
                let arrow = if open {
                    Icon::ChevronDown
                } else {
                    Icon::ChevronRight
                };
                arrow.image(palette.secondary, 18.0).paint_at(
                    ui,
                    Rect::from_center_size(
                        pos2(rect.right() - 10.0, rect.center().y),
                        egui::Vec2::splat(18.0),
                    ),
                );
                let label = gettext(locale, "Audio output");
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label.as_ref())
                });
                let response = response
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text(gettext(locale, "Sound output on this computer"));
                if response.clicked() {
                    open = !open;
                    if open {
                        app.actions.push(Action::ListOutputDevices);
                    }
                }
            }
            if open && is_local && app.local_ready {
                ui.add_space(4.0);
                output_choices(app, ui);
            }
            ui.add_space(6.0);
            jam_line(app, ui);
        });
    ui.data_mut(|data| data.insert_temp(open_id, open));
}

/// The Jam under the playing device: its name while one runs, an offer to
/// start one otherwise.
fn jam_line(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    let locale = app.locale;
    let live = matches!(app.jam, crate::model::JamState::Live(_));
    let label = if live {
        let owner = app
            .user
            .as_ref()
            .map(|user| user.name().to_string())
            .unwrap_or_default();
        // Translators: {name} is the account's display name.
        gettext(locale, "{name}'s Jam").replace("{name}", &owner)
    } else {
        gettext(locale, "Start a Jam").into_owned()
    };
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(6), palette.surface_hover);
    }
    Icon::Users
        .image(if live { palette.text } else { palette.secondary }, 16.0)
        .paint_at(
            ui,
            Rect::from_center_size(pos2(rect.left() + 12.0, rect.center().y), egui::Vec2::splat(16.0)),
        );
    ui.painter().with_clip_rect(rect).text(
        pos2(rect.left() + 32.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        if live {
            theme::semibold(13.0)
        } else {
            theme::regular(13.0)
        },
        if live { palette.text } else { palette.secondary },
    );
    if response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
    {
        app.actions.push(Action::StartJam);
    }
}

/// A row that opens one of Spotify's help pages in the browser.
fn help_link(app: &mut App, ui: &mut egui::Ui, label: &str, url: &str) {
    let palette = app.palette;
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 44.0), Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(6), palette.surface_hover);
    }
    ui.painter().with_clip_rect(rect).text(
        pos2(rect.left() + 12.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        theme::semibold(14.0),
        palette.text,
    );
    Icon::ExternalLink.image(palette.secondary, 16.0).paint_at(
        ui,
        Rect::from_center_size(pos2(rect.right() - 16.0, rect.center().y), egui::Vec2::splat(16.0)),
    );
    if response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
    {
        app.actions.push(Action::OpenUrl(url.to_string()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_then_frequent_outputs_rank_across_both_kinds_of_row() {
        let mut history = crate::device_history::DeviceHistory::default();
        let cast = crate::zeroconf::Receiver {
            name: "Z speaker".into(),
            device_id: Some("cast-id".into()),
            address: "192.0.2.1".parse().unwrap(),
            port: 8009,
            path: "/".into(),
            cast: true,
        };
        history.record("alice", &cast.preference_key().unwrap());
        let mut rows = vec![
            OutputRow::Device(Device {
                id: Some("unknown".into()),
                name: "A phone".into(),
                ..Default::default()
            }),
            OutputRow::Receiver(cast),
            OutputRow::Device(Device {
                id: Some("active".into()),
                name: "Current output".into(),
                ..Default::default()
            }),
        ];
        rank_outputs(&mut rows, &history, Some("alice"), Some("active"));
        assert_eq!(
            rows.iter().map(OutputRow::identity).collect::<Vec<_>>(),
            [Some("active"), Some("cast-id"), Some("unknown")]
        );
        rank_outputs(&mut rows, &history, Some("bob"), Some("active"));
        assert_eq!(
            rows.iter().map(OutputRow::identity).collect::<Vec<_>>(),
            [Some("active"), Some("unknown"), Some("cast-id")]
        );
    }

    #[test]
    fn a_same_named_cast_at_another_address_cannot_inherit_a_preference() {
        let speaker = crate::zeroconf::Receiver {
            name: "Z room".into(),
            device_id: Some("same-name-id".into()),
            address: "192.0.2.1".parse().unwrap(),
            port: 8009,
            path: "/".into(),
            cast: true,
        };
        let other = crate::zeroconf::Receiver {
            address: "192.0.2.2".parse().unwrap(),
            ..speaker.clone()
        };
        let mut history = crate::device_history::DeviceHistory::default();
        history.record("alice", &speaker.preference_key().unwrap());
        history.record("alice", "same-name-id");
        let mut rows = vec![
            OutputRow::Receiver(other),
            OutputRow::Device(Device {
                id: Some("phone".into()),
                name: "A phone".into(),
                ..Default::default()
            }),
            OutputRow::Receiver(speaker),
        ];
        rank_outputs(&mut rows, &history, Some("alice"), None);
        assert!(
            matches!(&rows[0], OutputRow::Receiver(row) if row.address == "192.0.2.1".parse::<std::net::IpAddr>().unwrap())
        );
        assert_eq!(rows[1].identity(), Some("phone"));
    }

    #[test]
    fn a_receivers_identity_matches_even_when_its_advertised_name_differs() {
        let receiver = crate::zeroconf::Receiver {
            name: "SpotifyConnect".into(),
            device_id: Some("living-room".into()),
            address: "127.0.0.1".parse().unwrap(),
            port: 80,
            path: "/".into(),
            cast: false,
        };
        let mut device = Device {
            id: Some("living-room".into()),
            name: "Living Room".into(),
            ..Default::default()
        };
        assert!(receiver_is_listed(&receiver, &[device.clone()]));
        let mut named = [Device {
            name: "unnamed".into(),
            ..device.clone()
        }];
        name_devices(&mut named, std::slice::from_ref(&receiver));
        assert_eq!(named[0].name, receiver.name);
        assert_eq!(
            named[0].id, device.id,
            "renaming preserves the transfer target"
        );
        device.id = Some("other-room".into());
        device.name = receiver.name.clone();
        assert!(
            !receiver_is_listed(&receiver, &[device]),
            "two identities may have the same name"
        );
    }
}
