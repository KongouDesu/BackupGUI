use wgpu_glyph::{ab_glyph, GlyphBrushBuilder, Section, Text, GlyphBrush};
use wgpu_glyph::ab_glyph::FontArc;

pub struct TextHandler {
    glyph_brush: GlyphBrush<(),FontArc>,
}

impl TextHandler {
    // Initialize a glyph brush instance
    pub fn init(device: &wgpu::Device, render_format: wgpu::TextureFormat) -> Self {
        let font = ab_glyph::FontArc::try_from_slice(include_bytes!("../Caladea-Regular.ttf"))
            .expect("Load font");

        let mut glyph_brush = GlyphBrushBuilder::using_font(font)
            .build(&device, render_format);
        TextHandler {
            glyph_brush
        }
    }

    // Queues a string to be drawn
    // Scale is the size of the text in PIXELS
    pub fn draw(&mut self, text: &str, x: f32, y: f32, scale: f32, color: [f32; 4]) {
        self.glyph_brush.queue(Section {
            screen_position: (x, y),
            text: vec![Text::new(text).with_scale(scale).with_color(color)],
            ..Section::default()
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

