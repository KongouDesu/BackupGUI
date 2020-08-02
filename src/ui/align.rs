/// This module serves 2 purposes
/// 1. Aligning vertices relative to something else
/// 2. Handling scaling

use crate::gui::Vertex;
use crate::gui::TexVertex;

pub struct AlignConfig {
    // width and height is multiplied by scale
    // This allows us to scale the UI size dynamically
    pub scale: f32,
    // Size of the window/render area
    // We need to know this to align things correctly (centering)
    // Must be updated when the window is resized
    pub win_width: f32,
    pub win_height: f32,
    // Texture size
    // Needed to compute (u,v) when drawing only part of the texture
    pub tex_width: f32,
    pub tex_height: f32,
}

#[allow(dead_code)]
pub enum Anchor {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    CenterLocal,  // Makes (x,y) the center to draw around
    CenterGlobal, // (x,y) is offset from window center - resulting coordinate is used as CenterLocal
}

impl AlignConfig {
    pub fn resize(&mut self, w: f32, h: f32) {
        self.win_width = w;
        self.win_height = h;
    }

    // Used by ui/mod.rs (file-tree rendering)
    // Unlike image and click code, the (x,y) coordinates are _not_ scaled, as it does that locally
    pub fn rectangle(&self, anchor: Anchor, x: f32, y: f32, w: f32, h: f32, color: [f32;4]) -> Vec<Vertex> {
        let w = w*self.scale;
        let h = h*self.scale;
        match anchor {
            Anchor::TopLeft => {
                Vertex::rect(x,y,w,h,color)
            },
            Anchor::TopRight => {
                Vertex::rect(self.win_width-x-w,y,w,h,color)
            },
            Anchor::BottomLeft => {
                Vertex::rect(x,self.win_height-y-h,w,h,color)
            },
            Anchor::BottomRight => {
                Vertex::rect(self.win_width-x-w,self.win_height-y,w,h,color)
            },
            Anchor::CenterLocal => {
                Vertex::rect(x-w/2.0,y-h/2.0,w,h,color)
            },
            Anchor::CenterGlobal => {
                let nx = self.win_width/2.0 + x;
                let ny = self.win_height/2.0 + y;
                Vertex::rect(nx-w/2.0,ny-h/2.0,w,h,color)
            }
        }
    }

    // 'section' is the top-left (x,y) coordinates and (w,h) (in pixels) of the image to draw
    // this lets us draw only part of the image
    // If the section is 'None', the whole image will be used
    #[allow(clippy::too_many_arguments)]
    pub fn image(&self, anchor: Anchor, x: f32, y: f32, w: f32, h: f32, angle: f32, section: Option<[f32;4]>) -> Vec<TexVertex> {
        let x = x*self.scale;
        let y = y*self.scale;
        let w = w*self.scale;
        let h = h*self.scale;
        let section = match section {
            Some(sec) => sec,
            None => [0.0,0.0,self.tex_width,self.tex_height],
        };
        match anchor {
            Anchor::TopLeft => {
                TexVertex::rect(x, y, w, h, angle, (self.tex_width, self.tex_height), section)
            },
            Anchor::TopRight => {
                TexVertex::rect(self.win_width-x-w, y, w, h, angle, (self.tex_width, self.tex_height), section)
            },
            Anchor::BottomLeft => {
                TexVertex::rect(x, self.win_height-y-h, w, h, angle, (self.tex_width, self.tex_height), section)
            },
            Anchor::BottomRight => {
                TexVertex::rect(self.win_width-x-w, self.win_height-y-h, w, h, angle, (self.tex_width, self.tex_height), section)
            },
            Anchor::CenterLocal => {
                TexVertex::rect(x-w/2.0, y-h/2.0, w, h, angle, (self.tex_width, self.tex_height), section)
            },
            Anchor::CenterGlobal => {
                let nx = self.win_width/2.0 + x;
                let ny = self.win_height/2.0 + y;
                TexVertex::rect(nx-w/2.0, ny-h/2.0, w, h, angle, (self.tex_width, self.tex_height), section)
            }
        }
    }

    // Returns 'true' if (cx,cy) was inside thw (x,y,w,h) rectangle, false otherwise
    // Handles scaling of coordinates and area size
    #[allow(clippy::too_many_arguments)]
    pub fn was_area_clicked(&self, anchor: Anchor, cx: f32, cy: f32, x: f32, y: f32, w: f32, h: f32) -> bool {
        let x = x*self.scale;
        let y = y*self.scale;
        let w = w*self.scale;
        let h = h*self.scale;
        match anchor {
            Anchor::TopLeft => {
                inside_rect(cx,cy,x,y,w,h)
            },
            Anchor::TopRight => {
                inside_rect(cx,cy,self.win_width - x,y,w,h)
            },
            Anchor::BottomLeft => {
                inside_rect(cx,cy,x,self.win_height - y,w,h)
            },
            Anchor::BottomRight => {
                inside_rect(cx,cy,self.win_width - x - w,self.win_height-y - h,w,h)
            },
            Anchor::CenterLocal => {
                inside_rect(cx,cy,x-w/2.0,y-h/2.0,w,h)
            },
            Anchor::CenterGlobal => {
                inside_rect(cx,cy,self.win_width/2.0 - w/2.0 + x,self.win_height/2.0 - h/2.0 + y,w,h)
            }
        }
    }
}

// Helper for 'was_area_clicked'
fn inside_rect(cx: f32, cy: f32, x: f32, y: f32, w: f32, h: f32) -> bool {
    cx >= x && cx <= x+w && cy >= y && cy <= y+h
}