use std::fmt;
use std::ops::Add;

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LString(String);

impl LString {
    pub fn new() -> Self {
        Self(String::new())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn length(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn substr(&self, start: usize) -> Self {
        self.substr_len(start, self.0.len().saturating_sub(start))
    }

    pub fn substr_len(&self, start: usize, len: usize) -> Self {
        let start = self.byte_boundary_at_or_end(start);
        let end = self.byte_boundary_at_or_end(start.saturating_add(len));
        Self(self.0[start..end].to_string())
    }

    pub fn mid(&self, start: usize) -> Self {
        self.substr(start)
    }

    pub fn mid_len(&self, start: usize, len: usize) -> Self {
        self.substr_len(start, len)
    }

    pub fn left(&self, len: usize) -> Self {
        self.substr_len(0, len)
    }

    pub fn right(&self, len: usize) -> Self {
        let len = len.min(self.0.len());
        self.substr(self.0.len() - len)
    }

    pub fn compare(&self, rhs: impl AsRef<str>) -> i32 {
        match self.0.as_str().cmp(rhs.as_ref()) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        }
    }

    pub fn equals(&self, rhs: impl AsRef<str>) -> bool {
        self.0 == rhs.as_ref()
    }

    pub fn equals_ignore_case(&self, rhs: impl AsRef<str>) -> bool {
        self.0.eq_ignore_ascii_case(rhs.as_ref())
    }

    pub fn to_upper_case(&self) -> Self {
        Self(self.0.to_ascii_uppercase())
    }

    pub fn to_lower_case(&self) -> Self {
        Self(self.0.to_ascii_lowercase())
    }

    pub fn index_of_char(&self, needle: char) -> isize {
        index_to_isize(self.0.find(needle))
    }

    pub fn last_index_of_char(&self, needle: char) -> isize {
        index_to_isize(self.0.rfind(needle))
    }

    pub fn index_of(&self, needle: impl AsRef<str>) -> isize {
        index_to_isize(self.0.find(needle.as_ref()))
    }

    pub fn last_index_of(&self, needle: impl AsRef<str>) -> isize {
        index_to_isize(self.0.rfind(needle.as_ref()))
    }

    pub fn index_one_of(&self, needles: impl AsRef<str>) -> isize {
        index_to_isize(self.0.find(|ch| needles.as_ref().contains(ch)))
    }

    pub fn last_index_one_of(&self, needles: impl AsRef<str>) -> isize {
        index_to_isize(self.0.rfind(|ch| needles.as_ref().contains(ch)))
    }

    pub fn starts_with(&self, prefix: impl AsRef<str>) -> bool {
        self.0.starts_with(prefix.as_ref())
    }

    pub fn ends_with(&self, suffix: impl AsRef<str>) -> bool {
        self.0.ends_with(suffix.as_ref())
    }

    pub fn to_int(&self) -> Option<i32> {
        parse_i32_prefix(self.0.as_str())
    }

    pub fn to_double(&self) -> Option<f64> {
        self.0.trim_start().parse::<f64>().ok()
    }

    pub fn from_int(value: i32) -> Self {
        Self(value.to_string())
    }

    pub fn from_real(value: f64, max_digits: usize) -> Self {
        let mut value = format!("{value:.max_digits$}");
        if value.contains('.') {
            while value.ends_with('0') {
                value.pop();
            }
            if value.ends_with('.') {
                value.pop();
            }
        }
        Self(value)
    }

    pub fn format(args: fmt::Arguments<'_>) -> Self {
        Self(args.to_string())
    }

    pub fn split(&self, delimiter: char) -> Vec<Self> {
        let mut values = Vec::new();
        let mut start = 0;
        for (index, ch) in self.0.char_indices() {
            if ch == delimiter {
                if index > start {
                    values.push(Self(self.0[start..index].to_string()));
                }
                start = index + ch.len_utf8();
            }
        }
        values.push(Self(self.0[start..].to_string()));
        values
    }

    pub fn split_of(&self, delimiters: impl AsRef<str>) -> Vec<Self> {
        let delimiters = delimiters.as_ref();
        let mut values = Vec::new();
        let mut start = 0;
        for (index, ch) in self.0.char_indices() {
            if delimiters.contains(ch) {
                if index > start {
                    values.push(Self(self.0[start..index].to_string()));
                }
                start = index + ch.len_utf8();
            }
        }
        if start < self.0.len() {
            values.push(Self(self.0[start..].to_string()));
        }
        values
    }

    pub fn join(separator: impl AsRef<str>, values: &[Self]) -> Self {
        Self(
            values
                .iter()
                .map(Self::as_str)
                .collect::<Vec<_>>()
                .join(separator.as_ref()),
        )
    }

    pub fn trim(&self) -> Self {
        self.trim_matches(" \t")
    }

    pub fn trim_matches(&self, chars: impl AsRef<str>) -> Self {
        let chars = chars.as_ref();
        Self(self.0.trim_matches(|ch| chars.contains(ch)).to_string())
    }

    pub fn chomp(&self) -> Self {
        self.chomp_matches("\r\n")
    }

    pub fn chomp_matches(&self, chars: impl AsRef<str>) -> Self {
        let chars = chars.as_ref();
        Self(self.0.trim_end_matches(|ch| chars.contains(ch)).to_string())
    }

    pub fn replace_char(&mut self, needle: char, replacement: char) -> usize {
        let mut count = 0;
        self.0 = self
            .0
            .chars()
            .map(|ch| {
                if ch == needle {
                    count += 1;
                    replacement
                } else {
                    ch
                }
            })
            .collect();
        count
    }

    pub fn replace_substr(
        &mut self,
        needle: impl AsRef<str>,
        replacement: impl AsRef<str>,
    ) -> usize {
        let needle = needle.as_ref();
        if needle.is_empty() {
            return 0;
        }
        let replacement = replacement.as_ref();
        let mut count = 0;
        let mut start = 0;
        while let Some(offset) = self.0[start..].find(needle) {
            let index = start + offset;
            self.0
                .replace_range(index..index + needle.len(), replacement);
            count += 1;
            start = index + replacement.len();
        }
        count
    }

    pub fn escape_quots(&self) -> Self {
        let mut result = String::new();
        for ch in self.0.chars() {
            match ch {
                '\\' => result.push_str("\\\\"),
                '"' => result.push_str("\\\""),
                '\n' => result.push_str("\\n"),
                '\r' => result.push_str("\\r"),
                '\t' => result.push_str("\\t"),
                _ => result.push(ch),
            }
        }
        Self(result)
    }

    fn byte_boundary_at_or_end(&self, index: usize) -> usize {
        if index >= self.0.len() {
            return self.0.len();
        }
        let mut index = index;
        while index < self.0.len() && !self.0.is_char_boundary(index) {
            index += 1;
        }
        index
    }
}

impl From<&str> for LString {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for LString {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl AsRef<str> for LString {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for LString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Add for LString {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.as_str())
    }
}

#[macro_export]
macro_rules! lformat {
    ($($arg:tt)*) => {
        $crate::LString::format(format_args!($($arg)*))
    };
}

fn index_to_isize(index: Option<usize>) -> isize {
    index.map(|value| value as isize).unwrap_or(-1)
}

fn parse_i32_prefix(value: &str) -> Option<i32> {
    let value = value.trim_start();
    if value.is_empty() {
        return None;
    }

    let mut chars = value.char_indices().peekable();
    let mut sign = 1_i64;
    if let Some((_, ch)) = chars.peek().copied() {
        match ch {
            '-' => {
                sign = -1;
                chars.next();
            }
            '+' => {
                chars.next();
            }
            _ => {}
        }
    }

    let mut rest_start = chars.peek().map(|(index, _)| *index).unwrap_or(value.len());
    let mut base = 10;
    if value[rest_start..].starts_with("0x") || value[rest_start..].starts_with("0X") {
        base = 16;
        rest_start += 2;
    }

    let mut end = rest_start;
    for (index, ch) in value[rest_start..].char_indices() {
        if ch.is_digit(base) {
            end = rest_start + index + ch.len_utf8();
        } else {
            break;
        }
    }
    if end == rest_start {
        return None;
    }
    i64::from_str_radix(&value[rest_start..end], base)
        .ok()
        .and_then(|parsed| i32::try_from(parsed * sign).ok())
}
