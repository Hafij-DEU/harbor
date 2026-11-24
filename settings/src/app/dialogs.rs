//! Modal dialogs: text prompts, rename, Wi-Fi password, cleaner, activity log, confirm.

use std::sync::mpsc::{self};

use eframe::egui::{
    self, Align, Align2, Color32, Context, CursorIcon, Id, Layout, Margin, Pos2, Rect, ScrollArea,
    Sense, Stroke, StrokeKind, Ui, Vec2,
};

use super::*;
use crate::cleaner;
use crate::hub::Action as HubAction;
use crate::icons::Icon;
use crate::inventory;
use crate::system::{self};
use crate::theme::{regular, semibold, Palette};
use crate::widgets::{self};

impl App {
    pub(crate) fn text_prompt_dialog(&mut self, ctx: &Context, pal: &Palette) {
        let Some(prompt) = self.text_prompt.as_mut() else {
            return;
        };
        let (title, label, hint) = match prompt.kind {
            PromptKind::RealName => (
                "Change your account name",
                "Name",
                "Shown on the sign-in screen and in the top bar.",
            ),
            PromptKind::Password => (
                "Change your password",
                "New password",
                "You will be asked for your administrator password to confirm.",
            ),
            PromptKind::ProxyHttp => (
                "HTTP and HTTPS proxy",
                "Server address",
                "Use the form host:port, for example proxy.example.com:8080.",
            ),
            PromptKind::ProxySocks => (
                "SOCKS proxy",
                "Server address",
                "Use the form host:port, for example 127.0.0.1:1080.",
            ),
            PromptKind::ProxyIgnore => (
                "Ignored hosts",
                "Hosts and domains",
                "Separate entries with commas, for example localhost, 127.0.0.0/8.",
            ),
        };
        let mut close = false;
        let mut save = false;

        egui::Window::new(title)
            .id(Id::new("text-prompt-dialog"))
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .frame(
                egui::Frame::new()
                    .fill(crate::widgets::solid(pal.card))
                    .stroke(Stroke::new(1.0, pal.card_border))
                    .corner_radius(8)
                    .inner_margin(Margin::same(22)),
            )
            .show(ctx, |ui| {
                ui.set_width(360.0);
                ui.label(
                    egui::RichText::new(title)
                        .font(semibold(20.0))
                        .color(pal.text),
                );
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(hint)
                        .font(regular(12.0))
                        .color(pal.text_faint),
                );
                ui.add_space(14.0);
                ui.label(
                    egui::RichText::new(label)
                        .font(regular(12.0))
                        .color(pal.text_faint),
                );
                ui.add_space(4.0);
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut prompt.value)
                        .password(prompt.kind == PromptKind::Password)
                        .font(regular(14.0))
                        .desired_width(f32::INFINITY)
                        .margin(Margin::symmetric(8, 6)),
                );
                if !prompt.busy && ctx.memory(|m| m.focused().is_none()) {
                    edit.request_focus();
                }
                if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    save = true;
                }
                if let Some(err) = &prompt.error {
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(err)
                            .font(regular(12.0))
                            .color(Color32::from_rgb(0xFF, 0x99, 0x99)),
                    );
                }
                if prompt.busy {
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(
                            egui::RichText::new("Saving…")
                                .font(regular(12.0))
                                .color(pal.text_faint),
                        );
                    });
                }
                ui.add_space(18.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if widgets::button(ui, pal, pal.accent, "Save", None).clicked() && !prompt.busy
                    {
                        save = true;
                    }
                    if widgets::ghost_button(ui, pal, "Cancel", None).clicked() && !prompt.busy {
                        close = true;
                    }
                });
            });

        let mut submit: Option<(PromptKind, String)> = None;
        if save && !prompt.busy {
            let value = prompt.value.trim().to_owned();
            prompt.error = None;
            match prompt.kind {
                PromptKind::RealName if value.is_empty() => {
                    prompt.error = Some("Enter your name.".to_owned());
                }
                PromptKind::Password if value.len() < 4 => {
                    prompt.error = Some("Use at least 4 characters.".to_owned());
                }
                PromptKind::ProxyHttp | PromptKind::ProxySocks
                    if parse_host_port(&value).0.is_empty() =>
                {
                    prompt.error = Some("Enter a server address.".to_owned());
                }
                kind => {
                    prompt.busy = true;
                    submit = Some((kind, value));
                }
            }
        }
        if close && !prompt.busy {
            self.text_prompt = None;
        }
        if let Some((kind, value)) = submit {
            match kind {
                PromptKind::RealName => self
                    .run_sys(SysAction::RealName(value.clone()), move || {
                        system::set_real_name(&value)
                    }),
                PromptKind::Password => {
                    self.run_sys(SysAction::Password, move || system::set_password(&value))
                }
                PromptKind::ProxyHttp => {
                    let (host, port) = parse_host_port(&value);
                    self.info.proxy_http_host = host;
                    self.info.proxy_http_port = port;
                    self.info.proxy_mode = "manual".to_owned();
                    self.apply_proxy();
                }
                PromptKind::ProxySocks => {
                    let (host, port) = parse_host_port(&value);
                    self.info.proxy_socks_host = host;
                    self.info.proxy_socks_port = port;
                    self.info.proxy_mode = "manual".to_owned();
                    self.apply_proxy();
                }
                PromptKind::ProxyIgnore => {
                    self.info.proxy_ignore = value;
                    self.apply_proxy();
                }
            }
        }
    }

    pub(crate) fn rename_dialog(&mut self, ctx: &Context, pal: &Palette) {
        let Some(dialog) = self.rename.as_mut() else {
            return;
        };
        let mut close = false;
        let mut save = false;

        egui::Window::new("Rename device")
            .id(Id::new("rename-dialog"))
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .frame(
                egui::Frame::new()
                    .fill(crate::widgets::solid(pal.card))
                    .stroke(Stroke::new(1.0, pal.card_border))
                    .corner_radius(8)
                    .inner_margin(Margin::same(22)),
            )
            .show(ctx, |ui| {
                ui.set_width(360.0);
                ui.label(
                    egui::RichText::new("Rename this device")
                        .font(semibold(20.0))
                        .color(pal.text),
                );
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(
                        "The new name is applied with hostnamectl and needs administrator approval.",
                    )
                    .font(regular(12.0))
                    .color(pal.text_faint),
                );
                ui.add_space(14.0);
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut dialog.buf)
                        .font(regular(14.0))
                        .desired_width(f32::INFINITY)
                        .margin(Margin::symmetric(8, 6)),
                );
                if !dialog.busy && ctx.memory(|m| m.focused().is_none()) {
                    edit.request_focus();
                }
                if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    save = true;
                }
                if let Some(err) = &dialog.error {
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(err)
                            .font(regular(12.0))
                            .color(Color32::from_rgb(0xFF, 0x99, 0x99)),
                    );
                }
                if dialog.busy {
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(
                            egui::RichText::new("Waiting for authentication…")
                                .font(regular(12.0))
                                .color(pal.text_faint),
                        );
                    });
                }
                ui.add_space(18.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if widgets::button(ui, pal, pal.accent, "Save", None).clicked() && !dialog.busy
                    {
                        save = true;
                    }
                    if widgets::ghost_button(ui, pal, "Cancel", None).clicked() && !dialog.busy {
                        close = true;
                    }
                });
            });

        if save && !dialog.busy {
            let name = dialog.buf.trim().to_owned();
            dialog.error = None;
            if name.is_empty() || name.len() > 63 {
                dialog.error = Some("Enter a name of 1–63 characters.".to_owned());
            } else if !name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
            {
                dialog.error =
                    Some("Use only letters, digits, '-' and '.' (no spaces).".to_owned());
            } else {
                dialog.busy = true;
                self.rename_target = name.clone();
                let (tx, rx) = mpsc::channel();
                self.rename_rx = Some(rx);
                std::thread::spawn(move || {
                    let _ = tx.send(system::rename_hostname(&name));
                });
            }
        }

        if close && !dialog.busy {
            self.rename = None;
        }
    }

    pub(crate) fn dialogs(&mut self, ctx: &Context, pal: &Palette) {
        self.text_prompt_dialog(ctx, pal);
        self.rename_dialog(ctx, pal);
        self.wifi_dialog(ctx, pal);
        self.activity_log_dialog(ctx, pal);
        self.cleaner_dialog(ctx, pal);
        self.confirm_dialog(ctx, pal);
        crate::welcome::show(self, ctx, pal);
    }

    pub(crate) fn wifi_dialog(&mut self, ctx: &Context, pal: &Palette) {
        let Some(dialog) = self.wifi_connect.as_mut() else {
            return;
        };
        let mut connect = false;
        let mut close = false;
        egui::Window::new("Join Wi-Fi")
            .id(Id::new("wifi-connect-dialog"))
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .frame(
                egui::Frame::new()
                    .fill(crate::widgets::solid(pal.card))
                    .stroke(Stroke::new(1.0, pal.card_border))
                    .corner_radius(8)
                    .inner_margin(Margin::same(22)),
            )
            .show(ctx, |ui| {
                ui.set_width(380.0);
                ui.label(
                    egui::RichText::new(format!("Join {}", dialog.ssid))
                        .font(semibold(20.0))
                        .color(pal.text),
                );
                ui.add_space(7.0);
                ui.label(
                    egui::RichText::new(if dialog.secured {
                        "Enter the Wi-Fi password. It is sent directly to NetworkManager and is not stored by Settings."
                    } else {
                        "This open network does not require a password."
                    })
                    .font(regular(12.0))
                    .color(pal.text_faint),
                );
                if dialog.secured {
                    ui.add_space(13.0);
                    let edit = ui.add(
                        egui::TextEdit::singleline(&mut dialog.password)
                            .password(true)
                            .hint_text("Wi-Fi password")
                            .font(regular(14.0))
                            .desired_width(f32::INFINITY)
                            .margin(Margin::symmetric(8, 6)),
                    );
                    if ctx.memory(|m| m.focused().is_none()) {
                        edit.request_focus();
                    }
                }
                if let Some(error) = &dialog.error {
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(error)
                            .font(regular(12.0))
                            .color(Color32::from_rgb(0xFF, 0x99, 0x99)),
                    );
                }
                ui.add_space(18.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::button(ui, pal, pal.accent, "Connect", None).clicked() {
                        connect = true;
                    }
                    if widgets::ghost_button(ui, pal, "Cancel", None).clicked() {
                        close = true;
                    }
                });
            });

        if close {
            self.wifi_connect = None;
            return;
        }
        if connect {
            let (ssid, password) = {
                let dialog = self.wifi_connect.as_ref().expect("dialog exists");
                (
                    dialog.ssid.clone(),
                    if dialog.secured {
                        Some(dialog.password.clone())
                    } else {
                        None
                    },
                )
            };
            if password.as_deref().is_some_and(str::is_empty) {
                if let Some(dialog) = self.wifi_connect.as_mut() {
                    dialog.error = Some("Enter the Wi-Fi password.".to_owned());
                }
                return;
            }
            match system::connect_wifi(&ssid, password.as_deref()) {
                Ok(()) => {
                    self.wifi_connect = None;
                    self.notify(format!("Connected to {ssid}"));
                }
                Err(error) => {
                    if let Some(dialog) = self.wifi_connect.as_mut() {
                        dialog.error = Some(if error.is_empty() {
                            "NetworkManager could not join this network.".to_owned()
                        } else {
                            error
                        });
                    }
                }
            }
        }
    }

    pub(crate) fn cleaner_dialog(&mut self, ctx: &Context, pal: &Palette) {
        if !self.cleaner.open {
            return;
        }
        let scanning = self.cleaner.scanning;
        let error = self.cleaner.error.clone();
        let groups_empty = self.cleaner.groups.is_empty();
        let selected_bytes = self
            .cleaner
            .groups
            .iter()
            .map(|g| g.selected_size())
            .sum::<i64>();
        let mut close = false;
        let mut retry = false;
        let mut clean = false;
        let mut toggle_item: Option<(usize, usize)> = None;
        let mut toggle_group: Option<usize> = None;

        egui::Window::new("Free Up Space")
            .id(Id::new("free-up-space-dialog"))
            .title_bar(false)
            .collapsible(false)
            .resizable(true)
            .default_width(600.0)
            .default_height(560.0)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .frame(
                egui::Frame::new()
                    .fill(crate::widgets::solid(pal.card))
                    .stroke(Stroke::new(1.0, pal.card_border))
                    .corner_radius(8)
                    .inner_margin(Margin::same(22)),
            )
            .show(ctx, |ui| {
                ui.set_width(556.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Free Up Space")
                            .font(semibold(20.0))
                            .color(pal.text),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::ghost_button(ui, pal, "Close", None).clicked() {
                            close = true;
                        }
                    });
                });
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new(
                        "What App Hub found: leftover files, unused packages and caches. Pick what to remove.",
                    )
                    .font(regular(12.0))
                    .color(pal.text_faint),
                );
                ui.add_space(10.0);

                if scanning {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(
                            egui::RichText::new("Scanning for cleanable files… (one moment)")
                                .font(regular(13.0))
                                .color(pal.text_muted),
                        );
                    });
                } else if let Some(err) = &error {
                    widgets::card(ui, pal, |ui| {
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new(err)
                                .font(regular(12.0))
                                .color(Color32::from_rgb(0xFF, 0x9B, 0x9B)),
                        );
                        ui.add_space(8.0);
                    });
                    ui.add_space(8.0);
                    if widgets::ghost_button(ui, pal, "Try again", Some(Icon::Update)).clicked() {
                        retry = true;
                    }
                } else if groups_empty {
                    widgets::card(ui, pal, |ui| {
                        widgets::empty_note(
                            ui,
                            pal,
                            "Nothing to clean — your system is tidy.",
                        );
                    });
                    ui.add_space(8.0);
                    if widgets::ghost_button(ui, pal, "Rescan", Some(Icon::Update)).clicked() {
                        retry = true;
                    }
                } else {
                    ScrollArea::vertical()
                        .id_salt("free-up-space-scroll")
                        .auto_shrink([false, false])
                        .max_height(400.0)
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            for (gi, group) in self.cleaner.groups.iter().enumerate() {
                                if gi > 0 {
                                    ui.add_space(10.0);
                                }
                                let size = cleaner::human_bytes(group.size());
                                ui.horizontal(|ui| {
                                    let master_checked = if group.granular {
                                        group.items.iter().all(|i| i.selected)
                                    } else {
                                        group.enabled
                                    };
                                    if cleaner_checkbox(ui, pal, master_checked)
                                        .clicked()
                                    {
                                        toggle_group = Some(gi);
                                    }
                                    ui.label(
                                        egui::RichText::new(&group.title)
                                            .font(semibold(13.5))
                                            .color(pal.text),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.label(
                                                egui::RichText::new(&size)
                                                    .font(semibold(12.0))
                                                    .color(pal.text_faint),
                                            );
                                        },
                                    );
                                });
                                if !group.description.is_empty() {
                                    ui.label(
                                        egui::RichText::new(&group.description)
                                            .font(regular(11.5))
                                            .color(pal.text_faint),
                                    );
                                }
                                if let Some(note) = (!group.note.is_empty()).then_some(&group.note) {
                                    ui.label(
                                        egui::RichText::new(note)
                                            .font(regular(11.0))
                                            .color(pal.text_faint),
                                    );
                                }
                                if group.granular {
                                    ui.add_space(4.0);
                                    widgets::card(ui, pal, |ui| {
                                        for (ii, item) in group.items.iter().enumerate() {
                                            if ii > 0 {
                                                widgets::divider(ui, pal);
                                            }
                                            if cleaner_item_row(ui, pal, item).clicked() {
                                                toggle_item = Some((gi, ii));
                                            }
                                        }
                                    });
                                } else {
                                    ui.add_space(2.0);
                                }
                            }
                        });

                    ui.add_space(10.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let label = if selected_bytes > 0 {
                            format!("Clean Up · {}", cleaner::human_bytes(selected_bytes))
                        } else {
                            "Clean Up".to_owned()
                        };
                        let button = widgets::button(ui, pal, pal.accent, &label, Some(Icon::Trash));
                        if selected_bytes > 0 && button.clicked() {
                            clean = true;
                        }
                        ui.label(
                            egui::RichText::new(format!(
                                "{} selected",
                                cleaner::human_bytes(selected_bytes)
                            ))
                            .font(regular(11.5))
                            .color(pal.text_faint),
                        );
                    });
                }
            });

        if close {
            self.cleaner.open = false;
        }
        if retry {
            self.cleaner.error = None;
            self.cleaner.groups.clear();
            self.cleaner_scan_start();
        }
        if let Some(gi) = toggle_group {
            if let Some(group) = self.cleaner.groups.get_mut(gi) {
                // Master checkbox toggles every item (granular) or the group itself.
                if group.granular {
                    let all = group.items.iter().all(|i| i.selected);
                    for item in group.items.iter_mut() {
                        item.selected = !all;
                    }
                } else {
                    group.enabled = !group.enabled;
                }
            }
        }
        if let Some((gi, ii)) = toggle_item {
            if let Some(item) = self
                .cleaner
                .groups
                .get_mut(gi)
                .and_then(|g| g.items.get_mut(ii))
            {
                item.selected = !item.selected;
            }
        }
        if clean {
            self.cleaner_clean();
        }
    }

    pub(crate) fn activity_log_dialog(&mut self, ctx: &Context, pal: &Palette) {
        if !self.activity_log_open {
            return;
        }
        // Capture the live job (if any) before borrowing `ui` mutably.
        let live = self
            .hub
            .job
            .as_ref()
            .map(|job| (job.label.clone(), shown_log(&job.log)))
            .or_else(|| {
                self.apt_job
                    .as_ref()
                    .map(|job| (job.label.clone(), shown_log(&job.log)))
            });
        egui::Window::new("Activity Log")
            .id(Id::new("activity-log-dialog"))
            .title_bar(false)
            .collapsible(false)
            .resizable(true)
            .default_width(620.0)
            .default_height(480.0)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .frame(
                egui::Frame::new()
                    .fill(crate::widgets::solid(pal.card))
                    .stroke(Stroke::new(1.0, pal.card_border))
                    .corner_radius(8)
                    .inner_margin(Margin::same(22)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Activity Log")
                            .font(semibold(20.0))
                            .color(pal.text),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if widgets::ghost_button(ui, pal, "Close", None).clicked() {
                            self.activity_log_open = false;
                        }
                        if !self.activity.is_empty()
                            && widgets::ghost_button(ui, pal, "Clear log", Some(Icon::Trash))
                                .clicked()
                        {
                            self.activity.clear();
                        }
                    });
                });
                ui.add_space(8.0);

                let bg = pal.input_bg;
                let border = pal.input_border;
                ScrollArea::vertical()
                    .id_salt("activity-log-scroll")
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .max_height(560.0)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        egui::Frame::new()
                            .fill(bg)
                            .stroke(Stroke::new(1.0, border))
                            .corner_radius(4)
                            .inner_margin(Margin::same(10))
                            .show(ui, |ui| {
                                if let Some((label, log)) = &live {
                                    ui.horizontal(|ui| {
                                        ui.spinner();
                                        ui.label(
                                            egui::RichText::new(format!("{label}…"))
                                                .font(semibold(13.0))
                                                .color(pal.text),
                                        );
                                    });
                                    ui.add_space(6.0);
                                    if log.is_empty() {
                                        ui.label(
                                            egui::RichText::new(
                                                "Waiting for output…",
                                            )
                                            .font(regular(12.0))
                                            .color(pal.text_faint),
                                        );
                                    } else {
                                        for line in log.iter().take(400) {
                                            ui.label(
                                                egui::RichText::new(line)
                                                    .font(regular(12.0))
                                                    .color(pal.text_faint),
                                            );
                                        }
                                    }
                                    ui.add_space(8.0);
                                    ui.separator();
                                    ui.add_space(8.0);
                                }
                                let mut any = false;
                                for entry in self.activity.iter().rev().take(120) {
                                    any = true;
                                    let color = if entry.error {
                                        Color32::from_rgb(0xFF, 0x9B, 0x9B)
                                    } else {
                                        pal.text_faint
                                    };
                                    ui.label(
                                        egui::RichText::new(&entry.title)
                                            .font(semibold(12.5))
                                            .color(pal.text),
                                    );
                                    if !entry.details.trim().is_empty() {
                                        ui.label(
                                            egui::RichText::new(&entry.details)
                                                .font(regular(12.0))
                                                .color(color),
                                        );
                                    }
                                    ui.add_space(6.0);
                                }
                                if !any && live.is_none() {
                                    ui.label(
                                        egui::RichText::new(
                                            "No activity yet. Updates, installs and uninstalls will appear here with their full command output.",
                                        )
                                        .font(regular(12.0))
                                        .color(pal.text_faint),
                                    );
                                }
                            });
                    });
            });
    }

    pub(crate) fn confirm_dialog(&mut self, ctx: &Context, pal: &Palette) {
        let Some(dialog) = self.confirm.as_mut() else {
            return;
        };
        let mut accepted = false;
        let mut closed = false;

        egui::Window::new("Confirm")
            .id(Id::new("confirm-dialog"))
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .frame(
                egui::Frame::new()
                    .fill(crate::widgets::solid(pal.card))
                    .stroke(Stroke::new(1.0, pal.card_border))
                    .corner_radius(8)
                    .inner_margin(Margin::same(22)),
            )
            .show(ctx, |ui| {
                ui.set_width(380.0);
                ui.label(
                    egui::RichText::new(&dialog.title)
                        .font(semibold(20.0))
                        .color(pal.text),
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(&dialog.body)
                        .font(regular(12.5))
                        .color(pal.text_faint),
                );
                ui.add_space(18.0);
                // A fixed-height row: a bare right-to-left layout would take the
                // whole screen height and stretch the dialog.
                let row = Vec2::new(ui.available_width(), 34.0);
                ui.allocate_ui_with_layout(row, Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if widgets::button(ui, pal, pal.accent, &dialog.confirm_label, None).clicked()
                        && !dialog.busy
                    {
                        accepted = true;
                    }
                    if widgets::ghost_button(ui, pal, "Cancel", None).clicked() && !dialog.busy {
                        closed = true;
                    }
                });
            });

        if closed && !dialog.busy {
            self.confirm = None;
            return;
        }
        if accepted && !dialog.busy {
            match &dialog.kind {
                ConfirmKind::InstallUpdates => {
                    self.confirm = None;
                    self.run_install_updates();
                }
                ConfirmKind::Uninstall { id, name } => {
                    let id = id.clone();
                    let _ = name;
                    self.confirm = None;
                    self.hub_act(HubAction::Remove, &id);
                }
                ConfirmKind::RemoveSysApp { app, .. } => {
                    let app = app.clone();
                    self.confirm = None;
                    let label = format!("Uninstalling {}", app.name);
                    self.record_activity(label.clone(), "Started.", false);
                    self.hub.sys_removing =
                        Some((app.channel.clone(), app.id.clone(), app.desktop.clone()));
                    self.hub.job = Some(HubJob {
                        label,
                        log: Vec::new(),
                        rx: inventory::remove(&app),
                    });
                }
                ConfirmKind::ForgetWifi => {
                    self.confirm = None;
                    match system::forget_active_wifi() {
                        Ok(message) => {
                            self.notify(&message);
                            self.record_activity("Wi-Fi network forgotten", message, false);
                            self.reload_info();
                        }
                        Err(err) => self.notify_error(&err),
                    }
                }
                ConfirmKind::InstallCompat { package, name } => {
                    let package = package.clone();
                    let name = name.clone();
                    self.confirm = None;
                    self.run_compat_install(&package, &name);
                }
                ConfirmKind::InstallFavorite { key } => {
                    let key = key.clone();
                    self.confirm = None;
                    self.run_favorite_install(&key);
                }
                ConfirmKind::RemoveExtension { uuid } => {
                    let uuid = uuid.clone();
                    self.confirm = None;
                    crate::desktop::uninstall(self, ctx, uuid);
                }
            }
        }
    }
}

/// Small square checkbox used by the Free Up Space dialog.
pub(crate) fn cleaner_checkbox(ui: &mut Ui, pal: &Palette, checked: bool) -> egui::Response {
    let size = 18.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(size + 2.0, 26.0), Sense::click());
    let box_rect = Rect::from_center_size(
        Pos2::new(rect.left() + size / 2.0, rect.center().y),
        Vec2::splat(size),
    );
    if checked {
        ui.painter().rect_filled(box_rect, 4, pal.accent);
        crate::icons::paint(
            ui.painter(),
            Icon::Check,
            box_rect.shrink(3.5),
            pal.on_accent,
        );
    } else {
        ui.painter().rect_filled(box_rect, 4, pal.input_bg);
        ui.painter().rect_stroke(
            box_rect,
            4,
            Stroke::new(1.0, pal.input_border),
            StrokeKind::Inside,
        );
    }
    response.on_hover_cursor(CursorIcon::PointingHand)
}

/// One selectable row inside a granular group (leftover files etc.).
pub(crate) fn cleaner_item_row(
    ui: &mut Ui,
    pal: &Palette,
    item: &cleaner::CleanerItem,
) -> egui::Response {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 34.0), Sense::click());
    if response.hovered() {
        let bg = Rect::from_min_max(
            Pos2::new(rect.left() + 2.0, rect.top() + 1.0),
            Pos2::new(rect.right() - 2.0, rect.bottom() - 1.0),
        );
        ui.painter().rect_filled(bg, 4, pal.hover);
    }
    let check = Rect::from_center_size(
        Pos2::new(rect.left() + 13.0, rect.center().y),
        Vec2::splat(17.0),
    );
    if item.selected {
        ui.painter().rect_filled(check, 4, pal.accent);
        crate::icons::paint(ui.painter(), Icon::Check, check.shrink(3.0), pal.on_accent);
    } else {
        ui.painter().rect_filled(check, 4, pal.input_bg);
        ui.painter().rect_stroke(
            check,
            4,
            Stroke::new(1.0, pal.input_border),
            StrokeKind::Inside,
        );
    }
    let size_text = cleaner::human_bytes(item.size);
    let galley = ui
        .painter()
        .layout_no_wrap(size_text, regular(11.5), pal.text_faint);
    let size_x = rect.right() - 10.0 - galley.size().x;
    ui.painter().galley(
        Pos2::new(size_x, rect.center().y - galley.size().y / 2.0),
        galley.clone(),
        pal.text_faint,
    );
    let label = if item.detail.is_empty() {
        item.label.clone()
    } else {
        format!("{}  ·  {}", item.label, item.detail)
    };
    ui.painter().text(
        Pos2::new(check.right() + 9.0, rect.center().y),
        Align2::LEFT_CENTER,
        &label,
        regular(12.5),
        pal.text,
    );
    response.on_hover_cursor(CursorIcon::PointingHand)
}

/// A live job log without apt's machine-readable progress lines.
pub(crate) fn shown_log(log: &[String]) -> Vec<String> {
    log.iter()
        .filter(|l| !crate::jobview::is_status_line(l))
        .cloned()
        .collect()
}
