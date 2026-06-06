use crate::{CueMolError, LString, Result};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LDom2Node {
    tag_name: String,
    value: String,
    type_name: String,
    contents: String,
    attr_flag: bool,
    default_flag: bool,
    consumed: bool,
    error_msgs: Vec<String>,
    children: Vec<LDom2Node>,
}

impl LDom2Node {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn tag_name(&self) -> &str {
        &self.tag_name
    }

    pub fn set_tag_name(&mut self, value: impl Into<String>) {
        self.tag_name = value.into();
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn set_value(&mut self, value: impl Into<String>) {
        self.value = value.into();
    }

    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    pub fn set_type_name(&mut self, value: impl Into<String>) {
        self.type_name = value.into();
    }

    pub fn contents(&self) -> &str {
        &self.contents
    }

    pub fn set_contents(&mut self, value: impl Into<String>) {
        self.contents = value.into();
    }

    pub fn append_contents(&mut self, value: impl AsRef<str>) {
        self.contents.push_str(value.as_ref());
    }

    pub fn clear_contents(&mut self) {
        self.contents.clear();
    }

    pub fn is_attr(&self) -> bool {
        self.attr_flag
    }

    pub fn set_attr_flag(&mut self, value: bool) {
        self.attr_flag = value;
    }

    pub fn is_default(&self) -> bool {
        self.default_flag
    }

    pub fn set_default_flag(&mut self, value: bool) {
        self.default_flag = value;
    }

    pub fn is_consumed(&self) -> bool {
        self.consumed
    }

    pub fn set_consumed(&mut self, value: bool) {
        self.consumed = value;
    }

    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    pub fn append_child(&mut self, tag_name: impl Into<String>) -> &mut LDom2Node {
        let mut child = LDom2Node::new();
        child.set_tag_name(tag_name);
        self.children.push(child);
        self.children.last_mut().unwrap()
    }

    pub fn append_child_node(&mut self, child: LDom2Node) {
        self.children.push(child);
    }

    pub fn find_child(&self, tag_name: &str) -> Option<&LDom2Node> {
        self.children
            .iter()
            .find(|child| child.tag_name == tag_name)
    }

    pub fn find_child_mut(&mut self, tag_name: &str) -> Option<&mut LDom2Node> {
        self.children
            .iter_mut()
            .find(|child| child.tag_name == tag_name)
    }

    pub fn remove_child(&mut self, tag_name: &str) -> bool {
        let before = self.children.len();
        self.children.retain(|child| child.tag_name != tag_name);
        self.children.len() != before
    }

    pub fn str_attr(&self, tag_name: &str) -> LString {
        self.find_child(tag_name)
            .filter(|child| !child.value.is_empty())
            .map(|child| LString::from(child.value.clone()))
            .unwrap_or_default()
    }

    pub fn set_str_attr(&mut self, tag_name: impl Into<String>, value: impl Into<String>) {
        let tag_name = tag_name.into();
        let value = value.into();
        if let Some(child) = self.find_child_mut(&tag_name) {
            child.value = value;
            child.attr_flag = true;
            return;
        }
        let mut child = LDom2Node::new();
        child.set_tag_name(tag_name);
        child.set_value(value);
        child.set_attr_flag(true);
        self.children.push(child);
    }

    pub fn bool_attr(&self, tag_name: &str) -> bool {
        self.str_attr(tag_name).equals("true")
    }

    pub fn children_consumed(&self) -> bool {
        self.children.iter().all(LDom2Node::is_consumed)
    }

    pub fn append_err_msg(&mut self, value: impl Into<String>) {
        self.error_msgs.push(value.into());
    }

    pub fn error_msgs(&self) -> LString {
        let mut values = self.error_msgs.clone();
        for child in &self.children {
            let child_msgs = child.error_msgs();
            if !child_msgs.is_empty() {
                values.push(child_msgs.to_string());
            }
        }
        LString::from(values.join("\n"))
    }

    pub fn children(&self) -> &[LDom2Node] {
        &self.children
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LDom2Tree {
    top: LDom2Node,
}

impl Default for LDom2Tree {
    fn default() -> Self {
        Self::new("")
    }
}

impl LDom2Tree {
    pub fn new(top_name: impl Into<String>) -> Self {
        let mut top = LDom2Node::new();
        top.set_tag_name(top_name);
        Self { top }
    }

    pub fn top(&self) -> &LDom2Node {
        &self.top
    }

    pub fn top_mut(&mut self) -> &mut LDom2Node {
        &mut self.top
    }

    pub fn detach(self) -> LDom2Node {
        self.top
    }
}

pub fn write_ldom2_xml(tree: &LDom2Tree) -> String {
    let mut output = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    write_node(&mut output, tree.top(), 0);
    output
}

pub fn parse_ldom2_xml(input: &str) -> Result<LDom2Tree> {
    XmlParser::new(input).parse_document()
}

fn write_node(output: &mut String, node: &LDom2Node, indent: usize) {
    let pad = "  ".repeat(indent);
    output.push_str(&pad);
    output.push('<');
    output.push_str(node.tag_name());

    let type_attr = node.str_attr("type").to_string();
    let type_value = if !node.type_name().is_empty() {
        node.type_name()
    } else {
        &type_attr
    };
    if !type_value.is_empty() {
        write_attr(output, "type", type_value);
    }
    if !node.value().is_empty() {
        write_attr(output, "value", node.value());
    }
    for child in node.children().iter().filter(|child| child.is_attr()) {
        if child.tag_name() != "type" && child.tag_name() != "id" && !child.value().is_empty() {
            write_attr(output, child.tag_name(), child.value());
        }
    }

    let element_children = node
        .children()
        .iter()
        .filter(|child| !child.is_attr() && !child.is_default())
        .collect::<Vec<_>>();

    if element_children.is_empty() && node.contents().is_empty() {
        output.push_str("/>");
        return;
    }

    output.push('>');
    if !node.contents().is_empty() {
        output.push_str("<![CDATA[");
        output.push_str(node.contents());
        output.push_str("]]>");
    }
    if !element_children.is_empty() {
        output.push('\n');
        for child in element_children {
            write_node(output, child, indent + 1);
            output.push('\n');
        }
        output.push_str(&pad);
    }
    output.push_str("</");
    output.push_str(node.tag_name());
    output.push('>');
}

fn write_attr(output: &mut String, name: &str, value: &str) {
    output.push(' ');
    output.push_str(name);
    output.push_str("=\"");
    output.push_str(&escape_xml_attr(value));
    output.push('"');
}

fn escape_xml_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

struct XmlParser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> XmlParser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input: input.strip_prefix('\u{feff}').unwrap_or(input),
            pos: 0,
        }
    }

    fn parse_document(mut self) -> Result<LDom2Tree> {
        self.skip_misc()?;
        let top = self.parse_element()?;
        self.skip_misc()?;
        if !self.is_eof() {
            return Err(self.error("unexpected content after root element"));
        }
        Ok(LDom2Tree { top })
    }

    fn parse_element(&mut self) -> Result<LDom2Node> {
        self.expect("<")?;
        if self.starts_with("/") || self.starts_with("!") || self.starts_with("?") {
            return Err(self.error("expected start element"));
        }

        let tag_name = self.parse_name()?;
        let mut node = LDom2Node::new();
        node.set_tag_name(tag_name);

        loop {
            self.skip_ws();
            if self.consume("/>") {
                return Ok(node);
            }
            if self.consume(">") {
                break;
            }
            let (key, value) = self.parse_attribute()?;
            if key == "type" {
                node.set_type_name(value.clone());
                node.set_str_attr(key, value);
            } else if key == "value" {
                node.set_value(value);
            } else {
                node.set_str_attr(key, value);
            }
        }

        loop {
            if self.consume("</") {
                let end_name = self.parse_name()?;
                self.skip_ws();
                self.expect(">")?;
                if end_name != node.tag_name() {
                    return Err(self.error(format!(
                        "mismatched end tag: expected {}, got {end_name}",
                        node.tag_name()
                    )));
                }
                return Ok(node);
            }
            if self.starts_with("<![CDATA[") {
                node.append_contents(self.parse_cdata()?);
                continue;
            }
            if self.starts_with("<!--") {
                self.skip_until("-->")?;
                continue;
            }
            if self.starts_with("<?") {
                self.skip_until("?>")?;
                continue;
            }
            if self.starts_with("<!") {
                self.skip_declaration()?;
                continue;
            }
            if self.starts_with("<") {
                let child = self.parse_element()?;
                node.append_child_node(child);
                continue;
            }
            if self.is_eof() {
                return Err(self.error("unexpected end of XML"));
            }
            let text = self.take_until('<');
            node.append_contents(unescape_xml(&text)?);
        }
    }

    fn parse_attribute(&mut self) -> Result<(String, String)> {
        let key = self.parse_name()?;
        self.skip_ws();
        self.expect("=")?;
        self.skip_ws();
        let quote = self
            .peek_char()
            .ok_or_else(|| self.error("expected attribute quote"))?;
        if quote != '"' && quote != '\'' {
            return Err(self.error("expected attribute quote"));
        }
        self.pos += quote.len_utf8();
        let value = self.take_until(quote);
        self.expect_char(quote)?;
        Ok((key, unescape_xml(&value)?))
    }

    fn parse_name(&mut self) -> Result<String> {
        let start = self.pos;
        while let Some(ch) = self.peek_char() {
            if ch.is_alphanumeric() || matches!(ch, '_' | '-' | ':' | '.') {
                self.pos += ch.len_utf8();
            } else {
                break;
            }
        }
        if self.pos == start {
            Err(self.error("expected XML name"))
        } else {
            Ok(self.input[start..self.pos].to_string())
        }
    }

    fn parse_cdata(&mut self) -> Result<String> {
        self.expect("<![CDATA[")?;
        let start = self.pos;
        let end = self
            .input
            .get(self.pos..)
            .and_then(|rest| rest.find("]]>"))
            .ok_or_else(|| self.error("unterminated CDATA section"))?;
        self.pos += end + 3;
        Ok(self.input[start..start + end].to_string())
    }

    fn skip_misc(&mut self) -> Result<()> {
        loop {
            self.skip_ws();
            if self.starts_with("<?") {
                self.skip_until("?>")?;
            } else if self.starts_with("<!--") {
                self.skip_until("-->")?;
            } else if self.starts_with("<!") {
                self.skip_declaration()?;
            } else {
                return Ok(());
            }
        }
    }

    fn skip_declaration(&mut self) -> Result<()> {
        self.expect("<!")?;
        let mut quote: Option<char> = None;
        while let Some(ch) = self.peek_char() {
            self.pos += ch.len_utf8();
            if let Some(active_quote) = quote {
                if ch == active_quote {
                    quote = None;
                }
            } else if ch == '"' || ch == '\'' {
                quote = Some(ch);
            } else if ch == '>' {
                return Ok(());
            }
        }
        Err(self.error("unterminated declaration"))
    }

    fn skip_until(&mut self, marker: &str) -> Result<()> {
        let rest = self
            .input
            .get(self.pos..)
            .ok_or_else(|| self.error("invalid parser offset"))?;
        let end = rest
            .find(marker)
            .ok_or_else(|| self.error(format!("unterminated XML section: {marker}")))?;
        self.pos += end + marker.len();
        Ok(())
    }

    fn skip_ws(&mut self) {
        while let Some(ch) = self.peek_char() {
            if ch.is_whitespace() {
                self.pos += ch.len_utf8();
            } else {
                break;
            }
        }
    }

    fn take_until(&mut self, marker: char) -> String {
        let start = self.pos;
        while let Some(ch) = self.peek_char() {
            if ch == marker {
                break;
            }
            self.pos += ch.len_utf8();
        }
        self.input[start..self.pos].to_string()
    }

    fn consume(&mut self, token: &str) -> bool {
        if self.starts_with(token) {
            self.pos += token.len();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, token: &str) -> Result<()> {
        if self.consume(token) {
            Ok(())
        } else {
            Err(self.error(format!("expected {token:?}")))
        }
    }

    fn expect_char(&mut self, expected: char) -> Result<()> {
        match self.peek_char() {
            Some(ch) if ch == expected => {
                self.pos += ch.len_utf8();
                Ok(())
            }
            _ => Err(self.error(format!("expected {expected:?}"))),
        }
    }

    fn starts_with(&self, token: &str) -> bool {
        self.input
            .get(self.pos..)
            .is_some_and(|rest| rest.starts_with(token))
    }

    fn peek_char(&self) -> Option<char> {
        self.input.get(self.pos..)?.chars().next()
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn error(&self, message: impl Into<String>) -> CueMolError {
        CueMolError::parse(format!("{} at byte {}", message.into(), self.pos))
    }
}

fn unescape_xml(value: &str) -> Result<String> {
    let mut output = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '&' {
            output.push(ch);
            continue;
        }

        let mut entity = String::new();
        let mut terminated = false;
        for next in chars.by_ref() {
            if next == ';' {
                terminated = true;
                break;
            }
            entity.push(next);
        }
        if !terminated {
            return Err(CueMolError::parse("unterminated XML entity"));
        }

        match entity.as_str() {
            "amp" => output.push('&'),
            "lt" => output.push('<'),
            "gt" => output.push('>'),
            "quot" => output.push('"'),
            "apos" => output.push('\''),
            _ if entity.starts_with("#x") => {
                let code = u32::from_str_radix(&entity[2..], 16)
                    .map_err(|_| CueMolError::parse("invalid XML numeric entity"))?;
                let decoded = char::from_u32(code)
                    .ok_or_else(|| CueMolError::parse("invalid XML code point"))?;
                output.push(decoded);
            }
            _ if entity.starts_with('#') => {
                let code = entity[1..]
                    .parse::<u32>()
                    .map_err(|_| CueMolError::parse("invalid XML numeric entity"))?;
                let decoded = char::from_u32(code)
                    .ok_or_else(|| CueMolError::parse("invalid XML code point"))?;
                output.push(decoded);
            }
            _ => return Err(CueMolError::parse(format!("unknown XML entity: {entity}"))),
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attr(tag: &str, value: &str) -> LDom2Node {
        let mut node = LDom2Node::new();
        node.set_tag_name(tag);
        node.set_value(value);
        node.set_attr_flag(true);
        node
    }

    #[test]
    fn ldom2_node_matches_reference_child_attr_and_copy_cases() {
        let mut parent = LDom2Node::new();
        assert_eq!(parent.child_count(), 0);
        assert_eq!(parent.tag_name(), "");
        assert_eq!(parent.value(), "");
        assert!(!parent.is_consumed());

        parent.append_child_node(attr("alpha", "hello"));
        parent.append_child_node(attr("beta", "world"));
        assert_eq!(parent.child_count(), 2);
        assert_eq!(parent.find_child("beta").unwrap().tag_name(), "beta");
        assert!(parent.find_child("missing").is_none());
        assert!(parent.str_attr("alpha").equals("hello"));
        assert!(!parent.bool_attr("alpha"));

        let mut copy = parent.clone();
        copy.find_child_mut("alpha").unwrap().set_value("modified");
        assert!(parent.str_attr("alpha").equals("hello"));
        assert!(copy.str_attr("alpha").equals("modified"));

        assert!(parent.remove_child("alpha"));
        assert!(!parent.remove_child("alpha"));
        assert_eq!(parent.child_count(), 1);
        parent.find_child_mut("beta").unwrap().set_consumed(true);
        assert!(parent.children_consumed());
    }

    #[test]
    fn ldom2_node_props_errors_and_xml_writer_match_reference_shape() {
        let mut node = LDom2Node::new();
        node.set_tag_name("root");
        node.set_str_attr("name", "myobj");
        node.set_str_attr("id", "root-id");
        node.set_str_attr("data", "a&b<c>\"'");
        node.set_type_name("MyClass");
        node.set_value("42");
        node.set_contents("raw content here");
        let child = node.append_child("defaultprop");
        child.set_default_flag(true);
        let child = node.append_child("child");
        child.set_default_flag(false);
        child.append_err_msg("error in child");

        assert!(node.error_msgs().index_of("error in child") >= 0);
        let tree = LDom2Tree { top: node };
        let xml = write_ldom2_xml(&tree);
        assert!(xml.contains("<?xml"));
        assert!(xml.contains("<root"));
        assert!(xml.contains("type=\"MyClass\""));
        assert!(xml.contains("value=\"42\""));
        assert!(!xml.contains("id=\"root-id\""));
        assert!(xml.contains("name=\"myobj\""));
        assert!(xml.contains("&amp;"));
        assert!(xml.contains("&lt;"));
        assert!(xml.contains("&gt;"));
        assert!(xml.contains("&quot;"));
        assert!(xml.contains("&apos;"));
        assert!(xml.contains("<![CDATA[raw content here]]>"));
        assert!(!xml.contains("defaultprop"));
        assert!(xml.contains("<child/>"));
    }

    #[test]
    fn ldom2_xml_parser_reads_elements_attrs_cdata_and_entities() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<!-- ignored -->
<scene type="qsys.Scene" value="rootval" name="main" data="a&amp;b&#x3c;c">
  <camera name='cam1' value="front"/>
  <notes><![CDATA[raw <xml> & text]]></notes>
</scene>"#;

        let tree = parse_ldom2_xml(xml).unwrap();
        let top = tree.top();
        assert_eq!(top.tag_name(), "scene");
        assert_eq!(top.type_name(), "qsys.Scene");
        assert!(top.str_attr("type").equals("qsys.Scene"));
        assert_eq!(top.value(), "rootval");
        assert!(top.str_attr("name").equals("main"));
        assert!(top.str_attr("data").equals("a&b<c"));
        assert_eq!(top.child_count(), 5);
        assert_eq!(
            top.children()
                .iter()
                .filter(|child| !child.is_attr())
                .count(),
            2
        );
        assert_eq!(top.find_child("camera").unwrap().value(), "front");
        assert!(top
            .find_child("camera")
            .unwrap()
            .str_attr("name")
            .equals("cam1"));
        assert_eq!(
            top.find_child("notes").unwrap().contents(),
            "raw <xml> & text"
        );
    }

    #[test]
    fn ldom2_xml_parser_roundtrips_writer_shape_without_reserved_attr_duplication() {
        let mut node = LDom2Node::new();
        node.set_tag_name("root");
        node.set_type_name("RootType");
        node.set_str_attr("type", "RootType");
        node.set_str_attr("name", "hello");
        node.append_child("child").set_str_attr("key", "value");

        let xml = write_ldom2_xml(&LDom2Tree { top: node });
        assert_eq!(xml.matches("type=\"RootType\"").count(), 1);
        assert!(xml.contains("name=\"hello\""));

        let parsed = parse_ldom2_xml(&xml).unwrap();
        assert_eq!(parsed.top().tag_name(), "root");
        assert_eq!(parsed.top().type_name(), "RootType");
        assert!(parsed.top().str_attr("type").equals("RootType"));
        assert!(parsed.top().str_attr("name").equals("hello"));
        assert!(parsed
            .top()
            .find_child("child")
            .unwrap()
            .str_attr("key")
            .equals("value"));
    }

    #[test]
    fn ldom2_xml_parser_rejects_representative_malformed_xml() {
        assert!(parse_ldom2_xml("<root><child></root>").is_err());
        assert!(parse_ldom2_xml("<root attr=noquote/>").is_err());
        assert!(parse_ldom2_xml("<root>&unknown;</root>").is_err());
        assert!(parse_ldom2_xml("<root><![CDATA[unterminated</root>").is_err());
    }
}
