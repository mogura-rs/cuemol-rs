use super::*;

#[derive(Clone)]
enum Expr {
    Leaf(Selector),
    List { and: bool, items: Vec<Expr> },
    Not(Box<Expr>),
    Unary(UnaryOp, Box<Expr>),
    Distance(DistanceOp, Box<Expr>, f64, Option<String>),
}

impl Expr {
    fn from_selector(selector: &Selector) -> Self {
        match selector {
            Selector::And(_, _) | Selector::Or(_, _) => {
                let and = matches!(selector, Selector::And(_, _));
                let mut pending = vec![selector];
                let mut items = Vec::new();
                while let Some(next) = pending.pop() {
                    match next {
                        Selector::And(a, b) if and => {
                            pending.push(b);
                            pending.push(a);
                        }
                        Selector::Or(a, b) if !and => {
                            pending.push(b);
                            pending.push(a);
                        }
                        other => items.push(Self::from_selector(other)),
                    }
                }
                Self::List { and, items }
            }
            Selector::Not(inner) => Self::Not(Box::new(Self::from_selector(inner))),
            Selector::Unary { op, inner } => Self::Unary(*op, Box::new(Self::from_selector(inner))),
            Selector::Distance {
                op,
                inner,
                distance,
                molecule,
            } => Self::Distance(
                *op,
                Box::new(Self::from_selector(inner)),
                *distance,
                molecule.clone(),
            ),
            Selector::Prepared(value) => Self::from_selector(&value.expression),
            leaf => {
                let mut leaf = leaf.clone();
                if let Some(names) = names_mut(&mut leaf) {
                    names.reverse();
                }
                if let Selector::ResidueId(ranges) = &mut leaf {
                    *ranges = merge_residues(ranges, "");
                }
                Self::Leaf(leaf)
            }
        }
    }

    fn constant(value: bool) -> Self {
        Self::Leaf(if value { Selector::All } else { Selector::None })
    }

    fn as_constant(&self) -> Option<bool> {
        match self {
            Self::Leaf(Selector::All) => Some(true),
            Self::Leaf(Selector::None) => Some(false),
            _ => None,
        }
    }

    fn collapse(and: bool, mut items: Vec<Self>) -> Self {
        match items.len() {
            0 => Self::constant(and),
            1 => items.pop().unwrap(),
            _ => Self::List { and, items },
        }
    }

    fn key(&self) -> String {
        match self {
            Self::List { and, items } => {
                let mut keys: Vec<_> = items.iter().map(Self::key).collect();
                keys.sort_unstable();
                format!("{and}:{keys:?}")
            }
            Self::Not(inner) => format!("!({})", inner.key()),
            Self::Unary(op, inner) => format!("{op:?}({})", inner.key()),
            Self::Distance(op, inner, distance, molecule) => format!(
                "{op:?}:{}:{molecule:?}({})",
                distance.to_bits(),
                inner.key()
            ),
            Self::Leaf(leaf) => leaf_key(leaf),
        }
    }

    fn complement_key(&self) -> String {
        match self {
            Self::Not(inner) => inner.key(),
            _ => format!("!({})", self.key()),
        }
    }

    fn factors(&self, and: bool) -> Vec<&Self> {
        match self {
            Self::List { and: kind, items } if *kind == and => items.iter().collect(),
            _ => vec![self],
        }
    }

    fn simplify(self) -> Self {
        match self {
            Self::List { and, items } => simplify_list(and, items),
            Self::Not(inner) => {
                let inner = inner.simplify();
                match inner {
                    Self::Not(value) => *value,
                    _ if inner.as_constant().is_some() => {
                        Self::constant(!inner.as_constant().unwrap())
                    }
                    _ => Self::Not(Box::new(inner)),
                }
            }
            Self::Unary(op, inner) => {
                let inner = inner.simplify();
                match inner {
                    Self::Unary(inner_op, _) if inner_op == op => inner,
                    _ => Self::Unary(op, Box::new(inner)),
                }
            }
            Self::Distance(op, inner, distance, molecule) => {
                Self::Distance(op, Box::new(inner.simplify()), distance, molecule)
            }
            _ => self,
        }
    }

    fn fully_simplified(mut self) -> Self {
        for _ in 0..64 {
            let key = self.key();
            self = self.simplify();
            if key == self.key() {
                break;
            }
        }
        self
    }

    fn operand(&self) -> String {
        match self {
            Self::Leaf(_) => self.text(),
            _ => format!("({})", self.text()),
        }
    }

    fn text(&self) -> String {
        match self {
            Self::Leaf(leaf) => leaf_text(leaf),
            Self::List { and, items } => items
                .iter()
                .map(|item| {
                    if matches!(item, Self::Not(_)) {
                        item.text()
                    } else {
                        item.operand()
                    }
                })
                .collect::<Vec<_>>()
                .join(if *and { " and " } else { " or " }),
            Self::Not(inner) => format!("not {}", inner.operand()),
            Self::Unary(op, inner) => format!("{} {}", op.dump(), inner.operand()),
            Self::Distance(op, inner, distance, molecule) => {
                let target = molecule
                    .as_ref()
                    .filter(|name| !name.is_empty())
                    .map(|name| format!(" [{}]", quote_name(name)))
                    .unwrap_or_default();
                format!(
                    "({}) {} {}{target}",
                    inner.text(),
                    op.dump(),
                    format_number(*distance)
                )
            }
        }
    }
}

fn simplify_list(and: bool, items: Vec<Expr>) -> Expr {
    let mut flat = Vec::new();
    for item in items {
        match item.simplify() {
            Expr::List { and: inner, items } if inner == and => flat.extend(items),
            other => flat.push(other),
        }
    }
    if flat.iter().any(|item| item.as_constant() == Some(!and)) {
        return Expr::constant(!and);
    }
    flat.retain(|item| item.as_constant() != Some(and));
    let mut keys = BTreeSet::new();
    flat.retain(|item| keys.insert(item.key()));
    if flat
        .iter()
        .any(|item| keys.contains(&item.complement_key()))
    {
        return Expr::constant(!and);
    }
    let mut index = 0;
    while index < flat.len() {
        let absorbed = match &flat[index] {
            Expr::List { and: inner, items } if *inner != and => {
                items.iter().any(|item| keys.contains(&item.key()))
            }
            _ => false,
        };
        if absorbed {
            keys.remove(&flat[index].key());
            flat.remove(index);
        } else {
            index += 1;
        }
    }
    for index in 0..flat.len() {
        let Expr::List { and: inner, items } = &flat[index] else {
            continue;
        };
        if *inner == and {
            continue;
        }
        let kept: Vec<_> = items
            .iter()
            .filter(|item| !keys.contains(&item.complement_key()))
            .cloned()
            .collect();
        if kept.len() != items.len() {
            keys.remove(&flat[index].key());
            flat[index] = Expr::collapse(!and, kept);
            keys.insert(flat[index].key());
        }
    }
    loop {
        let mut merged = None;
        'pairs: for i in 0..flat.len() {
            let left = flat[i].factors(!and);
            if left.len() < 2 {
                continue;
            }
            let left_keys: Vec<_> = left.iter().map(|part| part.key()).collect();
            'candidates: for j in i + 1..flat.len() {
                let right = flat[j].factors(!and);
                if left.len() != right.len() {
                    continue;
                }
                let mut right_keys: BTreeSet<_> = right.iter().map(|part| part.key()).collect();
                let mut common = Vec::new();
                let mut different = None;
                for (factor, key) in left.iter().zip(&left_keys) {
                    if right_keys.remove(key) {
                        common.push((*factor).clone());
                    } else if different.is_none() {
                        different = Some(*factor);
                    } else {
                        continue 'candidates;
                    }
                }
                if right_keys.len() == 1
                    && different.is_some_and(|part| right_keys.contains(&part.complement_key()))
                {
                    merged = Some((i, j, Expr::collapse(!and, common)));
                    break 'pairs;
                }
            }
        }
        let Some((i, j, replacement)) = merged else {
            break;
        };
        flat[i] = replacement;
        flat.remove(j);
    }
    if !and {
        let mut merged = Vec::<Expr>::new();
        for item in flat {
            if let Expr::Leaf(leaf) = &item {
                if merged.iter_mut().any(|existing| match existing {
                    Expr::Leaf(existing) => merge_leaves(existing, leaf),
                    _ => false,
                }) {
                    continue;
                }
            }
            merged.push(item);
        }
        flat = merged;
    }
    Expr::collapse(and, flat)
}

fn names_mut(selector: &mut Selector) -> Option<&mut Vec<NamePattern>> {
    match selector {
        Selector::Name(names)
        | Selector::Element(names)
        | Selector::Chain(names)
        | Selector::Alt(names)
        | Selector::ResidueName(names) => Some(names),
        _ => None,
    }
}

fn names_key(names: &[NamePattern]) -> Vec<String> {
    let mut keys: Vec<_> = names
        .iter()
        .map(|name| format!("{:?}", name.kind))
        .collect();
    keys.sort_unstable();
    keys
}

fn hier_names_key(text: &str) -> String {
    parse_name_list(text, ListKind::Name)
        .map(|names| format!("{:?}", names_key(&names)))
        .unwrap_or_else(|_| text.to_string())
}

fn leaf_key(leaf: &Selector) -> String {
    match leaf {
        Selector::Name(names) => format!("name:{:?}", names_key(names)),
        Selector::Element(names) => format!("elem:{:?}", names_key(names)),
        Selector::Chain(names) => format!("chain:{:?}", names_key(names)),
        Selector::Alt(names) => format!("alt:{:?}", names_key(names)),
        Selector::ResidueName(names) => format!("resn:{:?}", names_key(names)),
        Selector::Hier(hier) => format!(
            "hier:{:?}",
            (
                hier_names_key(&hier.chain),
                merge_residues(&hier.residue, ""),
                hier_names_key(&hier.atom)
            )
        ),
        _ => format!("{leaf:?}"),
    }
}

fn leaf_text(leaf: &Selector) -> String {
    let (keyword, names) = match leaf {
        Selector::Name(names) => ("name", names),
        Selector::Element(names) => ("elem", names),
        Selector::Chain(names) => ("chain", names),
        Selector::Alt(names) => ("alt", names),
        Selector::ResidueName(names) => ("resn", names),
        _ => return leaf.expression(),
    };
    format!(
        "{keyword} {}",
        names
            .iter()
            .map(NamePattern::display)
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn merge_leaves(left: &mut Selector, right: &Selector) -> bool {
    match (left, right) {
        (Selector::Name(left), Selector::Name(right))
        | (Selector::Element(left), Selector::Element(right))
        | (Selector::Chain(left), Selector::Chain(right))
        | (Selector::Alt(left), Selector::Alt(right))
        | (Selector::ResidueName(left), Selector::ResidueName(right)) => {
            for name in right {
                if !left.contains(name) {
                    left.push(name.clone());
                }
            }
        }
        (Selector::Aid(left), Selector::Aid(right)) => *left = left.append(right),
        (Selector::ResidueId(left), Selector::ResidueId(right)) => {
            *left = merge_residues(left, right)
        }
        (Selector::Hier(left), Selector::Hier(right))
            if hier_names_key(&left.chain) == hier_names_key(&right.chain)
                && hier_names_key(&left.atom) == hier_names_key(&right.atom) =>
        {
            left.residue = merge_residues(&left.residue, &right.residue);
        }
        _ => return false,
    }
    true
}

fn merge_residues(left: &str, right: &str) -> String {
    if left == "*" || right == "*" {
        return "*".to_string();
    }
    let mut ranges = Vec::new();
    for part in left
        .split(',')
        .chain(right.split(','))
        .filter(|part| !part.is_empty())
    {
        let (start, end) = part.split_once(':').unwrap_or((part, part));
        let Some((a, b)) = split_residue_insert(start).zip(split_residue_insert(end)) else {
            return format!("{left},{right}").trim_matches(',').to_string();
        };
        ranges.push((a.min(b), a.max(b)));
    }
    ranges.sort_unstable();
    let mut out: Vec<((i32, Option<char>), (i32, Option<char>))> = Vec::new();
    for (start, end) in ranges {
        if let Some((_, previous)) = out.last_mut() {
            if start <= *previous
                || (start.0 == previous.0
                    && start.1.map_or(0, |c| c as u32) == previous.1.map_or(0, |c| c as u32) + 1)
            {
                *previous = (*previous).max(end);
                continue;
            }
        }
        out.push((start, end));
    }
    let text = |(number, code): (i32, Option<char>)| match code {
        Some(code) => format!("{number}{code}"),
        None => number.to_string(),
    };
    out.into_iter()
        .map(|(start, end)| {
            if start == end {
                text(start)
            } else {
                format!("{}:{}", text(start), text(end))
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

pub(super) fn format_number(value: f64) -> String {
    for precision in 1..=17 {
        let text = format!("{value:.precision$}");
        if text.parse::<f64>().ok() == Some(value) {
            return text;
        }
    }
    format!("{value:.17}")
}

pub(super) fn quote_name(value: &str) -> String {
    if value.contains('"') && value.contains('\'') {
        value.to_string()
    } else if value.contains('"') {
        format!("'{value}'")
    } else {
        format!("\"{value}\"")
    }
}

pub(super) fn quote_value(value: &str) -> String {
    let mut chars = value.chars();
    let word = chars
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '\'' | '*'));
    let digits = value.strip_prefix('-').unwrap_or(value);
    let integer = !digits.is_empty()
        && digits.len() <= 9
        && (digits.len() == 1 || !digits.starts_with('0'))
        && digits.chars().all(|ch| ch.is_ascii_digit());
    if word || integer {
        value.to_string()
    } else {
        quote_name(value)
    }
}

impl Selector {
    pub fn simplified_expression(&self) -> String {
        let text = Expr::from_selector(self).fully_simplified().text();
        if parse_selector(&text).is_ok() {
            text
        } else {
            self.expression()
        }
    }
}

impl SelCommand {
    pub fn simplify(&self) -> String {
        self.selector
            .as_ref()
            .map(Selector::simplified_expression)
            .unwrap_or_else(|| self.original.clone())
    }

    pub fn invert_simplified(&self) -> String {
        self.rewrite(
            |root| Expr::Not(Box::new(root)),
            if self.is_empty() {
                "*".to_string()
            } else {
                format!("not ({})", self.original)
            },
        )
    }

    pub fn toggle_sidechain(&self) -> String {
        self.rewrite(
            |root| match root {
                Expr::Unary(UnaryOp::BySideChain, inner) => *inner,
                _ => Expr::Unary(UnaryOp::BySideChain, Box::new(root)),
            },
            format!("bysidech ({})", self.original),
        )
    }

    pub fn rewrite_around(&self, distance: f64, by_residue: bool) -> String {
        let fallback = format!("({}) around {}", self.original, format_number(distance));
        let fallback = if by_residue {
            format!("byres ({fallback})")
        } else {
            fallback
        };
        self.rewrite(
            |root| {
                let base = match root {
                    Expr::Distance(DistanceOp::Around, inner, _, _) => *inner,
                    Expr::Unary(UnaryOp::ByRes, inner) => match *inner {
                        Expr::Distance(DistanceOp::Around, inner, _, _) => *inner,
                        other => Expr::Unary(UnaryOp::ByRes, Box::new(other)),
                    },
                    other => other,
                };
                let around = Expr::Distance(DistanceOp::Around, Box::new(base), distance, None);
                if by_residue {
                    Expr::Unary(UnaryOp::ByRes, Box::new(around))
                } else {
                    around
                }
            },
            fallback,
        )
    }

    fn rewrite(&self, edit: impl FnOnce(Expr) -> Expr, fallback: String) -> String {
        let Some(selector) = self.selector.as_ref() else {
            return fallback;
        };
        let text = edit(Expr::from_selector(selector))
            .fully_simplified()
            .text();
        if parse_selector(&text).is_ok() {
            text
        } else {
            fallback
        }
    }
}
