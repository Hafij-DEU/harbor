//! App Hub-style view of a running or finished job: a heading with a
//! spinner, the current step, a progress bar with how much is done, and the
//! live log underneath.
//!
//! apt runs with `-o APT::Status-Fd=1`, so its machine-readable
//! `dlstatus:`/`pmstatus:` lines drive the bar. Those lines are hidden from
//! the log the user sees.

use eframe::egui::{self, Color32, Id, Margin, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use crate::icons::{self, Icon};
use crate::theme::{regular, semibold, Palette};
use crate::widgets::{self, ROW_PAD};

/// What the panel shows under the heading.
#[derive(Debug, Default, PartialEq)]
pub struct Progress {
    /// The current step, e.g. "Setting up firefox".
    pub step: String,
    /// 0.0..=1.0 when known; `None` draws a moving (indeterminate) bar.
    pub fraction: Option<f32>,
    /// Text under the bar, e.g. "Downloading: 12.3 MB / 45.2 MB".
    pub text: String,
}

/// A job that has ended; kept so the panel can show the result and log
/// until the user closes it.
pub struct DoneJob {
    pub label: String,
    pub log: Vec<String>,
    pub ok: bool,
}

impl DoneJob {
    pub fn new(label: &str, log: &str, ok: bool) -> Self {
        Self {
            label: label.to_owned(),
            log: log
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(str::to_owned)
                .collect(),
            ok,
        }
    }
}

/// Status lines meant for the progress bar, not for people.
pub fn is_status_line(line: &str) -> bool {
    let line = line.trim_start();
    line.starts_with("dlstatus:")
        || line.starts_with("pmstatus:")
        || line.starts_with("pmconffile:")
        || line.starts_with("media-change:")
}

/// Read the log from the top and work out the current step and progress.
pub fn progress(log: &[String]) -> Progress {
    let mut out = Progress::default();
    // Bytes apt said it will download ("Need to get 45.2 MB of archives.").
    let mut total: Option<f64> = None;
    for raw in log {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("== ") {
            continue;
        }
        if let Some(rest) = line.strip_prefix("dlstatus:") {
            if let Some((pct, msg)) = status_fields(rest) {
                out.fraction = Some((pct / 100.0).clamp(0.0, 1.0));
                out.text = match total {
                    Some(t) => format!(
                        "Downloading: {} / {}",
                        human_size(t * f64::from(pct) / 100.0),
                        human_size(t)
                    ),
                    None => format!("Downloading: {pct:.0}%"),
                };
                if !msg.is_empty() {
                    out.step = msg.to_owned();
                }
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("pmstatus:") {
            if let Some((pct, msg)) = status_fields(rest) {
                out.fraction = Some((pct / 100.0).clamp(0.0, 1.0));
                out.text = format!("Installing: {pct:.0}%");
                if !msg.is_empty() {
                    out.step = msg.to_owned();
                }
            }
            continue;
        }
        if is_status_line(line) {
            continue;
        }
        if let Some(bytes) = need_to_get(line) {
            total = Some(bytes);
            out.fraction = Some(0.0);
            out.text = format!("Downloading: 0 B / {}", human_size(bytes));
            out.step = "Downloading packages".to_owned();
            continue;
        }
        // A new phase (snap, Flatpak, the next command) starts a fresh bar.
        let lower = line.to_lowercase();
        if lower.starts_with("updating snap")
            || lower.starts_with("updating flatpak")
            || line.starts_with("$ ")
            || line.starts_with("+ ")
        {
            total = None;
            out.fraction = None;
            out.text.clear();
            out.step = match line.strip_prefix("$ ").or_else(|| line.strip_prefix("+ ")) {
                Some(cmd) => format!("Running {cmd}"),
                None => line
                    .trim_end_matches('…')
                    .trim_end_matches("...")
                    .to_owned(),
            };
            continue;
        }
        // Flatpak: "Updating 2/5… app/org.x.Y" style counters.
        if let Some((n, m)) = counter(line) {
            if ["updating", "installing", "uninstalling"]
                .iter()
                .any(|w| lower.starts_with(w))
            {
                out.fraction = Some(n as f32 / m as f32);
                out.text = format!("{} of {m}", n);
            }
        }
        out.step = line.to_owned();
    }
    out
}

/// `pkg:percent:message` (apt's status-fd format) → (percent, message).
fn status_fields(rest: &str) -> Option<(f32, &str)> {
    let mut parts = rest.splitn(3, ':');
    let _id = parts.next()?;
    let pct = parts.next()?.trim().parse::<f32>().ok()?;
    Some((pct, parts.next().unwrap_or("").trim()))
}

/// "Need to get 45.2 MB of archives." / "Need to get 1,234 kB/2,345 kB of archives."
fn need_to_get(line: &str) -> Option<f64> {
    // apt 3 prints "Download size: 1,234 kB / 2,345 kB" (to get / total).
    let rest = line
        .strip_prefix("Need to get ")
        .or_else(|| line.strip_prefix("Download size: "))?;
    let mut words = rest.split_whitespace();
    let number: f64 = words.next()?.replace(',', "").parse().ok()?;
    let unit = words.next()?.split('/').next()?;
    let scale = match unit {
        "B" => 1.0,
        "kB" => 1e3,
        "MB" => 1e6,
        "GB" => 1e9,
        _ => return None,
    };
    Some(number * scale)
}

/// The first "n/m" pair of numbers in a line, with 0 < n <= m.
fn counter(line: &str) -> Option<(u32, u32)> {
    line.split(|c: char| !(c.is_ascii_digit() || c == '/'))
        .filter_map(|word| {
            let (a, b) = word.split_once('/')?;
            let (n, m) = (a.parse::<u32>().ok()?, b.parse::<u32>().ok()?);
            (n > 0 && n <= m).then_some((n, m))
        })
        .next()
}

/// Decimal units, the way apt prints sizes.
pub fn human_size(bytes: f64) -> String {
    if bytes < 1e3 {
        format!("{bytes:.0} B")
    } else if bytes < 1e6 {
        format!("{:.0} kB", bytes / 1e3)
    } else if bytes < 1e9 {
        format!("{:.1} MB", bytes / 1e6)
    } else {
        format!("{:.2} GB", bytes / 1e9)
    }
}

const ERROR_RED: Color32 = Color32::from_rgb(0xd1, 0x34, 0x38);

/// Draw the panel. `done` is `None` while the job runs and `Some(ok)` once it
/// has ended. Returns true when the user presses Close.
pub fn panel(
    ui: &mut Ui,
    pal: &Palette,
    id: &str,
    label: &str,
    log: &[String],
    done: Option<bool>,
) -> bool {
    let mut close = false;
    let prog = progress(log);
    let show_id = Id::new(("job-log-shown", id));
    // The log is open by default, the way App Hub shows it.
    let mut shown = ui.data(|d| d.get_temp::<bool>(show_id)).unwrap_or(true);
    let width = ui.available_width() - 2.0 * ROW_PAD;

    ui.add_space(14.0);
    ui.horizontal(|ui| {
        ui.add_space(ROW_PAD);
        ui.spacing_mut().item_spacing.x = 10.0;
        match done {
            None => {
                ui.add(egui::Spinner::new().size(18.0).color(pal.accent));
            }
            Some(ok) => {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
                let color = if ok { pal.accent } else { ERROR_RED };
                ui.painter().circle_filled(rect.center(), 9.0, color);
                icons::paint(
                    ui.painter(),
                    if ok { Icon::Check } else { Icon::Info },
                    rect.shrink(3.0),
                    Color32::WHITE,
                );
            }
        }
        let heading = match done {
            None => format!("{label}…"),
            Some(true) => format!("{label} finished"),
            Some(false) => format!("{label} finished with errors"),
        };
        ui.label(
            egui::RichText::new(heading)
                .font(semibold(15.0))
                .color(pal.text),
        );
    });

    let step = match done {
        None if prog.step.is_empty() => "Starting…".to_owned(),
        None => prog.step.clone(),
        Some(true) => "Done.".to_owned(),
        Some(false) => "See the log below for what went wrong.".to_owned(),
    };
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.add_space(ROW_PAD);
        ui.add(
            egui::Label::new(
                egui::RichText::new(step)
                    .font(regular(12.5))
                    .color(pal.text_muted),
            )
            .truncate(),
        );
    });

    // Progress bar.
    ui.add_space(10.0);
    let (row, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 6.0), Sense::hover());
    let track = Rect::from_min_size(
        Pos2::new(row.left() + ROW_PAD, row.top()),
        Vec2::new(width.max(40.0), 6.0),
    );
    let painter = ui.painter();
    painter.rect_filled(track, 3.0, pal.track);
    let fraction = match done {
        Some(true) => Some(1.0),
        Some(false) => prog.fraction,
        None => prog.fraction,
    };
    match fraction {
        Some(f) => {
            let mut fill = track;
            fill.set_width((track.width() * f.clamp(0.0, 1.0)).max(if f > 0.0 {
                6.0
            } else {
                0.0
            }));
            let color = if done == Some(false) {
                ERROR_RED
            } else {
                pal.accent
            };
            painter.rect_filled(fill, 3.0, color);
        }
        None if done.is_none() => {
            // Indeterminate: a segment sweeping across, like a GTK pulse.
            let t = ui.input(|i| i.time) as f32;
            let seg = track.width() * 0.28;
            let x = (t * 0.6).fract() * (track.width() + seg) - seg;
            let left = (track.left() + x).max(track.left());
            let right = (track.left() + x + seg).min(track.right());
            if right > left {
                painter.rect_filled(
                    Rect::from_x_y_ranges(left..=right, track.y_range()),
                    3.0,
                    pal.accent,
                );
            }
            ui.ctx().request_repaint();
        }
        None => {}
    }

    // Text under the bar and the buttons.
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.add_space(ROW_PAD);
        let under = if done.is_none() && !prog.text.is_empty() {
            prog.text.clone()
        } else if done.is_none() {
            "Working…".to_owned()
        } else {
            format!("{} log lines", visible(log).count())
        };
        ui.label(
            egui::RichText::new(under)
                .font(regular(11.5))
                .color(pal.text_faint),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(ROW_PAD);
            if done.is_some() && widgets::button(ui, pal, pal.accent, "Close", None).clicked() {
                close = true;
            }
            let toggle = if shown { "Hide log" } else { "Show log" };
            if widgets::ghost_button(ui, pal, toggle, Some(Icon::Log)).clicked() {
                shown = !shown;
                ui.data_mut(|d| d.insert_temp(show_id, shown));
            }
        });
    });

    if shown {
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(ROW_PAD);
            let box_w = (ui.available_width() - ROW_PAD).min(width) - 22.0;
            egui::Frame::new()
                .fill(pal.input_bg)
                .stroke(Stroke::new(1.0, pal.input_border))
                .corner_radius(pal.corner(6))
                .inner_margin(Margin::same(10))
                .show(ui, |ui| {
                    ui.set_width(box_w);
                    ui.set_max_width(box_w);
                    egui::ScrollArea::vertical()
                        .id_salt(("job-log", id))
                        .max_height(190.0)
                        .auto_shrink([false, true])
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            ui.set_max_width(box_w);
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 1.0;
                                let lines: Vec<&String> = visible(log).collect();
                                if lines.is_empty() {
                                    ui.label(
                                        egui::RichText::new("Waiting for output…")
                                            .font(egui::FontId::monospace(11.5))
                                            .color(pal.text_faint),
                                    );
                                }
                                let skip = lines.len().saturating_sub(300);
                                for line in &lines[skip..] {
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(line.as_str())
                                                .font(egui::FontId::monospace(11.5))
                                                .color(pal.text_muted),
                                        )
                                        .wrap(),
                                    );
                                }
                            });
                        });
                });
        });
    }
    ui.add_space(14.0);
    close
}

fn visible(log: &[String]) -> impl Iterator<Item = &String> {
    log.iter()
        .filter(|l| !l.trim().is_empty() && !is_status_line(l))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(str::to_owned).collect()
    }

    #[test]
    fn apt_download_shows_bytes() {
        let log = lines(
            "Reading package lists...\n\
             Need to get 45.2 MB of archives.\n\
             dlstatus:1:0.0000:Retrieving file 1 of 3\n\
             dlstatus:2:50.0000:Retrieving file 2 of 3",
        );
        let p = progress(&log);
        assert_eq!(p.fraction, Some(0.5));
        assert_eq!(p.text, "Downloading: 22.6 MB / 45.2 MB");
        assert_eq!(p.step, "Retrieving file 2 of 3");
    }

    #[test]
    fn apt_install_and_phase_reset() {
        let log = lines(
            "pmstatus:firefox:75.0:Setting up firefox (amd64)\n\
             Updating snap apps…",
        );
        let p = progress(&log[..1]);
        assert_eq!(p.fraction, Some(0.75));
        assert_eq!(p.text, "Installing: 75%");
        let p = progress(&log);
        assert_eq!(p.fraction, None);
        assert_eq!(p.step, "Updating snap apps");
    }

    #[test]
    fn need_to_get_with_partial_download() {
        assert_eq!(
            need_to_get("Need to get 1,234 kB/2,345 kB of archives."),
            Some(1_234_000.0)
        );
        assert_eq!(need_to_get("Need to get 0 B/1 MB of archives."), Some(0.0));
        assert_eq!(
            need_to_get("Download size: 12.5 MB / 40 MB"),
            Some(12_500_000.0)
        );
        assert_eq!(need_to_get("Download size: 3,544 kB"), Some(3_544_000.0));
    }

    #[test]
    fn flatpak_counter_and_root_commands() {
        let p = progress(&lines("$ apt-get remove -y foo"));
        assert_eq!(p.step, "Running apt-get remove -y foo");
        let p = progress(&lines("Updating 2/4… app/org.gimp.GIMP/x86_64/stable"));
        assert_eq!(p.fraction, Some(0.5));
        assert_eq!(p.text, "2 of 4");
    }

    #[test]
    fn status_lines_are_hidden() {
        let log = lines("dlstatus:1:5:x\nGet:1 http://a b\npmstatus:x:1:y");
        assert_eq!(visible(&log).count(), 1);
    }
}
