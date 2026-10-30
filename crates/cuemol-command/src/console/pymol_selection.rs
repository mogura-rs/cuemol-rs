#[derive(Clone, Debug)]
pub(super) enum Selection {
    Literal(String),
    Name(String),
    Unary(&'static str, Box<Self>),
    Binary(&'static str, Box<Self>, Box<Self>),
    Proximity(&'static str, Box<Self>, String),
}

impl Selection {
    pub(super) fn emit(&self, resolve: &impl Fn(&str) -> Option<String>) -> String {
        match self {
            Self::Literal(value) => value.clone(),
            Self::Name(name) => resolve(name).unwrap_or_else(|| name.clone()),
            Self::Unary(op, child) => format!("{op} ({})", child.emit(resolve)),
            Self::Binary(op, left, right) => {
                format!("({}) {op} ({})", left.emit(resolve), right.emit(resolve))
            }
            Self::Proximity(op, child, distance) => {
                format!("({}) {op} {distance}", child.emit(resolve))
            }
        }
    }

    pub(super) fn names(&self, out: &mut Vec<String>) {
        match self {
            Self::Name(name) => out.push(name.clone()),
            Self::Unary(_, child) | Self::Proximity(_, child, _) => child.names(out),
            Self::Binary(_, left, right) => {
                left.names(out);
                right.names(out);
            }
            Self::Literal(_) => {}
        }
    }
}

struct Parser<'a> {
    source: &'a str,
    tokens: Vec<(&'a str, usize)>,
    at: usize,
}

pub(super) fn parse(source: &str) -> Result<Selection, String> {
    let mut tokens = Vec::new();
    let mut start = None;
    for (offset, ch) in source.char_indices() {
        if ch.is_whitespace() || matches!(ch, '(' | ')') {
            if let Some(start) = start.take() {
                tokens.push((&source[start..offset], start));
            }
            if matches!(ch, '(' | ')') {
                tokens.push((&source[offset..offset + 1], offset));
            }
        } else if start.is_none() {
            start = Some(offset);
        }
    }
    if let Some(start) = start {
        tokens.push((&source[start..], start));
    }
    let mut parser = Parser {
        source,
        tokens,
        at: 0,
    };
    let selection = parser.byres()?;
    if let Some((word, _)) = parser.tokens.get(parser.at) {
        return Err(parser.error(&format!("Unexpected selection token: {word}")));
    }
    Ok(selection)
}

impl Parser<'_> {
    fn error(&self, message: &str) -> String {
        let offset = self
            .tokens
            .get(self.at)
            .map_or(self.source.len(), |token| token.1);
        format!(
            "{}\n{}^ {message}",
            self.source,
            " ".repeat(self.source[..offset].chars().count())
        )
    }

    fn peek(&self) -> String {
        self.tokens
            .get(self.at)
            .map_or("", |token| token.0)
            .to_ascii_lowercase()
    }

    fn take(&mut self) -> Result<String, String> {
        let word = self
            .tokens
            .get(self.at)
            .ok_or_else(|| self.error("The selection ends early"))?
            .0
            .to_string();
        self.at += 1;
        Ok(word)
    }

    fn byres(&mut self) -> Result<Selection, String> {
        if matches!(self.peek().as_str(), "byres" | "br.") {
            self.at += 1;
            Ok(Selection::Unary("byres", Box::new(self.byres()?)))
        } else {
            self.proximity()
        }
    }

    fn proximity(&mut self) -> Result<Selection, String> {
        let mut result = self.or()?;
        loop {
            let word = self.peek();
            let op = match word.as_str() {
                "around" | "a." | "near_to" | "nto." => "around",
                "expand" | "x." | "within" | "w." | "beyond" | "be." => "expand",
                _ => break,
            };
            self.at += 1;
            let distance = self.take()?;
            if distance
                .parse::<f64>()
                .ok()
                .filter(|value| value.is_finite() && *value >= 0.)
                .is_none()
            {
                return Err(self.error("A proximity operator needs a nonnegative finite distance"));
            }
            if matches!(word.as_str(), "around" | "a." | "expand" | "x.") {
                result = Selection::Proximity(op, Box::new(result), distance);
            } else {
                if self.peek() != "of" {
                    return Err(self.error("Expected of after the distance"));
                }
                self.at += 1;
                let ball = Selection::Proximity(op, Box::new(self.or()?), distance);
                let join = if matches!(word.as_str(), "beyond" | "be.") {
                    "and not"
                } else {
                    "and"
                };
                result = Selection::Binary(join, Box::new(result), Box::new(ball));
            }
        }
        Ok(result)
    }

    fn or(&mut self) -> Result<Selection, String> {
        let mut result = self.and()?;
        while matches!(self.peek().as_str(), "or" | "|" | "+") {
            self.at += 1;
            result = Selection::Binary("or", Box::new(result), Box::new(self.and()?));
        }
        Ok(result)
    }

    fn and(&mut self) -> Result<Selection, String> {
        let mut result = self.not()?;
        while matches!(self.peek().as_str(), "and" | "&" | "-") {
            let op = if self.peek() == "-" { "and not" } else { "and" };
            self.at += 1;
            result = Selection::Binary(op, Box::new(result), Box::new(self.not()?));
        }
        Ok(result)
    }

    fn not(&mut self) -> Result<Selection, String> {
        if matches!(self.peek().as_str(), "not" | "!") {
            self.at += 1;
            return Ok(Selection::Unary("not", Box::new(self.not()?)));
        }
        self.primary()
    }

    fn primary(&mut self) -> Result<Selection, String> {
        let word = self.take()?;
        let lower = word.to_ascii_lowercase();
        if lower == "(" {
            let inner = self.byres()?;
            if self.peek() != ")" {
                return Err(self.error("Missing closing parenthesis"));
            }
            self.at += 1;
            return Ok(inner);
        }
        if lower == ")" {
            return Err(self.error("Unexpected closing parenthesis"));
        }
        let literal = match lower.as_str() {
            "all" | "*" => Some("*"),
            "none" => Some("none"),
            "protein" | "polymer.protein" => Some("protein"),
            "nucleic" | "polymer.nucleic" => Some("nucleic"),
            "polymer" => Some("(protein) or (nucleic)"),
            "water" | "solvent" => Some("water"),
            "hydro" | "hydrogens" => Some("hydrogen"),
            "hetatm" | "organic" => Some("ligand"),
            "backbone" => Some("((protein) and (name N,CA,C,O,OXT)) or ((nucleic) and (name P,OP1,OP2,O5*,C5*,C4*,C3*,O3*))"),
            "sidechain" => Some("((protein) and not (name N,CA,C,O,OXT)) and not ((nucleic) and (name P,OP1,OP2,O5*,C5*,C4*,C3*,O3*))"),
            "guide" => Some("(protein) and (name CA)"),
            _ => None,
        };
        if let Some(value) = literal {
            return Ok(Selection::Literal(value.into()));
        }
        let property = match lower.as_str() {
            "name" | "n." => "name",
            "elem" | "e." | "element" | "symbol" => "elem",
            "resn" | "r." | "resname" => "resn",
            "resi" | "i." | "resid" | "resv" => "resi",
            "chain" | "c." => "chain",
            "alt" | "altloc" => "alt",
            "id" => "aid",
            "ss" => "ss",
            "b" => "bfac",
            "q" => "occ",
            _ => "",
        };
        if matches!(property, "bfac" | "occ") {
            let op = self.take()?;
            let op = if op == "==" { "=" } else { &op };
            if !matches!(op, "<" | ">" | "=") {
                return Err(self.error("CueMol comparisons accept only <, > and ="));
            }
            let value = self.take()?;
            if value
                .parse::<f64>()
                .ok()
                .filter(|value| value.is_finite())
                .is_none()
            {
                return Err(self.error("A comparison needs a finite number"));
            }
            return Ok(Selection::Literal(format!("{property} {op} {value}")));
        }
        if !property.is_empty() {
            let value = self.take()?;
            if matches!(value.as_str(), "(" | ")") {
                return Err(self.error("Expected a property value"));
            }
            let values = value
                .split(['+', ','])
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>();
            let translated = if property == "resi" {
                values
                    .into_iter()
                    .map(residue_range)
                    .collect::<Result<Vec<_>, _>>()?
                    .join(",")
            } else if property == "ss" {
                let values = values
                    .into_iter()
                    .map(|value| match value.to_ascii_lowercase().as_str() {
                        "h" => Ok("(helix)"),
                        "s" => Ok("(sheet)"),
                        "l" => Ok("(coil)"),
                        _ => Err(format!("Expected H, S or L, not {value}")),
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                return Ok(Selection::Literal(values.join(" or ")));
            } else {
                values.join(",")
            };
            return Ok(Selection::Literal(format!("{property} {translated}")));
        }
        if [
            "rank",
            "partial_charge",
            "formal_charge",
            "inorganic",
            "metals",
            "donors",
            "acceptors",
            "visible",
            "enabled",
            "bonded",
            "masked",
            "protected",
            "center",
            "origin",
            "in",
            "like",
            "neighbor",
            "nbr.",
            "extend",
            "first",
            "last",
            "bound_to",
            "byobject",
            "bysegi",
            "bychain",
            "bycalpha",
            "bymolecule",
            "gap",
            "segi",
            "s.",
            "index",
            "model",
            "m.",
            "x",
            "y",
            "z",
            "flag",
            "f",
            "rep",
            "color",
            "pepseq",
            "ps",
            "text_type",
            "numeric_type",
            "stereo",
        ]
        .contains(&lower.as_str())
        {
            return Err(self.error(&format!("{word} has no CueMol selection equivalent")));
        }
        if word.contains('/') {
            return Err(self.error("Use chain, resi and name instead of slash notation"));
        }
        Ok(Selection::Name(word))
    }
}

fn residue_range(value: &str) -> Result<String, String> {
    let valid = |value: &str| {
        let value = value.strip_prefix('-').unwrap_or(value);
        !value.is_empty()
            && value
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    };
    if valid(value) {
        return Ok(value.into());
    }
    for (offset, ch) in value.char_indices().skip(1) {
        if ch == '-' && valid(&value[..offset]) && valid(&value[offset + 1..]) {
            return Ok(format!("{}:{}", &value[..offset], &value[offset + 1..]));
        }
    }
    Err(format!("Invalid residue number or range: {value}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pymol_selection_preserves_lists_ranges_and_operator_precedence() {
        let emit = |text| parse(text).unwrap().emit(&|_| None);
        assert_eq!(
            emit("byres chain A or chain B and not resi -5--1+1A-2B"),
            "byres ((chain A) or ((chain B) and (not (resi -5:-1,1A:2B))))"
        );
        assert_eq!(emit("name CA+CB - resi 2"), "(name CA,CB) and not (resi 2)");
        assert_eq!(
            emit("chain A within 5 of resi 2"),
            "(chain A) and ((resi 2) expand 5)"
        );
        assert_eq!(emit("ss H+S"), "(helix) or (sheet)");
        assert_eq!(emit("b == 12"), "bfac = 12");
        let selection = parse("first_molecule or (second_molecule and chain A)").unwrap();
        let mut names = Vec::new();
        selection.names(&mut names);
        assert_eq!(names, ["first_molecule", "second_molecule"]);
        assert_eq!(
            selection.emit(&|name| Some(
                if name == "first_molecule" {
                    "*"
                } else {
                    "none"
                }
                .into()
            )),
            "(*) or ((none) and (chain A))"
        );
        for source in [
            "",
            "chain",
            "chain A)",
            "(chain A",
            "b <= 3",
            "segi A",
            "rank 2",
            "resi --1",
            "chain A in chain B",
            "(all) around bad",
        ] {
            assert!(parse(source).is_err(), "{source}");
        }
    }
}
