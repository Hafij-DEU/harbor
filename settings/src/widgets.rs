//! Building blocks for both designs: cards, rows, switches, sliders, buttons.

use eframe::egui::{
    self, Align2, Color32, CursorIcon, Id, Margin, Pos2, Rect, Response, Sense, Stroke, StrokeKind,
    Ui, Vec2,
};

use crate::icons::{self, Icon};
use crate::theme::{regular, semibold, Palette};

pub const ROW_H: f32 = 62.0;
/// Windows 10 uses denser rows than Windows 11.
pub const ROW_H_10: f32 = 54.0;
pub const ROW_PAD: f32 = 16.0;
pub const CHIP: f32 = 28.0;

pub fn h1(ui: &mut Ui, pal: &Palette, text: &str) {
    ui.add_space(2.0);
    let size = if pal.design == crate::theme::Design::Win10 {
        24.0
    } else {
        28.0
    };
    ui.label(
        egui::RichText::new(text)
            .font(semibold(size))
            .color(pal.text),
    );
    ui.add_space(14.0);
}

pub fn h1_back(ui: &mut Ui, pal: &Palette, text: &str) -> bool {
    // Windows 10 navigates sub-pages from the navigation pane, so its
    // page titles carry no back arrow.
    if pal.design == crate::theme::Design::Win10 {
        h1(ui, pal, text);
        return false;
    }
    let mut back = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let (rect, resp) = ui.allocate_exact_size(Vec2::splat(34.0), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(rect, pal.corner(6), pal.hover);
        }
        let glyph = Rect::from_center_size(rect.center(), Vec2::splat(18.0));
        icons::paint(ui.painter(), Icon::Back, glyph, pal.text);
        if resp.clicked() {
            back = true;
        }
        let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
        let _ = resp;
        let size = if pal.design == crate::theme::Design::Win10 {
            24.0
        } else {
            28.0
        };
        ui.label(
            egui::RichText::new(text)
                .font(semibold(size))
                .color(pal.text),
        );
    });
    ui.add_space(14.0);
    back
}

pub fn section_label(ui: &mut Ui, pal: &Palette, text: &str) {
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new(text)
            .font(semibold(14.0))
            .color(pal.text),
    );
    ui.add_space(6.0);
}

pub fn card<R>(ui: &mut Ui, pal: &Palette, add: impl FnOnce(&mut Ui) -> R) -> R {
    let width = ui.available_width();
    // The fill is painted afterwards under the content, so glass styles can
    // add their sheen at the card's final size.
    let fill_at = ui.painter().add(egui::Shape::Noop);
    let frame = egui::Frame::new()
        .stroke(Stroke::new(1.0, pal.card_border))
        .corner_radius(pal.corner(8))
        .inner_margin(Margin::ZERO);
    let shown = frame.show(ui, |ui| {
        ui.set_width(width);
        ui.spacing_mut().item_spacing.y = 0.0;
        add(ui)
    });
    let rect = shown.response.rect;
    ui.painter()
        .set(fill_at, glass_fill(pal, rect, pal.corner(8), pal.card));
    shown.inner
}

/// A rounded fill in `color` with the glass look of `pal.glass`, layered the
/// way Fusion Desk draws its glass: optional shadow, the fill, the style's
/// shading, the gloss band, corner glare, the pointer light and the rim.
/// The same colour without transparency: dialogs and their text boxes stay
/// readable over a see-through glass window.
pub fn solid(color: Color32) -> Color32 {
    let [r, g, b, _] = color.to_srgba_unmultiplied();
    Color32::from_rgb(r, g, b)
}

pub fn glass_fill(pal: &Palette, rect: Rect, corner: u8, color: Color32) -> egui::Shape {
    use crate::theme::{GlassEffect, GlassMaterial, GlassStyle};
    let g = pal.glass;
    if !g.on() || rect.height() < 4.0 || rect.width() < 4.0 {
        return egui::Shape::rect_filled(rect, corner, color);
    }
    if g.style == GlassStyle::Planner {
        return planner_card(pal, rect, corner, color);
    }
    let dark = pal.bg.r() < 128;
    let r = (corner as f32).min(rect.height() / 2.0);
    let k = if g.effect == GlassEffect::Strong {
        1.0
    } else {
        0.6
    };
    // White layers are much weaker on dark surfaces, and on strong colours
    // (accent buttons) so the colour stays.
    let colourful = color.r().abs_diff(color.b()) > 60 || color.g().abs_diff(color.b()) > 60;
    let mut wk = if dark { k * 0.4 } else { k };
    if colourful {
        wk *= 0.7;
    }
    let gloss = g.material == GlassMaterial::Gloss;
    let scale = crate::theme::Glass::scale;
    let (ss, sh, rs, gs, ds, ls) = (
        scale(g.style_strength),
        scale(g.shine_strength),
        scale(g.rim_strength),
        scale(g.glare_strength),
        scale(g.shadow_strength),
        scale(g.light_strength),
    );
    let (top, bottom) = (rect.top(), rect.bottom());
    let mut shapes = Vec::new();
    if g.shadow && ds > 0.0 {
        for (grow, a) in [(7.0_f32, 0.05), (4.0, 0.08), (1.5, 0.12)] {
            let grow = grow * ds.max(0.3);
            shapes.push(egui::Shape::rect_filled(
                rect.translate(Vec2::new(0.0, grow * 0.5)).expand(grow),
                corner.saturating_add(grow as u8),
                black(a * ds * if dark { 2.2 } else { 1.0 }),
            ));
        }
    }
    shapes.push(egui::Shape::rect_filled(rect, corner, color));
    let full = |stops: &[(f32, Color32)]| rounded_rows(rect, r, top, bottom, stops);
    // The style's shading follows the style strength.
    let (wk_shine, k_full) = (wk, k);
    let (wk, k) = (wk * ss, k * ss);
    shapes.push(match g.style {
        GlassStyle::Frosted => full(&[(0.0, white(0.30 * wk)), (1.0, white(0.16 * wk))]),
        GlassStyle::Gel => full(&[
            (0.0, white(0.40 * wk)),
            (0.6, white(0.06 * wk)),
            (1.0, if dark { black(0.14 * k) } else { white(0.0) }),
        ]),
        GlassStyle::Tinted => full(&[
            (0.0, tinted(pal.accent, 0.34 * k)),
            (1.0, tinted(pal.accent, 0.12 * k)),
        ]),
        GlassStyle::Smoke => full(&[(0.0, black(0.06 * k)), (1.0, black(0.24 * k))]),
        GlassStyle::Halo => radial(
            rect.center() - Vec2::new(0.0, rect.height() * 0.05),
            rect.size() * 0.65,
            white(0.26 * wk),
            rect,
            r,
        ),
        GlassStyle::Clear | GlassStyle::Glow | GlassStyle::Planner => full(&[
            (0.0, white(0.18 * wk)),
            (0.7, white(0.03 * wk)),
            (1.0, white(0.0)),
        ]),
    });
    if gloss {
        // Polished gloss: a specular band over the top, capped in height so
        // tall cards keep a calm body, and a deeper bottom on dark surfaces.
        let band = (rect.height() * 0.5).min((rect.height() * 0.3).clamp(28.0, 160.0));
        shapes.push(rounded_rows(
            rect,
            r,
            top,
            top + band,
            &[
                (0.0, white(if dark { 0.40 } else { 0.70 } * wk_shine * sh)),
                (0.55, white(if dark { 0.12 } else { 0.28 } * wk_shine * sh)),
                (1.0, white(0.0)),
            ],
        ));
        if dark {
            shapes.push(full(&[
                (0.0, black(0.0)),
                (0.6, black(0.0)),
                (1.0, black(0.2 * k_full * sh)),
            ]));
        }
    }
    if g.glare && rect.height() >= 16.0 {
        // Light catching the glass: a glow near the top-left corner and a
        // sheen in the bottom-right corner.
        let (w, h) = (rect.width(), rect.height());
        let big = (w.max(h) * 0.45).min((w.min(h) * 1.6).max(60.0));
        let a = if dark { 0.4 } else { 0.5 } * wk_shine * gs * if gloss { 0.6 } else { 1.0 };
        shapes.push(radial(
            rect.min + Vec2::new(big * 0.3, big * 0.2),
            Vec2::splat(big),
            white(a),
            rect,
            r,
        ));
        let small = (big * 0.55).min(w).min(h);
        shapes.push(radial(
            rect.max,
            Vec2::splat(small),
            white(a * 0.7),
            rect,
            r,
        ));
    }
    if g.light_follows {
        if let Some(p) = pal.pointer.filter(|p| rect.contains(*p)) {
            let size = (rect.width().max(rect.height()) * 0.5).clamp(30.0, 140.0);
            shapes.push(radial(
                p,
                Vec2::splat(size),
                white(if dark { 0.12 } else { 0.35 } * ls),
                rect,
                r,
            ));
        }
    }
    if g.rim {
        let k = k_full * rs;
        let c = r.max(1.0);
        let edge = |y: f32, col: Color32| {
            egui::Shape::line_segment(
                [
                    Pos2::new(rect.left() + c, y),
                    Pos2::new(rect.right() - c, y),
                ],
                Stroke::new(1.0, col),
            )
        };
        shapes.push(egui::Shape::rect_stroke(
            rect,
            corner,
            Stroke::new(1.0, white(if dark { 0.16 } else { 0.45 } * k)),
            StrokeKind::Inside,
        ));
        if gloss {
            shapes.push(edge(top + 0.6, white(if dark { 0.6 } else { 0.95 } * k)));
            shapes.push(edge(
                bottom - 0.6,
                black(if dark { 0.38 } else { 0.16 } * k),
            ));
        } else {
            shapes.push(edge(top + 0.6, white(if dark { 0.32 } else { 0.95 } * k)));
        }
    }
    egui::Shape::Vec(shapes)
}

fn white(a: f32) -> Color32 {
    Color32::from_white_alpha((a.clamp(0.0, 1.0) * 255.0) as u8)
}

fn black(a: f32) -> Color32 {
    Color32::from_black_alpha((a.clamp(0.0, 1.0) * 255.0) as u8)
}

fn tinted(c: Color32, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (a.clamp(0.0, 1.0) * 255.0) as u8)
}

/// Keep `p` inside the rounded rect (`rect` with corner radius `r`).
fn clamp_rounded(p: Pos2, rect: Rect, r: f32) -> Pos2 {
    let mut p = rect.clamp(p);
    let cx = p.x.clamp(rect.left() + r, rect.right() - r);
    let cy = p.y.clamp(rect.top() + r, rect.bottom() - r);
    let d = p - Pos2::new(cx, cy);
    if d.length() > r && d.x != 0.0 && d.y != 0.0 {
        p = Pos2::new(cx, cy) + d.normalized() * r;
    }
    p
}

/// A soft radial light (bright centre, fading out at `radius`), kept inside
/// the rounded rect `bounds`.
fn radial(center: Pos2, radius: Vec2, color: Color32, bounds: Rect, r: f32) -> egui::Shape {
    let mut mesh = egui::Mesh::default();
    let at = |p: Pos2| clamp_rounded(p, bounds, r);
    mesh.colored_vertex(at(center), color);
    let mid = color.gamma_multiply(0.35);
    const N: u32 = 40;
    for i in 0..N {
        let a = i as f32 / N as f32 * std::f32::consts::TAU;
        let dir = Vec2::new(a.cos() * radius.x, a.sin() * radius.y);
        mesh.colored_vertex(at(center + dir * 0.45), mid);
        mesh.colored_vertex(at(center + dir), Color32::TRANSPARENT);
    }
    for i in 0..N {
        let j = (i + 1) % N;
        let (mi, oi, mj, oj) = (1 + i * 2, 2 + i * 2, 1 + j * 2, 2 + j * 2);
        mesh.add_triangle(0, mi, mj);
        mesh.add_triangle(mi, oi, oj);
        mesh.add_triangle(mi, oj, mj);
    }
    egui::Shape::mesh(mesh)
}

/// A vertical gradient (`stops` from `y0` to `y1`) clipped to the rounded
/// rect, built from rows that follow the rounded corners.
fn rounded_rows(rect: Rect, r: f32, y0: f32, y1: f32, stops: &[(f32, Color32)]) -> egui::Shape {
    let r = r.min(rect.height() / 2.0).min(rect.width() / 2.0).max(0.0);
    let (y0, y1) = (y0.max(rect.top()), y1.min(rect.bottom()));
    if y1 - y0 < 0.5 || stops.is_empty() {
        return egui::Shape::Noop;
    }
    let mut ys: Vec<f32> = stops.iter().map(|(t, _)| y0 + t * (y1 - y0)).collect();
    ys.extend([y0, y1]);
    for i in 0..=8 {
        let t = i as f32 / 8.0 * std::f32::consts::FRAC_PI_2;
        let a = r - r * t.cos();
        ys.extend([rect.top() + a, rect.bottom() - a]);
    }
    ys.retain(|y| *y >= y0 && *y <= y1);
    ys.sort_by(|a, b| a.total_cmp(b));
    ys.dedup_by(|a, b| (*a - *b).abs() < 0.25);
    let inset = |y: f32| {
        let dy = if y < rect.top() + r {
            rect.top() + r - y
        } else if y > rect.bottom() - r {
            y - (rect.bottom() - r)
        } else {
            0.0
        };
        r - (r * r - dy * dy).max(0.0).sqrt()
    };
    let color = |y: f32| {
        let t = (y - y0) / (y1 - y0);
        let mut prev = stops[0];
        for &(st, c) in stops {
            if t <= st {
                let span = st - prev.0;
                let f = if span > 0.0 { (t - prev.0) / span } else { 1.0 };
                return lerp_color(prev.1, c, f);
            }
            prev = (st, c);
        }
        prev.1
    };
    let mut mesh = egui::Mesh::default();
    for (n, &y) in ys.iter().enumerate() {
        let i = inset(y);
        let c = color(y);
        mesh.colored_vertex(Pos2::new(rect.left() + i, y), c);
        mesh.colored_vertex(Pos2::new(rect.right() - i, y), c);
        if n > 0 {
            let b = (n as u32) * 2;
            mesh.add_triangle(b - 2, b - 1, b);
            mesh.add_triangle(b - 1, b + 1, b);
        }
    }
    egui::Shape::mesh(mesh)
}

/// The Glow style's backdrop: soft coloured light behind the page, which
/// the translucent cards let through.
pub fn glow_backdrop(painter: &egui::Painter, rect: Rect, pal: &Palette, corner: f32) {
    if pal.glass.on() && pal.glass.style == crate::theme::GlassStyle::Planner {
        planner_backdrop(painter, rect, pal, corner);
        return;
    }
    if !pal.glass.on() || pal.glass.style != crate::theme::GlassStyle::Glow {
        return;
    }
    let dark = pal.bg.r() < 128;
    let a = if dark { 0.42 } else { 0.26 } * crate::theme::Glass::scale(pal.glass.style_strength);
    let (w, h) = (rect.width(), rect.height());
    let size = w.max(h);
    let blobs = [
        (Color32::from_rgb(150, 200, 40), 0.12, 0.02, 0.50),
        (Color32::from_rgb(20, 170, 170), 0.42, 0.80, 0.45),
        (Color32::from_rgb(230, 110, 90), 1.00, 0.95, 0.50),
        (pal.accent, 0.85, 0.10, 0.38),
    ];
    let wide = rect.expand(size);
    for (c, x, y, rad) in blobs {
        painter.add(radial(
            rect.min + Vec2::new(w * x, h * y),
            Vec2::new(size * rad, size * rad * 0.8),
            tinted(c, a),
            wide,
            0.0,
        ));
    }
}

/// A Planner glass card like the Daily Routine Planner app: translucent
/// fill with a top-to-bottom white sheen, a white rim, a bright top edge and
/// a soft coloured shadow. Strong colours (accent buttons) stay solid with
/// just the top edge.
fn planner_card(pal: &Palette, rect: Rect, corner: u8, color: Color32) -> egui::Shape {
    let dark = pal.bg.r() < 128;
    let wa = |a: f32| Color32::from_white_alpha((a.clamp(0.0, 1.0) * 255.0) as u8);
    // Glass effect Soft: flat frosted glass, faint rim, almost no shadow (planner_glass.soften)
    let soft = pal.glass.effect != crate::theme::GlassEffect::Strong;
    let ds = crate::theme::Glass::scale(pal.glass.shadow_strength) * if soft { 0.30 } else { 1.0 };
    let r = (corner as f32).min(rect.height() / 2.0);
    let colourful = color.r().abs_diff(color.b()) > 60 || color.g().abs_diff(color.b()) > 60;
    let flat_control = rect.height() < 44.0; // buttons, chips, inputs: no shadow
    let mut shapes = Vec::new();
    if color.a() == 0 {
        return egui::Shape::Noop;
    }
    if pal.glass.shadow && ds > 0.0 && !flat_control {
        let (cr, cg, cb) = if dark { (0, 0, 0) } else { (76, 61, 180) };
        for (grow, dy, a) in [(12.0_f32, 8.0_f32, 0.035), (6.0, 4.0, 0.05), (1.5, 1.5, 0.06)] {
            let a = a * ds * if dark { 2.6 } else { 1.0 };
            shapes.push(egui::Shape::rect_filled(
                rect.translate(Vec2::new(0.0, dy)).expand(grow),
                corner.saturating_add(grow as u8),
                Color32::from_rgba_unmultiplied(cr, cg, cb, (a.min(1.0) * 255.0) as u8),
            ));
        }
    }
    shapes.push(egui::Shape::rect_filled(rect, corner, color));
    if !colourful {
        let (t, b) = match (dark, soft) {
            (true, false) => (0.10, 0.03),
            (true, true) => (0.03, 0.03),
            (false, false) => (0.62, 0.30),
            (false, true) => (0.42, 0.42),
        };
        shapes.push(rounded_rows(rect, r, rect.top(), rect.bottom(), &[(0.0, wa(t)), (1.0, wa(b))]));
        shapes.push(egui::Shape::rect_stroke(
            rect.shrink(0.5),
            corner,
            egui::Stroke::new(1.0, wa(match (dark, soft) {
                (true, false) => 0.13,
                (true, true) => 0.06,
                (false, false) => 0.85,
                (false, true) => 0.40,
            })),
            egui::StrokeKind::Inside,
        ));
    }
    // bright top edge (the "glass" highlight)
    let inset = r.max(4.0);
    if rect.width() > inset * 2.0 + 4.0 {
        shapes.push(egui::Shape::line_segment(
            [egui::pos2(rect.left() + inset, rect.top() + 1.0), egui::pos2(rect.right() - inset, rect.top() + 1.0)],
            egui::Stroke::new(1.0, wa(match (colourful, dark, soft) {
                (true, _, _) => 0.30,
                (false, true, false) => 0.18,
                (false, true, true) => 0.0,
                (false, false, false) => 0.95,
                (false, false, true) => 0.0,
            })),
        ));
    }
    egui::Shape::Vec(shapes)
}

/// The Planner style's backdrop: the planner gradient (light lavender to sky,
/// dark navy) behind the page. `corner` keeps it inside a rounded frame.
/// In a see-through window it thins with the opacity so Fusion Desk's blur
/// shows behind it.
pub fn planner_backdrop(painter: &egui::Painter, rect: Rect, pal: &Palette, corner: f32) {
    let dark = pal.bg.r() < 128;
    let a = if crate::system::glass_window() {
        (pal.glass.opacity as f32 / 100.0).max(0.35)
    } else {
        1.0
    };
    let c = |r: u8, g: u8, b: u8| Color32::from_rgba_unmultiplied(r, g, b, (a * 255.0) as u8);
    let accent = [pal.accent.r(), pal.accent.g(), pal.accent.b()];
    let stops: Vec<(f32, Color32)> = if let Some(cols) = pal.glass.lumina_rgb(accent) {
        // Lumina Colourful: only a faint tint of the colours (same as planner_glass.py)
        let k = pal.glass.lumina_intensity as f32 / 100.0;
        let (base, amt) = if dark {
            ([20.0, 16.0, 40.0], [0.09, 0.08, 0.07])
        } else {
            ([250.0, 248.0, 255.0], [0.06, 0.045, 0.055])
        };
        let mix = |col: [u8; 3], t: f32| {
            let m = |i: usize| (base[i] + (col[i] as f32 - base[i]) * t * k).round() as u8;
            c(m(0), m(1), m(2))
        };
        vec![(0.0, mix(cols[0], amt[0])), (0.5, mix(cols[1], amt[1])), (1.0, mix(cols[2], amt[2]))]
    } else if dark {
        vec![(0.0, c(26, 21, 51)), (0.5, c(27, 31, 69)), (1.0, c(20, 26, 58))]
    } else {
        vec![(0.0, c(232, 218, 255)), (0.5, c(220, 224, 255)), (1.0, c(203, 220, 255))]
    };
    painter.add(rounded_rows(rect, corner, rect.top(), rect.bottom(), &stops));
}

/// True when a glass style is on.
pub fn glassy(pal: &Palette) -> bool {
    pal.glass.on()
}

/// A neutral glass slab for an option (nav items, ghost buttons, tiles).
pub fn glass_slab(pal: &Palette, rect: Rect, corner: u8, hovered: bool) -> egui::Shape {
    let dark = pal.bg.r() < 128;
    let base = match (dark, hovered) {
        (true, false) => Color32::from_white_alpha(10),
        (true, true) => Color32::from_white_alpha(22),
        (false, false) => Color32::from_white_alpha(120),
        (false, true) => Color32::from_white_alpha(190),
    };
    glass_fill(pal, rect, corner, base)
}

pub fn divider(ui: &mut Ui, pal: &Palette) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    let inset = Rect::from_min_max(
        Pos2::new(rect.left() + ROW_PAD, rect.top()),
        Pos2::new(rect.right() - ROW_PAD, rect.bottom()),
    );
    ui.painter().rect_filled(inset, 0, pal.divider);
}

/// The leading glyph of a row. Windows 11 paints it on an accent tile;
/// Windows 10 uses a flat, accent-coloured line glyph with no tile.
/// Seed for `Palette::tint`, so one icon keeps one colour everywhere.
fn icon_seed(icon: Icon) -> String {
    format!("icon{}", icon as u16)
}

/// An icon drawn without a tile: the accent, or its own colour when the look
/// is Colourful.
pub fn icon_ink(pal: &Palette, icon: Icon) -> Color32 {
    pal.tint(pal.accent, &icon_seed(icon))
}

pub fn icon_tile(
    painter: &egui::Painter,
    rect: Rect,
    icon: Icon,
    fill: Color32,
    fg: Color32,
    pal: &Palette,
) {
    let fill = pal.tint(fill, &icon_seed(icon));
    if pal.design == crate::theme::Design::Win10 {
        let glyph = Rect::from_center_size(rect.center(), Vec2::splat(rect.width() * 0.78));
        icons::paint(painter, icon, glyph, icon_ink(pal, icon));
        return;
    }
    painter.rect_filled(rect, 6, fill);
    let glyph = Rect::from_center_size(rect.center(), Vec2::splat(rect.width() * 0.62));
    icons::paint_square(painter, icon, glyph, fg);
}

pub enum Right<'a> {
    Chevron,
    Switch(&'a mut bool),
    AccentText(&'a str),
    Check,
    None,
}

pub struct RowOutcome {
    pub clicked: bool,
    pub toggled: bool,
}

impl RowOutcome {
    pub fn clicked(&self) -> bool {
        self.clicked
    }

    pub fn toggled(&self) -> bool {
        self.toggled
    }
}

pub fn row(
    ui: &mut Ui,
    pal: &Palette,
    accent: Color32,
    icon: Icon,
    title: &str,
    subtitle: &str,
    right: Right<'_>,
) -> RowOutcome {
    row_impl(ui, pal, accent, Some(icon), None, title, subtitle, right, None)
}

/// A row whose leading tile is the application's real icon when one is
/// available, with the painted vector glyph as fallback.
#[allow(dead_code)] // kept for pages that want an app row with a chevron
pub fn app_row(
    ui: &mut Ui,
    pal: &Palette,
    texture: Option<&egui::TextureHandle>,
    fallback: Icon,
    title: &str,
    subtitle: &str,
    right: Right<'_>,
) -> RowOutcome {
    row_impl(
        ui,
        pal,
        pal.accent,
        Some(fallback),
        texture,
        title,
        subtitle,
        right,
        None,
    )
}

pub fn info_row(
    ui: &mut Ui,
    pal: &Palette,
    title: &str,
    subtitle: &str,
    right: Right<'_>,
) -> RowOutcome {
    row_impl(ui, pal, pal.accent, None, None, title, subtitle, right, None)
}

/// Risk level of a setting: green safe, orange warning, red critical.
pub fn risk_color(level: &str) -> Color32 {
    match level {
        "critical" => Color32::from_rgb(0xD1, 0x34, 0x38),
        "warning" => Color32::from_rgb(0xE0, 0x86, 0x00),
        _ => Color32::from_rgb(0x2E, 0x9E, 0x4F),
    }
}

pub fn risk_label(level: &str) -> &'static str {
    match level {
        "critical" => "Critical",
        "warning" => "Warning",
        _ => "Safe",
    }
}

/// A row with a coloured risk pill after the title and, for warning and
/// critical, a third line in that colour saying what can go wrong.
pub fn risk_row(
    ui: &mut Ui,
    pal: &Palette,
    icon: Option<Icon>,
    title: &str,
    subtitle: &str,
    level: &str,
    why: &str,
    right: Right<'_>,
) -> RowOutcome {
    row_impl(
        ui,
        pal,
        pal.accent,
        icon,
        None,
        title,
        subtitle,
        right,
        Some((level, why)),
    )
}

/// A small coloured dot + word, e.g. for a legend.
pub fn risk_chip(ui: &mut Ui, level: &str, text: &str) {
    let color = risk_color(level);
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
        ui.painter().circle_filled(r.center(), 5.0, color);
        ui.label(egui::RichText::new(text).font(regular(12.0)).color(color));
    });
}

fn row_impl(
    ui: &mut Ui,
    pal: &Palette,
    accent: Color32,
    icon: Option<Icon>,
    texture: Option<&egui::TextureHandle>,
    title: &str,
    subtitle: &str,
    mut right: Right<'_>,
    risk: Option<(&str, &str)>,
) -> RowOutcome {
    // Colourful look: each row's icon and switch take a colour of their own.
    let accent = pal.tint(accent, title);
    let width = ui.available_width();
    let row_h = if pal.design == crate::theme::Design::Win10 {
        ROW_H_10
    } else {
        ROW_H
    };
    let note = risk.filter(|r| !r.1.is_empty());
    let row_h = row_h + if note.is_some() { 18.0 } else { 0.0 };
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, row_h), Sense::click());
    let hovered = response.hovered();
    let is_switch = matches!(&right, Right::Switch(_));
    let is_plain = matches!(&right, Right::None);
    let interactive = !is_plain || icon.is_some();

    if hovered && interactive {
        let bg = Rect::from_min_max(
            Pos2::new(rect.left() + 4.0, rect.top() + 2.0),
            Pos2::new(rect.right() - 4.0, rect.bottom() - 2.0),
        );
        ui.painter().rect_filled(bg, pal.corner(4), pal.hover);
    }

    let mut toggled = false;
    let text_x = if let Some(texture) = texture {
        let chip = Rect::from_center_size(
            Pos2::new(rect.left() + ROW_PAD + CHIP / 2.0, rect.center().y),
            Vec2::splat(CHIP),
        );
        egui::Image::new((texture.id(), chip.size()))
            .corner_radius(pal.corner(5))
            .paint_at(ui, chip);
        chip.right() + 14.0
    } else if let Some(icon) = icon {
        let chip = Rect::from_center_size(
            Pos2::new(rect.left() + ROW_PAD + CHIP / 2.0, rect.center().y),
            Vec2::splat(CHIP),
        );
        icon_tile(ui.painter(), chip, icon, accent, pal.on_accent, pal);
        chip.right() + 14.0
    } else {
        rect.left() + ROW_PAD + 2.0
    };

    // Keep the labels clear of whatever sits on the right of the row.
    let win10 = pal.design == crate::theme::Design::Win10;
    let reserve = match &right {
        Right::Chevron | Right::Check => 44.0,
        Right::Switch(_) => {
            if win10 {
                108.0
            } else {
                76.0
            }
        }
        Right::AccentText(text) => 30.0 + text.chars().count() as f32 * 7.2,
        Right::None => ROW_PAD,
    };
    let clip = Rect::from_min_max(
        Pos2::new(rect.left(), rect.top()),
        Pos2::new((rect.right() - reserve).max(text_x + 40.0), rect.bottom()),
    );
    let text_painter = ui.painter().with_clip_rect(clip.intersect(ui.clip_rect()));
    let has_subtitle = !subtitle.is_empty();
    // With a risk note the title and subtitle move up one line.
    let mid = rect.center().y - if note.is_some() { 9.0 } else { 0.0 };
    let title_y = if has_subtitle { mid - 8.5 } else { mid };
    let title_font = regular(14.0);
    let title_color = pal.text;
    let title_rect = text_painter.text(
        Pos2::new(text_x, title_y),
        Align2::LEFT_CENTER,
        title,
        title_font,
        title_color,
    );
    if let Some((level, why)) = risk {
        let color = risk_color(level);
        let word = risk_label(level);
        let galley = ui.painter().layout_no_wrap(word.to_owned(), regular(10.5), color);
        let pill = Rect::from_min_size(
            Pos2::new(title_rect.right() + 8.0, title_y - 8.0),
            Vec2::new(galley.size().x + 16.0, 16.0),
        );
        let fill = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 38);
        text_painter.rect_filled(pill, 8, fill);
        text_painter.circle_filled(Pos2::new(pill.left() + 7.0, pill.center().y), 3.0, color);
        text_painter.galley(
            Pos2::new(pill.left() + 12.0, pill.center().y - galley.size().y / 2.0),
            galley,
            color,
        );
        if let Some((_, _)) = note {
            text_painter.text(
                Pos2::new(text_x, mid + 28.5),
                Align2::LEFT_CENTER,
                format!(
                    "{} {why}",
                    match level {
                        "critical" => "✖",
                        "warning" => "⚠",
                        _ => "✔",
                    }
                ),
                regular(12.0),
                color,
            );
        }
    }
    if has_subtitle {
        text_painter.text(
            Pos2::new(text_x, mid + 10.5),
            Align2::LEFT_CENTER,
            subtitle,
            regular(12.0),
            pal.text_faint,
        );
    }

    let accent_text = matches!(&right, Right::AccentText(_));
    let switch_rect = match &mut right {
        Right::Chevron => {
            let chevron = Rect::from_center_size(
                Pos2::new(rect.right() - ROW_PAD - 7.0, rect.center().y),
                Vec2::splat(14.0),
            );
            icons::paint(ui.painter(), Icon::ChevronRight, chevron, pal.text_faint);
            None
        }
        Right::Switch(state) => {
            let r = Rect::from_center_size(
                Pos2::new(rect.right() - ROW_PAD - 20.0, rect.center().y),
                Vec2::new(40.0, 20.0),
            );
            let id = response.id.with("switch");
            let t = ui.ctx().animate_bool_with_time(id, **state, 0.11);
            paint_switch(ui.painter(), r, t, pal, accent);
            if win10 {
                // Windows 10 names the state next to every switch.
                ui.painter().text(
                    Pos2::new(r.left() - 12.0, rect.center().y),
                    Align2::RIGHT_CENTER,
                    if **state { "On" } else { "Off" },
                    regular(13.0),
                    pal.text_muted,
                );
            }
            Some(r)
        }
        Right::AccentText(text) => {
            let _ = accent_text;
            let color = accent;
            ui.painter().text(
                Pos2::new(rect.right() - ROW_PAD - 2.0, rect.center().y),
                Align2::RIGHT_CENTER,
                *text,
                regular(12.5),
                color,
            );
            None
        }
        Right::Check => {
            let check = Rect::from_center_size(
                Pos2::new(rect.right() - ROW_PAD - 8.0, rect.center().y),
                Vec2::splat(16.0),
            );
            icons::paint(ui.painter(), Icon::Check, check, accent);
            None
        }
        Right::None => None,
    };

    let _ = switch_rect;

    let clicked = response.clicked();
    let response = response.on_hover_cursor(if interactive {
        CursorIcon::PointingHand
    } else {
        CursorIcon::Default
    });

    if clicked {
        if let Right::Switch(state) = &mut right {
            **state = !**state;
            toggled = true;
        }
    }

    // Switch rows should not report navigation clicks.
    let clicked = clicked && !is_switch;

    let _ = response;
    let _ = hovered;
    RowOutcome { clicked, toggled }
}

pub fn paint_switch(painter: &egui::Painter, rect: Rect, t: f32, pal: &Palette, accent: Color32) {
    let win10 = pal.design == crate::theme::Design::Win10;
    if win10 {
        // Windows 10: a thin outlined track when off and a solid accent track
        // when on, with a white knob that always has a border.
        let radius = rect.height() / 2.0;
        painter.rect_filled(rect, radius, lerp_color(pal.card, accent, t));
        painter.rect_stroke(
            rect,
            radius,
            Stroke::new(1.0, lerp_color(pal.text_faint, accent, t)),
            StrokeKind::Inside,
        );
        let knob_x = egui::lerp((rect.left() + 10.0)..=(rect.right() - 10.0), t);
        painter.circle_filled(Pos2::new(knob_x, rect.center().y), 6.0, Color32::WHITE);
        painter.circle_stroke(
            Pos2::new(knob_x, rect.center().y),
            6.0,
            Stroke::new(1.0, pal.text_faint),
        );
        return;
    }

    let track = lerp_color(Color32::from_rgba_unmultiplied(0, 0, 0, 0), accent, t);
    painter.rect_filled(rect, rect.height() / 2.0, track);
    if t < 0.98 {
        painter.rect_stroke(
            rect,
            rect.height() / 2.0,
            Stroke::new(1.0, pal.toggle_knob_off.gamma_multiply(0.65)),
            StrokeKind::Inside,
        );
    }
    let knob_x = egui::lerp((rect.left() + 10.0)..=(rect.right() - 10.0), t);
    let knob_color = lerp_color(pal.toggle_knob_off, Color32::WHITE, t);
    painter.circle_filled(Pos2::new(knob_x, rect.center().y), 6.5, knob_color);
}

/// The Windows 10 home header: the title bar and the account band share
/// one colour that fades a little toward the accent at the bottom.
pub fn home10_header(pal: &Palette) -> (Color32, Color32) {
    let dark = pal.bg.r() < 128;
    let top = pal.nav_bg;
    (
        top,
        lerp_color(top, pal.accent, if dark { 0.16 } else { 0.10 }),
    )
}

/// Fill `rect` with a vertical gradient from `top` to `bottom`.
pub fn vgradient(painter: &egui::Painter, rect: Rect, top: Color32, bottom: Color32) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), top);
    mesh.colored_vertex(rect.left_bottom(), bottom);
    mesh.colored_vertex(rect.right_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 3, 2);
    painter.add(egui::Shape::mesh(mesh));
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    Color32::from_rgba_premultiplied(
        egui::lerp(a.r() as f32..=b.r() as f32, t) as u8,
        egui::lerp(a.g() as f32..=b.g() as f32, t) as u8,
        egui::lerp(a.b() as f32..=b.b() as f32, t) as u8,
        egui::lerp(a.a() as f32..=b.a() as f32, t) as u8,
    )
}

pub fn nav_item(
    ui: &mut Ui,
    pal: &Palette,
    accent: Color32,
    icon: Icon,
    label: &str,
    selected: bool,
) -> Response {
    let width = ui.available_width();
    let nav_h = if pal.design == crate::theme::Design::Win10 {
        40.0
    } else {
        38.0
    };
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, nav_h), Sense::click());

    let win10 = pal.design == crate::theme::Design::Win10;
    if win10 {
        // Windows 10: the selection fills the whole pane, with a 3 px accent
        // bar at the pane edge; hover is a flat fill too.
        if selected && glassy(pal) {
            ui.painter()
                .add(glass_slab(pal, rect.shrink2(Vec2::new(6.0, 2.0)), 4, true));
            let bar = Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height()));
            ui.painter().rect_filled(bar, 0, accent);
        } else if selected {
            ui.painter().rect_filled(rect, 0, pal.nav_selected);
            let bar = Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height()));
            ui.painter().rect_filled(bar, 0, accent);
        } else if response.hovered() {
            ui.painter().rect_filled(rect, 0, pal.hover);
        }
        let chip = Rect::from_center_size(
            Pos2::new(rect.left() + 24.0, rect.center().y),
            Vec2::splat(22.0),
        );
        icons::paint(ui.painter(), icon, chip, pal.text);
        ui.painter().text(
            Pos2::new(rect.left() + 44.0, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            regular(14.0),
            pal.text,
        );
        return response.on_hover_cursor(CursorIcon::PointingHand);
    }

    if selected && glassy(pal) {
        ui.painter()
            .add(glass_slab(pal, rect.shrink2(Vec2::new(0.0, 1.0)), 6, true));
        let pill = Rect::from_center_size(
            Pos2::new(rect.left() + 4.0, rect.center().y),
            Vec2::new(3.0, 16.0),
        );
        ui.painter().rect_filled(pill, 2, accent);
    } else if selected {
        ui.painter()
            .rect_filled(rect.shrink2(Vec2::new(0.0, 1.0)), 6, pal.nav_selected);
        let pill = Rect::from_center_size(
            Pos2::new(rect.left() + 4.0, rect.center().y),
            Vec2::new(3.0, 16.0),
        );
        ui.painter().rect_filled(pill, 2, accent);
    } else if response.hovered() {
        ui.painter()
            .rect_filled(rect.shrink2(Vec2::new(0.0, 1.0)), 6, pal.hover);
    }

    let chip = Rect::from_center_size(
        Pos2::new(rect.left() + 23.0, rect.center().y),
        Vec2::splat(24.0),
    );
    let accent = pal.tint(accent, &icon_seed(icon));
    let tile_color = if selected {
        accent
    } else {
        accent.gamma_multiply(0.92)
    };
    icon_tile(ui.painter(), chip, icon, tile_color, pal.on_accent, pal);

    ui.painter().text(
        Pos2::new(rect.left() + 45.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        regular(14.0),
        if selected { pal.text } else { pal.text_muted },
    );

    response.on_hover_cursor(CursorIcon::PointingHand)
}

pub fn search_box(
    ui: &mut Ui,
    pal: &Palette,
    query: &mut String,
    focus: bool,
    hint: &str,
) -> (Response, bool) {
    let width = ui.available_width();
    let glass_at = ui.painter().add(egui::Shape::Noop);
    let frame = egui::Frame::new()
        .fill(if glassy(pal) {
            Color32::TRANSPARENT
        } else {
            pal.input_bg
        })
        .stroke(Stroke::new(1.0, pal.input_border))
        .corner_radius(pal.corner(6))
        .inner_margin(Margin::symmetric(10, 6));
    let inner = frame.show(ui, |ui| {
        ui.set_width(width - 22.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let (icon_rect, _) = ui.allocate_exact_size(Vec2::splat(15.0), Sense::hover());
            icons::paint(ui.painter(), Icon::Search, icon_rect, pal.text_faint);
            let response = ui.add(
                egui::TextEdit::singleline(query)
                    .frame(egui::Frame::NONE)
                    .hint_text(
                        egui::RichText::new(hint)
                            .font(regular(13.0))
                            .color(pal.text_faint),
                    )
                    .font(regular(13.0))
                    .desired_width(ui.available_width())
                    .vertical_align(egui::Align::Center),
            );
            if focus {
                response.request_focus();
            }
            response
        })
        .inner
    });
    if glassy(pal) {
        let rect = inner.response.rect;
        ui.painter().set(
            glass_at,
            glass_slab(pal, rect, pal.corner(6), inner.inner.has_focus()),
        );
    }
    let response = inner.inner;
    let changed = response.changed();
    (response, changed)
}

pub fn button(
    ui: &mut Ui,
    pal: &Palette,
    accent: Color32,
    label: &str,
    icon: Option<Icon>,
) -> Response {
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), regular(14.0), pal.on_accent);
    let extra = if icon.is_some() { 30.0 } else { 0.0 };
    let size = Vec2::new(galley.size().x + 28.0 + extra, 34.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let fill = if response.is_pointer_button_down_on() {
        crate::theme::darken(accent, 18)
    } else if response.hovered() {
        crate::theme::lighten(accent, 12)
    } else {
        accent
    };
    ui.painter().add(glass_fill(pal, rect, pal.corner(5), fill));
    let mut x = rect.center().x - galley.size().x / 2.0 + extra / 2.0;
    if let Some(icon) = icon {
        let icon_rect = Rect::from_center_size(
            Pos2::new(rect.left() + 16.0, rect.center().y),
            Vec2::splat(15.0),
        );
        icons::paint(ui.painter(), icon, icon_rect, pal.on_accent);
        x = icon_rect.right() + 8.0;
    }
    ui.painter().galley(
        Pos2::new(x, rect.center().y - galley.size().y / 2.0),
        galley,
        pal.on_accent,
    );
    response.on_hover_cursor(CursorIcon::PointingHand)
}

pub fn ghost_button(ui: &mut Ui, pal: &Palette, label: &str, icon: Option<Icon>) -> Response {
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), regular(14.0), pal.text);
    let extra = if icon.is_some() { 30.0 } else { 0.0 };
    let size = Vec2::new(galley.size().x + 26.0 + extra, 34.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if glassy(pal) {
        ui.painter()
            .add(glass_slab(pal, rect, pal.corner(5), response.hovered()));
    } else {
        let fill = if response.hovered() {
            pal.hover
        } else {
            Color32::TRANSPARENT
        };
        ui.painter().rect_filled(rect, pal.corner(5), fill);
        ui.painter().rect_stroke(
            rect,
            5,
            Stroke::new(1.0, pal.card_border),
            StrokeKind::Inside,
        );
    }
    let mut x = rect.center().x - galley.size().x / 2.0 + extra / 2.0;
    if let Some(icon) = icon {
        let icon_rect = Rect::from_center_size(
            Pos2::new(rect.left() + 15.0, rect.center().y),
            Vec2::splat(15.0),
        );
        icons::paint(ui.painter(), icon, icon_rect, pal.text);
        x = icon_rect.right() + 8.0;
    }
    ui.painter().galley(
        Pos2::new(x, rect.center().y - galley.size().y / 2.0),
        galley,
        pal.text,
    );
    response.on_hover_cursor(CursorIcon::PointingHand)
}

pub fn slider(ui: &mut Ui, pal: &Palette, accent: Color32, value: &mut f32, id: Id) -> bool {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 30.0), Sense::click_and_drag());
    let track = Rect::from_center_size(rect.center(), Vec2::new(width, 4.0));
    ui.painter().rect_filled(track, 2, pal.track);

    let mut changed = false;
    if let Some(pos) = response.interact_pointer_pos() {
        if response.clicked() || response.dragged() {
            let t = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
            if (t - *value).abs() > f32::EPSILON {
                *value = t;
                changed = true;
            }
            ui.ctx().request_repaint();
        }
    }

    let t = value.clamp(0.0, 1.0);
    let knob = Pos2::new(rect.left() + rect.width() * t, rect.center().y);
    let filled = Rect::from_min_max(track.min, Pos2::new(knob.x, track.max.y));
    ui.painter().rect_filled(filled, pal.corner(2), accent);
    ui.painter().circle_filled(knob, 9.0, accent);
    ui.painter().circle_filled(knob, 4.0, pal.card);
    let _ = id;
    response.on_hover_cursor(CursorIcon::PointingHand);
    changed
}

/// The account picture's texture, published by the app every frame.
pub fn user_photo(ctx: &egui::Context) -> Option<egui::TextureId> {
    ctx.data(|d| d.get_temp::<egui::TextureId>(Id::new("user-photo")))
}

/// Draw a texture as a filled circle (the account picture).
pub fn paint_photo(painter: &egui::Painter, tex: egui::TextureId, center: Pos2, r: f32) {
    let n = 72u32;
    let mut mesh = egui::Mesh::with_texture(tex);
    mesh.vertices.push(egui::epaint::Vertex {
        pos: center,
        uv: Pos2::new(0.5, 0.5),
        color: Color32::WHITE,
    });
    for i in 0..=n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        let (sin, cos) = a.sin_cos();
        mesh.vertices.push(egui::epaint::Vertex {
            pos: center + Vec2::new(cos, sin) * r,
            uv: Pos2::new(0.5 + cos * 0.5, 0.5 + sin * 0.5),
            color: Color32::WHITE,
        });
    }
    for i in 1..=n {
        mesh.add_triangle(0, i, i + 1);
    }
    painter.add(egui::Shape::mesh(mesh));
}

pub fn avatar(ui: &mut Ui, pal: &Palette, name: &str, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    if let Some(tex) = user_photo(ui.ctx()) {
        paint_photo(ui.painter(), tex, rect.center(), size / 2.0);
        return;
    }
    let dark = pal.bg.r() < 128;
    let fill = if dark {
        Color32::from_rgb(0x54, 0x54, 0x54)
    } else {
        Color32::from_rgb(0xD9, 0xD9, 0xD9)
    };
    ui.painter().circle_filled(rect.center(), size / 2.0, fill);
    let letter = name
        .chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "?".to_owned());
    let color = if dark {
        Color32::WHITE
    } else {
        Color32::from_rgb(0x20, 0x20, 0x20)
    };
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        letter,
        semibold(size * 0.42),
        color,
    );
}

/// Small "screen in a laptop" illustration used on the System page.
pub fn device_art(painter: &egui::Painter, rect: Rect, pal: &Palette, accent: Color32) {
    let screen = Rect::from_min_max(
        Pos2::new(rect.left() + 5.0, rect.top()),
        Pos2::new(rect.right() - 5.0, rect.bottom() - 10.0),
    );
    painter.rect_filled(screen, 4, pal.card_hover);
    painter.rect_stroke(
        screen,
        4,
        Stroke::new(1.3, pal.text_faint),
        StrokeKind::Inside,
    );
    let inside = screen.shrink(4.5);
    painter.rect_filled(inside, 2, Color32::from_rgb(0x24, 0x24, 0x24));

    // A tiny desktop: window, accent title bar and taskbar.
    let win = Rect::from_min_size(
        inside.min + Vec2::new(5.0, 7.0),
        Vec2::new(inside.width() * 0.62, inside.height() * 0.52),
    );
    painter.rect_filled(win, 1.5, Color32::from_rgb(0x39, 0x39, 0x39));
    painter.rect_filled(
        Rect::from_min_max(win.min, Pos2::new(win.right(), win.top() + 4.0)),
        1,
        accent,
    );
    painter.rect_filled(
        Rect::from_min_max(
            win.min + Vec2::new(3.0, 8.0),
            Pos2::new(win.right() - 4.0, win.top() + 9.5),
        ),
        1,
        accent.gamma_multiply(0.55),
    );
    painter.rect_filled(
        Rect::from_min_max(
            win.min + Vec2::new(3.0, 12.0),
            Pos2::new(win.right() - 10.0, win.top() + 13.5),
        ),
        1,
        pal.text_faint.gamma_multiply(0.6),
    );

    let taskbar = Rect::from_min_max(Pos2::new(inside.left(), inside.bottom() - 6.0), inside.max);
    painter.rect_filled(taskbar, 0, accent.gamma_multiply(0.9));
    painter.circle_filled(
        Pos2::new(inside.center().x, taskbar.center().y),
        1.4,
        Color32::WHITE.gamma_multiply(0.9),
    );

    let base = Rect::from_center_size(
        Pos2::new(rect.center().x, rect.bottom() - 4.0),
        Vec2::new(rect.width() * 0.58, 3.0),
    );
    painter.rect_filled(base, 1.5, pal.text_faint);
}

/// `corner`: the window's corner radius; Close rounds its top-right corner
/// to match, so the red fill stays inside the rounded window.
pub fn caption_button(
    ui: &mut Ui,
    pal: &Palette,
    kind: Caption,
    accent: Color32,
    corner: u8,
) -> Response {
    // Full title-bar height: the hit area reaches the window edge.
    let height = ui.available_height().max(32.0);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(46.0, height), Sense::click());
    let close_hover = matches!(kind, Caption::Close) && response.hovered();
    if close_hover {
        // Rounded like the window's top-right corner (square when maximized,
        // so the very corner of the screen stays red).
        let red = if pal.design == crate::theme::Design::Win10 {
            Color32::from_rgb(0xE8, 0x11, 0x23)
        } else {
            Color32::from_rgb(0xC4, 0x2B, 0x1C)
        };
        let round = egui::CornerRadius {
            ne: corner,
            ..Default::default()
        };
        ui.painter().rect_filled(rect, round, red);
    } else if response.hovered() {
        ui.painter().rect_filled(rect, 0, pal.hover);
    }
    let color = if close_hover {
        Color32::WHITE
    } else {
        pal.text_muted
    };
    let c = rect.center();
    let s = 5.0;
    let stroke = Stroke::new(1.0, color);
    match kind {
        Caption::Minimize => {
            ui.painter().line_segment(
                [Pos2::new(c.x - s, c.y + 0.5), Pos2::new(c.x + s, c.y + 0.5)],
                stroke,
            );
        }
        Caption::Maximize => {
            let r = Rect::from_center_size(c, Vec2::splat(9.0));
            ui.painter().rect_stroke(r, 1, stroke, StrokeKind::Inside);
        }
        Caption::Restore => {
            let r = Rect::from_center_size(c + Vec2::new(1.0, 1.0), Vec2::splat(8.0));
            ui.painter().rect_stroke(r, 1, stroke, StrokeKind::Inside);
            let r2 =
                Rect::from_min_size(Pos2::new(r.left() - 2.0, r.top() - 2.0), Vec2::splat(8.0));
            ui.painter()
                .line_segment([r2.left_top(), r2.right_top()], stroke);
            ui.painter()
                .line_segment([r2.left_top(), r2.left_bottom()], stroke);
        }
        Caption::Close => {
            ui.painter().line_segment(
                [Pos2::new(c.x - s, c.y - s), Pos2::new(c.x + s, c.y + s)],
                stroke,
            );
            ui.painter().line_segment(
                [Pos2::new(c.x - s, c.y + s), Pos2::new(c.x + s, c.y - s)],
                stroke,
            );
        }
    }
    let _ = accent;
    response
}

#[derive(Clone, Copy)]
pub enum Caption {
    Minimize,
    Maximize,
    Restore,
    Close,
}

pub fn empty_note(ui: &mut Ui, pal: &Palette, text: &str) {
    let width = ui.available_width();
    ui.add_space(18.0);
    ui.allocate_ui_with_layout(
        Vec2::new(width, 24.0),
        egui::Layout::top_down(egui::Align::Center),
        |ui| {
            ui.label(
                egui::RichText::new(text)
                    .font(regular(13.0))
                    .color(pal.text_faint),
            );
        },
    );
    ui.add_space(18.0);
}
