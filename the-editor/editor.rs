use the_editor_renderer::{
  Application,
  Color,
  InputEvent,
  Renderer,
  TextSection,
  TextSegment,
};

use crate::core::document::Document;

pub struct Editor {
  text_content: String,
  document: Document,
}

impl Editor {
  pub fn new() -> Self {
    Self {
      text_content: String::from("Welcome to The Editor!!"),
      document: Document::new(),
    }
  }
}

impl Application for Editor {
  fn init(&mut self, _renderer: &mut Renderer) {
    println!("Editor initialized!");
  }

  fn render(&mut self, renderer: &mut Renderer) {
    renderer.draw_text(TextSection::simple(
      50.0,
      50.0,
      "Welcome to The Editor!",
      48.0,
      Color::rgb(0.9, 0.9, 0.9),
    ));

    renderer.draw_text(TextSection::simple(
      50.0,
      120.0,
      "A modern text editor built with Rust",
      24.0,
      Color::rgb(0.7, 0.7, 0.8),
    ));

    renderer.draw_text(TextSection::simple(
      50.0,
      180.0,
      "Press ESC to exit",
      18.0,
      Color::rgb(0.5, 0.5, 0.6),
    ));

    renderer.draw_text(
      TextSection::new(50.0, 250.0).add_text(
        TextSegment::new("// Sample code")
          .with_color(Color::rgb(0.6, 0.8, 0.6))
          .with_size(20.0),
      ),
    );

    renderer.draw_text(
      TextSection::new(50.0, 280.0)
        .add_text(
          TextSegment::new("fn ")
            .with_color(Color::rgb(0.5, 0.7, 0.9))
            .with_size(20.0),
        )
        .add_text(
          TextSegment::new("main")
            .with_color(Color::rgb(0.9, 0.9, 0.7))
            .with_size(20.0),
        )
        .add_text(
          TextSegment::new("() {")
            .with_color(Color::rgb(0.9, 0.9, 0.85))
            .with_size(20.0),
        ),
    );

    renderer.draw_text(
      TextSection::new(50.0, 310.0)
        .add_text(
          TextSegment::new("    println!")
            .with_color(Color::rgb(0.5, 0.7, 0.9))
            .with_size(20.0),
        )
        .add_text(
          TextSegment::new("(")
            .with_color(Color::rgb(0.9, 0.9, 0.85))
            .with_size(20.0),
        )
        .add_text(
          TextSegment::new("\"Hello, world!\"")
            .with_color(Color::rgb(0.8, 0.6, 0.4))
            .with_size(20.0),
        )
        .add_text(
          TextSegment::new(");")
            .with_color(Color::rgb(0.9, 0.9, 0.85))
            .with_size(20.0),
        ),
    );

    renderer.draw_text(TextSection::simple(
      50.0,
      340.0,
      "}",
      20.0,
      Color::rgb(0.9, 0.9, 0.85),
    ));

    let status_y = renderer.height() as f32 - 30.0;
    renderer.draw_text(TextSection::simple(
      10.0,
      status_y,
      format!("Ready | Size: {}x{}", renderer.width(), renderer.height()),
      14.0,
      Color::rgb(0.6, 0.6, 0.7),
    ));
  }

  fn handle_event(&mut self, event: InputEvent, _renderer: &mut Renderer) -> bool {
    match event {
      InputEvent::Keyboard(key_press) => {
        if key_press.pressed {
          println!("Key pressed: {:?}", key_press.code);
          true
        } else {
          false
        }
      },
      InputEvent::Mouse(mouse) => {
        println!("Mouse event at {:?}", mouse.position);
        false
      },
      InputEvent::Text(text) => {
        println!("Text input: {}", text);
        self.text_content.push_str(&text);
        true
      },
    }
  }

  fn resize(&mut self, width: u32, height: u32, _renderer: &mut Renderer) {
    println!("Window resized to {}x{}", width, height);
  }
}
