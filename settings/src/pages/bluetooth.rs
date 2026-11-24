//! Bluetooth & devices: radio, paired and nearby devices.

use eframe::egui::Ui;

use crate::app::App;
use crate::icons::Icon;
use crate::system::{self};
use crate::widgets::{self, Right};

use super::*;

pub fn bluetooth(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    widgets::h1(ui, &pal, "Bluetooth & devices");

    widgets::card(ui, &pal, |ui| {
        let mut on = app.info.bluetooth_on;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Bluetooth,
            "Bluetooth",
            if on {
                "On · discoverable by paired devices"
            } else {
                "Off"
            },
            Right::Switch(&mut on),
        );
        if outcome.toggled {
            match system::set_bluetooth(on) {
                Ok(()) => {
                    app.info.bluetooth_on = on;
                    app.notify(if on {
                        "Bluetooth turned on"
                    } else {
                        "Bluetooth turned off"
                    });
                }
                Err(err) => {
                    app.notify_error(if err.is_empty() {
                        "Could not change Bluetooth".to_owned()
                    } else {
                        err
                    });
                }
            }
        }
        if app.info.bluetooth_on {
            widgets::divider(ui, &pal);
            let mut discoverable = app.info.discoverable;
            let outcome = widgets::row(
                ui,
                &pal,
                pal.accent,
                Icon::Bluetooth,
                "Discoverable",
                "Let nearby devices find this computer",
                Right::Switch(&mut discoverable),
            );
            if outcome.toggled {
                app.set_discoverable(discoverable);
            }
        }
    });

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Devices");
    let devices = app.info.bluetooth_devices.clone();
    let mut device_action: Option<(String, String, BluetoothAction)> = None;
    widgets::card(ui, &pal, |ui| {
        if devices.is_empty() {
            widgets::empty_note(
                ui,
                &pal,
                "No Bluetooth devices found. Use Scan for devices below.",
            );
        }
        for (index, device) in devices.iter().enumerate() {
            if index > 0 {
                widgets::divider(ui, &pal);
            }
            let action = if device.connected {
                ("Disconnect", BluetoothAction::Disconnect)
            } else if device.paired {
                ("Connect", BluetoothAction::Connect)
            } else {
                ("Pair", BluetoothAction::Pair)
            };
            if widgets::row(
                ui,
                &pal,
                pal.accent,
                if device.connected {
                    Icon::Bluetooth
                } else {
                    Icon::Keyboard
                },
                &device.name,
                &format!(
                    "{} · {}",
                    device.mac,
                    if device.connected {
                        "Connected"
                    } else {
                        "Not connected"
                    }
                ),
                Right::AccentText(action.0),
            )
            .clicked()
            {
                device_action = Some((device.mac.clone(), device.name.clone(), action.1));
            }
        }
    });

    spacer(ui, 10.0);
    let scan_label = if app.bt_scanning {
        "Scanning…"
    } else {
        "Scan for devices"
    };
    let scan = widgets::ghost_button(ui, &pal, scan_label, Some(Icon::Bluetooth));
    if scan.clicked() && !app.bt_scanning {
        app.start_bluetooth_scan(ui.ctx());
    }
    if let Some((mac, name, action)) = device_action {
        let result = match action {
            BluetoothAction::Connect => system::connect_bluetooth(&mac, true),
            BluetoothAction::Disconnect => system::connect_bluetooth(&mac, false),
            BluetoothAction::Pair => system::pair_bluetooth(&mac),
        };
        match result {
            Ok(()) => {
                app.notify(match action {
                    BluetoothAction::Connect => format!("Connected to {name}"),
                    BluetoothAction::Disconnect => format!("Disconnected from {name}"),
                    BluetoothAction::Pair => format!("Paired with {name}"),
                });
                reload(app, ui);
            }
            Err(error) => app.notify_error(if error.is_empty() {
                format!("Could not update {name}")
            } else {
                error
            }),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum BluetoothAction {
    Connect,
    Disconnect,
    Pair,
}
