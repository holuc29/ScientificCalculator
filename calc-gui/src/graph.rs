//! The "Graph" tab: plots of y = f(x) for several functions.
//!
//! The first half of this file is pure, testable math (view transform, tick
//! placement, curve-segment building); the second half draws with egui.

use calc_core::{Calculator, PlotFn};
use eframe::egui::{
    self, Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Shape, Stroke, Vec2,
};

pub const PALETTE: [Color32; 6] = [
    Color32::from_rgb(86, 180, 233),
    Color32::from_rgb(230, 159, 0),
    Color32::from_rgb(0, 158, 115),
    Color32::from_rgb(240, 228, 66),
    Color32::from_rgb(204, 121, 167),
    Color32::from_rgb(213, 94, 0),
];

pub const MAX_CURVES: usize = 8;

/// Units per pixel of the default view (25 pixels per unit).
const DEFAULT_SCALE: f64 = 0.04;
const MIN_SCALE: f64 = 1e-12;
const MAX_SCALE: f64 = 1e12;
/// Samples per horizontal pixel.
const SAMPLES_PER_PIXEL: f32 = 3.0;

// ------------------------------------------------------------------ pure math

/// Which part of the plane is visible: a centre and the size of one pixel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub cx: f64,
    pub cy: f64,
    /// x units per pixel
    pub ux: f64,
    /// y units per pixel
    pub uy: f64,
}

impl Default for View {
    fn default() -> Self {
        View {
            cx: 0.0,
            cy: 0.0,
            ux: DEFAULT_SCALE,
            uy: DEFAULT_SCALE,
        }
    }
}

impl View {
    /// Plane x of a pixel column, for a canvas of `width` pixels.
    pub fn x_at(&self, px: f64, width: f64) -> f64 {
        self.cx + (px - width / 2.0) * self.ux
    }

    pub fn y_at(&self, py: f64, height: f64) -> f64 {
        self.cy - (py - height / 2.0) * self.uy
    }

    pub fn px_of(&self, x: f64, width: f64) -> f64 {
        (x - self.cx) / self.ux + width / 2.0
    }

    pub fn py_of(&self, y: f64, height: f64) -> f64 {
        height / 2.0 - (y - self.cy) / self.uy
    }

    /// Moves the picture by a pixel drag.
    pub fn pan(&mut self, dx_px: f64, dy_px: f64) {
        self.cx -= dx_px * self.ux;
        self.cy += dy_px * self.uy;
    }

    /// Scales the view by `factor` (> 1 zooms in) keeping the plane point
    /// under pixel (`px`, `py`) fixed.
    pub fn zoom_at(&mut self, px: f64, py: f64, width: f64, height: f64, factor: f64) {
        if !(factor.is_finite() && factor > 0.0) {
            return;
        }
        let (x, y) = (self.x_at(px, width), self.y_at(py, height));
        self.ux = (self.ux / factor).clamp(MIN_SCALE, MAX_SCALE);
        self.uy = (self.uy / factor).clamp(MIN_SCALE, MAX_SCALE);
        self.cx = x - (px - width / 2.0) * self.ux;
        self.cy = y + (py - height / 2.0) * self.uy;
    }

    /// Chooses the vertical range so that the bulk of `values` is visible,
    /// ignoring the most extreme 2 % on each side (poles, spikes).
    pub fn fit_y(&mut self, values: &mut Vec<f64>, height: f64) {
        values.retain(|v| v.is_finite());
        if values.is_empty() || height < 1.0 {
            return;
        }
        values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let n = values.len();
        let lo = values[(n as f64 * 0.02) as usize];
        let hi = values[((n as f64 * 0.98).ceil() as usize).clamp(1, n) - 1];
        let (lo, hi) = if hi - lo < 1e-9 * hi.abs().max(lo.abs()).max(1e-9) {
            (lo - 1.0, hi + 1.0)
        } else {
            (lo, hi)
        };
        let pad = (hi - lo) * 0.1;
        self.cy = (lo + hi) / 2.0;
        self.uy = ((hi - lo + 2.0 * pad) / height).clamp(MIN_SCALE, MAX_SCALE);
    }
}

/// A "nice" step (1, 2 or 5 times a power of ten) of at least `raw`.
pub fn nice_step(raw: f64) -> f64 {
    if !(raw.is_finite() && raw > 0.0) {
        return 1.0;
    }
    let p = 10f64.powf(raw.log10().floor());
    let f = raw / p;
    let n = if f <= 1.0 + 1e-9 {
        1.0
    } else if f <= 2.0 + 1e-9 {
        2.0
    } else if f <= 5.0 + 1e-9 {
        5.0
    } else {
        10.0
    };
    n * p
}

/// Tick positions in `[min, max]` roughly `target` of them. Returns the step
/// and the tick values.
pub fn ticks(min: f64, max: f64, target: usize) -> (f64, Vec<f64>) {
    if !(min.is_finite() && max.is_finite()) || max <= min {
        return (1.0, Vec::new());
    }
    let step = nice_step((max - min) / target.max(1) as f64);
    let first = (min / step).ceil() as i64;
    let last = (max / step).floor() as i64;
    if last - first > 1000 {
        return (step, Vec::new());
    }
    ((step), (first..=last).map(|k| k as f64 * step).collect())
}

/// Label for a tick value; `step` decides how many decimals are needed.
pub fn format_tick(v: f64, step: f64) -> String {
    if v.abs() < step * 1e-9 {
        return "0".into();
    }
    if v.abs() >= 1e7 || v.abs() < 1e-4 {
        return trim_sci(format!("{v:.4e}"));
    }
    let decimals = (-step.log10().floor()).max(0.0) as usize;
    format!("{v:.decimals$}")
}

fn trim_sci(s: String) -> String {
    match s.split_once('e') {
        Some((m, e)) => {
            let m = if m.contains('.') {
                m.trim_end_matches('0').trim_end_matches('.')
            } else {
                m
            };
            format!("{m}e{e}")
        }
        None => s,
    }
}

/// A number with about six significant digits, for read-outs.
pub fn format_value(v: f64) -> String {
    if v == 0.0 {
        return "0".into();
    }
    if !v.is_finite() {
        return "undefined".into();
    }
    let exp = v.abs().log10().floor() as i32;
    if !(-4..7).contains(&exp) {
        return trim_sci(format!("{v:.5e}"));
    }
    let decimals = (5 - exp).clamp(0, 12) as usize;
    let s = format!("{v:.decimals$}");
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    }
}

/// Splits a sampled curve (pixel x, pixel y) into drawable polylines.
///
/// A line is broken where the function is undefined (NaN or infinite) and
/// where it jumps across the picture in the direction opposite to its previous
/// movement, which is how poles such as those of `tan(x)` or `1/x` show up.
/// A steep but continuous function keeps its direction and is not broken.
pub fn build_segments(points: &[(f32, f64)], height: f32) -> Vec<Vec<Pos2>> {
    const CLAMP: f64 = 1e5;
    let jump = 0.5 * height as f64;
    let mut out: Vec<Vec<Pos2>> = Vec::new();
    let mut cur: Vec<Pos2> = Vec::new();
    let mut last: Option<f64> = None;
    let mut last_delta = 0.0f64;
    let flush = |cur: &mut Vec<Pos2>, out: &mut Vec<Vec<Pos2>>| {
        if cur.len() >= 2 {
            out.push(std::mem::take(cur));
        } else {
            cur.clear();
        }
    };
    for &(px, py) in points {
        if !py.is_finite() {
            flush(&mut cur, &mut out);
            last = None;
            last_delta = 0.0;
            continue;
        }
        if let Some(prev) = last {
            let d = py - prev;
            if d.abs() > jump && last_delta != 0.0 && d.signum() != last_delta.signum() {
                flush(&mut cur, &mut out);
                last_delta = 0.0;
            } else if d != 0.0 {
                last_delta = d;
            }
        }
        cur.push(Pos2::new(px, py.clamp(-CLAMP, CLAMP) as f32));
        last = Some(py);
    }
    flush(&mut cur, &mut out);
    out
}

// ------------------------------------------------------------------ app state

pub struct Curve {
    pub text: String,
    pub color: Color32,
    pub visible: bool,
    /// Problem with the formula or definition, shown under the row.
    pub error: Option<String>,
    /// Problem storing a definition; stays until the text is edited.
    pub def_error: Option<String>,
}

pub struct GraphState {
    pub curves: Vec<Curve>,
    pub view: View,
    next_color: usize,
    fit_requested: bool,
}

impl GraphState {
    pub fn new() -> Self {
        let mut g = GraphState {
            curves: Vec::new(),
            view: View::default(),
            next_color: 0,
            fit_requested: false,
        };
        g.add_curve("x^2 - 4");
        g
    }

    pub fn add_curve(&mut self, text: &str) {
        if self.curves.len() >= MAX_CURVES {
            return;
        }
        self.curves.push(Curve {
            text: text.to_string(),
            color: PALETTE[self.next_color % PALETTE.len()],
            visible: true,
            error: None,
            def_error: None,
        });
        self.next_color += 1;
    }
}

impl Default for GraphState {
    fn default() -> Self {
        Self::new()
    }
}

// -------------------------------------------------------------------- drawing

impl GraphState {
    /// The list of curves and the buttons above the canvas.
    pub fn curves_ui(&mut self, ui: &mut egui::Ui, calc: &mut Calculator) {
        let mut remove: Option<usize> = None;
        egui::ScrollArea::vertical()
            .id_salt("curves")
            .max_height(150.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for (i, c) in self.curves.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        // Colour swatch: click to take the next palette colour.
                        let (rect, resp) =
                            ui.allocate_exact_size(Vec2::splat(16.0), Sense::click());
                        ui.painter().rect_filled(rect, 3.0, c.color);
                        if resp.on_hover_text("Click to change the colour").clicked() {
                            let at = PALETTE.iter().position(|p| *p == c.color).unwrap_or(0);
                            c.color = PALETTE[(at + 1) % PALETTE.len()];
                        }
                        ui.checkbox(&mut c.visible, "").on_hover_text("Show or hide");
                        let width = (ui.available_width() - 30.0).max(60.0);
                        let r = ui.add(
                            egui::TextEdit::singleline(&mut c.text)
                                .font(FontId::monospace(14.0))
                                .desired_width(width)
                                .hint_text("e.g. sin(x)  or  f(x) = x^2"),
                        );
                        if r.changed() {
                            c.def_error = None;
                        }
                        if r.lost_focus() {
                            // A definition is stored so other formulas can call it.
                            if let Err(e) = calc.define_function(&c.text) {
                                c.def_error = Some(e.to_string());
                            }
                        }
                        if ui.small_button("x").on_hover_text("Remove").clicked() {
                            remove = Some(i);
                        }
                    });
                    if let Some(err) = c.def_error.as_ref().or(c.error.as_ref()) {
                        ui.add(
                            egui::Label::new(
                                RichText::new(err)
                                    .small()
                                    .color(Color32::from_rgb(230, 80, 80)),
                            )
                            .wrap(),
                        );
                    }
                }
            });
        if let Some(i) = remove {
            self.curves.remove(i);
        }
        ui.horizontal(|ui| {
            if ui
                .add_enabled(self.curves.len() < MAX_CURVES, egui::Button::new("+ Add function"))
                .clicked()
            {
                self.add_curve("");
            }
            if ui.button("Reset view").on_hover_text("Also: double-click the plot").clicked() {
                self.view = View::default();
            }
            if ui.button("Fit y").on_hover_text("Scale vertically to the curves").clicked() {
                self.fit_requested = true;
            }
        });
    }
}

fn grid_color(ui: &egui::Ui, alpha: f32) -> Color32 {
    ui.visuals().text_color().gamma_multiply(alpha)
}

impl GraphState {
    /// Compiles every curve (cheap) so edits to variables, functions and the
    /// angle mode show up immediately. Records compile errors on the curves.
    fn compile(&mut self, calc: &mut Calculator) -> Vec<Option<PlotFn>> {
        self.curves
            .iter_mut()
            .map(|c| {
                if c.text.trim().is_empty() {
                    c.error = None;
                    return None;
                }
                match calc.plot_fn(&c.text, "x") {
                    Ok(f) => {
                        c.error = None;
                        Some(f)
                    }
                    Err(e) => {
                        c.error = Some(e.to_string());
                        None
                    }
                }
            })
            .collect()
    }

    /// The plot area, with pan, zoom and the hover read-out.
    pub fn canvas_ui(&mut self, ui: &mut egui::Ui, calc: &mut Calculator) {
        let fns = self.compile(calc);

        let avail = ui.available_size();
        let size = Vec2::new(avail.x.max(100.0), (avail.y - 24.0).max(120.0));
        let (resp, painter) = ui.allocate_painter(size, Sense::click_and_drag());
        let rect = resp.rect;
        let (w, h) = (rect.width() as f64, rect.height() as f64);

        // --- interaction
        if resp.dragged() {
            let d = resp.drag_delta();
            self.view.pan(d.x as f64, d.y as f64);
        }
        if resp.double_clicked() {
            self.view = View::default();
        }
        if resp.hovered() {
            let (scroll, pinch) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let factor = (scroll as f64 * 0.005).exp() * pinch as f64;
            if (factor - 1.0).abs() > 1e-9 {
                if let Some(p) = resp.hover_pos() {
                    let local = p - rect.min;
                    self.view
                        .zoom_at(local.x as f64, local.y as f64, w, h, factor);
                }
            }
        }
        if std::mem::take(&mut self.fit_requested) {
            let mut values = Vec::new();
            for (c, f) in self.curves.iter().zip(&fns) {
                if let (true, Some(f)) = (c.visible, f) {
                    for i in 0..=200 {
                        let x = self.view.x_at(w * i as f64 / 200.0, w);
                        values.push(f.eval(x));
                    }
                }
            }
            self.view.fit_y(&mut values, h);
        }
        let view = self.view;

        // --- background, grid, axes
        let painter = painter.with_clip_rect(rect);
        painter.rect_filled(rect, 2.0, ui.visuals().extreme_bg_color);
        let (x_min, x_max) = (view.x_at(0.0, w), view.x_at(w, w));
        let (y_min, y_max) = (view.y_at(h, h), view.y_at(0.0, h));
        let to_screen = |px: f64, py: f64| Pos2::new(rect.left() + px as f32, rect.top() + py as f32);
        let label_font = FontId::proportional(11.0);
        let label_color = grid_color(ui, 0.7);

        let (xstep, xticks) = ticks(x_min, x_max, (w / 90.0).max(2.0) as usize);
        let (ystep, yticks) = ticks(y_min, y_max, (h / 60.0).max(2.0) as usize);
        for &t in &xticks {
            let px = view.px_of(t, w);
            painter.line_segment(
                [to_screen(px, 0.0), to_screen(px, h)],
                Stroke::new(1.0, grid_color(ui, 0.12)),
            );
        }
        for &t in &yticks {
            let py = view.py_of(t, h);
            painter.line_segment(
                [to_screen(0.0, py), to_screen(w, py)],
                Stroke::new(1.0, grid_color(ui, 0.12)),
            );
        }
        let axis = Stroke::new(1.5, grid_color(ui, 0.55));
        let x_axis_py = view.py_of(0.0, h);
        let y_axis_px = view.px_of(0.0, w);
        if (0.0..=h).contains(&x_axis_py) {
            painter.line_segment([to_screen(0.0, x_axis_py), to_screen(w, x_axis_py)], axis);
        }
        if (0.0..=w).contains(&y_axis_px) {
            painter.line_segment([to_screen(y_axis_px, 0.0), to_screen(y_axis_px, h)], axis);
        }
        // Labels stay on the axis when it is visible, else at the border.
        let label_y = x_axis_py.clamp(2.0, h - 14.0) as f32;
        let label_x = y_axis_px.clamp(4.0, w - 40.0) as f32;
        for &t in &xticks {
            if t.abs() < xstep * 1e-9 && (0.0..=w).contains(&y_axis_px) {
                continue; // the "0" is drawn once, with the y labels
            }
            let px = view.px_of(t, w) as f32;
            painter.text(
                Pos2::new(rect.left() + px + 2.0, rect.top() + label_y + 2.0),
                Align2::LEFT_TOP,
                format_tick(t, xstep),
                label_font.clone(),
                label_color,
            );
        }
        for &t in &yticks {
            let py = view.py_of(t, h) as f32;
            painter.text(
                Pos2::new(rect.left() + label_x + 3.0, rect.top() + py - 12.0),
                Align2::LEFT_TOP,
                format_tick(t, ystep),
                label_font.clone(),
                label_color,
            );
        }

        // --- curves
        let n = ((rect.width() * SAMPLES_PER_PIXEL) as usize).max(2);
        for (c, f) in self.curves.iter().zip(&fns) {
            let (true, Some(f)) = (c.visible, f) else {
                continue;
            };
            let pts: Vec<(f32, f64)> = (0..=n)
                .map(|i| {
                    let px = w * i as f64 / n as f64;
                    (px as f32, view.py_of(f.eval(view.x_at(px, w)), h))
                })
                .collect();
            for seg in build_segments(&pts, rect.height()) {
                let line: Vec<Pos2> = seg.into_iter().map(|p| p + rect.min.to_vec2()).collect();
                painter.add(Shape::line(line, Stroke::new(2.0, c.color)));
            }
        }

        // --- hover read-out
        let mut status = String::from("drag: pan   scroll: zoom   double-click: reset");
        if let Some(p) = resp.hover_pos() {
            let local = p - rect.min;
            let x = view.x_at(local.x as f64, w);
            let y = view.y_at(local.y as f64, h);
            status = format!("x = {}   y = {}", format_value(x), format_value(y));
            let line_x = rect.left() + local.x;
            painter.line_segment(
                [Pos2::new(line_x, rect.top()), Pos2::new(line_x, rect.bottom())],
                Stroke::new(1.0, grid_color(ui, 0.35)),
            );
            let mut rows: Vec<(Color32, String)> = Vec::new();
            for (c, f) in self.curves.iter().zip(&fns) {
                if let (true, Some(f)) = (c.visible, f) {
                    let v = f.eval(x);
                    let name = c.text.split('=').next().unwrap_or("").trim();
                    let name = if name.is_empty() { "f" } else { name };
                    rows.push((c.color, format!("{name}: {}", format_value(v))));
                    let py = view.py_of(v, h);
                    if v.is_finite() && (0.0..=h).contains(&py) {
                        painter.circle_filled(to_screen(local.x as f64, py), 4.0, c.color);
                    }
                }
            }
            if !rows.is_empty() {
                let galleys: Vec<_> = rows
                    .iter()
                    .map(|(col, t)| painter.layout_no_wrap(t.clone(), FontId::monospace(12.0), *col))
                    .collect();
                let bw = galleys.iter().map(|g| g.size().x).fold(0.0, f32::max) + 10.0;
                let bh = galleys.iter().map(|g| g.size().y).sum::<f32>() + 8.0;
                let mut origin = p + Vec2::new(14.0, 14.0);
                if origin.x + bw > rect.right() {
                    origin.x = p.x - bw - 14.0;
                }
                if origin.y + bh > rect.bottom() {
                    origin.y = p.y - bh - 14.0;
                }
                let bg = Rect::from_min_size(origin, Vec2::new(bw, bh));
                painter.rect_filled(bg, 4.0, ui.visuals().window_fill().gamma_multiply(0.92));
                painter.rect_stroke(
                    bg,
                    4.0,
                    Stroke::new(1.0, grid_color(ui, 0.3)),
                    egui::StrokeKind::Inside,
                );
                let mut y = bg.top() + 4.0;
                for g in galleys {
                    let h = g.size().y;
                    painter.galley(Pos2::new(bg.left() + 5.0, y), g, Color32::WHITE);
                    y += h;
                }
            }
        }
        painter.rect_stroke(
            rect,
            2.0,
            Stroke::new(1.0, grid_color(ui, 0.3)),
            egui::StrokeKind::Inside,
        );
        ui.label(RichText::new(status).small().weak());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9 * a.abs().max(b.abs()).max(1.0)
    }

    #[test]
    fn nice_steps() {
        assert_eq!(nice_step(0.7), 1.0);
        assert_eq!(nice_step(1.3), 2.0);
        assert_eq!(nice_step(3.0), 5.0);
        assert_eq!(nice_step(7.0), 10.0);
        assert!(close(nice_step(0.013), 0.02));
        assert_eq!(nice_step(1.0), 1.0);
        assert_eq!(nice_step(-1.0), 1.0);
        assert_eq!(nice_step(f64::NAN), 1.0);
    }

    #[test]
    fn tick_positions() {
        let (step, t) = ticks(-10.0, 10.0, 8);
        assert_eq!(step, 5.0);
        assert_eq!(t, vec![-10.0, -5.0, 0.0, 5.0, 10.0]);
        let (step, t) = ticks(-10.0, 10.0, 12);
        assert_eq!(step, 2.0);
        assert_eq!(t.len(), 11);
        let (step, t) = ticks(0.12, 0.38, 5);
        assert!(close(step, 0.1));
        assert_eq!(t.len(), 2);
        assert!(t.iter().all(|v| *v >= 0.12 && *v <= 0.38));
        assert!(ticks(1.0, 1.0, 5).1.is_empty());
        assert!(ticks(f64::NAN, 1.0, 5).1.is_empty());
        // absurdly small steps relative to range are rejected, not allocated
        assert!(ticks(0.0, 1e300, 1_000_000).1.len() <= 1001);
    }

    #[test]
    fn tick_labels() {
        assert_eq!(format_tick(2.0, 2.0), "2");
        assert_eq!(format_tick(-0.5, 0.5), "-0.5");
        assert_eq!(format_tick(0.25, 0.05), "0.25");
        assert_eq!(format_tick(1e-17, 1.0), "0");
        assert_eq!(format_tick(2.5e8, 5e7), "2.5e8");
        assert_eq!(format_tick(0.00002, 0.00001), "2e-5");
        assert_eq!(format_tick(1000.0, 500.0), "1000");
    }

    #[test]
    fn value_formatting() {
        assert_eq!(format_value(0.0), "0");
        assert_eq!(format_value(1.0), "1");
        assert_eq!(format_value(0.5), "0.5");
        assert_eq!(format_value(std::f64::consts::PI), "3.14159");
        assert_eq!(format_value(-123456.789), "-123457");
        assert_eq!(format_value(12.3456789), "12.3457");
        assert_eq!(format_value(1.5e10), "1.5e10");
        assert_eq!(format_value(f64::NAN), "undefined");
        assert_eq!(format_value(0.00001234), "1.234e-5");
    }

    #[test]
    fn view_transform_round_trips() {
        let v = View { cx: 3.0, cy: -2.0, ux: 0.1, uy: 0.5 };
        let (w, h) = (400.0, 300.0);
        for (x, y) in [(0.0, 0.0), (5.5, -7.25), (-100.0, 40.0)] {
            assert!(close(v.x_at(v.px_of(x, w), w), x));
            assert!(close(v.y_at(v.py_of(y, h), h), y));
        }
        // centre of the canvas is the view centre; up is +y
        assert!(close(v.x_at(w / 2.0, w), 3.0));
        assert!(v.y_at(0.0, h) > v.y_at(h, h));
    }

    #[test]
    fn zoom_keeps_the_point_under_the_cursor() {
        let mut v = View::default();
        let (w, h) = (500.0, 400.0);
        let (px, py) = (123.0, 321.0);
        let (x, y) = (v.x_at(px, w), v.y_at(py, h));
        v.zoom_at(px, py, w, h, 2.0);
        assert!(close(v.ux, DEFAULT_SCALE / 2.0));
        assert!(close(v.x_at(px, w), x));
        assert!(close(v.y_at(py, h), y));
        v.zoom_at(px, py, w, h, 0.0); // invalid factor is ignored
        v.zoom_at(px, py, w, h, f64::NAN);
        assert!(close(v.ux, DEFAULT_SCALE / 2.0));
        for _ in 0..200 {
            v.zoom_at(px, py, w, h, 10.0);
        }
        assert!(v.ux >= MIN_SCALE);
    }

    #[test]
    fn pan_moves_with_the_drag() {
        let mut v = View::default();
        let before = v.x_at(100.0, 500.0);
        v.pan(10.0, 0.0); // drag right: the picture moves right
        assert!(close(v.x_at(110.0, 500.0), before));
        let y_before = v.y_at(100.0, 400.0);
        v.pan(0.0, 10.0);
        assert!(close(v.y_at(110.0, 400.0), y_before));
    }

    #[test]
    fn fit_y_ignores_spikes() {
        let mut v = View::default();
        let mut vals: Vec<f64> = (0..1000).map(|i| (i as f64 / 100.0).sin()).collect();
        vals.push(1e12);
        vals.push(f64::NAN);
        v.fit_y(&mut vals, 400.0);
        let top = v.y_at(0.0, 400.0);
        let bottom = v.y_at(400.0, 400.0);
        assert!(top > 1.0 && top < 2.0, "top {top}");
        assert!(bottom < -1.0 && bottom > -2.0, "bottom {bottom}");
        // constant function: still a usable range
        let mut c = vec![5.0; 50];
        v.fit_y(&mut c, 400.0);
        assert!(v.uy > 0.0 && close(v.cy, 5.0));
        let before = v;
        v.fit_y(&mut vec![f64::NAN], 400.0);
        assert_eq!(v, before);
    }

    fn px_series(f: impl Fn(f64) -> f64, x0: f64, x1: f64, h: f32) -> Vec<Vec<Pos2>> {
        // 400 px wide, 300 px tall canvas, y in [-10, 10]
        let view = View { cx: (x0 + x1) / 2.0, cy: 0.0, ux: (x1 - x0) / 400.0, uy: 20.0 / h as f64 };
        let n = 1200;
        let pts: Vec<(f32, f64)> = (0..=n)
            .map(|i| {
                let px = 400.0 * i as f64 / n as f64;
                (px as f32, view.py_of(f(view.x_at(px, 400.0)), h as f64))
            })
            .collect();
        build_segments(&pts, h)
    }

    #[test]
    fn smooth_curves_are_one_segment() {
        let segs = px_series(f64::sin, -6.0, 6.0, 300.0);
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].len(), 1201);
    }

    #[test]
    fn undefined_regions_break_the_line() {
        let segs = px_series(f64::sqrt, -5.0, 5.0, 300.0);
        assert_eq!(segs.len(), 1); // only x >= 0 is drawn
        assert!(segs[0][0].x > 199.0);
        let segs = px_series(|x| if x.abs() < 1.0 { f64::NAN } else { x }, -5.0, 5.0, 300.0);
        assert_eq!(segs.len(), 2);
    }

    #[test]
    fn poles_break_the_line() {
        let segs = px_series(|x| 1.0 / x, -5.0, 5.0, 300.0);
        assert_eq!(segs.len(), 2, "1/x");
        // tan has poles at +-pi/2, +-3pi/2 -> five pieces over [-6, 6]... at least 4 breaks
        let segs = px_series(f64::tan, -6.0, 6.0, 300.0);
        assert!(segs.len() >= 4, "tan gave {} segments", segs.len());
        // nothing connects across the pole: every segment is monotone-ish and
        // no step inside a segment spans more than half the canvas
        for s in &segs {
            for w in s.windows(2) {
                assert!((w[1].y - w[0].y).abs() <= 150.0 || (w[1].y - w[0].y).signum() == (s[1].y - s[0].y).signum());
            }
        }
    }

    #[test]
    fn steep_continuous_curves_are_not_broken() {
        let segs = px_series(|x| x.powi(9), -2.0, 2.0, 300.0);
        assert_eq!(segs.len(), 1);
        let segs = px_series(|x| x * x, -50.0, 50.0, 300.0);
        assert_eq!(segs.len(), 1);
    }

    #[test]
    fn huge_values_are_clamped() {
        let segs = build_segments(&[(0.0, 1e300), (1.0, -1e300), (2.0, 0.0)], 300.0);
        for s in segs {
            for p in s {
                assert!(p.y.abs() <= 1e5);
            }
        }
    }
}
