use crate::{CueMolError, LString, Result};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock};

pub struct LChar;

impl LChar {
    pub fn to_lower(value: u8) -> u8 {
        value.to_ascii_lowercase()
    }

    pub fn to_upper(value: u8) -> u8 {
        value.to_ascii_uppercase()
    }

    pub fn lower_in_place(value: &mut [u8]) {
        value.make_ascii_lowercase();
    }

    pub fn upper_in_place(value: &mut [u8]) {
        value.make_ascii_uppercase();
    }

    pub fn substr(source: &[u8], start: usize, end: usize) -> Vec<u8> {
        source[start..end].to_vec()
    }

    pub fn copy(source: &[u8], destination_len: usize) -> Vec<u8> {
        source
            .iter()
            .copied()
            .take(destination_len.saturating_sub(1))
            .take_while(|byte| *byte != 0)
            .collect()
    }

    pub fn trim<'a>(value: &'a str, whitespace: &str) -> &'a str {
        value.trim_matches(|character| whitespace.contains(character))
    }

    pub fn is_empty(value: Option<&str>) -> bool {
        value.is_none_or(str::is_empty)
    }

    pub fn index_of(value: &str, character: char) -> isize {
        value.find(character).map_or(-1, |index| index as isize)
    }

    pub fn starts_with(value: &str, prefix: &str) -> bool {
        value.starts_with(prefix)
    }

    pub fn to_double(value: &str) -> Option<f64> {
        if value.is_empty() || value.chars().last().is_some_and(char::is_whitespace) {
            return None;
        }
        value.trim_start().parse().ok()
    }

    pub fn to_int(value: &str, base: u32) -> Option<i32> {
        parse_integer(value, base, true)
    }
}

/// Decodes one reference-compatible UTF-8 character into a single UCS-16 unit.
/// Four-byte UTF-8 sequences are rejected because the original API has no
/// surrogate-pair return channel.
pub fn utf8_bytes_to_ucs16_one(input: &[u8], index: &mut usize) -> Result<u16> {
    let first = *input
        .get(*index)
        .ok_or_else(|| CueMolError::invalid_argument("end of string"))?;
    if first <= 0x7f {
        *index += 1;
        return Ok(u16::from(first));
    }

    let (count, mut value, minimum) = if first & 0xe0 == 0xc0 {
        (2, u32::from(first & 0x1f), 0x80)
    } else if first & 0xf0 == 0xe0 {
        (3, u32::from(first & 0x0f), 0x800)
    } else {
        return Err(CueMolError::invalid_argument(
            "UTF-8 surrogate not supported",
        ));
    };
    if input.len().saturating_sub(*index) < count {
        return Err(CueMolError::invalid_argument("invalid UTF-8 encoding"));
    }
    for offset in 1..count {
        let continuation = input[*index + offset];
        if continuation & 0xc0 != 0x80 {
            return Err(CueMolError::invalid_argument("invalid UTF-8 encoding"));
        }
        value = (value << 6) | u32::from(continuation & 0x3f);
    }
    if value < minimum || value > 0xffff || (0xd800..=0xdfff).contains(&value) {
        return Err(CueMolError::invalid_argument("invalid UTF-8 encoding"));
    }
    *index += count;
    Ok(value as u16)
}

pub fn utf8_to_ucs16(input: &str) -> Result<Vec<u16>> {
    let mut output = Vec::with_capacity(input.len());
    let mut index = 0;
    while index < input.len() {
        output.push(utf8_bytes_to_ucs16_one(input.as_bytes(), &mut index)?);
    }
    Ok(output)
}

pub fn ucs16_to_utf8(input: &[u16]) -> Result<LString> {
    let mut output = String::new();
    for &unit in input.iter().take_while(|unit| **unit != 0) {
        if (0xd800..=0xdfff).contains(&unit) {
            return Err(CueMolError::invalid_argument(
                "UCS-16 surrogate not supported",
            ));
        }
        let character = char::from_u32(u32::from(unit))
            .ok_or_else(|| CueMolError::invalid_argument("invalid UCS-16 character"))?;
        output.push(character);
    }
    Ok(output.into())
}

pub fn is_ascii(value: &LString) -> bool {
    value.as_str().is_ascii()
}

pub fn native_to_utf8(value: &str) -> LString {
    value.into()
}

pub fn utf8_to_native(value: &LString) -> String {
    value.to_string()
}

pub type TagId = u32;

#[derive(Debug)]
pub struct TagNameTable {
    next_id: TagId,
    id_to_string: BTreeMap<TagId, Arc<str>>,
    string_to_id: BTreeMap<Arc<str>, TagId>,
}

impl Default for TagNameTable {
    fn default() -> Self {
        Self::new()
    }
}

impl TagNameTable {
    pub fn new() -> Self {
        Self {
            next_id: TagName::TAG_NULL + 1,
            id_to_string: BTreeMap::new(),
            string_to_id: BTreeMap::new(),
        }
    }

    pub fn search_tag_id(&self, value: &str) -> TagId {
        self.string_to_id
            .get(value)
            .copied()
            .unwrap_or(TagName::TAG_NULL)
    }

    pub fn search_str(&self, id: TagId) -> Option<Arc<str>> {
        self.id_to_string.get(&id).cloned()
    }

    pub fn create_new_tag(&mut self, value: &str) -> (TagId, Arc<str>) {
        if let Some(id) = self.string_to_id.get(value).copied() {
            return (id, self.id_to_string[&id].clone());
        }
        let id = self.next_id;
        self.next_id = self
            .next_id
            .checked_add(1)
            .expect("TagName ID space expired");
        let value: Arc<str> = Arc::from(value);
        self.id_to_string.insert(id, value.clone());
        self.string_to_id.insert(value.clone(), id);
        (id, value)
    }

    pub fn clear(&mut self) {
        *self = Self::new();
    }
}

#[derive(Debug, Clone)]
pub struct TagName {
    id: TagId,
    value: Arc<str>,
}

impl Default for TagName {
    fn default() -> Self {
        Self {
            id: Self::TAG_NULL,
            value: Arc::from(""),
        }
    }
}

impl TagName {
    pub const TAG_NULL: TagId = 0;

    pub fn new(value: impl AsRef<str>) -> Self {
        let (id, value) = global_tag_table()
            .lock()
            .expect("TagName table lock poisoned")
            .create_new_tag(value.as_ref());
        Self { id, value }
    }

    pub fn from_id(id: TagId) -> Self {
        let value = global_tag_table()
            .lock()
            .expect("TagName table lock poisoned")
            .search_str(id);
        value.map_or_else(Self::default, |value| Self { id, value })
    }

    pub fn id(&self) -> TagId {
        self.id
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }

    pub fn len(&self) -> usize {
        self.value.len()
    }

    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    pub fn get_at(&self, index: usize) -> u8 {
        self.value.as_bytes()[index]
    }

    pub fn equals_str(&self, value: &str) -> bool {
        global_tag_table()
            .lock()
            .expect("TagName table lock poisoned")
            .search_tag_id(value)
            == self.id
    }

    /// Preserves the original implementation's common-prefix behavior.
    pub fn starts_with(&self, value: &str) -> bool {
        self.value
            .bytes()
            .zip(value.bytes())
            .all(|(left, right)| left == right)
    }

    pub fn index_of(&self, character: u8) -> isize {
        self.value
            .as_bytes()
            .iter()
            .position(|value| *value == character)
            .map_or(-1, |index| index as isize)
    }

    pub fn last_index_of(&self, character: u8) -> isize {
        self.value
            .as_bytes()
            .iter()
            .rposition(|value| *value == character)
            .map_or(-1, |index| index as isize)
    }

    pub fn to_int(&self) -> Option<i32> {
        parse_integer(self.as_str(), 0, false)
    }

    pub fn init() -> bool {
        let _ = global_tag_table();
        true
    }

    pub fn fini() -> bool {
        global_tag_table()
            .lock()
            .expect("TagName table lock poisoned")
            .clear();
        true
    }
}

impl PartialEq for TagName {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for TagName {}

impl PartialOrd for TagName {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TagName {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id.cmp(&other.id)
    }
}

impl Hash for TagName {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl std::fmt::Display for TagName {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl From<&str> for TagName {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

fn global_tag_table() -> &'static Mutex<TagNameTable> {
    static TABLE: OnceLock<Mutex<TagNameTable>> = OnceLock::new();
    TABLE.get_or_init(|| Mutex::new(TagNameTable::new()))
}

fn parse_integer(value: &str, requested_base: u32, require_full: bool) -> Option<i32> {
    let value = value.trim_start();
    if value.is_empty() || !(requested_base == 0 || (2..=36).contains(&requested_base)) {
        return None;
    }
    let (negative, unsigned) = match value.as_bytes()[0] {
        b'-' => (true, &value[1..]),
        b'+' => (false, &value[1..]),
        _ => (false, value),
    };
    let (base, digits) = if requested_base == 0 {
        if unsigned.starts_with("0x") || unsigned.starts_with("0X") {
            (16, &unsigned[2..])
        } else if unsigned.len() > 1 && unsigned.starts_with('0') {
            (8, unsigned)
        } else {
            (10, unsigned)
        }
    } else if requested_base == 16 && (unsigned.starts_with("0x") || unsigned.starts_with("0X")) {
        (16, &unsigned[2..])
    } else {
        (requested_base, unsigned)
    };
    let digit_count = digits
        .chars()
        .take_while(|character| character.is_digit(base))
        .count();
    if digit_count == 0 || (require_full && digit_count != digits.chars().count()) {
        return None;
    }
    let parsed = i64::from_str_radix(&digits[..digit_count], base).ok()?;
    let signed = if negative { -parsed } else { parsed };
    i32::try_from(signed).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lchar_matches_ascii_trim_and_numeric_conversion() {
        assert_eq!(LChar::to_lower(b'A'), b'a');
        assert_eq!(LChar::to_upper(b'z'), b'Z');
        let mut bytes = *b"AbC";
        LChar::lower_in_place(&mut bytes);
        assert_eq!(&bytes, b"abc");
        assert_eq!(LChar::trim(" \tvalue\r\n", " \t\r\n"), "value");
        assert_eq!(LChar::to_int("0x2a", 0), Some(42));
        assert_eq!(LChar::to_int("077", 0), Some(63));
        assert_eq!(LChar::to_double(" 1.25"), Some(1.25));
        assert_eq!(LChar::to_double("1.25 "), None);
        assert_eq!(LChar::copy(b"abcdef", 4), b"abc");
    }

    #[test]
    fn unicode_bmp_conversion_matches_reference_limits() {
        let source = "Aé水";
        let units = utf8_to_ucs16(source).unwrap();
        assert_eq!(units, vec![0x41, 0xe9, 0x6c34]);
        assert_eq!(ucs16_to_utf8(&units).unwrap().as_str(), source);
        assert!(is_ascii(&"ASCII".into()));
        assert!(!is_ascii(&source.into()));
        assert!(utf8_to_ucs16("😀").is_err());
        assert!(ucs16_to_utf8(&[0xd83d, 0xde00]).is_err());
    }

    #[test]
    fn unicode_decoder_rejects_truncated_continuation_and_overlong_sequences() {
        let mut index = 0;
        assert!(utf8_bytes_to_ucs16_one(&[0xc2], &mut index).is_err());
        index = 0;
        assert!(utf8_bytes_to_ucs16_one(&[0xe2, 0x28, 0xa1], &mut index).is_err());
        index = 0;
        assert!(utf8_bytes_to_ucs16_one(&[0xc0, 0x80], &mut index).is_err());
    }

    #[test]
    fn tag_names_are_interned_and_roundtrip_by_id() {
        let first = TagName::new("scene");
        let same = TagName::new("scene");
        let second = TagName::new("object");

        assert_eq!(first, same);
        assert_ne!(first, second);
        assert_eq!(TagName::from_id(first.id()), first);
        assert!(first.equals_str("scene"));
        assert_eq!(TagName::from_id(u32::MAX), TagName::default());
    }

    #[test]
    fn tag_name_search_and_numeric_prefix_follow_reference_behavior() {
        let name = TagName::new("datachunk");
        assert_eq!(name.index_of(b'a'), 1);
        assert_eq!(name.last_index_of(b'a'), 3);
        assert!(name.starts_with("data"));
        assert!(TagName::new("data").starts_with("datachunk"));
        assert_eq!(TagName::new("123tail").to_int(), Some(123));
    }
}
