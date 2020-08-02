use wgpu_glyph::{ab_glyph, GlyphBrushBuilder, Section, Text, GlyphBrush, Layout, VerticalAlign, HorizontalAlign, BuiltInLineBreaker};
use wgpu_glyph::ab_glyph::FontArc;

pub struct TextHandler {
    glyph_brush: GlyphBrush<(),FontArc>,
}

impl TextHandler {
    // Initialize a glyph brush instance
    pub fn init(device: &wgpu::Device, render_format: wgpu::TextureFormat) -> Self {
        let font = ab_glyph::FontArc::try_from_slice(include_bytes!("../Caladea-Regular.ttf"))
            .expect("Load font");

        let glyph_brush = GlyphBrushBuilder::using_font(font)
            .build(&device, render_format);
        TextHandler {
            glyph_brush
        }
    }

    // Queues a string to be drawn
    // size is the size of the text in PIXELS
    // limit is the max width for the text
    pub fn draw(&mut self, text: &str, x: f32, y: f32, size: f32, limit: f32, color: [f32; 4]) {
        self.glyph_brush.queue(Section {
            screen_position: (x, y),
            text: vec![Text::new(text).with_scale(size).with_color(color)],
            bounds: (limit, f32::INFINITY),
            layout: Layout::default_single_line(),
            ..Section::default()
        });
    }

    pub fn draw_centered(&mut self, text: &str, x: f32, y: f32, size: f32, limit: f32, color: [f32; 4]) {
        self.glyph_brush.queue(Section {
            screen_position: (x, y),
            text: vec![Text::new(text).with_scale(size).with_color(color)],
            bounds: (limit, f32::INFINITY),
            layout: Layout::SingleLine {
                line_breaker: BuiltInLineBreaker::AnyCharLineBreaker, // Apparently UnicodeLineBreaker is weird with '/'
                h_align: HorizontalAlign::Center,
                v_align: VerticalAlign::Center,
            },
        });
    }

    // Flushes the queue, rendering the text to screen
    pub fn flush(&mut self, device: &wgpu::Device, mut encoder: &mut wgpu::CommandEncoder, frame: &wgpu::SwapChainOutput, size: (u32,u32)) {
        self.glyph_brush.draw_queued(
            &device,
            &mut encoder,
            &frame.view,
            size.0,
            size.1,
        ).unwrap();
    }
}

