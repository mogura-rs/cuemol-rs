use crate::{CueMolError, LString, Result};
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct LRegExpr {
    pattern: LString,
    subject: LString,
    captures: Vec<Option<LString>>,
    names: HashMap<String, usize>,
}

impl LRegExpr {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_pattern(pattern: impl Into<LString>) -> Self {
        Self {
            pattern: pattern.into(),
            subject: LString::new(),
            captures: Vec::new(),
            names: HashMap::new(),
        }
    }

    pub fn set_pattern(&mut self, pattern: impl Into<LString>) {
        self.pattern = pattern.into();
        self.captures.clear();
        self.names.clear();
    }

    pub fn get_pattern(&self) -> &LString {
        &self.pattern
    }

    pub fn to_script_string(&self) -> LString {
        self.pattern.clone()
    }

    pub fn match_text(&mut self, subject: impl Into<LString>) -> Result<bool> {
        self.match_impl(subject.into(), false)
    }

    pub fn r#match(&mut self, subject: impl Into<LString>) -> Result<bool> {
        self.match_text(subject)
    }

    pub fn match_ignore_case(&mut self, subject: impl Into<LString>) -> Result<bool> {
        self.match_impl(subject.into(), true)
    }

    pub fn get_substr_count(&self) -> usize {
        self.captures.len()
    }

    pub fn get_substr(&self, index: usize) -> Result<LString> {
        self.captures
            .get(index)
            .and_then(Clone::clone)
            .ok_or_else(|| CueMolError::invalid_argument("substring not found"))
    }

    pub fn get_named_substr(&self, name: impl AsRef<str>) -> Result<LString> {
        let index = self
            .names
            .get(name.as_ref())
            .copied()
            .ok_or_else(|| CueMolError::invalid_argument("substring not found"))?;
        self.get_substr(index)
    }

    fn match_impl(&mut self, subject: LString, ignore_case: bool) -> Result<bool> {
        let compiled = CompiledPattern::parse(self.pattern.as_str())?;
        self.subject = subject;
        self.names = compiled.names.clone();

        for start in char_boundaries(self.subject.as_str()) {
            let mut captures = vec![None; compiled.capture_count];
            if let Some(end) = match_sequence(
                &compiled.pieces,
                self.subject.as_str(),
                start,
                ignore_case,
                &mut captures,
            ) {
                captures[0] = Some(self.subject.substr_len(start, end - start));
                self.captures = captures;
                return Ok(true);
            }
        }

        self.captures.clear();
        Ok(false)
    }
}

#[derive(Debug, Clone)]
struct CompiledPattern {
    pieces: Vec<Piece>,
    names: HashMap<String, usize>,
    capture_count: usize,
}

impl CompiledPattern {
    fn parse(pattern: &str) -> Result<Self> {
        let mut parser = Parser {
            input: pattern.chars().collect(),
            pos: 0,
            capture_count: 1,
            names: HashMap::new(),
        };
        let pieces = parser.parse_sequence(false)?;
        if parser.pos != parser.input.len() {
            return Err(CueMolError::invalid_argument("invalid regular expression"));
        }
        Ok(Self {
            pieces,
            names: parser.names,
            capture_count: parser.capture_count,
        })
    }
}

#[derive(Debug, Clone)]
struct Piece {
    atom: Atom,
    quantifier: Quantifier,
}

#[derive(Debug, Clone)]
enum Atom {
    Literal(char),
    Any,
    Digit,
    Space,
    Word,
    Class(Vec<CharRange>),
    Group { index: usize, pieces: Vec<Piece> },
}

#[derive(Debug, Clone)]
struct CharRange {
    start: char,
    end: char,
}

#[derive(Debug, Clone, Copy)]
enum Quantifier {
    One,
    OneOrMore,
    Exact(usize),
}

struct Parser {
    input: Vec<char>,
    pos: usize,
    capture_count: usize,
    names: HashMap<String, usize>,
}

impl Parser {
    fn parse_sequence(&mut self, in_group: bool) -> Result<Vec<Piece>> {
        let mut pieces = Vec::new();
        while self.pos < self.input.len() {
            if self.input[self.pos] == ')' {
                if in_group {
                    self.pos += 1;
                    return Ok(pieces);
                }
                return Err(CueMolError::invalid_argument("invalid regular expression"));
            }
            let atom = self.parse_atom()?;
            let quantifier = self.parse_quantifier()?;
            pieces.push(Piece { atom, quantifier });
        }
        if in_group {
            return Err(CueMolError::invalid_argument("invalid regular expression"));
        }
        Ok(pieces)
    }

    fn parse_atom(&mut self) -> Result<Atom> {
        let ch = self
            .next()
            .ok_or_else(|| CueMolError::invalid_argument("invalid regular expression"))?;
        match ch {
            '\\' => self.parse_escape(),
            '(' => self.parse_group(),
            '[' => self.parse_class(),
            '.' => Ok(Atom::Any),
            '+' | '{' | '}' => Err(CueMolError::invalid_argument("invalid regular expression")),
            _ => Ok(Atom::Literal(ch)),
        }
    }

    fn parse_escape(&mut self) -> Result<Atom> {
        let ch = self
            .next()
            .ok_or_else(|| CueMolError::invalid_argument("invalid regular expression"))?;
        Ok(match ch {
            'd' => Atom::Digit,
            's' => Atom::Space,
            'w' => Atom::Word,
            _ => Atom::Literal(ch),
        })
    }

    fn parse_group(&mut self) -> Result<Atom> {
        let mut name = None;
        if self.peek() == Some('?') {
            self.next();
            if self.next() != Some('P') || self.next() != Some('<') {
                return Err(CueMolError::invalid_argument("invalid regular expression"));
            }
            let mut parsed = String::new();
            while let Some(ch) = self.next() {
                if ch == '>' {
                    break;
                }
                parsed.push(ch);
            }
            if parsed.is_empty() || self.input.get(self.pos - 1) != Some(&'>') {
                return Err(CueMolError::invalid_argument("invalid regular expression"));
            }
            name = Some(parsed);
        }

        let index = self.capture_count;
        self.capture_count += 1;
        if let Some(name) = name {
            self.names.insert(name, index);
        }
        let pieces = self.parse_sequence(true)?;
        Ok(Atom::Group { index, pieces })
    }

    fn parse_class(&mut self) -> Result<Atom> {
        let mut ranges = Vec::new();
        let mut previous: Option<char> = None;
        while let Some(ch) = self.next() {
            if ch == ']' {
                if let Some(prev) = previous.take() {
                    ranges.push(CharRange {
                        start: prev,
                        end: prev,
                    });
                }
                return Ok(Atom::Class(ranges));
            }
            if ch == '-' {
                let start = previous
                    .take()
                    .ok_or_else(|| CueMolError::invalid_argument("invalid regular expression"))?;
                let end = self
                    .next()
                    .ok_or_else(|| CueMolError::invalid_argument("invalid regular expression"))?;
                if end == ']' {
                    return Err(CueMolError::invalid_argument("invalid regular expression"));
                }
                ranges.push(CharRange { start, end });
            } else {
                if let Some(prev) = previous.replace(ch) {
                    ranges.push(CharRange {
                        start: prev,
                        end: prev,
                    });
                }
            }
        }
        Err(CueMolError::invalid_argument("invalid regular expression"))
    }

    fn parse_quantifier(&mut self) -> Result<Quantifier> {
        match self.peek() {
            Some('+') => {
                self.next();
                Ok(Quantifier::OneOrMore)
            }
            Some('{') => {
                self.next();
                let mut digits = String::new();
                while let Some(ch) = self.next() {
                    if ch == '}' {
                        let value = digits.parse::<usize>().map_err(|_| {
                            CueMolError::invalid_argument("invalid regular expression")
                        })?;
                        return Ok(Quantifier::Exact(value));
                    }
                    if !ch.is_ascii_digit() {
                        return Err(CueMolError::invalid_argument("invalid regular expression"));
                    }
                    digits.push(ch);
                }
                Err(CueMolError::invalid_argument("invalid regular expression"))
            }
            _ => Ok(Quantifier::One),
        }
    }

    fn peek(&self) -> Option<char> {
        self.input.get(self.pos).copied()
    }

    fn next(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += 1;
        Some(ch)
    }
}

fn match_sequence(
    pieces: &[Piece],
    subject: &str,
    mut pos: usize,
    ignore_case: bool,
    captures: &mut [Option<LString>],
) -> Option<usize> {
    for piece in pieces {
        match piece.quantifier {
            Quantifier::One => {
                pos = match_atom(&piece.atom, subject, pos, ignore_case, captures)?;
            }
            Quantifier::OneOrMore => {
                let mut count = 0;
                while let Some(next) = match_atom(&piece.atom, subject, pos, ignore_case, captures)
                {
                    if next == pos {
                        break;
                    }
                    pos = next;
                    count += 1;
                }
                if count == 0 {
                    return None;
                }
            }
            Quantifier::Exact(count) => {
                for _ in 0..count {
                    pos = match_atom(&piece.atom, subject, pos, ignore_case, captures)?;
                }
            }
        }
    }
    Some(pos)
}

fn match_atom(
    atom: &Atom,
    subject: &str,
    pos: usize,
    ignore_case: bool,
    captures: &mut [Option<LString>],
) -> Option<usize> {
    match atom {
        Atom::Literal(expected) => {
            let (actual, next) = char_at(subject, pos)?;
            if chars_equal(*expected, actual, ignore_case) {
                Some(next)
            } else {
                None
            }
        }
        Atom::Any => char_at(subject, pos).map(|(_, next)| next),
        Atom::Digit => match_char(subject, pos, |ch| ch.is_ascii_digit()),
        Atom::Space => match_char(subject, pos, |ch| ch.is_ascii_whitespace()),
        Atom::Word => match_char(subject, pos, |ch| ch.is_ascii_alphanumeric() || ch == '_'),
        Atom::Class(ranges) => match_char(subject, pos, |ch| {
            let ch = if ignore_case {
                ch.to_ascii_lowercase()
            } else {
                ch
            };
            ranges.iter().any(|range| {
                let start = if ignore_case {
                    range.start.to_ascii_lowercase()
                } else {
                    range.start
                };
                let end = if ignore_case {
                    range.end.to_ascii_lowercase()
                } else {
                    range.end
                };
                start <= ch && ch <= end
            })
        }),
        Atom::Group { index, pieces } => {
            let end = match_sequence(pieces, subject, pos, ignore_case, captures)?;
            captures[*index] = Some(LString::from(subject[pos..end].to_string()));
            Some(end)
        }
    }
}

fn match_char<F>(subject: &str, pos: usize, predicate: F) -> Option<usize>
where
    F: FnOnce(char) -> bool,
{
    let (ch, next) = char_at(subject, pos)?;
    predicate(ch).then_some(next)
}

fn char_at(subject: &str, pos: usize) -> Option<(char, usize)> {
    let ch = subject.get(pos..)?.chars().next()?;
    Some((ch, pos + ch.len_utf8()))
}

fn chars_equal(lhs: char, rhs: char, ignore_case: bool) -> bool {
    if ignore_case {
        lhs.eq_ignore_ascii_case(&rhs)
    } else {
        lhs == rhs
    }
}

fn char_boundaries(subject: &str) -> impl Iterator<Item = usize> + '_ {
    subject
        .char_indices()
        .map(|(index, _)| index)
        .chain(std::iter::once(subject.len()))
}
