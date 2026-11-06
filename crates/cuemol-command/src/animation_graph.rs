use cuemol_anim::AnimMgr;
use cuemol_core::{time_from_milliseconds, time_to_milliseconds};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

pub fn editor_time_ms(value: f64) -> i64 {
    time_from_milliseconds((value + 0.5).floor())
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub uid: u64,
    pub name: String,
    pub time_ref_name: String,
    pub start_ms: f64,
    pub end_ms: f64,
    pub index: usize,
    pub state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub ref_index: i64,
    pub abs_start_ms: Option<f64>,
    pub abs_end_ms: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub first: BTreeMap<String, usize>,
    pub duplicates: BTreeSet<String>,
}

impl Graph {
    pub fn read(manager: &AnimMgr) -> Self {
        Self::build(
            manager
                .motions()
                .iter()
                .enumerate()
                .map(|(index, motion)| {
                    let base = motion.base();
                    Node {
                        uid: base.uid().get(),
                        name: base.name().into(),
                        time_ref_name: base.time_ref_name().into(),
                        start_ms: time_to_milliseconds(base.rel_start()),
                        end_ms: time_to_milliseconds(base.rel_end()),
                        index,
                        state: "ok",
                        error: None,
                        ref_index: -1,
                        abs_start_ms: None,
                        abs_end_ms: None,
                    }
                })
                .collect(),
        )
    }

    fn build(mut nodes: Vec<Node>) -> Self {
        let mut first = BTreeMap::new();
        let mut duplicates = BTreeSet::new();
        for node in &nodes {
            if !node.name.is_empty() {
                if first.contains_key(&node.name) {
                    duplicates.insert(node.name.clone());
                } else {
                    first.insert(node.name.clone(), node.index);
                }
            }
        }
        for node in &mut nodes {
            node.ref_index = if node.time_ref_name.is_empty() {
                -1
            } else {
                first
                    .get(&node.time_ref_name)
                    .map_or(-1, |index| *index as i64)
            };
        }
        let mut settled = vec![false; nodes.len()];
        let mut on_path = vec![None; nodes.len()];
        let mut path = Vec::new();
        for start in 0..nodes.len() {
            if settled[start] {
                continue;
            }
            let mut current = start;
            while !settled[current] {
                on_path[current] = Some(path.len());
                path.push(current);
                let node = &mut nodes[current];
                if node.time_ref_name.is_empty() {
                    node.abs_start_ms = Some(node.start_ms);
                    node.abs_end_ms = Some(node.end_ms);
                    settled[current] = true;
                    break;
                }
                if node.ref_index < 0 {
                    node.state = "missing";
                    node.error = Some(format!(
                        "\"{}\" is relative to \"{}\", which does not exist",
                        node.name, node.time_ref_name
                    ));
                    settled[current] = true;
                    break;
                }
                let next = node.ref_index as usize;
                if let Some(at) = on_path[next] {
                    let cycle = &path[at..];
                    let mut names = cycle
                        .iter()
                        .map(|&index| nodes[index].name.as_str())
                        .collect::<Vec<_>>();
                    names.push(&nodes[cycle[0]].name);
                    let error = format!("Cyclic reference: {}", names.join(" -> "));
                    for &index in cycle {
                        nodes[index].state = "cycle";
                        nodes[index].error = Some(error.clone());
                        settled[index] = true;
                    }
                    break;
                }
                current = next;
            }
            for &index in path.iter().rev() {
                if !settled[index] {
                    let reference = nodes[index].ref_index as usize;
                    if let Some(end) = nodes[reference].abs_end_ms {
                        nodes[index].abs_start_ms = Some(end + nodes[index].start_ms);
                        nodes[index].abs_end_ms = Some(end + nodes[index].end_ms);
                    } else {
                        nodes[index].state = "upstream";
                        nodes[index].error = Some(format!(
                            "\"{}\" chains to \"{}\", whose timing does not resolve",
                            nodes[index].name, nodes[reference].name
                        ));
                    }
                    settled[index] = true;
                }
                on_path[index] = None;
            }
            path.clear();
        }
        Self {
            nodes,
            first,
            duplicates,
        }
    }

    pub fn auto_chain_name(&self, insertion_index: usize) -> String {
        insertion_index
            .checked_sub(1)
            .and_then(|index| self.nodes.get(index))
            .filter(|node| {
                node.state == "ok" && !node.name.is_empty() && !self.duplicates.contains(&node.name)
            })
            .map(|node| node.name.clone())
            .unwrap_or_default()
    }

    pub fn detach_dependents(&self, manager: &mut AnimMgr, indices: &[usize]) {
        let removed = indices.iter().copied().collect::<BTreeSet<_>>();
        for node in &self.nodes {
            if removed.contains(&node.index) || node.time_ref_name.is_empty() {
                continue;
            }
            let Some(&target) = self.first.get(&node.time_ref_name) else {
                continue;
            };
            if !removed.contains(&target) {
                continue;
            }
            let base = manager.motions_mut()[node.index].base_mut();
            if let Some((start, end)) = node.abs_start_ms.zip(node.abs_end_ms) {
                base.set_rel_start(editor_time_ms(start));
                base.set_rel_end(editor_time_ms(end));
            }
            base.set_time_ref_name("");
        }
    }

    pub fn rebase_span(&self, index: usize, reference: &str) -> Result<Option<(f64, f64)>, String> {
        let base = self.check_reference(index, reference)?;
        let node = &self.nodes[index];
        Ok(node.abs_start_ms.zip(node.abs_end_ms).map(|(start, end)| {
            let relative_start = (start - base).max(0.);
            (relative_start, relative_start + (end - start).max(0.))
        }))
    }

    pub fn error(&self) -> Option<&str> {
        self.nodes
            .iter()
            .find(|node| matches!(node.state, "missing" | "cycle"))
            .or_else(|| self.nodes.iter().find(|node| node.state != "ok"))
            .and_then(|node| node.error.as_deref())
    }

    pub fn check_reference(&self, index: usize, reference: &str) -> Result<f64, String> {
        if reference.is_empty() {
            return Ok(0.);
        }
        if self.duplicates.contains(reference) {
            let count = self
                .nodes
                .iter()
                .filter(|node| node.name == reference)
                .count();
            return Err(format!(
                "\"{reference}\" is carried by {count} elements; rename one first"
            ));
        }
        let &target = self
            .first
            .get(reference)
            .ok_or_else(|| format!("No element is named \"{reference}\""))?;
        if target == index {
            return Err("An element cannot be relative to itself".into());
        }
        let mut path = vec![self.nodes[index].name.as_str(), reference];
        let mut seen = BTreeSet::from([target]);
        let mut current = target;
        while self.nodes[current].ref_index >= 0 {
            let next = self.nodes[current].ref_index as usize;
            if seen.contains(&next) {
                break;
            }
            if next == index {
                path.push(&self.nodes[index].name);
                return Err(format!(
                    "Relative to \"{reference}\" would create a cycle: {}",
                    path.join(" -> ")
                ));
            }
            seen.insert(next);
            current = next;
            path.push(&self.nodes[current].name);
        }
        self.nodes[target].abs_end_ms.ok_or_else(|| {
            format!(
                "\"{reference}\" cannot be used as a reference: {}",
                self.nodes[target]
                    .error
                    .as_deref()
                    .unwrap_or("unresolved timing")
            )
        })
    }

    pub fn check_name(&self, index: usize, value: &str) -> Result<String, String> {
        let name = value.trim();
        if name.is_empty() {
            return Err("Name cannot be empty".into());
        }
        if self
            .nodes
            .iter()
            .any(|node| node.index != index && node.name == name)
        {
            return Err(format!("Another element is already named \"{name}\""));
        }
        Ok(name.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn normalize_numbers(value: &mut Value) {
        match value {
            Value::Number(number) => {
                *number = serde_json::Number::from_f64(number.as_f64().unwrap()).unwrap()
            }
            Value::Array(values) => values.iter_mut().for_each(normalize_numbers),
            Value::Object(values) => values.values_mut().for_each(normalize_numbers),
            _ => {}
        }
    }

    #[test]
    fn graph_and_edit_validation_match_the_current_reference() {
        let Ok(reference) = std::env::var("CUEMOL_ANIM_REFERENCE") else {
            return;
        };
        let references = ["", "missing", "A", "B", "C", "D"];
        let names = ["", "  ", " renamed ", "A", "B", "C"];
        let mut input = Vec::new();
        let mut expected = Vec::new();
        let mut seed = 7_u64;
        for sample in 0..128 {
            let mut nodes = Vec::new();
            for index in 0..(2 + sample % 6) {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let name = ["A", "B", "C", "D", "", "duplicate"][(seed >> 32) as usize % 6];
                let reference = references[(seed >> 40) as usize % references.len()];
                nodes.push(Node {
                    uid: index as u64 + 1,
                    name: name.into(),
                    time_ref_name: reference.into(),
                    start_ms: (index as f64 - 1.) * 100.,
                    end_ms: 500. + index as f64 * 100.,
                    index,
                    state: "ok",
                    error: None,
                    ref_index: -1,
                    abs_start_ms: None,
                    abs_end_ms: None,
                });
            }
            input.push(json!({"nodes":nodes.iter().map(|node|json!({"uid":node.uid,"name":node.name,"timeRefName":node.time_ref_name,"startMs":node.start_ms,"endMs":node.end_ms})).collect::<Vec<_>>(), "references":references,"names":names}));
            let graph = Graph::build(nodes);
            let checks = graph.nodes.iter().map(|node|references.iter().map(|reference|match graph.check_reference(node.index, reference) {
                Ok(base) => json!({"ok":true,"baseEndMs":base,"refIndex":graph.first.get(*reference).map_or(-1, |index| *index as i64)}),
                Err(error) => json!({"error":error}),
            }).collect::<Vec<_>>()).collect::<Vec<_>>();
            let name_checks = graph
                .nodes
                .iter()
                .map(|node| {
                    names
                        .iter()
                        .map(|name| match graph.check_name(node.index, name) {
                            Ok(name) => json!({"ok":true,"name":name}),
                            Err(error) => json!({"error":error}),
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            expected.push(json!({"nodes":graph.nodes,"error":graph.error(),"references":checks,"names":name_checks}));
        }
        let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../scripts/check-reference-animation.mjs");
        let mut child = std::process::Command::new("node")
            .arg(script)
            .arg(reference)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        use std::io::Write;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(serde_json::to_string(&input).unwrap().as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        let mut actual: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
        actual.iter_mut().for_each(normalize_numbers);
        expected.iter_mut().for_each(normalize_numbers);
        for (index, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
            assert_eq!(actual, expected, "reference graph {index}");
        }
        assert_eq!(actual.len(), 128);
        eprintln!("Compared 128 reference graphs with every candidate name and time reference");
    }
}
