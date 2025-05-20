use raylib::{color::Color as RayColor, prelude::RaylibDraw, RaylibHandle, RaylibThread};

use crate::{Color, RenderGUICommand};

pub struct Gui {
    rl: RaylibHandle,
    thread: RaylibThread,
}

impl Gui {
    pub fn new(width: i32, height: i32) -> Self {
        let (rl, thread) = raylib::init()
            .title("The Editor")
            .size(width, height)
            .resizable()
            .build();
        
        Self {
            rl,
            thread,
        }
    }

    pub fn size(&self) -> (usize, usize) {
        (
            self.rl.get_screen_width() as usize,
            self.rl.get_screen_height() as usize,
        )
    }

    pub fn process_commands(&mut self, commands: &[RenderGUICommand]) {
        let mut draw_handle = self.rl.begin_drawing(&self.thread);
        
        for command in commands {
            match command {
                RenderGUICommand::ClearBackground(color) => {
                    let raylib_color = process_raylib_color(color);
                    draw_handle.clear_background(raylib_color);
                }
            }
        }
    }
}

fn process_raylib_color(color: &Color) -> RayColor {
    match color {
        Color::BLACK => RayColor::BLACK,
        Color::WHITE => RayColor::WHITE
    }
}

