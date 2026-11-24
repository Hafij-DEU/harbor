//! Network & internet: Wi-Fi, wired, proxy.

use eframe::egui::Ui;

use crate::app::{App, PromptKind};
use crate::icons::Icon;
use crate::widgets::{self, Right};

use super::*;

pub fn network(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    widgets::h1(ui, &pal, "Network & internet");

    let devices = app.info.net_devices.clone();
    widgets::card(ui, &pal, |ui| {
        if devices.is_empty() {
            widgets::empty_note(ui, &pal, "NetworkManager reported no devices.");
        }
        for (index, device) in devices.iter().enumerate() {
            if index > 0 {
                widgets::divider(ui, &pal);
            }
            let title = if device.connection.is_empty() {
                device.name.clone()
            } else {
                device.connection.clone()
            };
            let mut subtitle = format!("{} · {}", device.kind, device.state);
            if !device.address.is_empty() {
                subtitle.push_str(&format!(" · {}", device.address));
            }
            widgets::row(
                ui,
                &pal,
                pal.accent,
                if device.kind.contains("wifi") {
                    Icon::Wifi
                } else {
                    Icon::Globe
                },
                &title,
                &subtitle,
                Right::None,
            );
        }
    });

    spacer(ui, 14.0);
    widgets::card(ui, &pal, |ui| {
        let mut on = app.info.wifi_enabled;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Wifi,
            "Wi-Fi",
            if on {
                "On · the radio scans for networks"
            } else {
                "Off · the radio is disabled"
            },
            Right::Switch(&mut on),
        );
        if outcome.toggled {
            app.set_wifi_radio(on);
        }
    });

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Proxy");
    let mut proxy_mode: Option<&'static str> = None;
    widgets::card(ui, &pal, |ui| {
        for (index, (value, label, subtitle)) in [
            ("none", "Off", "Connect to the internet directly"),
            ("manual", "Manual", "Use the proxy servers configured below"),
            (
                "auto",
                "Automatic",
                "Use the proxy configuration script from the network",
            ),
        ]
        .iter()
        .enumerate()
        {
            if index > 0 {
                widgets::divider(ui, &pal);
            }
            let selected = app.info.proxy_mode == *value;
            if widgets::row(
                ui,
                &pal,
                pal.accent,
                Icon::Globe,
                label,
                subtitle,
                if selected { Right::Check } else { Right::None },
            )
            .clicked()
                && !selected
            {
                proxy_mode = Some(*value);
            }
        }
        if app.info.proxy_mode == "manual" {
            widgets::divider(ui, &pal);
            let http = if app.info.proxy_http_port > 0 {
                format!(
                    "{}:{}",
                    if app.info.proxy_http_host.is_empty() {
                        "Not set"
                    } else {
                        &app.info.proxy_http_host
                    },
                    app.info.proxy_http_port
                )
            } else {
                "Not set".to_owned()
            };
            if widgets::row(
                ui,
                &pal,
                pal.accent,
                Icon::Globe,
                "HTTP and HTTPS proxy",
                &http,
                Right::AccentText("Edit…"),
            )
            .clicked()
            {
                app.open_proxy_prompt(PromptKind::ProxyHttp);
            }
            widgets::divider(ui, &pal);
            let socks = if app.info.proxy_socks_port > 0 {
                format!(
                    "{}:{}",
                    if app.info.proxy_socks_host.is_empty() {
                        "Not set"
                    } else {
                        &app.info.proxy_socks_host
                    },
                    app.info.proxy_socks_port
                )
            } else {
                "Not set".to_owned()
            };
            if widgets::row(
                ui,
                &pal,
                pal.accent,
                Icon::Globe,
                "SOCKS proxy",
                &socks,
                Right::AccentText("Edit…"),
            )
            .clicked()
            {
                app.open_proxy_prompt(PromptKind::ProxySocks);
            }
            widgets::divider(ui, &pal);
            let ignore = if app.info.proxy_ignore.trim().is_empty() {
                "None".to_owned()
            } else {
                shorten(&app.info.proxy_ignore, 40)
            };
            if widgets::row(
                ui,
                &pal,
                pal.accent,
                Icon::Shield,
                "Ignored hosts",
                &ignore,
                Right::AccentText("Edit…"),
            )
            .clicked()
            {
                app.open_proxy_prompt(PromptKind::ProxyIgnore);
            }
        }
    });
    if let Some(mode) = proxy_mode {
        app.set_proxy_mode(mode);
    }

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Wi-Fi networks");
    let networks = app.info.wifi_networks.clone();
    let mut join: Option<(String, bool)> = None;
    widgets::card(ui, &pal, |ui| {
        if networks.is_empty() {
            widgets::empty_note(
                ui,
                &pal,
                "No Wi-Fi networks were reported. Refresh to scan again.",
            );
        }
        for (index, network) in networks.iter().enumerate() {
            if index > 0 {
                widgets::divider(ui, &pal);
            }
            let secured = !network.security.is_empty() && network.security != "--";
            let subtitle = format!(
                "{} · {}% signal{}",
                if secured { "Secured" } else { "Open" },
                network.signal,
                if network.active { " · Connected" } else { "" }
            );
            if widgets::row(
                ui,
                &pal,
                pal.accent,
                Icon::Wifi,
                &network.ssid,
                &subtitle,
                if network.active {
                    Right::Check
                } else {
                    Right::AccentText("Connect")
                },
            )
            .clicked()
                && !network.active
            {
                join = Some((network.ssid.clone(), secured));
            }
        }
    });
    if let Some(active) = networks.iter().find(|n| n.active) {
        spacer(ui, 16.0);
        widgets::section_label(ui, &pal, "Connected network");
        widgets::card(ui, &pal, |ui| {
            let outcome = widgets::row(
                ui,
                &pal,
                pal.accent,
                Icon::Wifi,
                &active.ssid,
                "Forget the saved connection, including its password",
                Right::AccentText("Forget"),
            );
            if outcome.clicked() {
                app.open_forget_wifi();
            }
        });
    }

    spacer(ui, 10.0);
    if widgets::ghost_button(ui, &pal, "Refresh networks", Some(Icon::Update)).clicked() {
        reload(app, ui);
    }
    if let Some((ssid, secured)) = join {
        app.open_wifi_connect(&ssid, secured);
    }
    note(
        ui,
        &pal,
        "NetworkManager connects to the selected Wi-Fi network. Passwords are sent only to NetworkManager and are never saved by Settings.",
    );
}
