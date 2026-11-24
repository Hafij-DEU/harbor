//! System > Display: monitors, resolution, scale, brightness, night light.

use eframe::egui::{self, Align2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};

use crate::app::App;
use crate::icons::Icon;
use crate::system::{self};
use crate::theme::{regular, semibold, with_alpha};
use crate::widgets::{self, Right, ROW_PAD};

use super::*;

/// Pick the closest scale the monitor actually supports.
pub(crate) fn nearest_scale(want: f32, scales: &[f32]) -> f32 {
    if scales.is_empty() {
        return 1.0;
    }
    scales
        .iter()
        .copied()
        .min_by(|a, b| {
            (a - want)
                .abs()
                .partial_cmp(&(b - want).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or(1.0)
}

pub fn display(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    if widgets::h1_back(ui, &pal, "Display") {
        app.back();
        return;
    }

    // A small preview of the current mode.
    let width = ui.available_width().min(520.0);
    let height = width * 9.0 / 16.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
    let preview = Rect::from_center_size(rect.center(), Vec2::new(width, height));
    ui.painter().rect_filled(preview, pal.corner(10), pal.card);
    ui.painter().rect_stroke(
        preview,
        pal.corner(10),
        Stroke::new(1.0, pal.card_border),
        StrokeKind::Inside,
    );
    let screen = preview.shrink(10.0);
    ui.painter()
        .rect_filled(screen, pal.corner(6), with_alpha(pal.accent, 34));
    ui.painter().text(
        screen.center() - Vec2::new(0.0, 12.0),
        Align2::CENTER_CENTER,
        &app.info.resolution,
        semibold(28.0),
        pal.text,
    );
    ui.painter().text(
        screen.center() + Vec2::new(0.0, 22.0),
        Align2::CENTER_CENTER,
        format!(
            "{} · {:.0}% scale",
            app.info.refresh,
            app.info
                .display_monitors
                .first()
                .map(|m| m.scale)
                .unwrap_or(1.0)
                * 100.0
        ),
        regular(13.0),
        pal.text_faint,
    );

    spacer(ui, 18.0);

    widgets::card(ui, &pal, |ui| {
        let mut value = app.test_brightness;
        let available = app.info.brightness.is_some();
        let subtitle = match app.info.brightness {
            Some(pct) => format!("{}%", pct),
            None => "Not available on this device".to_owned(),
        };
        let mut changed = false;
        if available {
            changed = slider_row(ui, &pal, Icon::Sun, "Brightness", subtitle, &mut value);
        } else {
            widgets::row(
                ui,
                &pal,
                pal.accent,
                Icon::Sun,
                "Brightness",
                "No backlight was detected",
                Right::None,
            );
        }
        if changed {
            let pct = (value * 100.0).round() as u8;
            if Some(pct) != app.info.brightness {
                match system::set_brightness(pct) {
                    Ok(()) => {
                        app.test_brightness = value;
                        app.info.brightness = Some(pct);
                    }
                    Err(err) => app.notify_error(err),
                }
            }
        }
        widgets::divider(ui, &pal);
        let mut night = app.info.night_light;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Moon,
            "Night light",
            "Warm the screen in the evening",
            Right::Switch(&mut night),
        );
        if outcome.toggled {
            if system::set_night_light(night) {
                app.info.night_light = night;
                app.notify(if night {
                    "Night light turned on"
                } else {
                    "Night light turned off"
                });
            } else {
                app.notify_error("Could not change night light");
            }
        }
        widgets::divider(ui, &pal);
        let mut temp = app.info.night_light_temp as f32;
        let temp_label = format!("{} K", temp.round() as u32);
        if range_row(
            ui,
            &pal,
            Icon::Sun,
            "Night light strength",
            temp_label,
            &mut temp,
            1700.0,
            6500.0,
            100.0,
        ) {
            let value = temp.round() as u32;
            if system::set_night_light_temp(value) {
                app.info.night_light_temp = value;
            } else {
                app.notify_error("Could not change the night light strength");
            }
        }
    });

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Display mode");
    let monitors = app.info.display_monitors.clone();
    let mut apply: Option<(String, String, f32)> = None;
    if monitors.is_empty() {
        widgets::card(ui, &pal, |ui| {
            widgets::empty_note(
                ui,
                &pal,
                "The desktop did not report any displays. On Wayland the compositor provides this list.",
            );
        });
    } else {
        widgets::card(ui, &pal, |ui| {
            for (index, monitor) in monitors.iter().enumerate() {
                if index > 0 {
                    widgets::divider(ui, &pal);
                }
                if monitors.len() > 1 {
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.add_space(ROW_PAD);
                        ui.label(
                            egui::RichText::new(format!("Display {}", index + 1))
                                .font(semibold(12.0))
                                .color(pal.text_muted),
                        );
                    });
                }
                let current = monitor
                    .modes
                    .iter()
                    .find(|m| m.current)
                    .or_else(|| monitor.modes.first())
                    .cloned()
                    .unwrap_or_default();

                // Resolutions
                let mut sizes: Vec<(u32, u32)> =
                    monitor.modes.iter().map(|m| (m.width, m.height)).collect();
                sizes.sort_by(|a, b| (b.0 * b.1).cmp(&(a.0 * a.1)));
                sizes.dedup();
                let res_options: Vec<(String, String)> = sizes
                    .iter()
                    .map(|(w, h)| (format!("{w}x{h}"), format!("{w} × {h}")))
                    .collect();
                let current_res = format!("{}x{}", current.width, current.height);
                if let Some(res) = combo_row(
                    ui,
                    &pal,
                    Icon::Display,
                    "Resolution",
                    "Pixels",
                    &format!("display-res-{}", monitor.connector),
                    &res_options,
                    &current_res,
                    170.0,
                ) {
                    if let Some((w, h)) = res.split_once('x') {
                        let (w, h) = (w.parse::<u32>().unwrap_or(0), h.parse::<u32>().unwrap_or(0));
                        if let Some(mode) = monitor
                            .modes
                            .iter()
                            .filter(|m| m.width == w && m.height == h)
                            .max_by(|a, b| {
                                // Prefer the rate closest to the current one.
                                (a.rate - current.rate)
                                    .abs()
                                    .partial_cmp(&(b.rate - current.rate).abs())
                                    .unwrap_or(std::cmp::Ordering::Equal)
                            })
                        {
                            apply = Some((
                                monitor.connector.clone(),
                                mode.id.clone(),
                                nearest_scale(monitor.scale, &mode.scales),
                            ));
                        }
                    }
                }

                // Refresh rates available at this resolution. Many panels expose
                // several modes (VRR, CEA, reduced blanking) that all round to the
                // same whole number, so keep one entry per displayed rate.
                let mut refresh_options: Vec<(String, String)> = Vec::new();
                for mode in monitor
                    .modes
                    .iter()
                    .filter(|m| m.width == current.width && m.height == current.height)
                {
                    let label = format!("{:.0} Hz", mode.rate);
                    if let Some(existing) =
                        refresh_options.iter_mut().find(|(_, text)| *text == label)
                    {
                        // Prefer the mode that is already active, else the
                        // non-VRR variant which is the plain panel mode.
                        let keep_current = mode.id == current.id;
                        let existing_is_current = existing.0 == current.id;
                        if keep_current && !existing_is_current {
                            *existing = (mode.id.clone(), label);
                        }
                    } else {
                        refresh_options.push((mode.id.clone(), label));
                    }
                }
                let refresh_hint = if refresh_options.len() <= 1 {
                    "This display reports one rate"
                } else {
                    "Updates per second"
                };
                if let Some(id) = combo_row(
                    ui,
                    &pal,
                    Icon::Clock,
                    "Refresh rate",
                    refresh_hint,
                    &format!("display-refresh-{}", monitor.connector),
                    &refresh_options,
                    &current.id,
                    150.0,
                ) {
                    if let Some(mode) = monitor.modes.iter().find(|m| m.id == id) {
                        apply = Some((
                            monitor.connector.clone(),
                            id,
                            nearest_scale(monitor.scale, &mode.scales),
                        ));
                    }
                }

                // Fractional scaling
                let scales = if current.scales.is_empty() {
                    vec![1.0]
                } else {
                    current.scales.clone()
                };
                let scale_options: Vec<(String, String)> = scales
                    .iter()
                    .map(|s| (format!("{s}"), format!("{:.0}%", s * 100.0)))
                    .collect();
                let current_scale = format!("{}", nearest_scale(monitor.scale, &scales));
                if let Some(scale) = combo_row(
                    ui,
                    &pal,
                    Icon::Personalise,
                    "Scale",
                    "Size of windows and text",
                    &format!("display-scale-{}", monitor.connector),
                    &scale_options,
                    &current_scale,
                    130.0,
                ) {
                    if let Ok(value) = scale.parse::<f32>() {
                        apply = Some((monitor.connector.clone(), current.id.clone(), value));
                    }
                }
            }
        });
    }
    if let Some((connector, mode_id, scale)) = apply {
        app.apply_display(connector, mode_id, scale);
    }

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Night light schedule");
    widgets::card(ui, &pal, |ui| {
        let mut scheduled = app.info.night_light_scheduled;
        let schedule_hint = if scheduled {
            if app.info.night_light_auto {
                "Follow sunrise and sunset"
            } else {
                "Turn on during your own hours"
            }
        } else {
            "Off — the Night light switch applies all day"
        };
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Clock,
            "Schedule night light",
            schedule_hint,
            Right::Switch(&mut scheduled),
        );
        if outcome.toggled {
            let from = if system::night_light_all_day(
                app.info.night_light_from,
                app.info.night_light_to,
            ) {
                system::NIGHT_LIGHT_DEFAULT_FROM
            } else {
                app.info.night_light_from
            };
            let to = if system::night_light_all_day(
                app.info.night_light_from,
                app.info.night_light_to,
            ) {
                system::NIGHT_LIGHT_DEFAULT_TO
            } else {
                app.info.night_light_to
            };
            let auto = app.info.night_light_auto && scheduled;
            if system::set_night_light_scheduled(scheduled, auto, from, to) {
                app.info.night_light_scheduled = scheduled;
                app.info.night_light_auto = auto;
                app.info.night_light_from = from;
                app.info.night_light_to = to;
                app.notify(if scheduled {
                    "Night light will follow a schedule"
                } else {
                    "Night light stays on all day"
                });
            } else {
                app.notify_error("Could not change the night light schedule");
            }
        }

        if app.info.night_light_scheduled {
            widgets::divider(ui, &pal);
            let mut auto = app.info.night_light_auto;
            let outcome = widgets::row(
                ui,
                &pal,
                pal.accent,
                Icon::Moon,
                "Sunset to sunrise",
                "Follow the local sunset and sunrise times",
                Right::Switch(&mut auto),
            );
            if outcome.toggled {
                if system::set_night_light_schedule(
                    auto,
                    app.info.night_light_from,
                    app.info.night_light_to,
                ) {
                    app.info.night_light_auto = auto;
                    app.notify(if auto {
                        "Night light follows the sun"
                    } else {
                        "Night light uses your schedule"
                    });
                } else {
                    app.notify_error("Could not change the night light schedule");
                }
            }
        }

        if app.info.night_light_scheduled && !app.info.night_light_auto {
            let time_label = |value: f32| {
                format!(
                    "{:02}:{:02}",
                    value as u32,
                    ((value.fract() * 60.0).round() as u32) % 60
                )
            };
            widgets::divider(ui, &pal);
            let mut from = app.info.night_light_from;
            let from_text = time_label(from);
            if range_row(
                ui,
                &pal,
                Icon::Clock,
                "Starts at",
                from_text,
                &mut from,
                0.0,
                23.75,
                0.25,
            ) && system::set_night_light_schedule(false, from, app.info.night_light_to)
            {
                app.info.night_light_from = from;
            }
            widgets::divider(ui, &pal);
            let mut to = app.info.night_light_to;
            let to_text = time_label(to);
            if range_row(
                ui,
                &pal,
                Icon::Clock,
                "Ends at",
                to_text,
                &mut to,
                0.0,
                23.75,
                0.25,
            ) && system::set_night_light_schedule(false, app.info.night_light_from, to)
            {
                app.info.night_light_to = to;
            }
        }
    });
    if !app.info.night_light_scheduled {
        note(
            ui,
            &pal,
            "With no schedule, the Night light switch warms the screen immediately and keeps it warm until you turn it off.",
        );
    } else {
        note(
            ui,
            &pal,
            "While a schedule is set, night light only applies inside that window. Turn the schedule off to keep it on all day.",
        );
    }
    note(
        ui,
        &pal,
        "Changes are applied through the desktop's display service, so they take effect immediately.",
    );
}
