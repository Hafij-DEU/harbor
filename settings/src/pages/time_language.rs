//! Time & language: time zone, clock, NTP, locale, input sources.

use eframe::egui::{self, Ui};

use crate::app::App;
use crate::icons::Icon;
use crate::system::{self};
use crate::widgets::{self, Right};

use super::*;

pub fn time_language(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    widgets::h1(ui, &pal, "Time & language");

    if app.clock_refresh.elapsed().as_secs() >= 1 {
        app.info.local_time = system::local_time();
        app.clock_refresh = std::time::Instant::now();
    }

    widgets::card(ui, &pal, |ui| {
        widgets::info_row(ui, &pal, "Time zone", &app.info.timezone, Right::None);
        widgets::divider(ui, &pal);
        widgets::info_row(ui, &pal, "Local time", &app.info.local_time, Right::None);
        widgets::divider(ui, &pal);
        let mut ntp = app.info.ntp_enabled;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Globe,
            "Set time automatically",
            "Synchronise the clock with internet time servers",
            Right::Switch(&mut ntp),
        );
        if outcome.toggled {
            app.set_ntp(ntp);
        }
        widgets::divider(ui, &pal);
        let mut locales = system::list_locales();
        let current = if app.info.locale.is_empty() {
            "Not set".to_owned()
        } else {
            app.info.locale.clone()
        };
        if !locales.iter().any(|l| *l == current) && !app.info.locale.is_empty() {
            locales.insert(0, current.clone());
        }
        let locale_options: Vec<(String, String)> = locales
            .iter()
            .map(|l| {
                let base = l.split('.').next().unwrap_or(l);
                (
                    l.clone(),
                    base.replace('_', " (") + if base.contains('_') { ")" } else { "" },
                )
            })
            .collect();
        if let Some(locale) = combo_row(
            ui,
            &pal,
            Icon::Globe,
            "Language",
            "Applied the next time you sign in",
            "system-language",
            &locale_options,
            &current,
            200.0,
        ) {
            app.set_locale(locale);
        }
        widgets::divider(ui, &pal);
        let mut clock = app.info.clock_24h;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Clock,
            "24-hour clock",
            "Show 18:30 instead of 6:30 PM",
            Right::Switch(&mut clock),
        );
        if outcome.toggled {
            if system::set_clock_24h(clock) {
                app.info.clock_24h = clock;
                app.notify("Clock format updated");
            } else {
                app.notify_error("Could not change the clock format");
            }
        }
        widgets::divider(ui, &pal);
        let mut show_date = app.info.clock_show_date;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Clock,
            "Show date",
            "Display the date next to the time",
            Right::Switch(&mut show_date),
        );
        if outcome.toggled {
            if system::set_clock_detail("clock-show-date", show_date) {
                app.info.clock_show_date = show_date;
            } else {
                app.notify_error("Could not change the top bar clock");
            }
        }
        widgets::divider(ui, &pal);
        let mut weekday = app.info.clock_show_weekday;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Clock,
            "Show weekday",
            "Display the day of the week next to the time",
            Right::Switch(&mut weekday),
        );
        if outcome.toggled {
            if system::set_clock_detail("clock-show-weekday", weekday) {
                app.info.clock_show_weekday = weekday;
            } else {
                app.notify_error("Could not change the top bar clock");
            }
        }
        widgets::divider(ui, &pal);
        let mut seconds = app.info.clock_show_seconds;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Clock,
            "Show seconds",
            "Display seconds next to the time",
            Right::Switch(&mut seconds),
        );
        if outcome.toggled {
            if system::set_clock_detail("clock-show-seconds", seconds) {
                app.info.clock_show_seconds = seconds;
            } else {
                app.notify_error("Could not change the top bar clock");
            }
        }
    });

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Change time zone");
    let mut filter = app.timezone_filter.clone();
    let mut chosen = None;
    widgets::card(ui, &pal, |ui| {
        _ = widgets::search_box(
            ui,
            &pal,
            &mut filter,
            false,
            "Filter time zones, e.g. Berlin",
        );
        let query = filter.trim().to_lowercase();
        ui.add_space(6.0);
        let zones = system::list_timezones();
        let mut visible: Vec<&String> = zones
            .iter()
            .filter(|z| query.is_empty() || z.to_lowercase().contains(&query))
            .collect();
        if query.is_empty() {
            // Show the active zone first so it is visible without searching.
            if let Some(index) = visible.iter().position(|z| **z == app.info.timezone) {
                let current = visible.remove(index);
                visible.insert(0, current);
            }
        }
        let mut shown = 0usize;
        egui::ScrollArea::vertical()
            .max_height(260.0)
            .id_salt("tz-list")
            .show(ui, |ui| {
                for zone in visible.iter().take(400) {
                    shown += 1;
                    let current = **zone == app.info.timezone;
                    if widgets::row(
                        ui,
                        &pal,
                        pal.accent,
                        Icon::Globe,
                        zone,
                        if current { "Current" } else { "" },
                        if current { Right::Check } else { Right::None },
                    )
                    .clicked()
                        && !current
                    {
                        chosen = Some((*zone).clone());
                    }
                }
                if shown == 0 {
                    widgets::empty_note(ui, &pal, "No time zone matches that filter.");
                }
            });
    });
    if filter != app.timezone_filter {
        app.timezone_filter = filter;
    }
    if let Some(zone) = chosen {
        app.set_timezone(zone);
    }
}
