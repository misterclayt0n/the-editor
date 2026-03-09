use std::{
  fmt::Write,
  sync::LazyLock,
};

use chrono::{
  Duration,
  NaiveDate,
  NaiveDateTime,
  NaiveTime,
};
use regex::Regex;

pub fn increment(selected_text: &str, amount: i64) -> Option<String> {
  if selected_text.is_empty() {
    return None;
  }

  FORMATS.iter().find_map(|format| {
    let captures = format.regex.captures(selected_text)?;
    if captures.len() - 1 != format.fields.len() {
      return None;
    }

    let date_time = captures.get(0)?;
    let has_date = format.fields.iter().any(|f| f.unit.is_date());
    let has_time = format.fields.iter().any(|f| f.unit.is_time());
    let date_time = &selected_text[date_time.start()..date_time.end()];

    match (has_date, has_time) {
      (true, true) => {
        let date_time = NaiveDateTime::parse_from_str(date_time, format.fmt).ok()?;
        Some(
          date_time
            .checked_add_signed(Duration::try_minutes(amount)?)?
            .format(format.fmt)
            .to_string(),
        )
      },
      (true, false) => {
        let date = NaiveDate::parse_from_str(date_time, format.fmt).ok()?;
        Some(
          date
            .checked_add_signed(Duration::try_days(amount)?)?
            .format(format.fmt)
            .to_string(),
        )
      },
      (false, true) => {
        let time = NaiveTime::parse_from_str(date_time, format.fmt).ok()?;
        let (adjusted_time, _) = time.overflowing_add_signed(Duration::try_minutes(amount)?);
        Some(adjusted_time.format(format.fmt).to_string())
      },
      (false, false) => None,
    }
  })
}

static FORMATS: LazyLock<Vec<Format>> = LazyLock::new(|| {
  vec![
    Format::new("%Y-%m-%d %H:%M:%S"),
    Format::new("%Y/%m/%d %H:%M:%S"),
    Format::new("%Y-%m-%d %H:%M"),
    Format::new("%Y/%m/%d %H:%M"),
    Format::new("%Y-%m-%d"),
    Format::new("%Y/%m/%d"),
    Format::new("%a %b %d %Y"),
    Format::new("%d-%b-%Y"),
    Format::new("%Y %b %d"),
    Format::new("%b %d, %Y"),
    Format::new("%-I:%M:%S %P"),
    Format::new("%-I:%M %P"),
    Format::new("%-I:%M:%S %p"),
    Format::new("%-I:%M %p"),
    Format::new("%H:%M:%S"),
    Format::new("%H:%M"),
  ]
});

#[derive(Debug)]
struct Format {
  fmt:     &'static str,
  fields:  Vec<DateField>,
  regex:   Regex,
  max_len: usize,
}

impl Format {
  fn new(fmt: &'static str) -> Self {
    let mut remaining = fmt;
    let mut fields = Vec::new();
    let mut regex = "^".to_string();
    let mut max_len = 0;

    while let Some(i) = remaining.find('%') {
      let after = &remaining[i + 1..];
      let mut chars = after.chars();
      let c = chars.next().expect("format specifier must exist");

      let spec_len = if c == '-' {
        1 + chars
          .next()
          .expect("format specifier must exist")
          .len_utf8()
      } else {
        c.len_utf8()
      };

      let specifier = &after[..spec_len];
      let field = DateField::from_specifier(specifier).expect("unknown date format specifier");
      fields.push(field);
      max_len += field.max_len + remaining[..i].len();
      regex += &remaining[..i];
      write!(regex, "({})", field.regex).expect("formatting regex");
      remaining = &after[spec_len..];
    }
    regex += "$";

    let regex = Regex::new(&regex).expect("date-time regex must compile");

    Self {
      fmt,
      fields,
      regex,
      max_len,
    }
  }
}

impl PartialEq for Format {
  fn eq(&self, other: &Self) -> bool {
    self.fmt == other.fmt && self.fields == other.fields && self.max_len == other.max_len
  }
}

impl Eq for Format {}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct DateField {
  regex:   &'static str,
  unit:    DateUnit,
  max_len: usize,
}

impl DateField {
  fn from_specifier(specifier: &str) -> Option<Self> {
    match specifier {
      "Y" => {
        Some(Self {
          regex:   r"\d{4}",
          unit:    DateUnit::Years,
          max_len: 5,
        })
      },
      "y" => {
        Some(Self {
          regex:   r"\d\d",
          unit:    DateUnit::Years,
          max_len: 2,
        })
      },
      "m" => {
        Some(Self {
          regex:   r"[0-1]\d",
          unit:    DateUnit::Months,
          max_len: 2,
        })
      },
      "d" => {
        Some(Self {
          regex:   r"[0-3]\d",
          unit:    DateUnit::Days,
          max_len: 2,
        })
      },
      "-d" => {
        Some(Self {
          regex:   r"[1-3]?\d",
          unit:    DateUnit::Days,
          max_len: 2,
        })
      },
      "a" => {
        Some(Self {
          regex:   r"Sun|Mon|Tue|Wed|Thu|Fri|Sat",
          unit:    DateUnit::Days,
          max_len: 3,
        })
      },
      "A" => {
        Some(Self {
          regex:   r"Sunday|Monday|Tuesday|Wednesday|Thursday|Friday|Saturday",
          unit:    DateUnit::Days,
          max_len: 9,
        })
      },
      "b" | "h" => {
        Some(Self {
          regex:   r"Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec",
          unit:    DateUnit::Months,
          max_len: 3,
        })
      },
      "B" => {
        Some(Self {
          regex:   r"January|February|March|April|May|June|July|August|September|October|November|December",
          unit:    DateUnit::Months,
          max_len: 9,
        })
      },
      "H" => {
        Some(Self {
          regex:   r"[0-2]\d",
          unit:    DateUnit::Hours,
          max_len: 2,
        })
      },
      "M" => {
        Some(Self {
          regex:   r"[0-5]\d",
          unit:    DateUnit::Minutes,
          max_len: 2,
        })
      },
      "S" => {
        Some(Self {
          regex:   r"[0-5]\d",
          unit:    DateUnit::Seconds,
          max_len: 2,
        })
      },
      "I" => {
        Some(Self {
          regex:   r"[0-1]\d",
          unit:    DateUnit::Hours,
          max_len: 2,
        })
      },
      "-I" => {
        Some(Self {
          regex:   r"1?\d",
          unit:    DateUnit::Hours,
          max_len: 2,
        })
      },
      "P" => {
        Some(Self {
          regex:   r"am|pm",
          unit:    DateUnit::AmPm,
          max_len: 2,
        })
      },
      "p" => {
        Some(Self {
          regex:   r"AM|PM",
          unit:    DateUnit::AmPm,
          max_len: 2,
        })
      },
      _ => None,
    }
  }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum DateUnit {
  Years,
  Months,
  Days,
  Hours,
  Minutes,
  Seconds,
  AmPm,
}

impl DateUnit {
  fn is_date(self) -> bool {
    matches!(self, Self::Years | Self::Months | Self::Days)
  }

  fn is_time(self) -> bool {
    matches!(self, Self::Hours | Self::Minutes | Self::Seconds)
  }
}
