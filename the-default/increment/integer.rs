const SEPARATOR: char = '_';

pub fn increment(selected_text: &str, amount: i64) -> Option<String> {
  if selected_text.is_empty()
    || selected_text.ends_with(SEPARATOR)
    || selected_text.starts_with(SEPARATOR)
  {
    return None;
  }

  let radix = if selected_text.starts_with("0x") {
    16
  } else if selected_text.starts_with("0o") {
    8
  } else if selected_text.starts_with("0b") {
    2
  } else {
    10
  };

  let separator_rtl_indexes: Vec<usize> = selected_text
    .chars()
    .rev()
    .enumerate()
    .filter_map(|(i, c)| if c == SEPARATOR { Some(i) } else { None })
    .collect();

  let word: String = selected_text.chars().filter(|&c| c != SEPARATOR).collect();

  let mut new_text = if radix == 10 {
    let number = &word;
    let value = i128::from_str_radix(number, radix).ok()?;
    let new_value = value.saturating_add(amount as i128);

    let format_length = match (value.is_negative(), new_value.is_negative()) {
      (true, false) => number.len() - 1,
      (false, true) => number.len() + 1,
      _ => number.len(),
    } - separator_rtl_indexes.len();

    if number.starts_with('0') || number.starts_with("-0") {
      format!("{:01$}", new_value, format_length)
    } else {
      format!("{new_value}")
    }
  } else {
    let number = &word[2..];
    let value = u128::from_str_radix(number, radix).ok()?;
    let new_value = (value as i128).saturating_add(amount as i128);
    let new_value = if new_value < 0 { 0 } else { new_value };
    let format_length = selected_text.len() - 2 - separator_rtl_indexes.len();

    match radix {
      2 => format!("0b{:01$b}", new_value, format_length),
      8 => format!("0o{:01$o}", new_value, format_length),
      16 => {
        let (lower_count, upper_count): (usize, usize) =
          number.chars().fold((0, 0), |(lower, upper), c| {
            (
              lower + c.is_ascii_lowercase() as usize,
              upper + c.is_ascii_uppercase() as usize,
            )
          });
        if upper_count > lower_count {
          format!("0x{:01$X}", new_value, format_length)
        } else {
          format!("0x{:01$x}", new_value, format_length)
        }
      },
      _ => return None,
    }
  };

  for &rtl_index in &separator_rtl_indexes {
    if rtl_index < new_text.len() {
      let new_index = new_text.len().saturating_sub(rtl_index);
      if new_index > 0 {
        new_text.insert(new_index, SEPARATOR);
      }
    }
  }

  if new_text.len() > selected_text.len() && !separator_rtl_indexes.is_empty() {
    let spacing = match separator_rtl_indexes.as_slice() {
      [.., b, a] => a - b - 1,
      _ => separator_rtl_indexes[0],
    };

    let prefix_length = if radix == 10 { 0 } else { 2 };
    if let Some(mut index) = new_text.find(SEPARATOR) {
      while index - prefix_length > spacing {
        index -= spacing;
        new_text.insert(index, SEPARATOR);
      }
    }
  }

  Some(new_text)
}
