use raylib::{
    color::Color as RayColor, math::Vector2, prelude::RaylibDraw, text::{RaylibFont, WeakFont}, RaylibHandle, RaylibThread
};

use utils::{error, info};

use crate::{Color, RenderGUICommand};

pub struct Gui {
    pub rl: RaylibHandle,
    thread: RaylibThread,
    pub font: WeakFont, // Whatever that means.
}

impl Gui {
    const FONT_SPACING: f32 = 1.0;

    pub fn new(width: i32, height: i32, font_path: &str) -> Self {
        let (mut rl, thread) = raylib::init()
            .title("The Editor")
            .size(width, height)
            .resizable()
            .build();

        rl.set_target_fps(60);
        rl.set_exit_key(None);

        let font = match rl.load_font(&thread, font_path) {
            Ok(f) => {
                info!("Successfully loaded font: {}", font_path);
                f.make_weak()
            }
            Err(e) => {
                error!(
                    "Failed to load font '{}': {}. Using default font.",
                    font_path, e
                );
                rl.get_font_default()
            }
        };

        Self { rl, thread, font }
    }

    pub fn size(&self) -> (i32, i32) {
        (
            self.rl.get_screen_width(),
            self.rl.get_screen_height(),
        )
    }

    pub fn gui_measure_font_width(&self, text: &str, font_size: f32) -> f32 {
        self.font.measure_text(text, font_size, Self::FONT_SPACING).x
    }

    pub fn process_commands(&mut self, commands: &[RenderGUICommand]) {
        let mut draw_handle = self.rl.begin_drawing(&self.thread);

        for command in commands {
            match command {
                RenderGUICommand::ClearBackground(color) => {
                    let raylib_color = process_raylib_color(color);
                    draw_handle.clear_background(raylib_color);
                }
                RenderGUICommand::DrawText(text, x, y, font_size, color) => {
                    let raylib_color = process_raylib_color(color);
                    draw_handle.draw_text_ex(
                        &self.font,
                        text,
                        Vector2::new(*x as f32, *y as f32),
                        *font_size as f32,
                        Self::FONT_SPACING,
                        raylib_color,
                    );
                }
                RenderGUICommand::DrawRectangle(x, y, width, height, rect_color) => {
                    let raylib_color = process_raylib_color(rect_color);
                    draw_handle.draw_rectangle(*x, *y, *width, *height, raylib_color);
                }
                RenderGUICommand::DrawCursor(x, y, width, height, cursor_color, alpha) => {
                    // Block cursor btw.
                    let mut raylib_color = process_raylib_color(cursor_color);
                    raylib_color.a = *alpha;
                    draw_handle.draw_rectangle(*x, *y, *width, *height, raylib_color);
                }
            }
        }
    }
}

fn process_raylib_color(color: &Color) -> RayColor {
    match color {
        Color::BLACK => RayColor::BLACK,
        Color::WHITE => RayColor::WHITE,
        Color::LIGHTGRAY => RayColor::LIGHTGRAY,
        Color::BLUE => RayColor::BLUE,
    }
}
