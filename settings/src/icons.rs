//! Hand drawn fluent-style glyphs. No icon font required: everything is vector
//! painted at the requested size, so they stay crisp at any scale factor.

use eframe::egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    System,
    Bluetooth,
    Wifi,
    Personalise,
    Apps,
    Hub,
    Accounts,
    Accessibility,
    Clock,
    Compatibility,
    Update,
    Display,
    Sound,
    Bell,
    Battery,
    Storage,
    Info,
    Search,
    ChevronRight,
    Back,
    Sun,
    Moon,
    Copy,
    Check,
    Power,
    Globe,
    Shield,
    Keyboard,
    Sparkle,
    Trash,
    Log,
    Pointer,
    Image,
    Home,
}

struct Pen<'a> {
    painter: &'a Painter,
    rect: Rect,
    color: Color32,
    width: f32,
}

impl<'a> Pen<'a> {
    fn new(painter: &'a Painter, rect: Rect, color: Color32) -> Self {
        let side = rect.width().min(rect.height());
        Self {
            painter,
            rect,
            color,
            width: (side / 13.0).clamp(1.1, 2.6),
        }
    }

    fn p(&self, x: f32, y: f32) -> Pos2 {
        self.rect.min + Vec2::new(x * self.rect.width(), y * self.rect.height())
    }

    fn line(&self, a: (f32, f32), b: (f32, f32), w: f32) {
        self.painter.line_segment(
            [self.p(a.0, a.1), self.p(b.0, b.1)],
            Stroke::new(self.width * w, self.color),
        );
    }

    fn polyline(&self, pts: &[(f32, f32)], w: f32) {
        let pts = pts.iter().map(|(x, y)| self.p(*x, *y)).collect::<Vec<_>>();
        self.painter
            .add(Shape::line(pts, Stroke::new(self.width * w, self.color)));
    }

    fn circle(&self, c: (f32, f32), r: f32, w: f32) {
        let r = r * self.rect.width().min(self.rect.height());
        self.painter
            .circle_stroke(self.p(c.0, c.1), r, Stroke::new(self.width * w, self.color));
    }

    fn dot(&self, c: (f32, f32), r: f32) {
        let r = r * self.rect.width().min(self.rect.height());
        self.painter.circle_filled(self.p(c.0, c.1), r, self.color);
    }

    fn rect(&self, a: (f32, f32), b: (f32, f32), radius: f32, w: f32) {
        let rect = Rect::from_min_max(self.p(a.0, a.1), self.p(b.0, b.1));
        self.painter.rect_stroke(
            rect,
            radius,
            Stroke::new(self.width * w, self.color),
            eframe::egui::StrokeKind::Inside,
        );
    }

    fn fill_rect(&self, a: (f32, f32), b: (f32, f32), radius: f32) {
        let rect = Rect::from_min_max(self.p(a.0, a.1), self.p(b.0, b.1));
        self.painter.rect_filled(rect, radius, self.color);
    }

    fn arc(&self, c: (f32, f32), r: f32, start: f32, end: f32, w: f32) {
        let center = self.p(c.0, c.1);
        let rr = r * self.rect.width().min(self.rect.height());
        let steps = 24;
        let pts = (0..=steps)
            .map(|i| {
                let t = start + (end - start) * i as f32 / steps as f32;
                center + Vec2::new(t.cos(), t.sin()) * rr
            })
            .collect::<Vec<_>>();
        self.painter
            .add(Shape::line(pts, Stroke::new(self.width * w, self.color)));
    }

    fn poly_fill(&self, pts: &[(f32, f32)]) {
        let pts = pts.iter().map(|(x, y)| self.p(*x, *y)).collect::<Vec<_>>();
        self.painter
            .add(Shape::convex_polygon(pts, self.color, Stroke::NONE));
    }
}

pub fn paint(painter: &Painter, icon: Icon, rect: Rect, color: Color32) {
    let pen = Pen::new(painter, rect, color);
    // All geometry lives in a 0..1 box; a little padding keeps strokes inside.
    let l = 0.14;
    let r = 0.86;
    match icon {
        Icon::System => {
            for (y, knob) in [(0.29, 0.36), (0.5, 0.66), (0.71, 0.48)] {
                pen.line((l, y), (r, y), 1.0);
                pen.dot((knob, y), 0.055);
            }
        }
        Icon::Bluetooth => {
            pen.line((0.5, 0.12), (0.5, 0.88), 1.0);
            pen.polyline(&[(0.5, 0.12), (0.72, 0.32), (0.28, 0.68)], 1.0);
            pen.polyline(&[(0.5, 0.88), (0.72, 0.68), (0.28, 0.32)], 1.0);
        }
        Icon::Wifi => {
            for (radius, span) in [(0.59, 1.85), (0.40, 1.55), (0.21, 1.25)] {
                let half = span / 2.0;
                pen.arc(
                    (0.5, 0.78),
                    radius,
                    -std::f32::consts::FRAC_PI_2 - half,
                    -std::f32::consts::FRAC_PI_2 + half,
                    1.0,
                );
            }
            pen.dot((0.5, 0.76), 0.06);
        }
        Icon::Personalise => {
            pen.circle((0.5, 0.5), 0.36, 1.0);
            pen.dot((0.40, 0.37), 0.055);
            pen.dot((0.63, 0.40), 0.055);
            pen.dot((0.66, 0.60), 0.055);
            pen.dot((0.48, 0.66), 0.045);
        }
        Icon::Apps => {
            for (x, y) in [(0.14, 0.14), (0.54, 0.14), (0.14, 0.54), (0.54, 0.54)] {
                pen.rect((x, y), (x + 0.32, y + 0.32), 2.0, 1.0);
            }
        }
        Icon::Hub => {
            pen.rect((0.12, 0.26), (0.88, 0.84), 2.0, 1.0);
            pen.line((0.12, 0.42), (0.88, 0.42), 1.0);
            pen.line((0.42, 0.26), (0.42, 0.42), 1.0);
            pen.line((0.58, 0.26), (0.58, 0.42), 1.0);
            pen.line((0.38, 0.60), (0.62, 0.60), 1.0);
        }
        Icon::Accounts => {
            pen.circle((0.5, 0.34), 0.15, 1.0);
            pen.arc(
                (0.5, 0.84),
                0.29,
                std::f32::consts::PI + 0.25,
                std::f32::consts::TAU - 0.25,
                1.0,
            );
        }
        Icon::Accessibility => {
            pen.circle((0.5, 0.18), 0.09, 1.0);
            pen.line((0.5, 0.30), (0.5, 0.60), 1.0);
            pen.line((0.20, 0.38), (0.80, 0.38), 1.0);
            pen.line((0.5, 0.60), (0.34, 0.86), 1.0);
            pen.line((0.5, 0.60), (0.66, 0.86), 1.0);
        }
        Icon::Clock => {
            pen.circle((0.5, 0.5), 0.37, 1.0);
            pen.line((0.5, 0.5), (0.5, 0.27), 1.0);
            pen.line((0.5, 0.5), (0.68, 0.58), 1.0);
        }
        Icon::Compatibility => {
            pen.rect((0.1, 0.22), (0.62, 0.66), 2.0, 1.0);
            pen.rect((0.38, 0.38), (0.9, 0.82), 2.0, 1.0);
            pen.line((0.1, 0.66), (0.54, 0.66), 1.0);
        }
        Icon::Update => {
            pen.arc((0.5, 0.5), 0.34, -0.55, 4.9, 1.0);
            pen.polyline(&[(0.62, 0.10), (0.85, 0.24), (0.60, 0.35)], 1.0);
        }
        Icon::Trash => {
            // Lid
            pen.line((0.30, 0.26), (0.70, 0.26), 1.0);
            pen.line((0.42, 0.26), (0.46, 0.19), 1.0);
            pen.line((0.58, 0.26), (0.54, 0.19), 1.0);
            // Bin body (trapezoid outline)
            pen.polyline(
                &[(0.36, 0.32), (0.42, 0.86), (0.58, 0.86), (0.64, 0.32)],
                1.0,
            );
            // Inner ribs
            pen.line((0.455, 0.44), (0.475, 0.72), 1.0);
            pen.line((0.545, 0.44), (0.525, 0.72), 1.0);
        }
        Icon::Log => {
            // Page outline with folded corner
            pen.polyline(
                &[
                    (0.30, 0.12),
                    (0.56, 0.12),
                    (0.72, 0.28),
                    (0.72, 0.88),
                    (0.30, 0.88),
                    (0.30, 0.12),
                ],
                1.0,
            );
            pen.polyline(&[(0.56, 0.12), (0.56, 0.28), (0.72, 0.28)], 1.0);
            // Text lines
            pen.line((0.40, 0.44), (0.62, 0.44), 1.0);
            pen.line((0.40, 0.56), (0.62, 0.56), 1.0);
            pen.line((0.40, 0.68), (0.55, 0.68), 1.0);
        }
        Icon::Display => {
            pen.rect((0.08, 0.16), (0.92, 0.68), 2.0, 1.0);
            pen.line((0.5, 0.68), (0.5, 0.82), 1.0);
            pen.line((0.32, 0.84), (0.68, 0.84), 1.0);
        }
        Icon::Sound => {
            pen.poly_fill(&[
                (0.10, 0.40),
                (0.30, 0.40),
                (0.46, 0.22),
                (0.46, 0.78),
                (0.30, 0.60),
                (0.10, 0.60),
            ]);
            pen.arc((0.44, 0.5), 0.24, -0.9, 0.9, 1.0);
            pen.arc((0.44, 0.5), 0.42, -0.8, 0.8, 1.0);
        }
        Icon::Bell => {
            pen.polyline(
                &[
                    (0.20, 0.68),
                    (0.24, 0.62),
                    (0.24, 0.42),
                    (0.28, 0.30),
                    (0.5, 0.22),
                    (0.72, 0.30),
                    (0.76, 0.42),
                    (0.76, 0.62),
                    (0.80, 0.68),
                    (0.20, 0.68),
                ],
                1.0,
            );
            pen.arc((0.5, 0.72), 0.11, 0.15, std::f32::consts::PI - 0.15, 1.0);
        }
        Icon::Battery => {
            pen.rect((0.10, 0.32), (0.80, 0.68), 3.0, 1.0);
            pen.fill_rect((0.82, 0.42), (0.90, 0.58), 1.0);
            pen.fill_rect((0.17, 0.39), (0.58, 0.61), 1.0);
        }
        Icon::Storage => {
            pen.rect((0.10, 0.26), (0.90, 0.74), 3.0, 1.0);
            pen.line((0.10, 0.48), (0.90, 0.48), 1.0);
            pen.dot((0.75, 0.61), 0.055);
            pen.line((0.20, 0.61), (0.48, 0.61), 1.0);
        }
        Icon::Info => {
            pen.circle((0.5, 0.5), 0.37, 1.0);
            pen.dot((0.5, 0.32), 0.05);
            pen.line((0.5, 0.43), (0.5, 0.69), 1.0);
        }
        Icon::Search => {
            pen.circle((0.43, 0.43), 0.27, 1.0);
            pen.line((0.62, 0.62), (0.84, 0.84), 1.1);
        }
        Icon::ChevronRight => {
            pen.polyline(&[(0.40, 0.26), (0.64, 0.5), (0.40, 0.74)], 1.0);
        }
        Icon::Home => {
            pen.polyline(&[(0.12, 0.5), (0.5, 0.16), (0.88, 0.5)], 1.0);
            pen.polyline(
                &[(0.24, 0.42), (0.24, 0.84), (0.76, 0.84), (0.76, 0.42)],
                1.0,
            );
            pen.polyline(&[(0.42, 0.84), (0.42, 0.62), (0.58, 0.62), (0.58, 0.84)], 1.0);
        }
        Icon::Back => {
            pen.line((0.16, 0.5), (0.84, 0.5), 1.0);
            pen.polyline(&[(0.16, 0.5), (0.38, 0.28)], 1.0);
            pen.polyline(&[(0.16, 0.5), (0.38, 0.72)], 1.0);
        }
        Icon::Sun => {
            pen.circle((0.5, 0.5), 0.17, 1.0);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                pen.line(
                    (0.5 + a.cos() * 0.29, 0.5 + a.sin() * 0.29),
                    (0.5 + a.cos() * 0.38, 0.5 + a.sin() * 0.38),
                    1.0,
                );
            }
        }
        Icon::Moon => {
            let outer = |a: f32| (0.5 + a.cos() * 0.34, 0.5 + a.sin() * 0.34);
            let inner_center = (0.62, 0.38);
            let inner = |a: f32| {
                (
                    inner_center.0 + a.cos() * 0.33,
                    inner_center.1 + a.sin() * 0.33,
                )
            };
            let mut pts = Vec::new();
            for i in 0..=24 {
                let a = -1.9 + 4.9 * i as f32 / 24.0;
                pts.push(outer(a));
            }
            for i in 0..=24 {
                let a = 0.5 - 3.6 * i as f32 / 24.0;
                pts.push(inner(a));
            }
            pen.polyline(&pts, 1.0);
        }
        Icon::Copy => {
            pen.rect((0.34, 0.30), (0.84, 0.86), 2.0, 1.0);
            pen.polyline(
                &[
                    (0.24, 0.66),
                    (0.16, 0.66),
                    (0.16, 0.14),
                    (0.66, 0.14),
                    (0.66, 0.26),
                ],
                1.0,
            );
        }
        Icon::Check => {
            pen.polyline(&[(0.16, 0.52), (0.40, 0.74), (0.84, 0.28)], 1.2);
        }
        Icon::Power => {
            pen.arc(
                (0.5, 0.52),
                0.33,
                -std::f32::consts::FRAC_PI_2 + 0.55,
                std::f32::consts::FRAC_PI_2 + std::f32::consts::PI - 0.55,
                1.0,
            );
            pen.line((0.5, 0.20), (0.5, 0.52), 1.0);
        }
        Icon::Globe => {
            pen.circle((0.5, 0.5), 0.36, 1.0);
            pen.line((0.14, 0.5), (0.86, 0.5), 1.0);
            pen.arc(
                (0.5, 0.5),
                0.18,
                -std::f32::consts::FRAC_PI_2,
                std::f32::consts::FRAC_PI_2,
                1.0,
            );
            pen.arc(
                (0.5, 0.5),
                0.18,
                std::f32::consts::FRAC_PI_2,
                std::f32::consts::PI * 1.5,
                1.0,
            );
        }
        Icon::Shield => {
            pen.polyline(
                &[
                    (0.5, 0.10),
                    (0.82, 0.22),
                    (0.82, 0.50),
                    (0.76, 0.68),
                    (0.5, 0.88),
                    (0.24, 0.68),
                    (0.18, 0.50),
                    (0.18, 0.22),
                    (0.5, 0.10),
                ],
                1.0,
            );
            pen.polyline(&[(0.36, 0.48), (0.47, 0.6), (0.66, 0.38)], 1.0);
        }
        Icon::Pointer => {
            pen.rect((0.32, 0.08), (0.68, 0.92), 4.0, 1.0);
            pen.line((0.5, 0.08), (0.5, 0.32), 1.0);
            pen.line((0.5, 0.38), (0.5, 0.50), 1.4);
        }
        Icon::Image => {
            pen.rect((0.10, 0.16), (0.90, 0.84), 2.0, 1.0);
            pen.circle((0.36, 0.40), 0.085, 1.0);
            pen.polyline(&[(0.10, 0.72), (0.38, 0.46), (0.56, 0.64), (0.68, 0.52), (0.90, 0.74)], 1.0);
        }
        Icon::Keyboard => {
            pen.rect((0.08, 0.30), (0.92, 0.70), 2.0, 1.0);
            for x in [0.20, 0.34, 0.48, 0.62, 0.76] {
                pen.dot((x, 0.44), 0.03);
            }
            pen.line((0.26, 0.58), (0.74, 0.58), 1.0);
        }
        Icon::Sparkle => {
            pen.line((0.5, 0.12), (0.5, 0.88), 1.0);
            pen.line((0.12, 0.5), (0.88, 0.5), 1.0);
            pen.line((0.24, 0.24), (0.76, 0.76), 1.0);
            pen.line((0.76, 0.24), (0.24, 0.76), 1.0);
        }
    }
}

/// Draw a glyph centred inside `rect` with a fixed, square footprint.
pub fn paint_square(painter: &Painter, icon: Icon, rect: Rect, color: Color32) {
    let side = rect.width().min(rect.height());
    let square = Rect::from_center_size(rect.center(), Vec2::splat(side));
    paint(painter, icon, square, color);
}
