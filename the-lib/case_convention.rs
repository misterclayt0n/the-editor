use crate::Tendril;

pub fn to_pascal_case(text: impl Iterator<Item = char>) -> Tendril {
  let mut res = Tendril::new();
  to_pascal_case_with(text, &mut res);
  res
}

pub fn to_pascal_case_with(text: impl Iterator<Item = char>, buf: &mut Tendril) {
  text.fold(true, |word_start, c| {
    if !c.is_alphanumeric() {
      return true;
    }
    if word_start {
      buf.extend(c.to_uppercase());
    } else {
      buf.extend(c.to_lowercase());
    }
    false
  });
}

pub fn to_camel_case(text: impl Iterator<Item = char>) -> Tendril {
  let mut res = Tendril::new();
  to_camel_case_with(text, &mut res);
  res
}

pub fn to_camel_case_with(text: impl Iterator<Item = char>, buf: &mut Tendril) {
  text.fold((true, true), |(first_word, word_start), c| {
    if !c.is_alphanumeric() {
      return (first_word && word_start, true);
    }
    if first_word || !word_start {
      buf.extend(c.to_lowercase());
    } else {
      buf.extend(c.to_uppercase());
    }
    (false, false)
  });
}

pub fn to_upper_case(text: impl Iterator<Item = char>) -> Tendril {
  let mut res = Tendril::new();
  to_upper_case_with(text, &mut res);
  res
}

pub fn to_upper_case_with(text: impl Iterator<Item = char>, buf: &mut Tendril) {
  text.for_each(|c| buf.extend(c.to_uppercase()));
}

pub fn to_lower_case(text: impl Iterator<Item = char>) -> Tendril {
  let mut res = Tendril::new();
  to_lower_case_with(text, &mut res);
  res
}

pub fn to_lower_case_with(text: impl Iterator<Item = char>, buf: &mut Tendril) {
  text.for_each(|c| buf.extend(c.to_lowercase()));
}

pub fn to_snake_case(text: impl Iterator<Item = char>) -> Tendril {
  let mut res = Tendril::new();
  to_snake_case_with(text, &mut res);
  res
}

pub fn to_snake_case_with(text: impl Iterator<Item = char>, buf: &mut Tendril) {
  to_separated_case_with(text, buf, '_');
}

pub fn to_kebab_case(text: impl Iterator<Item = char>) -> Tendril {
  let mut res = Tendril::new();
  to_kebab_case_with(text, &mut res);
  res
}

pub fn to_kebab_case_with(text: impl Iterator<Item = char>, buf: &mut Tendril) {
  to_separated_case_with(text, buf, '-');
}

/// Shared implementation for snake_case and kebab-case.
/// Handles:
/// - Word boundaries from non-alphanumeric separators (_, -, space, etc.)
/// - Word boundaries from camelCase transitions (lowercase -> uppercase)
/// - Consecutive uppercase runs (e.g., "HTTPServer" -> "http_server")
fn to_separated_case_with(text: impl Iterator<Item = char>, buf: &mut Tendril, sep: char) {
  // State: (has_content, prev_was_upper, prev_was_separator)
  text.fold(
    (false, false, false),
    |(has_content, prev_was_upper, prev_was_sep), c| {
      if !c.is_alphanumeric() {
        // Mark that we need a separator before the next alphanumeric char
        return (has_content, false, has_content);
      }

      let is_upper = c.is_uppercase();

      // Insert separator if:
      // 1. We had a pending separator from non-alphanumeric chars, OR
      // 2. Transitioning from lowercase to uppercase (camelCase boundary), OR
      // 3. Current is lowercase after uppercase run (e.g., "HTTPServer" ->
      //    ...http_Server) but only if we're not at the start
      if has_content && !prev_was_sep {
        if is_upper && !prev_was_upper {
          // lowercase -> uppercase transition
          buf.push(sep);
        }
      } else if prev_was_sep {
        buf.push(sep);
      }

      buf.extend(c.to_lowercase());
      (true, is_upper, false)
    },
  );
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_to_pascal_case() {
    assert_eq!(to_pascal_case("hello_world".chars()).as_str(), "HelloWorld");
    assert_eq!(to_pascal_case("HELLO_WORLD".chars()).as_str(), "HelloWorld");
    assert_eq!(to_pascal_case("hello-world".chars()).as_str(), "HelloWorld");
    assert_eq!(to_pascal_case("helloWorld".chars()).as_str(), "Helloworld");
    assert_eq!(to_pascal_case("__leading__".chars()).as_str(), "Leading");
    assert_eq!(to_pascal_case("".chars()).as_str(), "");
    assert_eq!(to_pascal_case("a".chars()).as_str(), "A");
  }

  #[test]
  fn test_to_camel_case() {
    assert_eq!(to_camel_case("hello_world".chars()).as_str(), "helloWorld");
    assert_eq!(to_camel_case("HELLO_WORLD".chars()).as_str(), "helloWorld");
    assert_eq!(to_camel_case("hello-world".chars()).as_str(), "helloWorld");
    assert_eq!(to_camel_case("HelloWorld".chars()).as_str(), "helloworld");
    assert_eq!(to_camel_case("__leading__".chars()).as_str(), "leading");
    assert_eq!(to_camel_case("".chars()).as_str(), "");
    assert_eq!(to_camel_case("A".chars()).as_str(), "a");
  }

  #[test]
  fn test_to_upper_case() {
    assert_eq!(to_upper_case("hello".chars()).as_str(), "HELLO");
    assert_eq!(to_upper_case("Hello World".chars()).as_str(), "HELLO WORLD");
    assert_eq!(to_upper_case("ALREADY".chars()).as_str(), "ALREADY");
    assert_eq!(to_upper_case("".chars()).as_str(), "");
    assert_eq!(to_upper_case("café".chars()).as_str(), "CAFÉ");
  }

  #[test]
  fn test_to_lower_case() {
    assert_eq!(to_lower_case("HELLO".chars()).as_str(), "hello");
    assert_eq!(to_lower_case("Hello World".chars()).as_str(), "hello world");
    assert_eq!(to_lower_case("already".chars()).as_str(), "already");
    assert_eq!(to_lower_case("".chars()).as_str(), "");
    assert_eq!(to_lower_case("CAFÉ".chars()).as_str(), "café");
  }

  #[test]
  fn test_to_snake_case() {
    // From various input formats
    assert_eq!(to_snake_case("helloWorld".chars()).as_str(), "hello_world");
    assert_eq!(to_snake_case("HelloWorld".chars()).as_str(), "hello_world");
    assert_eq!(to_snake_case("hello-world".chars()).as_str(), "hello_world");
    assert_eq!(to_snake_case("hello world".chars()).as_str(), "hello_world");
    assert_eq!(to_snake_case("HELLO_WORLD".chars()).as_str(), "hello_world");

    // Acronyms and consecutive uppercase
    assert_eq!(to_snake_case("HTTPServer".chars()).as_str(), "httpserver");
    assert_eq!(
      to_snake_case("getHTTPResponse".chars()).as_str(),
      "get_httpresponse"
    );

    // Edge cases
    assert_eq!(to_snake_case("".chars()).as_str(), "");
    assert_eq!(to_snake_case("a".chars()).as_str(), "a");
    assert_eq!(to_snake_case("A".chars()).as_str(), "a");
    assert_eq!(
      to_snake_case("__leading__trailing__".chars()).as_str(),
      "leading_trailing"
    );
    assert_eq!(
      to_snake_case("already_snake".chars()).as_str(),
      "already_snake"
    );
  }

  #[test]
  fn test_to_kebab_case() {
    // From various input formats
    assert_eq!(to_kebab_case("helloWorld".chars()).as_str(), "hello-world");
    assert_eq!(to_kebab_case("HelloWorld".chars()).as_str(), "hello-world");
    assert_eq!(to_kebab_case("hello_world".chars()).as_str(), "hello-world");
    assert_eq!(to_kebab_case("hello world".chars()).as_str(), "hello-world");
    assert_eq!(to_kebab_case("HELLO-WORLD".chars()).as_str(), "hello-world");

    // Acronyms and consecutive uppercase
    assert_eq!(to_kebab_case("HTTPServer".chars()).as_str(), "httpserver");
    assert_eq!(
      to_kebab_case("getHTTPResponse".chars()).as_str(),
      "get-httpresponse"
    );

    // Edge cases
    assert_eq!(to_kebab_case("".chars()).as_str(), "");
    assert_eq!(to_kebab_case("a".chars()).as_str(), "a");
    assert_eq!(to_kebab_case("A".chars()).as_str(), "a");
    assert_eq!(
      to_kebab_case("--leading--trailing--".chars()).as_str(),
      "leading-trailing"
    );
    assert_eq!(
      to_kebab_case("already-kebab".chars()).as_str(),
      "already-kebab"
    );
  }
}
