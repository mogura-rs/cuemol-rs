use cuemol_core::{CueMolError, Result, Uid};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererInstance {
    uid: Uid,
    type_name: String,
}

impl RendererInstance {
    pub fn uid(&self) -> Uid {
        self.uid
    }

    pub fn type_name(&self) -> &str {
        &self.type_name
    }
}

#[derive(Debug, Clone)]
struct RendererSpec {
    compatible_with_null: bool,
}

#[derive(Debug, Clone)]
pub struct RendererFactory {
    registry: BTreeMap<String, RendererSpec>,
    next_uid: u64,
}

impl Default for RendererFactory {
    fn default() -> Self {
        let mut factory = Self {
            registry: BTreeMap::new(),
            next_uid: 1,
        };
        factory.register("*group", true);
        factory
    }
}

impl RendererFactory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, type_name: impl Into<String>, compatible_with_null: bool) -> bool {
        self.registry
            .insert(
                type_name.into(),
                RendererSpec {
                    compatible_with_null,
                },
            )
            .is_none()
    }

    pub fn contains(&self, type_name: &str) -> bool {
        self.registry.contains_key(type_name)
    }

    pub fn create(&mut self, type_name: &str) -> Result<RendererInstance> {
        if !self.registry.contains_key(type_name) {
            return Err(CueMolError::invalid_argument("unknown renderer type"));
        }
        let uid = Uid::new(self.next_uid);
        self.next_uid += 1;
        Ok(RendererInstance {
            uid,
            type_name: type_name.to_string(),
        })
    }

    pub fn search_compatible_renderers(&self, has_object: bool) -> Vec<String> {
        self.registry
            .iter()
            .filter(|(_, spec)| has_object || spec.compatible_with_null)
            .map(|(type_name, _)| type_name.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renderer_factory_defaults_match_reference_rendgroup_flow() {
        let mut factory = RendererFactory::new();
        assert!(factory.contains("*group"));

        let renderer = factory.create("*group").unwrap();
        assert_eq!(renderer.type_name(), "*group");

        let compatible = factory.search_compatible_renderers(false);
        assert!(compatible.iter().any(|name| name == "*group"));
        assert!(factory.create("__no_such_renderer__").is_err());

        let first = factory.create("*group").unwrap();
        let second = factory.create("*group").unwrap();
        assert_ne!(first.uid(), second.uid());
    }
}
