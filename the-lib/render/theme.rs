use std::{
    collections::HashMap,
    str,
    sync::OnceLock,
};

use crate::syntax::Highlight;
use tracing::warn;
use serde::{Deserialize, Deserializer};
use toml::{map::Map, Value};

use super::graphics::UnderlineStyle;
pub use super::graphics::{Color, Modifier, Style};

static DEFAULT_THEME_DATA: OnceLock<Value> = OnceLock::new();
static BASE16_DEFAULT_THEME_DATA: OnceLock<Value> = OnceLock::new();
static DEFAULT_THEME: OnceLock<Theme> = OnceLock::new();
static BASE16_DEFAULT_THEME: OnceLock<Theme> = OnceLock::new();

fn load_embedded_theme(contents: &str, name: &str) -> Value {
    toml::from_str(contents).unwrap_or_else(|err| {
        warn!("Failed to parse embedded theme '{name}': {err}");
        Value::Table(Map::new())
    })
}

fn default_theme_data() -> &'static Value {
    DEFAULT_THEME_DATA.get_or_init(|| load_embedded_theme(include_str!("../theme.toml"), "default"))
}

fn base16_theme_data() -> &'static Value {
    BASE16_DEFAULT_THEME_DATA.get_or_init(|| {
        load_embedded_theme(include_str!("../base16_theme.toml"), "base16_default")
    })
}

pub fn default_theme() -> &'static Theme {
    DEFAULT_THEME.get_or_init(|| Theme {
        name: "default".into(),
        ..Theme::from(default_theme_data().clone())
    })
}

pub fn base16_default_theme() -> &'static Theme {
    BASE16_DEFAULT_THEME.get_or_init(|| Theme {
        name: "base16_default".into(),
        ..Theme::from(base16_theme_data().clone())
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Mode {
    Dark,
    Light,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    light: String,
    dark: String,
    /// A theme to choose when the terminal did not declare either light or dark mode.
    /// When not specified the dark theme is preferred.
    fallback: Option<String>,
}

impl Config {
    pub fn choose(&self, preference: Option<Mode>) -> &str {
        match preference {
            Some(Mode::Light) => &self.light,
            Some(Mode::Dark) => &self.dark,
            None => self.fallback.as_ref().unwrap_or(&self.dark),
        }
    }
}

impl<'de> Deserialize<'de> for Config {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged, deny_unknown_fields, rename_all = "kebab-case")]
        enum InnerConfig {
            Constant(String),
            Adaptive {
                dark: String,
                light: String,
                fallback: Option<String>,
            },
        }

        let inner = InnerConfig::deserialize(deserializer)?;

        let (light, dark, fallback) = match inner {
            InnerConfig::Constant(theme) => (theme.clone(), theme.clone(), None),
            InnerConfig::Adaptive {
                light,
                dark,
                fallback,
            } => (light, dark, fallback),
        };

        Ok(Self {
            light,
            dark,
            fallback,
        })
    }
}
// Theme loading (IO + inheritance) intentionally lives outside the-lib.

#[derive(Clone, Debug, Default)]
pub struct Theme {
    name: String,

    // UI styles are stored in a HashMap
    styles: HashMap<String, Style>,
    // tree-sitter highlight styles are stored in a Vec to optimize lookups
    scopes: Vec<String>,
    highlights: Vec<Style>,
    rainbow_length: usize,
}

impl From<Value> for Theme {
    fn from(value: Value) -> Self {
        let (theme, warnings) = Theme::from_toml(value);
        for warning in warnings {
            warn!("{}", warning);
        }
        theme
    }
}

impl<'de> Deserialize<'de> for Theme {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let values = Map::<String, Value>::deserialize(deserializer)?;
        let (theme, warnings) = Theme::from_keys(values);
        for warning in warnings {
            warn!("{}", warning);
        }
        Ok(theme)
    }
}

#[allow(clippy::type_complexity)]
fn build_theme_values(
    mut values: Map<String, Value>,
) -> (
    HashMap<String, Style>,
    Vec<String>,
    Vec<Style>,
    usize,
    Vec<String>,
) {
    let mut styles = HashMap::new();
    let mut scopes = Vec::new();
    let mut highlights = Vec::new();
    let mut rainbow_length = 0;

    let mut warnings = Vec::new();

    // TODO: alert user of parsing failures in editor
    let palette = values
        .remove("palette")
        .map(|value| {
            ThemePalette::try_from(value).unwrap_or_else(|err| {
                warnings.push(err);
                ThemePalette::default()
            })
        })
        .unwrap_or_default();
    // remove inherits from value to prevent errors
    let _ = values.remove("inherits");
    styles.reserve(values.len());
    scopes.reserve(values.len());
    highlights.reserve(values.len());

    for (i, style) in values
        .remove("rainbow")
        .and_then(|value| match palette.parse_style_array(value) {
            Ok(styles) => Some(styles),
            Err(err) => {
                warnings.push(err);
                None
            }
        })
        .unwrap_or_else(default_rainbow)
        .into_iter()
        .enumerate()
    {
        let name = format!("rainbow.{i}");
        styles.insert(name.clone(), style);
        scopes.push(name);
        highlights.push(style);
        rainbow_length += 1;
    }

    for (name, style_value) in values {
        let mut style = Style::default();
        if let Err(err) = palette.parse_style(&mut style, style_value) {
            warnings.push(format!("Failed to parse style for key {name:?}. {err}"));
        }

        // these are used both as UI and as highlights
        styles.insert(name.clone(), style);
        scopes.push(name);
        highlights.push(style);
    }

    (styles, scopes, highlights, rainbow_length, warnings)
}

fn default_rainbow() -> Vec<Style> {
    vec![
        Style::default().fg(Color::Red),
        Style::default().fg(Color::Yellow),
        Style::default().fg(Color::Green),
        Style::default().fg(Color::Blue),
        Style::default().fg(Color::Cyan),
        Style::default().fg(Color::Magenta),
    ]
}
impl Theme {
    /// To allow `Highlight` to represent arbitrary RGB colors without turning it into an enum,
    /// we interpret the last 256^3 numbers as RGB.
    const RGB_START: u32 = (u32::MAX << (8 + 8 + 8)) - 1 - (u32::MAX - Highlight::MAX);

    /// Interpret a Highlight with the RGB foreground
    fn decode_rgb_highlight(highlight: Highlight) -> Option<(u8, u8, u8)> {
        (highlight.get() > Self::RGB_START).then(|| {
            let [b, g, r, ..] = (highlight.get() + 1).to_le_bytes();
            (r, g, b)
        })
    }

    /// Create a Highlight that represents an RGB color
    pub fn rgb_highlight(r: u8, g: u8, b: u8) -> Highlight {
        // -1 because highlight is "non-max": u32::MAX is reserved for the null pointer
        // optimization.
        Highlight::new(u32::from_le_bytes([b, g, r, u8::MAX]) - 1)
    }

    #[inline]
    pub fn highlight(&self, highlight: Highlight) -> Style {
        if let Some((red, green, blue)) = Self::decode_rgb_highlight(highlight) {
            Style::new().fg(Color::Rgb(red, green, blue))
        } else {
            self.highlights[highlight.idx()]
        }
    }

    #[inline]
    pub fn scope(&self, highlight: Highlight) -> &str {
        &self.scopes[highlight.idx()]
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn get(&self, scope: &str) -> Style {
        self.try_get(scope).unwrap_or_default()
    }

    /// Get the style of a scope, falling back to dot separated broader
    /// scopes. For example if `ui.text.focus` is not defined in the theme,
    /// `ui.text` is tried and then `ui` is tried.
    pub fn try_get(&self, scope: &str) -> Option<Style> {
        std::iter::successors(Some(scope), |s| Some(s.rsplit_once('.')?.0))
            .find_map(|s| self.styles.get(s).copied())
    }

    /// Get the style of a scope, without falling back to dot separated broader
    /// scopes. For example if `ui.text.focus` is not defined in the theme, it
    /// will return `None`, even if `ui.text` is.
    pub fn try_get_exact(&self, scope: &str) -> Option<Style> {
        self.styles.get(scope).copied()
    }

    #[inline]
    pub fn scopes(&self) -> &[String] {
        &self.scopes
    }

    pub fn find_highlight_exact(&self, scope: &str) -> Option<Highlight> {
        self.scopes()
            .iter()
            .position(|s| s == scope)
            .map(|idx| Highlight::new(idx as u32))
    }

    pub fn find_highlight(&self, mut scope: &str) -> Option<Highlight> {
        loop {
            if let Some(highlight) = self.find_highlight_exact(scope) {
                return Some(highlight);
            }
            if let Some(new_end) = scope.rfind('.') {
                scope = &scope[..new_end];
            } else {
                return None;
            }
        }
    }

    pub fn is_16_color(&self) -> bool {
        self.styles.iter().all(|(_, style)| {
            [style.fg, style.bg]
                .into_iter()
                .all(|color| !matches!(color, Some(Color::Rgb(..))))
        })
    }

    pub fn rainbow_length(&self) -> usize {
        self.rainbow_length
    }

    fn from_toml(value: Value) -> (Self, Vec<String>) {
        if let Value::Table(table) = value {
            Theme::from_keys(table)
        } else {
            warn!("Expected theme TOML value to be a table, found {:?}", value);
            Default::default()
        }
    }

    fn from_keys(toml_keys: Map<String, Value>) -> (Self, Vec<String>) {
        let (styles, scopes, highlights, rainbow_length, load_errors) =
            build_theme_values(toml_keys);

        let theme = Self {
            styles,
            scopes,
            highlights,
            rainbow_length,
            ..Default::default()
        };
        (theme, load_errors)
    }
}

struct ThemePalette {
    palette: HashMap<String, Color>,
}

impl Default for ThemePalette {
    fn default() -> Self {
        let mut palette = HashMap::new();
        palette.insert("default".to_string(), Color::Reset);
        palette.insert("black".to_string(), Color::Black);
        palette.insert("red".to_string(), Color::Red);
        palette.insert("green".to_string(), Color::Green);
        palette.insert("yellow".to_string(), Color::Yellow);
        palette.insert("blue".to_string(), Color::Blue);
        palette.insert("magenta".to_string(), Color::Magenta);
        palette.insert("cyan".to_string(), Color::Cyan);
        palette.insert("gray".to_string(), Color::Gray);
        palette.insert("light-red".to_string(), Color::LightRed);
        palette.insert("light-green".to_string(), Color::LightGreen);
        palette.insert("light-yellow".to_string(), Color::LightYellow);
        palette.insert("light-blue".to_string(), Color::LightBlue);
        palette.insert("light-magenta".to_string(), Color::LightMagenta);
        palette.insert("light-cyan".to_string(), Color::LightCyan);
        palette.insert("light-gray".to_string(), Color::LightGray);
        palette.insert("white".to_string(), Color::White);
        Self {
            palette,
        }
    }
}

impl ThemePalette {
    pub fn new(palette: HashMap<String, Color>) -> Self {
        let ThemePalette {
            palette: mut default,
        } = ThemePalette::default();

        default.extend(palette);
        Self { palette: default }
    }

    pub fn string_to_rgb(s: &str) -> Result<Color, String> {
        if s.starts_with('#') {
            Self::hex_string_to_rgb(s)
        } else {
            Self::ansi_string_to_rgb(s)
        }
    }

    fn ansi_string_to_rgb(s: &str) -> Result<Color, String> {
        if let Ok(index) = s.parse::<u8>() {
            return Ok(Color::Indexed(index));
        }
        Err(format!("Malformed ANSI: {}", s))
    }

    fn hex_string_to_rgb(s: &str) -> Result<Color, String> {
        if s.len() >= 7 {
            if let (Ok(red), Ok(green), Ok(blue)) = (
                u8::from_str_radix(&s[1..3], 16),
                u8::from_str_radix(&s[3..5], 16),
                u8::from_str_radix(&s[5..7], 16),
            ) {
                return Ok(Color::Rgb(red, green, blue));
            }
        }

        Err(format!("Malformed hexcode: {}", s))
    }

    fn parse_value_as_str(value: &Value) -> Result<&str, String> {
        value
            .as_str()
            .ok_or(format!("Unrecognized value: {}", value))
    }

    pub fn parse_color(&self, value: Value) -> Result<Color, String> {
        let value = Self::parse_value_as_str(&value)?;

        self.palette
            .get(value)
            .copied()
            .ok_or("")
            .or_else(|_| Self::string_to_rgb(value))
    }

    pub fn parse_modifier(value: &Value) -> Result<Modifier, String> {
        value
            .as_str()
            .and_then(|s| s.parse().ok())
            .ok_or(format!("Invalid modifier: {}", value))
    }

    pub fn parse_underline_style(value: &Value) -> Result<UnderlineStyle, String> {
        value
            .as_str()
            .and_then(|s| s.parse().ok())
            .ok_or(format!("Invalid underline style: {}", value))
    }

    pub fn parse_style(&self, style: &mut Style, value: Value) -> Result<(), String> {
        if let Value::Table(entries) = value {
            for (name, mut value) in entries {
                match name.as_str() {
                    "fg" => *style = style.fg(self.parse_color(value)?),
                    "bg" => *style = style.bg(self.parse_color(value)?),
                    "underline" => {
                        let table = value.as_table_mut().ok_or("Underline must be table")?;
                        if let Some(value) = table.remove("color") {
                            *style = style.underline_color(self.parse_color(value)?);
                        }
                        if let Some(value) = table.remove("style") {
                            *style = style.underline_style(Self::parse_underline_style(&value)?);
                        }

                        if let Some(attr) = table.keys().next() {
                            return Err(format!("Invalid underline attribute: {attr}"));
                        }
                    }
                    "modifiers" => {
                        let modifiers = value.as_array().ok_or("Modifiers should be an array")?;

                        for modifier in modifiers {
                            if modifier.as_str() == Some("underlined") {
                                *style = style.underline_style(UnderlineStyle::Line);
                            } else {
                                *style = style.add_modifier(Self::parse_modifier(modifier)?);
                            }
                        }
                    }
                    _ => return Err(format!("Invalid style attribute: {}", name)),
                }
            }
        } else {
            *style = style.fg(self.parse_color(value)?);
        }
        Ok(())
    }

    fn parse_style_array(&self, value: Value) -> Result<Vec<Style>, String> {
        let mut styles = Vec::new();

        for v in value
            .as_array()
            .ok_or_else(|| format!("Could not parse value as an array: '{value}'"))?
        {
            let mut style = Style::default();
            self.parse_style(&mut style, v.clone())?;
            styles.push(style);
        }

        Ok(styles)
    }
}

impl TryFrom<Value> for ThemePalette {
    type Error = String;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        let map = match value {
            Value::Table(entries) => entries,
            _ => return Ok(Self::default()),
        };

        let mut palette = HashMap::with_capacity(map.len());
        for (name, value) in map {
            let value = Self::parse_value_as_str(&value)?;
            let color = Self::string_to_rgb(value)?;
            palette.insert(name, color);
        }

        Ok(Self::new(palette))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_style_string() {
        let fg = Value::String("#ffffff".to_string());

        let mut style = Style::default();
        let palette = ThemePalette::default();
        palette.parse_style(&mut style, fg).unwrap();

        assert_eq!(style, Style::default().fg(Color::Rgb(255, 255, 255)));
    }

    #[test]
    fn test_palette() {
        let fg = Value::String("my_color".to_string());

        let mut style = Style::default();
        let palette = ThemePalette::new(HashMap::from([(
            "my_color".to_string(),
            Color::Rgb(255, 255, 255),
        )]));
        palette.parse_style(&mut style, fg).unwrap();

        assert_eq!(style, Style::default().fg(Color::Rgb(255, 255, 255)));
    }

    #[test]
    fn test_parse_style_table() {
        let table = toml::toml! {
            "keyword" = {
                fg = "#ffffff",
                bg = "#000000",
                modifiers = ["bold"],
            }
        };

        let mut style = Style::default();
        let palette = ThemePalette::default();
        for (_name, value) in table {
            palette.parse_style(&mut style, value).unwrap();
        }

        assert_eq!(
            style,
            Style::default()
                .fg(Color::Rgb(255, 255, 255))
                .bg(Color::Rgb(0, 0, 0))
                .add_modifier(Modifier::BOLD)
        );
    }

    // tests for parsing an RGB `Highlight`

    #[test]
    fn convert_to_and_from() {
        let (r, g, b) = (0xFF, 0xFE, 0xFA);
        let highlight = Theme::rgb_highlight(r, g, b);
        assert_eq!(Theme::decode_rgb_highlight(highlight), Some((r, g, b)));
    }

    /// make sure we can store all the colors at the end
    #[test]
    fn full_numeric_range() {
        assert_eq!(Highlight::MAX - Theme::RGB_START, 256_u32.pow(3));
    }

    #[test]
    fn retrieve_color() {
        // color in the middle
        let (r, g, b) = (0x14, 0xAA, 0xF7);
        assert_eq!(
            Theme::default().highlight(Theme::rgb_highlight(r, g, b)),
            Style::new().fg(Color::Rgb(r, g, b))
        );
        // pure black
        let (r, g, b) = (0x00, 0x00, 0x00);
        assert_eq!(
            Theme::default().highlight(Theme::rgb_highlight(r, g, b)),
            Style::new().fg(Color::Rgb(r, g, b))
        );
        // pure white
        let (r, g, b) = (0xff, 0xff, 0xff);
        assert_eq!(
            Theme::default().highlight(Theme::rgb_highlight(r, g, b)),
            Style::new().fg(Color::Rgb(r, g, b))
        );
    }

    #[test]
    #[should_panic(expected = "index out of bounds: the len is 0 but the index is 4278190078")]
    fn out_of_bounds() {
        let highlight = Highlight::new(Theme::rgb_highlight(0, 0, 0).get() - 1);
        Theme::default().highlight(highlight);
    }
}
