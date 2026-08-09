//! CIF/mmCIF lexical and structural parser.
//!
//! This is the Rust counterpart of CueMol2's `xtal::CifParser`. It keeps the
//! parser independent of molecule and crystallography types so both readers use
//! exactly the same CIF tokenization rules.

use crate::{CueMolError, Result};

/// One non-loop `_category.item value` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CifItem {
    pub name: String,
    pub value: String,
    pub line: usize,
}

impl CifItem {
    pub fn category(&self) -> Option<&str> {
        self.name.split_once('.').map(|(category, _)| category)
    }

    pub fn field(&self) -> Option<&str> {
        self.name.split_once('.').map(|(_, field)| field)
    }
}

/// One CIF `loop_` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CifLoop {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub line: usize,
}

impl CifLoop {
    pub fn category(&self) -> Option<&str> {
        self.columns
            .first()
            .and_then(|name| name.split_once('.').map(|(category, _)| category))
    }

    pub fn category_is(&self, expected: &str) -> bool {
        self.category()
            .is_some_and(|category| category.eq_ignore_ascii_case(expected))
    }

    pub fn column_index(&self, field: &str) -> Option<usize> {
        self.columns.iter().position(|column| {
            column
                .rsplit_once('.')
                .is_some_and(|(_, value)| value.eq_ignore_ascii_case(field))
        })
    }

    pub fn value<'a>(&self, row: &'a [String], field: &str) -> Option<&'a str> {
        self.column_index(field)
            .and_then(|index| row.get(index))
            .map(String::as_str)
            .filter(|value| !matches!(*value, "." | "?"))
    }
}

/// One `data_` block. CIF files without an explicit block are represented by an
/// unnamed block so fragment inputs remain useful to readers and tests.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CifBlock {
    pub name: String,
    pub items: Vec<CifItem>,
    pub loops: Vec<CifLoop>,
}

impl CifBlock {
    pub fn item(&self, name: &str) -> Option<&str> {
        self.items
            .iter()
            .find(|item| item.name.eq_ignore_ascii_case(name))
            .map(|item| item.value.as_str())
    }
}

/// Parsed CIF document.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CifDocument {
    pub blocks: Vec<CifBlock>,
}

impl CifDocument {
    pub fn items(&self) -> impl Iterator<Item = &CifItem> {
        self.blocks.iter().flat_map(|block| block.items.iter())
    }

    pub fn loops(&self) -> impl Iterator<Item = &CifLoop> {
        self.blocks.iter().flat_map(|block| block.loops.iter())
    }

    pub fn item(&self, name: &str) -> Option<&str> {
        self.items()
            .find(|item| item.name.eq_ignore_ascii_case(name))
            .map(|item| item.value.as_str())
    }
}

/// Stateful compatibility facade for C++ `xtal::CifParser`.
#[derive(Debug, Clone, Default)]
pub struct CifParser;

impl CifParser {
    pub const fn new() -> Self {
        Self
    }

    pub fn read(&self, input: &str) -> Result<CifDocument> {
        self.parse(input)
    }

    pub fn parse(&self, input: &str) -> Result<CifDocument> {
        parse_cif(input)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Token {
    value: String,
    line: usize,
    quoted: bool,
}

impl Token {
    fn is_word(&self, expected: &str) -> bool {
        !self.quoted && self.value.eq_ignore_ascii_case(expected)
    }

    fn is_data(&self) -> bool {
        !self.quoted
            && self
                .value
                .get(..5)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("data_"))
    }

    fn is_name(&self) -> bool {
        !self.quoted && self.value.starts_with('_')
    }

    fn is_control(&self) -> bool {
        self.is_name()
            || self.is_data()
            || self.is_word("loop_")
            || self.is_word("stop_")
            || self.is_word("global_")
            || self
                .value
                .get(..5)
                .is_some_and(|prefix| !self.quoted && prefix.eq_ignore_ascii_case("save_"))
    }
}

/// Parse a complete CIF/mmCIF document.
pub fn parse_cif(input: &str) -> Result<CifDocument> {
    let mut tokens = tokenize(input)?;
    let mut document = CifDocument::default();
    let mut current_block: Option<usize> = None;
    let mut index = 0;

    while index < tokens.len() {
        let token = &tokens[index];
        if token.is_data() {
            let value = std::mem::take(&mut tokens[index].value);
            let name = value[5..].to_string();
            document.blocks.push(CifBlock {
                name,
                ..CifBlock::default()
            });
            current_block = Some(document.blocks.len() - 1);
            index += 1;
            continue;
        }

        if token.is_word("loop_") {
            let block_index = ensure_block(&mut document, &mut current_block);
            let loop_line = token.line;
            index += 1;
            let mut columns = Vec::new();
            while index < tokens.len() && tokens[index].is_name() {
                columns.push(std::mem::take(&mut tokens[index].value));
                index += 1;
            }
            if columns.is_empty() {
                return Err(parse_error(loop_line, "loop_ has no data names"));
            }
            validate_loop_categories(&columns, loop_line)?;

            let mut values = Vec::new();
            while index < tokens.len() && !tokens[index].is_control() {
                values.push(std::mem::take(&mut tokens[index].value));
                index += 1;
            }
            if index < tokens.len() && tokens[index].is_word("stop_") {
                index += 1;
            }
            if values.len() % columns.len() != 0 {
                return Err(parse_error(
                    loop_line,
                    format!(
                        "loop_ has {} values for {} columns",
                        values.len(),
                        columns.len()
                    ),
                ));
            }
            let rows = values
                .chunks(columns.len())
                .map(|row| row.to_vec())
                .collect();
            document.blocks[block_index].loops.push(CifLoop {
                columns,
                rows,
                line: loop_line,
            });
            continue;
        }

        if token.is_name() {
            let block_index = ensure_block(&mut document, &mut current_block);
            let item_line = token.line;
            let name = std::mem::take(&mut tokens[index].value);
            index += 1;
            let Some(value) = tokens.get(index) else {
                return Err(parse_error(
                    item_line,
                    format!("data name {name} has no value"),
                ));
            };
            if value.is_control() {
                return Err(parse_error(
                    item_line,
                    format!("data name {name} has no value"),
                ));
            }
            document.blocks[block_index].items.push(CifItem {
                name,
                value: std::mem::take(&mut tokens[index].value),
                line: item_line,
            });
            index += 1;
            continue;
        }

        index += 1;
    }

    Ok(document)
}

fn ensure_block(document: &mut CifDocument, current: &mut Option<usize>) -> usize {
    if let Some(index) = *current {
        return index;
    }
    document.blocks.push(CifBlock::default());
    let index = document.blocks.len() - 1;
    *current = Some(index);
    index
}

fn validate_loop_categories(columns: &[String], line: usize) -> Result<()> {
    let category = columns[0]
        .split_once('.')
        .map(|(value, _)| value)
        .ok_or_else(|| parse_error(line, format!("invalid CIF data name {}", columns[0])))?;
    for column in &columns[1..] {
        let Some((other, _)) = column.split_once('.') else {
            return Err(parse_error(line, format!("invalid CIF data name {column}")));
        };
        if !category.eq_ignore_ascii_case(other) {
            return Err(parse_error(
                line,
                format!("loop_ category mismatch ({category} != {other})"),
            ));
        }
    }
    Ok(())
}

fn tokenize(input: &str) -> Result<Vec<Token>> {
    let lines = input.lines().collect::<Vec<_>>();
    let mut tokens = Vec::new();
    let mut line_index = 0;

    while line_index < lines.len() {
        let line = lines[line_index];
        let line_number = line_index + 1;
        if let Some(first_line) = line.strip_prefix(';') {
            let start_line = line_number;
            let mut value = String::new();
            if !first_line.is_empty() {
                value.push_str(first_line);
                value.push('\n');
            }
            line_index += 1;
            let mut closed = false;
            while line_index < lines.len() {
                let text_line = lines[line_index];
                if text_line.starts_with(';') {
                    closed = true;
                    line_index += 1;
                    break;
                }
                value.push_str(text_line);
                value.push('\n');
                line_index += 1;
            }
            if !closed {
                return Err(parse_error(start_line, "unterminated semicolon text field"));
            }
            if value.ends_with('\n') {
                value.pop();
            }
            tokens.push(Token {
                value,
                line: start_line,
                quoted: true,
            });
            continue;
        }

        tokenize_line(line, line_number, &mut tokens)?;
        line_index += 1;
    }
    Ok(tokens)
}

fn tokenize_line(line: &str, line_number: usize, tokens: &mut Vec<Token>) -> Result<()> {
    let mut pos = 0;
    while pos < line.len() {
        let ch = line[pos..]
            .chars()
            .next()
            .expect("position is within the line");
        if ch.is_whitespace() {
            pos += ch.len_utf8();
            continue;
        }
        if ch == '#' {
            break;
        }

        if matches!(ch, '\'' | '"') {
            let quote = ch;
            pos += ch.len_utf8();
            let start = pos;
            let mut end = None;
            while pos < line.len() {
                let next = line[pos..]
                    .chars()
                    .next()
                    .expect("position is within the line");
                if next == quote {
                    let after = pos + next.len_utf8();
                    let boundary = after == line.len()
                        || line[after..]
                            .chars()
                            .next()
                            .is_some_and(|value| value.is_whitespace() || value == '#');
                    if boundary {
                        end = Some(pos);
                        pos = after;
                        break;
                    }
                }
                pos += next.len_utf8();
            }
            let Some(end) = end else {
                return Err(parse_error(line_number, "unterminated quoted CIF value"));
            };
            tokens.push(Token {
                value: line[start..end].to_string(),
                line: line_number,
                quoted: true,
            });
            continue;
        }

        let start = pos;
        while pos < line.len() {
            let next = line[pos..]
                .chars()
                .next()
                .expect("position is within the line");
            if next.is_whitespace() {
                break;
            }
            pos += next.len_utf8();
        }
        tokens.push(Token {
            value: line[start..pos].to_string(),
            line: line_number,
            quoted: false,
        });
    }
    Ok(())
}

fn parse_error(line: usize, message: impl AsRef<str>) -> CueMolError {
    CueMolError::parse(format!("{} at CIF line {line}", message.as_ref()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_items_quotes_and_case_insensitive_lookup() {
        let document = parse_cif(
            "data_demo\n\
             _cell.length_a 10.0\n\
             _symmetry.space_group_name_H-M 'P 21 21 21'\n",
        )
        .unwrap();
        assert_eq!(document.blocks[0].name, "demo");
        assert_eq!(document.item("_CELL.LENGTH_A"), Some("10.0"));
        assert_eq!(
            document.item("_symmetry.space_group_name_h-m"),
            Some("P 21 21 21")
        );
    }

    #[test]
    fn groups_loop_values_across_physical_lines() {
        let document = parse_cif(
            "data_demo\n\
             loop_\n\
             _atom.id\n\
             _atom.name\n\
             1 N 2\n\
             CA\n\
             3 'C B'\n",
        )
        .unwrap();
        let cif_loop = document.loops().next().unwrap();
        assert_eq!(
            cif_loop.rows,
            vec![vec!["1", "N"], vec!["2", "CA"], vec!["3", "C B"],]
        );
    }

    #[test]
    fn preserves_semicolon_text_as_one_value() {
        let document = parse_cif(
            "data_demo\n\
             _struct.title\n\
;first line
second line
;
",
        )
        .unwrap();
        assert_eq!(
            document.item("_struct.title"),
            Some("first line\nsecond line")
        );
    }

    #[test]
    fn rejects_incomplete_loop_rows() {
        let error = parse_cif("data_demo\nloop_\n_atom.id\n_atom.name\n1 N 2\n")
            .expect_err("incomplete row must fail");
        assert!(error.message().contains("3 values for 2 columns"));
    }

    #[test]
    fn supports_multiple_data_blocks() {
        let document = parse_cif("data_one\n_test.id 1\ndata_two\n_test.id 2\n").unwrap();
        assert_eq!(document.blocks.len(), 2);
        assert_eq!(document.blocks[0].item("_test.id"), Some("1"));
        assert_eq!(document.blocks[1].item("_test.id"), Some("2"));
    }
}
