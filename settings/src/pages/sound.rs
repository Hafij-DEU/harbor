//! System > Sound: output and input devices, volume, mute, test sound.

use eframe::egui::Ui;

use crate::app::App;
use crate::icons::Icon;
use crate::system::{self};
use crate::widgets::{self, Right};

use super::*;

pub fn sound(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    if widgets::h1_back(ui, &pal, "Sound") {
        app.back();
        return;
    }

    if !system::audio_available() {
        widgets::card(ui, &pal, |ui| {
            widgets::empty_note(
                ui,
                &pal,
                "No audio control tool was found. Install pipewire-pulse or pulseaudio-utils.",
            );
        });
        spacer(ui, 14.0);
    }
    widgets::card(ui, &pal, |ui| {
        let mut volume = app.test_volume;
        let subtitle = if app.info.muted {
            "Muted".to_owned()
        } else {
            format!("{}%", app.info.volume)
        };
        let changed = slider_row(ui, &pal, Icon::Sound, "Volume", subtitle, &mut volume);
        if changed {
            let pct = (volume * 100.0).round() as u32;
            if pct != app.info.volume {
                if system::set_volume(pct) {
                    app.info.volume = pct;
                    app.test_volume = volume;
                    if app.info.muted {
                        system::set_muted(false);
                        app.info.muted = false;
                    }
                } else {
                    app.notify_error("Could not change the volume");
                }
            }
        }
        widgets::divider(ui, &pal);
        let mut muted = app.info.muted;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Sound,
            "Mute",
            "Silence all audio output",
            Right::Switch(&mut muted),
        );
        if outcome.toggled {
            if system::set_muted(muted) {
                app.info.muted = muted;
                app.notify(if muted { "Muted" } else { "Unmuted" });
            } else {
                app.notify_error("Could not mute audio");
            }
        }
        widgets::divider(ui, &pal);
        if widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Sparkle,
            "Test output",
            "Play a short system sound",
            Right::Chevron,
        )
        .clicked()
        {
            system::test_sound();
            app.notify("Playing a test sound");
        }
    });

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Output");
    let sinks = app.info.sinks.clone();
    if sinks.is_empty() {
        widgets::card(ui, &pal, |ui| {
            widgets::empty_note(
                ui,
                &pal,
                "No audio outputs were reported by PulseAudio/PipeWire.",
            );
        });
    } else {
        let mut selected = None;
        widgets::card(ui, &pal, |ui| {
            for (index, sink) in sinks.iter().enumerate() {
                if index > 0 {
                    widgets::divider(ui, &pal);
                }
                let is_default = sink.name == app.info.default_sink;
                if widgets::row(
                    ui,
                    &pal,
                    pal.accent,
                    Icon::Sound,
                    &sink.description,
                    &sink.name,
                    if is_default {
                        Right::Check
                    } else {
                        Right::None
                    },
                )
                .clicked()
                    && !is_default
                {
                    selected = Some(sink.name.clone());
                }
            }
        });
        if let Some(name) = selected {
            if system::set_default_sink(&name) {
                app.info.default_sink = name;
                app.notify("Default output changed");
            } else {
                app.notify_error("Could not switch output device");
            }
        }
    }

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Input");
    widgets::card(ui, &pal, |ui| {
        let mut mic = app.test_mic_volume;
        let subtitle = if app.info.input_muted {
            "Microphone muted".to_owned()
        } else {
            format!("{}%", app.info.microphone_volume)
        };
        let changed = slider_row(
            ui,
            &pal,
            Icon::Sound,
            "Microphone volume",
            subtitle,
            &mut mic,
        );
        if changed {
            let pct = (mic * 100.0).round() as u32;
            if pct != app.info.microphone_volume {
                if system::set_input_volume(pct) {
                    app.info.microphone_volume = pct;
                    app.test_mic_volume = mic;
                    if app.info.input_muted {
                        system::set_input_muted(false);
                        app.info.input_muted = false;
                    }
                } else {
                    app.notify_error("Could not change the microphone volume");
                }
            }
        }
        widgets::divider(ui, &pal);
        let mut muted = app.info.input_muted;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Sound,
            "Mute microphone",
            "Silence all audio input",
            Right::Switch(&mut muted),
        );
        if outcome.toggled {
            if system::set_input_muted(muted) {
                app.info.input_muted = muted;
                app.notify(if muted {
                    "Microphone muted"
                } else {
                    "Microphone on"
                });
            } else {
                app.notify_error("Could not mute the microphone");
            }
        }
    });

    spacer(ui, 14.0);
    let sources = app.info.sources.clone();
    if !sources.is_empty() {
        widgets::section_label(ui, &pal, "Input device");
        let mut selected = None;
        widgets::card(ui, &pal, |ui| {
            for (index, source) in sources.iter().enumerate() {
                if index > 0 {
                    widgets::divider(ui, &pal);
                }
                let is_default = source.name == app.info.default_source;
                if widgets::row(
                    ui,
                    &pal,
                    pal.accent,
                    Icon::Sound,
                    &source.description,
                    &source.name,
                    if is_default {
                        Right::Check
                    } else {
                        Right::None
                    },
                )
                .clicked()
                    && !is_default
                {
                    selected = Some(source.name.clone());
                }
            }
        });
        if let Some(name) = selected {
            if system::set_default_source(&name) {
                app.info.default_source = name;
                app.notify("Default input changed");
            } else {
                app.notify_error("Could not switch the input device");
            }
        }
    }
}
