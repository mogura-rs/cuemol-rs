//! Python pickle virtual machine — the opcode interpreter underneath a PyMOL
//! `.pse` session reader. Ports the role of C++ `importers::PickleInStream`.
//!
//! This parses a pickle byte stream (protocols 0-2, the binary opcode subset
//! PyMOL emits, plus the common protocol-0 text opcodes) into a [`PickleValue`]
//! tree. It is verifiable in isolation: Python's own `pickle` module is the
//! oracle (a known object pickled by Python must parse back to the matching
//! tree).
//!
//! SCOPE / honesty: this is the VALUE-TREE layer only. Reconstructing PyMOL's
//! version-specific object graph (the `GLOBAL`/`OBJ`/`BUILD`/`NEWOBJ` opcodes
//! that rebuild ChemPy/molecule objects) into a `MolCoord` is NOT done — that
//! needs a real `.pse` fixture, which the repo lacks. Object opcodes are parsed
//! to a transparent [`PickleValue::Object`] placeholder so the surrounding tree
//! still loads. Memoised values use value (clone) semantics, so shared-then-
//! mutated references and cycles are not preserved (acyclic session headers are).

use cuemol_core::{CueMolError, Result};

/// A decoded pickle value.
#[derive(Debug, Clone, PartialEq)]
pub enum PickleValue {
    None,
    Bool(bool),
    Int(i64),
    Float(f64),
    /// A unicode string (`BINUNICODE` / text `UNICODE`).
    Str(String),
    /// A byte string (`BINSTRING` / `SHORT_BINSTRING`; Python-2 `str`).
    Bytes(Vec<u8>),
    List(Vec<PickleValue>),
    Tuple(Vec<PickleValue>),
    /// Ordered key/value pairs (pickle preserves insertion order).
    Dict(Vec<(PickleValue, PickleValue)>),
    /// A `GLOBAL` reference `module.name`.
    Global(String, String),
    /// An opaque object placeholder (the `OBJ`/`NEWOBJ`/`BUILD` graph is not
    /// reconstructed): the callable plus its argument/state values.
    Object(Vec<PickleValue>),
}

impl PickleValue {
    /// Look up a string key in a `Dict` value. Matches both `Str` (Python-3
    /// unicode / BINUNICODE) and `Bytes` (Python-2 `str` / BINSTRING) keys —
    /// real PyMOL `.pse` are Python-2 pickles whose dict keys arrive as `Bytes`.
    pub fn get(&self, key: &str) -> Option<&PickleValue> {
        match self {
            PickleValue::Dict(pairs) => pairs.iter().find_map(|(k, v)| match k {
                PickleValue::Str(s) if s == key => Some(v),
                PickleValue::Bytes(b) if b.as_slice() == key.as_bytes() => Some(v),
                _ => None,
            }),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            PickleValue::Int(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            PickleValue::Float(v) => Some(*v),
            PickleValue::Int(v) => Some(*v as f64),
            _ => None,
        }
    }

    /// The text of a `Str`, or a `Bytes` interpreted as UTF-8 (Python-2 `str`).
    pub fn as_str(&self) -> Option<&str> {
        match self {
            PickleValue::Str(s) => Some(s),
            PickleValue::Bytes(b) => std::str::from_utf8(b).ok(),
            _ => None,
        }
    }

    /// The elements of a `List` or `Tuple`.
    pub fn as_list(&self) -> Option<&[PickleValue]> {
        match self {
            PickleValue::List(v) | PickleValue::Tuple(v) => Some(v),
            _ => None,
        }
    }

    /// Index into a `List`/`Tuple` (the pickle equivalent of `seq[i]`).
    pub fn index(&self, i: usize) -> Option<&PickleValue> {
        self.as_list().and_then(|v| v.get(i))
    }
}

#[derive(Debug)]
enum Slot {
    Mark,
    Value(PickleValue),
}

struct PickleMachine<'a> {
    bytes: &'a [u8],
    pos: usize,
    stack: Vec<Slot>,
    memo: Vec<Option<PickleValue>>,
}

impl<'a> PickleMachine<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            pos: 0,
            stack: Vec::new(),
            memo: Vec::new(),
        }
    }

    fn byte(&mut self) -> Result<u8> {
        let b = *self
            .bytes
            .get(self.pos)
            .ok_or_else(|| CueMolError::parse("unexpected end of pickle stream"))?;
        self.pos += 1;
        Ok(b)
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(n)
            .ok_or_else(|| CueMolError::parse("pickle length overflow"))?;
        let slice = self
            .bytes
            .get(self.pos..end)
            .ok_or_else(|| CueMolError::parse("unexpected end of pickle data"))?;
        self.pos = end;
        Ok(slice)
    }

    fn read_line(&mut self) -> Result<String> {
        let start = self.pos;
        while self.pos < self.bytes.len() && self.bytes[self.pos] != b'\n' {
            self.pos += 1;
        }
        let line = std::str::from_utf8(&self.bytes[start..self.pos])
            .map_err(|_| CueMolError::parse("pickle line is not UTF-8"))?
            .to_string();
        self.pos += 1; // consume newline
        Ok(line)
    }

    fn u32_le(&mut self) -> Result<u32> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn push(&mut self, value: PickleValue) {
        self.stack.push(Slot::Value(value));
    }

    fn pop(&mut self) -> Result<PickleValue> {
        match self.stack.pop() {
            Some(Slot::Value(v)) => Ok(v),
            _ => Err(CueMolError::parse("pickle stack underflow")),
        }
    }

    /// Pop values back to (and removing) the topmost mark, in stack order.
    fn pop_to_mark(&mut self) -> Result<Vec<PickleValue>> {
        let mut items = Vec::new();
        loop {
            match self.stack.pop() {
                Some(Slot::Mark) => break,
                Some(Slot::Value(v)) => items.push(v),
                None => return Err(CueMolError::parse("pickle mark not found")),
            }
        }
        items.reverse();
        Ok(items)
    }

    fn memo_put(&mut self, index: usize) -> Result<()> {
        let top = match self.stack.last() {
            Some(Slot::Value(v)) => v.clone(),
            _ => return Err(CueMolError::parse("pickle memo put on empty stack")),
        };
        if index >= self.memo.len() {
            self.memo.resize(index + 1, None);
        }
        self.memo[index] = Some(top);
        Ok(())
    }

    fn memo_get(&mut self, index: usize) -> Result<()> {
        let value = self
            .memo
            .get(index)
            .and_then(|v| v.clone())
            .ok_or_else(|| CueMolError::parse("pickle memo get miss"))?;
        self.push(value);
        Ok(())
    }

    fn top_list_extend(&mut self, items: Vec<PickleValue>) -> Result<()> {
        match self.stack.last_mut() {
            Some(Slot::Value(PickleValue::List(list))) => {
                list.extend(items);
                Ok(())
            }
            _ => Err(CueMolError::parse("pickle APPEND(S) target is not a list")),
        }
    }

    fn top_dict_set(&mut self, pairs: Vec<(PickleValue, PickleValue)>) -> Result<()> {
        match self.stack.last_mut() {
            Some(Slot::Value(PickleValue::Dict(dict))) => {
                dict.extend(pairs);
                Ok(())
            }
            _ => Err(CueMolError::parse("pickle SETITEM(S) target is not a dict")),
        }
    }

    fn run(&mut self) -> Result<PickleValue> {
        loop {
            let op = self.byte()?;
            match op {
                0x80 => {
                    self.byte()?; // PROTO: protocol version
                }
                0x95 => {
                    self.take(8)?; // FRAME: 8-byte frame length (ignored)
                }
                b'}' => self.push(PickleValue::Dict(Vec::new())), // EMPTY_DICT
                b']' => self.push(PickleValue::List(Vec::new())), // EMPTY_LIST
                b')' => self.push(PickleValue::Tuple(Vec::new())), // EMPTY_TUPLE
                b'(' => self.stack.push(Slot::Mark),              // MARK
                b'd' => {
                    // DICT: build from mark (k1,v1,...)
                    let items = self.pop_to_mark()?;
                    self.push(PickleValue::Dict(into_pairs(items)?));
                }
                b'l' => {
                    let items = self.pop_to_mark()?; // LIST
                    self.push(PickleValue::List(items));
                }
                b't' => {
                    let items = self.pop_to_mark()?; // TUPLE
                    self.push(PickleValue::Tuple(items));
                }
                0x85 => {
                    let v = self.pop()?; // TUPLE1
                    self.push(PickleValue::Tuple(vec![v]));
                }
                0x86 => {
                    let b = self.pop()?; // TUPLE2
                    let a = self.pop()?;
                    self.push(PickleValue::Tuple(vec![a, b]));
                }
                0x87 => {
                    let c = self.pop()?; // TUPLE3
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(PickleValue::Tuple(vec![a, b, c]));
                }
                b'N' => self.push(PickleValue::None), // NONE
                0x88 => self.push(PickleValue::Bool(true)), // NEWTRUE
                0x89 => self.push(PickleValue::Bool(false)), // NEWFALSE
                b'K' => {
                    let v = self.byte()?; // BININT1
                    self.push(PickleValue::Int(v as i64));
                }
                b'M' => {
                    let b = self.take(2)?; // BININT2
                    self.push(PickleValue::Int(u16::from_le_bytes([b[0], b[1]]) as i64));
                }
                b'J' => {
                    let b = self.take(4)?; // BININT (signed)
                    self.push(PickleValue::Int(
                        i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as i64,
                    ));
                }
                b'I' => {
                    let line = self.read_line()?; // INT (text)
                    let value = match line.as_str() {
                        "00" => PickleValue::Bool(false),
                        "01" => PickleValue::Bool(true),
                        other => PickleValue::Int(
                            other
                                .parse::<i64>()
                                .map_err(|_| CueMolError::parse("pickle INT parse"))?,
                        ),
                    };
                    self.push(value);
                }
                0x8a => {
                    let n = self.byte()? as usize; // LONG1: short signed long
                    let raw = self.take(n)?;
                    self.push(PickleValue::Int(decode_long(raw)));
                }
                b'L' => {
                    let mut line = self.read_line()?; // LONG (text), trailing 'L'
                    if line.ends_with('L') {
                        line.pop();
                    }
                    self.push(PickleValue::Int(
                        line.parse::<i64>()
                            .map_err(|_| CueMolError::parse("pickle LONG parse"))?,
                    ));
                }
                b'G' => {
                    let b = self.take(8)?; // BINFLOAT (big-endian f64)
                    self.push(PickleValue::Float(f64::from_be_bytes([
                        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
                    ])));
                }
                b'F' => {
                    let line = self.read_line()?; // FLOAT (text)
                    self.push(PickleValue::Float(
                        line.parse::<f64>()
                            .map_err(|_| CueMolError::parse("pickle FLOAT parse"))?,
                    ));
                }
                b'X' => {
                    let n = self.u32_le()? as usize; // BINUNICODE
                    let raw = self.take(n)?;
                    self.push(PickleValue::Str(
                        std::str::from_utf8(raw)
                            .map_err(|_| CueMolError::parse("BINUNICODE not UTF-8"))?
                            .to_string(),
                    ));
                }
                0x8c => {
                    let n = self.byte()? as usize; // SHORT_BINUNICODE
                    let raw = self.take(n)?;
                    self.push(PickleValue::Str(
                        std::str::from_utf8(raw)
                            .map_err(|_| CueMolError::parse("SHORT_BINUNICODE not UTF-8"))?
                            .to_string(),
                    ));
                }
                b'U' => {
                    let n = self.byte()? as usize; // SHORT_BINSTRING
                    let raw = self.take(n)?.to_vec();
                    self.push(PickleValue::Bytes(raw));
                }
                b'T' => {
                    let n = self.u32_le()? as usize; // BINSTRING
                    let raw = self.take(n)?.to_vec();
                    self.push(PickleValue::Bytes(raw));
                }
                b'V' => {
                    let line = self.read_line()?; // UNICODE (text)
                    self.push(PickleValue::Str(line));
                }
                b'q' => {
                    let i = self.byte()? as usize; // BINPUT
                    self.memo_put(i)?;
                }
                b'r' => {
                    let i = self.u32_le()? as usize; // LONG_BINPUT
                    self.memo_put(i)?;
                }
                b'p' => {
                    let i: usize = self // PUT (text)
                        .read_line()?
                        .parse()
                        .map_err(|_| CueMolError::parse("pickle PUT index"))?;
                    self.memo_put(i)?;
                }
                b'h' => {
                    let i = self.byte()? as usize; // BINGET
                    self.memo_get(i)?;
                }
                b'j' => {
                    let i = self.u32_le()? as usize; // LONG_BINGET
                    self.memo_get(i)?;
                }
                b'g' => {
                    let i: usize = self // GET (text)
                        .read_line()?
                        .parse()
                        .map_err(|_| CueMolError::parse("pickle GET index"))?;
                    self.memo_get(i)?;
                }
                b'a' => {
                    let v = self.pop()?; // APPEND
                    self.top_list_extend(vec![v])?;
                }
                b'e' => {
                    let items = self.pop_to_mark()?; // APPENDS
                    self.top_list_extend(items)?;
                }
                b's' => {
                    let value = self.pop()?; // SETITEM
                    let key = self.pop()?;
                    self.top_dict_set(vec![(key, value)])?;
                }
                b'u' => {
                    let items = self.pop_to_mark()?; // SETITEMS
                    self.top_dict_set(into_pairs(items)?)?;
                }
                b'c' => {
                    let module = self.read_line()?; // GLOBAL
                    let name = self.read_line()?;
                    self.push(PickleValue::Global(module, name));
                }
                b'o' => {
                    let items = self.pop_to_mark()?; // OBJ (deferred: opaque)
                    self.push(PickleValue::Object(items));
                }
                0x81 => {
                    let args = self.pop()?; // NEWOBJ (deferred: opaque)
                    let class = self.pop()?;
                    self.push(PickleValue::Object(vec![class, args]));
                }
                b'b' => {
                    // BUILD (deferred): attach state to the object below it.
                    let state = self.pop()?;
                    let target = self.pop()?;
                    self.push(PickleValue::Object(vec![target, state]));
                }
                b'R' => {
                    // REDUCE (deferred): callable(argtuple) — kept opaque so the
                    // surrounding tree (e.g. a .pse session) still parses.
                    let args = self.pop()?;
                    let callable = self.pop()?;
                    self.push(PickleValue::Object(vec![callable, args]));
                }
                b'0' => {
                    self.pop()?; // POP
                }
                b'.' => return self.pop(), // STOP
                other => {
                    return Err(CueMolError::parse(format!(
                        "unsupported pickle opcode 0x{other:02x}"
                    )))
                }
            }
        }
    }
}

fn into_pairs(items: Vec<PickleValue>) -> Result<Vec<(PickleValue, PickleValue)>> {
    if !items.len().is_multiple_of(2) {
        return Err(CueMolError::parse("pickle dict has an odd item count"));
    }
    let mut pairs = Vec::with_capacity(items.len() / 2);
    let mut iter = items.into_iter();
    while let (Some(k), Some(v)) = (iter.next(), iter.next()) {
        pairs.push((k, v));
    }
    Ok(pairs)
}

/// Decode a little-endian two's-complement signed integer (pickle LONG1 body).
fn decode_long(raw: &[u8]) -> i64 {
    if raw.is_empty() {
        return 0;
    }
    let mut value: i64 = 0;
    for (i, &b) in raw.iter().enumerate() {
        value |= (b as i64) << (8 * i);
    }
    let bits = raw.len() * 8;
    if bits < 64 && (raw[raw.len() - 1] & 0x80) != 0 {
        value -= 1i64 << bits; // sign-extend
    }
    value
}

/// Parse a pickle byte stream into a [`PickleValue`] tree.
pub fn parse_pickle(bytes: &[u8]) -> Result<PickleValue> {
    PickleMachine::new(bytes).run()
}

#[cfg(test)]
mod tests {
    use super::*;

    // `pickle.dumps({...}, protocol=2)` of the nested object in the module test
    // (generated with CPython; this is the oracle — Python's encoder).
    const PROTO2: &[u8] = &[
        128, 2, 125, 113, 0, 40, 88, 7, 0, 0, 0, 118, 101, 114, 115, 105, 111, 110, 113, 1, 71, 63,
        251, 215, 10, 61, 112, 163, 215, 88, 5, 0, 0, 0, 110, 97, 109, 101, 115, 113, 2, 93, 113,
        3, 40, 88, 4, 0, 0, 0, 109, 111, 108, 49, 113, 4, 88, 4, 0, 0, 0, 109, 111, 108, 50, 113,
        5, 101, 88, 5, 0, 0, 0, 99, 111, 117, 110, 116, 113, 6, 75, 2, 88, 5, 0, 0, 0, 102, 108,
        97, 103, 115, 113, 7, 136, 137, 78, 135, 113, 8, 88, 4, 0, 0, 0, 109, 101, 116, 97, 113, 9,
        125, 113, 10, 40, 88, 5, 0, 0, 0, 116, 105, 116, 108, 101, 113, 11, 88, 4, 0, 0, 0, 116,
        101, 115, 116, 113, 12, 88, 3, 0, 0, 0, 110, 97, 116, 113, 13, 77, 71, 1, 117, 117, 46,
    ];

    fn expected() -> PickleValue {
        PickleValue::Dict(vec![
            (PickleValue::Str("version".into()), PickleValue::Float(1.74)),
            (
                PickleValue::Str("names".into()),
                PickleValue::List(vec![
                    PickleValue::Str("mol1".into()),
                    PickleValue::Str("mol2".into()),
                ]),
            ),
            (PickleValue::Str("count".into()), PickleValue::Int(2)),
            (
                PickleValue::Str("flags".into()),
                PickleValue::Tuple(vec![
                    PickleValue::Bool(true),
                    PickleValue::Bool(false),
                    PickleValue::None,
                ]),
            ),
            (
                PickleValue::Str("meta".into()),
                PickleValue::Dict(vec![
                    (
                        PickleValue::Str("title".into()),
                        PickleValue::Str("test".into()),
                    ),
                    (PickleValue::Str("nat".into()), PickleValue::Int(327)),
                ]),
            ),
        ])
    }

    #[test]
    fn parses_python_protocol2_pickle_against_cpython_oracle() {
        let value = parse_pickle(PROTO2).expect("parse pickle");
        assert_eq!(value, expected());
    }

    #[test]
    fn convenience_accessors_navigate_the_tree() {
        let value = parse_pickle(PROTO2).unwrap();
        assert!((value.get("version").unwrap().as_f64().unwrap() - 1.74).abs() < 1e-9);
        assert_eq!(value.get("count").unwrap().as_i64(), Some(2));
        assert_eq!(
            value.get("meta").unwrap().get("title").unwrap().as_str(),
            Some("test")
        );
        assert_eq!(
            value.get("meta").unwrap().get("nat").unwrap().as_i64(),
            Some(327)
        );
    }

    #[test]
    fn parses_protocol0_text_pickle() {
        // `pickle.dumps({'count': 2, 'flag': True, 'name': 'x'}, protocol=0)`.
        let proto0: &[u8] = &[
            40, 100, 112, 48, 10, 86, 99, 111, 117, 110, 116, 10, 112, 49, 10, 73, 50, 10, 115, 86,
            102, 108, 97, 103, 10, 112, 50, 10, 73, 48, 49, 10, 115, 86, 110, 97, 109, 101, 10,
            112, 51, 10, 86, 120, 10, 112, 52, 10, 115, 46,
        ];
        let value = parse_pickle(proto0).expect("parse proto0");
        assert_eq!(value.get("count").unwrap().as_i64(), Some(2));
        assert_eq!(value.get("flag"), Some(&PickleValue::Bool(true)));
        assert_eq!(value.get("name").unwrap().as_str(), Some("x"));
    }

    #[test]
    fn accessors_match_python2_style_bytes_keys() {
        // Hand-encoded proto-1 pickle with SHORT_BINSTRING keys, as real
        // (Python-2) PyMOL .pse emit: `{b'count': 2, b'name': b'x'}`. The keys
        // decode to `Bytes`, but the &str accessors must still find them.
        let bytes: &[u8] = &[
            0x7d, 0x28, 0x55, 5, b'c', b'o', b'u', b'n', b't', 0x4b, 2, 0x55, 4, b'n', b'a', b'm',
            b'e', 0x55, 1, b'x', 0x75, 0x2e,
        ];
        let value = parse_pickle(bytes).unwrap();
        // The keys really are Bytes, not Str...
        match &value {
            PickleValue::Dict(pairs) => {
                assert!(matches!(pairs[0].0, PickleValue::Bytes(_)));
            }
            _ => panic!("expected dict"),
        }
        // ...yet the string accessors resolve them (Python-2 .pse compatibility).
        assert_eq!(value.get("count").unwrap().as_i64(), Some(2));
        assert_eq!(value.get("name").unwrap().as_str(), Some("x"));
    }

    #[test]
    fn rejects_unknown_opcode() {
        assert!(parse_pickle(&[0xff, b'.']).is_err());
    }
}
