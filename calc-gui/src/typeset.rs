//! Typeset maths for the GUI: LaTeX -> PNG (RaTeX) -> egui texture, cached.

use std::collections::HashMap;

use eframe::egui::{self, Color32, ColorImage, TextureHandle, TextureOptions, Ui, Vec2};
use ratex_layout::{layout, to_display_list, LayoutOptions};
use ratex_parser::parser::parse;
use ratex_render::{render_to_png, RenderOptions};
use ratex_types::color::Color;
use ratex_types::math_style::MathStyle;

type Key = (String, u32, u32, [u8; 3]);

struct Entry {
    texture: TextureHandle,
    /// Size in egui points.
    size: Vec2,
}

#[derive(Default)]
pub struct Typesetter {
    cache: HashMap<Key, Option<Entry>>,
}

impl Typesetter {
    /// Draws `latex` at the given font size (points). Returns `false`, and draws
    /// nothing, if the formula could not be typeset.
    pub fn show(&mut self, ui: &mut Ui, latex: &str, size_pt: f32) -> bool {
        let ppp = ui.ctx().pixels_per_point();
        let c = ui.visuals().text_color();
        let key: Key = (
            latex.to_string(),
            (size_pt * 10.0) as u32,
            (ppp * 100.0) as u32,
            [c.r(), c.g(), c.b()],
        );
        if self.cache.len() > 48 {
            self.cache.clear();
        }
        let entry = self
            .cache
            .entry(key)
            .or_insert_with(|| render(ui.ctx(), latex, size_pt, ppp, c));
        match entry {
            Some(e) => {
                ui.add(egui::Image::new(egui::load::SizedTexture::new(
                    e.texture.id(),
                    e.size,
                )));
                true
            }
            None => false,
        }
    }
}

fn render(ctx: &egui::Context, latex: &str, size_pt: f32, ppp: f32, c: Color32) -> Option<Entry> {
    let (r, g, b) = (c.r() as f32 / 255.0, c.g() as f32 / 255.0, c.b() as f32 / 255.0);
    let ast = parse(latex).ok()?;
    let opts = LayoutOptions::default()
        .with_style(MathStyle::Display)
        .with_color(Color::rgb(r, g, b));
    let list = to_display_list(&layout(&ast, &opts));
    let ro = RenderOptions {
        font_size: size_pt,
        padding: 3.0,
        background_color: Color::new(r, g, b, 0.0),
        font_dir: String::new(),
        device_pixel_ratio: ppp,
    };
    let bytes = render_to_png(&list, &ro).ok()?;

    let mut reader = png::Decoder::new(&bytes[..]).read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return None;
    }
    let (w, h) = (info.width as usize, info.height as usize);
    let image = ColorImage::from_rgba_unmultiplied([w, h], &buf[..info.buffer_size()]);
    let texture = ctx.load_texture("typeset", image, TextureOptions::LINEAR);
    Some(Entry {
        texture,
        size: Vec2::new(w as f32 / ppp, h as f32 / ppp),
    })
}
