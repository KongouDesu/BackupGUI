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
    pub win_height: f32
}

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

    pub fn rectangle(&self, anchor: Anchor, x: f32, y: f32, w: f32, h: f32, color: [f32;4]) -> Vec<Vertex> {
        let w = w*self.scale;
        let h = h*self.scale;
        match anchor {
            Anchor::TopLeft => {
                Vertex::rect(x,y,w,h,color)
            },
            Anchor::TopRight => {
                Vertex::rect(x-w,y,w,h,color)
            },
            Anchor::BottomLeft => {
                Vertex::rect(x,y-h,w,h,color)
            },
            Anchor::BottomRight => {
                Vertex::rect(x-w,y,w,h,color)
            },
            Anchor::CenterLocal => {
                Vertex::rect(x-w/2.0,y-h/2.0,w,h,color)
            },
            Anchor::CenterGlobal => {
                let nx = self.win_width/2.0 + x*self.scale;
                let ny = self.win_height/2.0 + y*self.scale;
                Vertex::rect(nx-w/2.0,ny-h/2.0,w,h,color)
            }
        }
    }

    pub fn image(&self, anchor: Anchor, x: f32, y: f32, w: f32, h: f32, a: f32) -> Vec<TexVertex> {
        let w = w*self.scale;
        let h = h*self.scale;
        match anchor {
            Anchor::TopLeft => {
                TexVertex::rect(x,y,w,h,a)
            },
            Anchor::TopRight => {
                TexVertex::rect(x-w,y,w,h,a)
            },
            Anchor::BottomLeft => {
                TexVertex::rect(x,y-h,w,h,a)
            },
            Anchor::BottomRight => {
                TexVertex::rect(x-w,y,w,h,a)
            },
            Anchor::CenterLocal => {
                TexVertex::rect(x-w/2.0,y-h/2.0,w,h,a)
            },
            Anchor::CenterGlobal => {
                let nx = self.win_width/2.0 + x*self.scale;
                let ny = self.win_height/2.0 + y*self.scale;
                TexVertex::rect(nx-w/2.0,ny-h/2.0,w,h,a)
            }
        }
    }
}
