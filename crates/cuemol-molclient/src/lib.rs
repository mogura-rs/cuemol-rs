use cuemol_core::{CueMolError, Result, Vector4};
use cuemol_molstr::{Atom, BondOrder, MolCoord};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MolClientManager {
    connections: Vec<XmlRpcConn>,
}

impl Default for MolClientManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MolClientManager {
    pub fn new() -> Self {
        Self {
            connections: Vec::new(),
        }
    }

    pub fn connect(&mut self, endpoint: impl Into<String>) -> XmlRpcConn {
        let connection = XmlRpcConn::new(endpoint);
        self.connections.push(connection.clone());
        connection
    }

    pub fn connections(&self) -> &[XmlRpcConn] {
        &self.connections
    }

    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlRpcConn {
    endpoint: String,
    enabled: bool,
    request_count: usize,
}

impl XmlRpcConn {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            enabled: true,
            request_count: 0,
        }
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_enabled(&mut self, value: bool) {
        self.enabled = value;
    }

    pub fn request_count(&self) -> usize {
        self.request_count
    }

    pub fn mol_from_smiles(&mut self, smiles: &str) -> Result<MolCoord> {
        if !self.enabled {
            return Err(CueMolError::invalid_argument(
                "molclient connection is disabled",
            ));
        }
        self.request_count += 1;
        SmilesParser::new(smiles).parse()
    }
}

struct SmilesParser<'a> {
    input: &'a str,
    chars: Vec<char>,
    pos: usize,
    mol: MolCoord,
    aromatic: Vec<bool>,
    current_atom: Option<usize>,
    branch_stack: Vec<Option<usize>>,
    ring_open: BTreeMap<String, (usize, BondOrder)>,
    pending_bond: Option<BondOrder>,
}

impl<'a> SmilesParser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            chars: input.chars().collect(),
            pos: 0,
            mol: MolCoord::new(),
            aromatic: Vec::new(),
            current_atom: None,
            branch_stack: Vec::new(),
            ring_open: BTreeMap::new(),
            pending_bond: None,
        }
    }

    fn parse(mut self) -> Result<MolCoord> {
        while let Some(ch) = self.peek() {
            match ch {
                '-' => {
                    self.pending_bond = Some(BondOrder::Single);
                    self.pos += 1;
                }
                '=' => {
                    self.pending_bond = Some(BondOrder::Double);
                    self.pos += 1;
                }
                '#' => {
                    self.pending_bond = Some(BondOrder::Triple);
                    self.pos += 1;
                }
                ':' => {
                    self.pending_bond = Some(BondOrder::Aromatic);
                    self.pos += 1;
                }
                '/' | '\\' => {
                    self.pending_bond.get_or_insert(BondOrder::Single);
                    self.pos += 1;
                }
                '(' => {
                    self.branch_stack.push(self.current_atom);
                    self.pos += 1;
                }
                ')' => {
                    self.current_atom = self
                        .branch_stack
                        .pop()
                        .ok_or_else(|| CueMolError::parse("unmatched SMILES branch close"))?;
                    self.pos += 1;
                }
                '.' => {
                    self.current_atom = None;
                    self.pending_bond = None;
                    self.pos += 1;
                }
                '0'..='9' | '%' => self.parse_ring_closure()?,
                '[' => {
                    let atom = self.parse_bracket_atom()?;
                    self.add_atom(atom);
                }
                _ if ch.is_ascii_alphabetic() => {
                    let atom = self.parse_atom()?;
                    self.add_atom(atom);
                }
                _ if ch.is_whitespace() => {
                    self.pos += 1;
                }
                _ => {
                    return Err(CueMolError::parse(format!(
                        "unsupported SMILES token '{ch}' in {}",
                        self.input
                    )))
                }
            }
        }

        if !self.branch_stack.is_empty() {
            return Err(CueMolError::parse("unmatched SMILES branch open"));
        }
        if !self.ring_open.is_empty() {
            return Err(CueMolError::parse("unclosed SMILES ring"));
        }
        if self.mol.atom_count() == 0 {
            return Err(CueMolError::parse("empty SMILES"));
        }
        Ok(self.mol)
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn parse_atom(&mut self) -> Result<SmilesAtom> {
        let ch = self
            .peek()
            .ok_or_else(|| CueMolError::parse("unexpected end of SMILES"))?;
        let next = self.chars.get(self.pos + 1).copied();
        let aromatic = ch.is_ascii_lowercase();
        let element = match (ch, next) {
            ('C', Some('l')) => {
                self.pos += 2;
                "Cl".to_string()
            }
            ('B', Some('r')) => {
                self.pos += 2;
                "Br".to_string()
            }
            ('B' | 'C' | 'N' | 'O' | 'P' | 'S' | 'F' | 'I', _) => {
                self.pos += 1;
                ch.to_string()
            }
            ('b' | 'c' | 'n' | 'o' | 'p' | 's', _) => {
                self.pos += 1;
                ch.to_ascii_uppercase().to_string()
            }
            _ => {
                return Err(CueMolError::parse(format!(
                    "unsupported SMILES atom '{ch}'"
                )))
            }
        };
        Ok(SmilesAtom {
            element,
            aromatic,
            formal_charge: None,
        })
    }

    fn parse_bracket_atom(&mut self) -> Result<SmilesAtom> {
        self.pos += 1;
        let start = self.pos;
        while let Some(ch) = self.peek() {
            if ch == ']' {
                break;
            }
            self.pos += 1;
        }
        if self.peek() != Some(']') {
            return Err(CueMolError::parse("unterminated bracket atom"));
        }
        let content = self.chars[start..self.pos].iter().collect::<String>();
        self.pos += 1;
        parse_bracket_content(&content)
    }

    fn parse_ring_closure(&mut self) -> Result<()> {
        let ring_id = if self.peek() == Some('%') {
            self.pos += 1;
            let first = self
                .peek()
                .filter(char::is_ascii_digit)
                .ok_or_else(|| CueMolError::parse("invalid two-digit SMILES ring"))?;
            self.pos += 1;
            let second = self
                .peek()
                .filter(char::is_ascii_digit)
                .ok_or_else(|| CueMolError::parse("invalid two-digit SMILES ring"))?;
            self.pos += 1;
            format!("{first}{second}")
        } else {
            let digit = self
                .peek()
                .filter(char::is_ascii_digit)
                .ok_or_else(|| CueMolError::parse("invalid SMILES ring"))?;
            self.pos += 1;
            digit.to_string()
        };
        let atom = self
            .current_atom
            .ok_or_else(|| CueMolError::parse("ring closure without atom"))?;
        let order = self.pending_bond.take().unwrap_or(BondOrder::Single);
        if let Some((other_atom, other_order)) = self.ring_open.remove(&ring_id) {
            let final_order = if order == BondOrder::Single {
                other_order
            } else {
                order
            };
            self.mol.add_bond(other_atom, atom, final_order);
        } else {
            self.ring_open.insert(ring_id, (atom, order));
        }
        Ok(())
    }

    fn add_atom(&mut self, smiles_atom: SmilesAtom) {
        let atom_index = self.mol.atom_count();
        let serial = atom_index as i32 + 1;
        let y_offset = self.branch_stack.len() as f64 * 1.25;
        self.mol.push_atom(Atom {
            serial,
            name: format!("{}{}", smiles_atom.element, serial),
            alt_loc: None,
            res_name: "SMI".to_string(),
            chain_id: "A".to_string(),
            res_id: 1,
            ins_code: None,
            position: Vector4::new3(atom_index as f64 * 1.5, y_offset, 0.0),
            occupancy: 1.0,
            b_factor: 0.0,
            element: smiles_atom.element,
            formal_charge: smiles_atom.formal_charge,
            int_props: BTreeMap::new(),
        });
        self.aromatic.push(smiles_atom.aromatic);
        if let Some(previous) = self.current_atom {
            let order = self.pending_bond.take().unwrap_or_else(|| {
                if self.aromatic.get(previous).copied().unwrap_or(false) && smiles_atom.aromatic {
                    BondOrder::Aromatic
                } else {
                    BondOrder::Single
                }
            });
            self.mol.add_bond(previous, atom_index, order);
        }
        self.current_atom = Some(atom_index);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SmilesAtom {
    element: String,
    aromatic: bool,
    formal_charge: Option<i32>,
}

fn parse_bracket_content(content: &str) -> Result<SmilesAtom> {
    let mut chars = content.chars().peekable();
    while matches!(chars.peek(), Some('0'..='9')) {
        chars.next();
    }
    let first = chars
        .next()
        .ok_or_else(|| CueMolError::parse("empty bracket atom"))?;
    let aromatic = first.is_ascii_lowercase();
    let mut element = first.to_ascii_uppercase().to_string();
    if let Some(second) = chars.peek().copied() {
        if first.is_ascii_uppercase() && second.is_ascii_lowercase() {
            element.push(second);
            chars.next();
        }
    }
    let rest = chars.collect::<String>();
    let formal_charge = parse_charge(&rest);
    Ok(SmilesAtom {
        element,
        aromatic,
        formal_charge,
    })
}

fn parse_charge(input: &str) -> Option<i32> {
    let chars = input.chars().collect::<Vec<_>>();
    let index = chars.iter().position(|ch| matches!(ch, '+' | '-'))?;
    let sign = if chars[index] == '+' { 1 } else { -1 };
    let mut count = 1;
    let mut digits = String::new();
    for ch in chars.iter().skip(index + 1) {
        match ch {
            '+' if sign > 0 => count += 1,
            '-' if sign < 0 => count += 1,
            '0'..='9' => digits.push(*ch),
            _ => break,
        }
    }
    if !digits.is_empty() {
        digits.parse::<i32>().ok().map(|value| sign * value)
    } else {
        Some(sign * count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manager_connect_records_endpoint() {
        let mut manager = MolClientManager::new();
        let conn = manager.connect("http://localhost:8080/RPC2");

        assert_eq!(conn.endpoint(), "http://localhost:8080/RPC2");
        assert_eq!(manager.connection_count(), 1);
        assert_eq!(manager.connections()[0].endpoint(), conn.endpoint());
    }

    #[test]
    fn mol_from_smiles_builds_linear_molecule() {
        let mut conn = XmlRpcConn::new("offline");
        let mol = conn.mol_from_smiles("CCO").unwrap();

        assert_eq!(mol.atom_count(), 3);
        assert_eq!(mol.bond_count(), 2);
        assert_eq!(mol.atoms()[2].element, "O");
        assert_eq!(conn.request_count(), 1);
    }

    #[test]
    fn mol_from_smiles_supports_branch_ring_and_bond_orders() {
        let mut conn = XmlRpcConn::new("offline");
        let mol = conn.mol_from_smiles("C1=CC(Cl)=CC=C1").unwrap();

        assert_eq!(mol.atom_count(), 7);
        assert_eq!(mol.bond_count(), 7);
        assert_eq!(mol.atoms()[3].element, "Cl");
        assert!(mol
            .bonds()
            .iter()
            .any(|bond| bond.order == BondOrder::Double));
    }

    #[test]
    fn mol_from_smiles_supports_aromatic_atoms() {
        let mut conn = XmlRpcConn::new("offline");
        let mol = conn.mol_from_smiles("c1ccccc1").unwrap();

        assert_eq!(mol.atom_count(), 6);
        assert_eq!(mol.bond_count(), 6);
        assert!(mol
            .bonds()
            .iter()
            .any(|bond| bond.order == BondOrder::Aromatic));
    }

    #[test]
    fn disabled_connection_rejects_requests() {
        let mut conn = XmlRpcConn::new("offline");
        conn.set_enabled(false);

        assert!(conn.mol_from_smiles("C").is_err());
        assert_eq!(conn.request_count(), 0);
    }
}
