//! "Remap keys and shortcuts" card (like PowerToys Keyboard Manager): a key
//! or shortcut types another key or shortcut, or nothing, on every keyboard
//! or one keyboard. Edits stay in `KeyboardUi::draft` until Apply.

use eframe::egui::{self, Ui};

use super::state;
use crate::app::App;
use crate::icons::Icon;
use crate::keyboard::config::{Rule, RuleKind};
use crate::keyboard::devices::{Keyboard, Target};
use crate::keyboard::keys::{
    self, mod_bit, mod_label, Combo, DISABLED, MODS, MOD_ALT, MOD_CTRL, MOD_SHIFT, MOD_SUPER,
};
use crate::theme::Palette;
use crate::widgets::{self, Right};

/// Which side of a rule a Type button edits.
const FROM: u8 = 0;
const TO: u8 = 1;

pub fn show(app: &mut App, ui: &mut Ui, pal: &Palette) {
    widgets::section_label(ui, pal, "Remap keys and shortcuts");
    let saved = app.keyboard.saved().clone();
    let mut draft = app.keyboard.draft.clone().unwrap_or_else(|| saved.clone());
    let boards = app.keyboard.boards.clone();
    let running = app.keyboard.service_running;
    let saving = app.keyboard.busy();
    let mut apply = false;

    widgets::card(ui, pal, |ui| {
        let sub = if !draft.remap {
            "Off: every key types as usual".to_owned()
        } else if draft.rules.is_empty() {
            "Add a remap below".to_owned()
        } else if running {
            format!("On: {} remap(s)", draft.rules.len())
        } else {
            "Not running yet: select Apply".to_owned()
        };
        widgets::row(
            ui,
            pal,
            pal.accent,
            Icon::Keyboard,
            "Remap keys and shortcuts",
            &sub,
            Right::Switch(&mut draft.remap),
        );
        // Same left and right inset as the row text above.
        egui::Frame::NONE
            .inner_margin(egui::Margin::symmetric(16, 0))
            .show(ui, |ui| {
            let mut remove = None;
            for (kind, title, sub) in [
                (
                    RuleKind::Key,
                    "Remap a key",
                    "A key types another key or shortcut, or nothing. Select Type and press the key.",
                ),
                (RuleKind::Shortcut, "Remap a shortcut", "A shortcut types another shortcut or key"),
            ] {
                widgets::divider(ui, pal);
                ui.add_space(8.0);
                ui.label(egui::RichText::new(title).size(14.5).color(pal.text));
                ui.label(egui::RichText::new(sub).size(12.0).color(pal.text_faint));
                ui.add_space(6.0);
                for (i, rule) in draft.rules.iter_mut().enumerate() {
                    if rule.kind != kind {
                        continue;
                    }
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        let capture = &mut app.keyboard.capture;
                        combo_editor(ui, pal, (i, FROM), &mut rule.from, kind == RuleKind::Shortcut, false, capture);
                        ui.label(egui::RichText::new("  →  ").size(15.0).color(pal.text_muted));
                        combo_editor(ui, pal, (i, TO), &mut rule.to, true, true, capture);
                        keyboard_picker(ui, i, &mut rule.on, &boards);
                        if ui.link(egui::RichText::new("Remove").color(pal.accent)).clicked() {
                            remove = Some(i);
                        }
                    });
                    ui.add_space(4.0);
                }
                let add = match kind {
                    RuleKind::Key => "+ Add a key remap",
                    RuleKind::Shortcut => "+ Add a shortcut remap",
                };
                if ui.link(egui::RichText::new(add).color(pal.accent)).clicked() {
                    let from = match kind {
                        RuleKind::Key => Combo { mods: 0, key: 58 }, // Caps Lock
                        RuleKind::Shortcut => Combo { mods: MOD_CTRL | MOD_ALT, key: 20 }, // Ctrl+Alt+T
                    };
                    draft.rules.push(Rule { kind, from, to: Combo { mods: 0, key: 1 }, on: Target::Every });
                }
                ui.add_space(6.0);
            }
            if let Some(i) = remove {
                draft.rules.remove(i);
                app.keyboard.capture = None;
            }
            widgets::divider(ui, pal);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let changed = draft.remap != saved.remap || draft.rules != saved.rules;
                let label = if saving { "Saving…" } else { "Apply" };
                let resp = widgets::button(ui, pal, pal.accent, label, None);
                if resp.clicked() && !saving && (changed || !running) {
                    apply = true;
                }
                let note = if changed {
                    "Unsaved changes"
                } else {
                    "Also works at the login screen"
                };
                ui.label(egui::RichText::new(note).size(12.0).color(pal.text_faint));
            });
            });
        ui.add_space(6.0);
    });

    if apply {
        let mut cfg = saved;
        cfg.remap = draft.remap;
        cfg.rules = draft.rules.clone();
        state::save(app, cfg, "Remaps saved");
    }
    app.keyboard.draft = Some(draft);
}

/// One key (with modifiers when `mods`): a Type button that records the next
/// key press, modifier pills and a key list. `can_disable` adds Disable.
fn combo_editor(
    ui: &mut Ui,
    pal: &Palette,
    id: (usize, u8),
    combo: &mut Combo,
    mods: bool,
    can_disable: bool,
    capture: &mut Option<(usize, u8)>,
) {
    if *capture == Some(id) {
        if let Some(typed) = typed_combo(ui, mods) {
            *combo = typed;
            *capture = None;
        }
        ui.ctx().request_repaint();
    }
    let disabled = can_disable && combo.key == DISABLED;
    if !disabled {
        let waiting = *capture == Some(id);
        if super::super::segment(
            ui,
            pal,
            if waiting { "Press a key…" } else { "Type" },
            waiting,
        ) {
            *capture = if waiting { None } else { Some(id) };
        }
        if mods {
            for (bit, _, _) in MODS {
                if super::super::segment(ui, pal, mod_label(bit), combo.mods & bit != 0) {
                    combo.mods ^= bit;
                }
            }
        }
        egui::ComboBox::from_id_salt(("keymap-key", id.0, id.1))
            .selected_text(keys::label(combo.key))
            .width(150.0)
            .height(320.0)
            .show_ui(ui, |ui| {
                for (_, code, label) in keys::KEYS.iter().filter(|k| k.1 != DISABLED) {
                    ui.selectable_value(&mut combo.key, *code, *label);
                }
            });
    }
    if can_disable && super::super::segment(ui, pal, "Disable", disabled) {
        *combo = Combo {
            mods: 0,
            key: if disabled { 1 } else { DISABLED },
        };
        if *capture == Some(id) {
            *capture = None;
        }
    }
}

/// The key just pressed in Settings, as a combo (None while waiting).
/// A shortcut waits for its main key; a single key may be a modifier.
fn typed_combo(ui: &Ui, mods: bool) -> Option<Combo> {
    let (events, held_super) = ui.input(|i| {
        (
            i.events.clone(),
            i.keys_down.contains(&egui::Key::SuperLeft)
                || i.keys_down.contains(&egui::Key::SuperRight),
        )
    });
    for event in events {
        let egui::Event::Key {
            key,
            physical_key,
            pressed: true,
            repeat: false,
            modifiers,
            ..
        } = event
        else {
            continue;
        };
        let Some(code) = egui_code(physical_key.unwrap_or(key)) else {
            continue;
        };
        if mods && mod_bit(code) != 0 {
            continue;
        }
        let mut bits = 0;
        if mods {
            for (on, bit) in [
                (modifiers.ctrl, MOD_CTRL),
                (modifiers.alt, MOD_ALT),
                (modifiers.shift, MOD_SHIFT),
                (held_super, MOD_SUPER),
            ] {
                if on {
                    bits |= bit;
                }
            }
        }
        return Some(Combo {
            mods: bits,
            key: code,
        });
    }
    None
}

/// The Linux key code for a key egui reports. Caps Lock, Menu and media keys
/// never reach the app: those are picked from the list.
fn egui_code(key: egui::Key) -> Option<u16> {
    use egui::Key as K;
    let name = match key {
        K::Escape => "ESC",
        K::Tab => "TAB",
        K::Backspace => "BACKSPACE",
        K::Enter => "ENTER",
        K::Space => "SPACE",
        K::Insert => "INSERT",
        K::Delete => "DELETE",
        K::Home => "HOME",
        K::End => "END",
        K::PageUp => "PAGEUP",
        K::PageDown => "PAGEDOWN",
        K::ArrowUp => "UP",
        K::ArrowDown => "DOWN",
        K::ArrowLeft => "LEFT",
        K::ArrowRight => "RIGHT",
        K::Comma => "COMMA",
        K::Backslash => "BACKSLASH",
        K::Slash => "SLASH",
        K::OpenBracket => "LEFTBRACE",
        K::CloseBracket => "RIGHTBRACE",
        K::Backtick => "GRAVE",
        K::Minus => "MINUS",
        K::Period => "DOT",
        K::Equals => "EQUAL",
        K::Semicolon => "SEMICOLON",
        K::Quote => "APOSTROPHE",
        K::ShiftLeft => "LEFTSHIFT",
        K::ShiftRight => "RIGHTSHIFT",
        K::ControlLeft => "LEFTCTRL",
        K::ControlRight => "RIGHTCTRL",
        K::AltLeft => "LEFTALT",
        K::AltRight => "RIGHTALT",
        K::SuperLeft => "LEFTMETA",
        K::SuperRight => "RIGHTMETA",
        K::IntlBackslash => "102ND",
        other => {
            // Letters, digits ("Num1" -> "1") and F-keys share their names.
            let n = other.name();
            let n = n.strip_prefix("Num").filter(|d| d.len() == 1).unwrap_or(n);
            return keys::code(&n.to_ascii_uppercase());
        }
    };
    keys::code(name)
}

/// Which keyboard a rule works on: every keyboard, the laptop's own, or one
/// plugged-in keyboard (listed by name, saved by vendor:product).
fn keyboard_picker(ui: &mut Ui, i: usize, on: &mut Target, boards: &[Keyboard]) {
    let text = match on {
        Target::Every => "Every keyboard".to_owned(),
        Target::Internal => "Laptop keyboard".to_owned(),
        Target::Id(id) => boards
            .iter()
            .find(|k| &k.id == id)
            .map_or_else(|| format!("Keyboard {id} (unplugged)"), |k| k.name.clone()),
    };
    egui::ComboBox::from_id_salt(("keymap-board", i))
        .selected_text(text)
        .width(190.0)
        .show_ui(ui, |ui| {
            ui.selectable_value(on, Target::Every, "Every keyboard");
            if boards.iter().any(|k| k.is_internal()) {
                ui.selectable_value(on, Target::Internal, "Laptop keyboard");
            }
            for k in boards.iter().filter(|k| !k.is_internal()) {
                ui.selectable_value(on, k.target(), &k.name);
            }
        });
}
