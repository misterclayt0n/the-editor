use std::{
  fmt::{
    self,
    Display,
  },
  num::NonZeroU8,
};

use globset::Glob;
use serde::{
  Deserialize,
  Serialize,
};
use the_stdx::rope;

use crate::auto_pairs::{
  AutoPairs,
  AutoPairsError,
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SyntaxConfiguration {
  pub language: Vec<LanguageSyntaxConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct LanguageSyntaxConfig {
  #[serde(rename = "name")]
  pub language_id:          String,
  pub scope:                String,
  pub file_types:           Vec<FileType>,
  #[serde(default)]
  pub shebangs:             Vec<String>,
  #[serde(
    default,
    skip_serializing,
    deserialize_with = "from_comment_tokens",
    alias = "comment-token"
  )]
  pub comment_tokens:       Option<Vec<String>>,
  #[serde(
    default,
    skip_serializing,
    deserialize_with = "from_block_comment_tokens"
  )]
  pub block_comment_tokens: Option<Vec<BlockCommentToken>>,
  pub text_width:           Option<usize>,
  pub soft_wrap:            Option<SoftWrap>,
  pub grammar:              Option<String>,
  #[serde(default, skip_serializing, deserialize_with = "deserialize_regex")]
  pub injection_regex:      Option<rope::Regex>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub indent:               Option<IndentationConfiguration>,
  #[serde(default, skip_serializing, deserialize_with = "deserialize_auto_pairs")]
  pub auto_pairs:           Option<AutoPairs>,
  pub rulers:               Option<Vec<u16>>,
  pub rainbow_brackets:     Option<bool>,
}

impl LanguageSyntaxConfig {
  pub fn grammar_name(&self) -> &str {
    self.grammar.as_deref().unwrap_or(&self.language_id)
  }
}

#[derive(Debug)]
pub enum FileType {
  Extension(String),
  Glob(Glob),
}

impl Serialize for FileType {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    use serde::ser::SerializeMap;

    match self {
      FileType::Extension(extension) => serializer.serialize_str(extension),
      FileType::Glob(glob) => {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry("glob", glob.glob())?;
        map.end()
      },
    }
  }
}

impl<'de> Deserialize<'de> for FileType {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::de::Deserializer<'de>,
  {
    struct FileTypeVisitor;

    impl<'de> serde::de::Visitor<'de> for FileTypeVisitor {
      type Value = FileType;

      fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("string or table")
      }

      fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
      where
        E: serde::de::Error,
      {
        Ok(FileType::Extension(value.to_string()))
      }

      fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
      where
        M: serde::de::MapAccess<'de>,
      {
        match map.next_entry::<String, String>()? {
          Some((key, mut glob)) if key == "glob" => {
            if !glob.starts_with('/') && !glob.starts_with("*/") {
              glob.insert_str(0, "*/");
            }
            Glob::new(glob.as_str())
              .map(FileType::Glob)
              .map_err(|err| serde::de::Error::custom(format!("invalid `glob` pattern: {err}")))
          },
          Some((key, _)) => {
            Err(serde::de::Error::custom(format!(
              "unknown key in `file-types` list: {key}"
            )))
          },
          None => {
            Err(serde::de::Error::custom(
              "expected a `glob` key in the `file-types` entry",
            ))
          },
        }
      }
    }

    deserializer.deserialize_any(FileTypeVisitor)
  }
}

fn from_comment_tokens<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
  D: serde::Deserializer<'de>,
{
  #[derive(Deserialize)]
  #[serde(untagged)]
  enum CommentTokens {
    Multiple(Vec<String>),
    Single(String),
  }

  Ok(
    Option::<CommentTokens>::deserialize(deserializer)?.map(|tokens| {
      match tokens {
        CommentTokens::Single(val) => vec![val],
        CommentTokens::Multiple(vals) => vals,
      }
    }),
  )
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockCommentToken {
  pub start: String,
  pub end:   String,
}

impl Default for BlockCommentToken {
  fn default() -> Self {
    Self {
      start: "/*".to_string(),
      end:   "*/".to_string(),
    }
  }
}

fn from_block_comment_tokens<'de, D>(
  deserializer: D,
) -> Result<Option<Vec<BlockCommentToken>>, D::Error>
where
  D: serde::Deserializer<'de>,
{
  #[derive(Deserialize)]
  #[serde(untagged)]
  enum BlockCommentTokens {
    Multiple(Vec<BlockCommentToken>),
    Single(BlockCommentToken),
  }

  Ok(
    Option::<BlockCommentTokens>::deserialize(deserializer)?.map(|tokens| {
      match tokens {
        BlockCommentTokens::Single(val) => vec![val],
        BlockCommentTokens::Multiple(vals) => vals,
      }
    }),
  )
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct IndentationConfiguration {
  #[serde(deserialize_with = "deserialize_tab_width")]
  pub tab_width: usize,
  pub unit:      String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IndentationHeuristic {
  Simple,
  TreeSitter,
  #[default]
  Hybrid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields, untagged)]
enum AutoPairConfig {
  Enable(bool),
  Pairs(std::collections::HashMap<char, char>),
}

impl Default for AutoPairConfig {
  fn default() -> Self {
    Self::Enable(true)
  }
}

impl FromStr for AutoPairConfig {
  type Err = std::str::ParseBoolError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    let enable: bool = s.parse()?;
    Ok(Self::Enable(enable))
  }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub struct SoftWrap {
  pub enable:             Option<bool>,
  pub max_wrap:           Option<u16>,
  pub max_indent_retain:  Option<u16>,
  pub wrap_indicator:     Option<String>,
  pub wrap_at_text_width: Option<bool>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub struct WordCompletion {
  pub enable:         Option<bool>,
  pub trigger_length: Option<NonZeroU8>,
}

fn deserialize_regex<'de, D>(deserializer: D) -> Result<Option<rope::Regex>, D::Error>
where
  D: serde::Deserializer<'de>,
{
  Option::<String>::deserialize(deserializer)?
    .map(|buf| rope::Regex::new(&buf).map_err(serde::de::Error::custom))
    .transpose()
}

fn deserialize_tab_width<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
  D: serde::Deserializer<'de>,
{
  usize::deserialize(deserializer).and_then(|n| {
    if (1..=16).contains(&n) {
      Ok(n)
    } else {
      Err(serde::de::Error::custom(
        "tab width must be a value from 1 to 16 inclusive",
      ))
    }
  })
}

pub fn deserialize_auto_pairs<'de, D>(deserializer: D) -> Result<Option<AutoPairs>, D::Error>
where
  D: serde::Deserializer<'de>,
{
  let Some(config) = Option::<AutoPairConfig>::deserialize(deserializer)? else {
    return Ok(None);
  };

  let pairs = match config {
    AutoPairConfig::Enable(false) => None,
    AutoPairConfig::Enable(true) => Some(AutoPairs::default()),
    AutoPairConfig::Pairs(pairs) => {
      Some(
        AutoPairs::new(pairs.into_iter())
          .map_err(|err: AutoPairsError| serde::de::Error::custom(err.to_string()))?,
      )
    },
  };

  Ok(pairs)
}

use std::str::FromStr;

impl Display for FileType {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      FileType::Extension(ext) => write!(f, "{ext}"),
      FileType::Glob(glob) => write!(f, "{}", glob.glob()),
    }
  }
}

#[cfg(test)]
mod test {
  use super::*;

  fn parse_language(src: &str) -> LanguageSyntaxConfig {
    toml::from_str::<LanguageSyntaxConfig>(src).unwrap()
  }

  #[test]
  fn grammar_name_falls_back_to_language_id() {
    let config = LanguageSyntaxConfig {
      language_id:          "rust".into(),
      scope:                "source.rust".into(),
      file_types:           vec![FileType::Extension("rs".into())],
      shebangs:             Vec::new(),
      comment_tokens:       None,
      block_comment_tokens: None,
      text_width:           None,
      soft_wrap:            None,
      grammar:              None,
      injection_regex:      None,
      indent:               None,
      auto_pairs:           None,
      rulers:               None,
      rainbow_brackets:     None,
    };

    assert_eq!(config.grammar_name(), "rust");
  }

  #[test]
  fn file_type_glob_deserialization_normalizes_relative_patterns() {
    let config = parse_language(
      r#"
name = "rust"
scope = "source.rust"
file-types = [{ glob = "Cargo.*" }]
"#,
    );

    assert!(matches!(&config.file_types[0], FileType::Glob(glob) if glob.glob() == "*/Cargo.*"));
  }

  #[test]
  fn comment_and_block_comment_tokens_accept_single_or_multiple_values() {
    let single = parse_language(
      r#"
name = "rust"
scope = "source.rust"
file-types = ["rs"]
comment-token = "//"
block-comment-tokens = { start = "/*", end = "*/" }
"#,
    );
    assert_eq!(
      single.comment_tokens.as_deref(),
      Some(&["//".to_string()][..])
    );
    assert_eq!(single.block_comment_tokens.as_ref().unwrap()[0].start, "/*");

    let multiple = parse_language(
      r##"
name = "shell"
scope = "source.shell"
file-types = ["sh"]
comment-token = ["#", "//"]
block-comment-tokens = [
  { start = "/*", end = "*/" },
  { start = "<!--", end = "-->" },
]
"##,
    );
    assert_eq!(
      multiple.comment_tokens,
      Some(vec!["#".to_string(), "//".to_string()])
    );
    assert_eq!(multiple.block_comment_tokens.as_ref().unwrap().len(), 2);
  }

  #[test]
  fn auto_pairs_deserialization_supports_default_disabled_and_custom_pairs() {
    let default_pairs = parse_language(
      r#"
name = "rust"
scope = "source.rust"
file-types = ["rs"]
auto-pairs = true
"#,
    );
    assert!(default_pairs.auto_pairs.is_some());

    let disabled = parse_language(
      r#"
name = "text"
scope = "text.plain"
file-types = ["txt"]
auto-pairs = false
"#,
    );
    assert!(disabled.auto_pairs.is_none());

    let custom = parse_language(
      r#"
name = "templ"
scope = "source.templ"
file-types = ["templ"]
auto-pairs = { "<" = ">", "{" = "}" }
"#,
    );
    let pairs = custom.auto_pairs.unwrap();
    assert!(pairs.matches_char('<'));
    assert!(pairs.matches_char('>'));
    assert!(pairs.matches_char('{'));
    assert!(pairs.matches_char('}'));
  }

  #[test]
  fn tab_width_must_be_between_one_and_sixteen() {
    let err = toml::from_str::<IndentationConfiguration>(
      r#"
tab-width = 0
unit = "  "
"#,
    )
    .unwrap_err();
    assert!(
      err
        .to_string()
        .contains("tab width must be a value from 1 to 16 inclusive")
    );

    let parsed = toml::from_str::<IndentationConfiguration>(
      r#"
tab-width = 4
unit = "    "
"#,
    )
    .unwrap();
    assert_eq!(parsed.tab_width, 4);
  }
}
