use cuemol_core::{parse_ldom2_xml, CueMolError, LDom2Node, Result, Vector4};
use cuemol_molstr::{read_sdf_str, Atom, BondOrder, MolCoord};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::{Command, Stdio};
use std::time::Duration;

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
        if self.endpoint == "offline" {
            return self.mol_from_smiles_local(smiles);
        }
        let request = xml_rpc_smiles_request(smiles);
        let response = http_xml_rpc_post(&self.endpoint, request.as_bytes())?;
        let sdf = xml_rpc_string_result(&response)?;
        read_sdf_str(&sdf)
    }

    /// Parse SMILES without contacting the XML-RPC service.
    pub fn mol_from_smiles_local(&self, smiles: &str) -> Result<MolCoord> {
        SmilesParser::new(smiles).parse()
    }
}

fn xml_rpc_smiles_request(smiles: &str) -> String {
    format!(
        "<?xml version=\"1.0\"?>\n<methodCall><methodName>mol_from_smiles</methodName><params><param><value><string>{}</string></value></param></params></methodCall>\n",
        escape_xml(smiles)
    )
}

fn http_xml_rpc_post(endpoint: &str, body: &[u8]) -> Result<String> {
    let target = HttpTarget::parse(endpoint)?;
    if target.secure {
        return curl_xml_rpc_post(endpoint, body);
    }
    let mut stream = TcpStream::connect((target.host.as_str(), target.port))?;
    let timeout = Some(Duration::from_secs(30));
    stream.set_read_timeout(timeout)?;
    stream.set_write_timeout(timeout)?;
    let request = format!(
        "POST {} HTTP/1.1\r\nHost: {}\r\nContent-Type: text/xml\r\nContent-Length: {}\r\nConnection: close\r\nUser-Agent: cuemol2-rs/0.1\r\n\r\n",
        target.path,
        target.host_header,
        body.len()
    );
    stream.write_all(request.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()?;

    let mut response = Vec::new();
    stream.read_to_end(&mut response)?;
    parse_http_response(&response)
}

fn curl_xml_rpc_post(endpoint: &str, body: &[u8]) -> Result<String> {
    let mut child = Command::new("curl")
        .args([
            "--silent",
            "--show-error",
            "--fail",
            "--max-time",
            "30",
            "--header",
            "Content-Type: text/xml",
            "--data-binary",
            "@-",
            endpoint,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| CueMolError::Io(format!("failed to start curl: {error}")))?;
    child
        .stdin
        .take()
        .ok_or_else(|| CueMolError::Io("curl stdin is unavailable".to_string()))?
        .write_all(body)?;
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(CueMolError::Io(format!(
            "XML-RPC HTTPS request failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    String::from_utf8(output.stdout)
        .map_err(|error| CueMolError::parse(format!("XML-RPC response is not UTF-8: {error}")))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HttpTarget {
    secure: bool,
    host: String,
    host_header: String,
    port: u16,
    path: String,
}

impl HttpTarget {
    fn parse(endpoint: &str) -> Result<Self> {
        let endpoint = endpoint.trim();
        let (secure, rest) = if let Some(rest) = endpoint.strip_prefix("http://") {
            (false, rest)
        } else if let Some(rest) = endpoint.strip_prefix("https://") {
            (true, rest)
        } else {
            return Err(CueMolError::invalid_argument(
                "XML-RPC endpoint must use http:// or https://",
            ));
        };
        let (authority, path) = rest
            .split_once('/')
            .map_or((rest, "/".to_string()), |(authority, path)| {
                (authority, format!("/{path}"))
            });
        if authority.is_empty() {
            return Err(CueMolError::invalid_argument(
                "XML-RPC endpoint host is empty",
            ));
        }

        let (host, port) = if let Some(bracketed) = authority.strip_prefix('[') {
            let (host, suffix) = bracketed
                .split_once(']')
                .ok_or_else(|| CueMolError::invalid_argument("invalid bracketed XML-RPC host"))?;
            let port = suffix
                .strip_prefix(':')
                .map(str::parse)
                .transpose()
                .map_err(|_| CueMolError::invalid_argument("invalid XML-RPC port"))?
                .unwrap_or(if secure { 443 } else { 80 });
            (host.to_string(), port)
        } else if let Some((host, port)) = authority.rsplit_once(':') {
            let port = port
                .parse()
                .map_err(|_| CueMolError::invalid_argument("invalid XML-RPC port"))?;
            (host.to_string(), port)
        } else {
            (authority.to_string(), if secure { 443 } else { 80 })
        };
        if host.is_empty() {
            return Err(CueMolError::invalid_argument(
                "XML-RPC endpoint host is empty",
            ));
        }
        Ok(Self {
            secure,
            host,
            host_header: authority.to_string(),
            port,
            path,
        })
    }
}

fn parse_http_response(response: &[u8]) -> Result<String> {
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| CueMolError::parse("invalid HTTP response"))?;
    let headers = std::str::from_utf8(&response[..header_end])
        .map_err(|error| CueMolError::parse(format!("HTTP headers are not UTF-8: {error}")))?;
    let mut lines = headers.split("\r\n");
    let status_line = lines
        .next()
        .ok_or_else(|| CueMolError::parse("HTTP status line is missing"))?;
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| CueMolError::parse("invalid HTTP status line"))?;
    if !(200..300).contains(&status) {
        return Err(CueMolError::Io(format!(
            "XML-RPC server returned HTTP {status}"
        )));
    }

    let chunked = lines.clone().any(|line| {
        line.split_once(':').is_some_and(|(name, value)| {
            name.eq_ignore_ascii_case("transfer-encoding")
                && value
                    .split(',')
                    .any(|encoding| encoding.trim().eq_ignore_ascii_case("chunked"))
        })
    });
    let body = &response[header_end + 4..];
    let body = if chunked {
        decode_chunked_body(body)?
    } else {
        body.to_vec()
    };
    String::from_utf8(body)
        .map_err(|error| CueMolError::parse(format!("XML-RPC response is not UTF-8: {error}")))
}

fn decode_chunked_body(input: &[u8]) -> Result<Vec<u8>> {
    let mut position = 0;
    let mut output = Vec::new();
    loop {
        let line_end = input[position..]
            .windows(2)
            .position(|window| window == b"\r\n")
            .map(|offset| position + offset)
            .ok_or_else(|| CueMolError::parse("invalid chunked HTTP body"))?;
        let size_text = std::str::from_utf8(&input[position..line_end])
            .map_err(|_| CueMolError::parse("invalid HTTP chunk size"))?;
        let size = usize::from_str_radix(size_text.split(';').next().unwrap_or_default(), 16)
            .map_err(|_| CueMolError::parse("invalid HTTP chunk size"))?;
        position = line_end + 2;
        if size == 0 {
            return Ok(output);
        }
        let end = position
            .checked_add(size)
            .filter(|end| *end + 2 <= input.len())
            .ok_or_else(|| CueMolError::parse("truncated HTTP chunk"))?;
        output.extend_from_slice(&input[position..end]);
        if &input[end..end + 2] != b"\r\n" {
            return Err(CueMolError::parse("invalid HTTP chunk terminator"));
        }
        position = end + 2;
    }
}

fn xml_rpc_string_result(response: &str) -> Result<String> {
    let tree = parse_ldom2_xml(response)?;
    if tree.top().tag_name() != "methodResponse" {
        return Err(CueMolError::parse("XML-RPC methodResponse is missing"));
    }
    if let Some(fault) = descendant(tree.top(), "fault") {
        let message = descendant(fault, "string")
            .map(LDom2Node::contents)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("XML-RPC fault");
        return Err(CueMolError::invalid_argument(message.trim()));
    }
    descendant(tree.top(), "string")
        .map(|node| node.contents().to_string())
        .ok_or_else(|| CueMolError::parse("XML-RPC string result is missing"))
}

fn descendant<'a>(node: &'a LDom2Node, tag: &str) -> Option<&'a LDom2Node> {
    if node.tag_name() == tag {
        return Some(node);
    }
    node.children()
        .iter()
        .find_map(|child| descendant(child, tag))
}

fn escape_xml(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
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
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn manager_connect_records_endpoint() {
        let mut manager = MolClientManager::new();
        let conn = manager.connect("http://localhost:8080/RPC2");

        assert_eq!(conn.endpoint(), "http://localhost:8080/RPC2");
        assert_eq!(manager.connection_count(), 1);
        assert_eq!(manager.connections()[0].endpoint(), conn.endpoint());
    }

    #[test]
    fn http_target_accepts_https_and_ipv6_defaults() {
        let secure = HttpTarget::parse("https://example.org/RPC2").unwrap();
        assert!(secure.secure);
        assert_eq!(secure.host, "example.org");
        assert_eq!(secure.port, 443);
        assert_eq!(secure.path, "/RPC2");

        let ipv6 = HttpTarget::parse("http://[::1]:8080/rpc").unwrap();
        assert!(!ipv6.secure);
        assert_eq!(ipv6.host, "::1");
        assert_eq!(ipv6.port, 8080);
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

    #[test]
    fn mol_from_smiles_calls_xml_rpc_and_reads_sdf_result() {
        let source = XmlRpcConn::new("offline")
            .mol_from_smiles_local("CCO")
            .unwrap();
        let sdf = cuemol_molstr::write_sdf_string(&source);
        let response_xml = format!(
            "<?xml version=\"1.0\"?><methodResponse><params><param><value><string>{}</string></value></param></params></methodResponse>",
            escape_xml(&sdf)
        );
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let request = read_http_request(&mut socket);
            let request = String::from_utf8(request).unwrap();
            assert!(request.starts_with("POST /RPC2 HTTP/1.1\r\n"));
            assert!(request.contains("<methodName>mol_from_smiles</methodName>"));
            assert!(request.contains("<string>CCO</string>"));
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_xml.len(),
                response_xml
            );
            socket.write_all(response.as_bytes()).unwrap();
        });

        let mut connection = XmlRpcConn::new(format!("http://{address}/RPC2"));
        let molecule = connection.mol_from_smiles("CCO").unwrap();
        assert_eq!(molecule.atom_count(), 3);
        assert_eq!(molecule.bond_count(), 2);
        assert_eq!(molecule.atoms()[2].element, "O");
        assert_eq!(connection.request_count(), 1);
        server.join().unwrap();
    }

    fn read_http_request(socket: &mut TcpStream) -> Vec<u8> {
        let mut request = Vec::new();
        let mut buffer = [0_u8; 1024];
        loop {
            let count = socket.read(&mut buffer).unwrap();
            assert!(count != 0, "client closed before completing HTTP request");
            request.extend_from_slice(&buffer[..count]);
            let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n")
            else {
                continue;
            };
            let headers = std::str::from_utf8(&request[..header_end]).unwrap();
            let content_length = headers
                .split("\r\n")
                .find_map(|line| {
                    line.split_once(':').and_then(|(name, value)| {
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().unwrap())
                    })
                })
                .unwrap();
            if request.len() >= header_end + 4 + content_length {
                return request;
            }
        }
    }
}
