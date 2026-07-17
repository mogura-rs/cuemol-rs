use cuemol_core::LDom2Node;
use std::collections::BTreeMap;

pub const MAT_AMBIENT: usize = 0;
pub const MAT_DIFFUSE: usize = 1;
pub const MAT_SPECULAR: usize = 2;
pub const MAT_SHININESS: usize = 3;
pub const MAT_EMISSION: usize = 4;

const DEFAULT_VALUES: [f64; 5] = [0.2, 0.8, 0.0, 0.0, 0.0];

#[derive(Debug, Clone, PartialEq)]
pub struct Material {
    system_values: [f64; 5],
    has_system_values: bool,
    dependent_values: BTreeMap<String, String>,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            system_values: DEFAULT_VALUES,
            has_system_values: false,
            dependent_values: BTreeMap::new(),
        }
    }
}

impl Material {
    pub const MAT_AMBIENT: usize = MAT_AMBIENT;
    pub const MAT_DIFFUSE: usize = MAT_DIFFUSE;
    pub const MAT_SPECULAR: usize = MAT_SPECULAR;
    pub const MAT_SHININESS: usize = MAT_SHININESS;
    pub const MAT_EMISSION: usize = MAT_EMISSION;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_sys_value(&mut self, parameter: usize, value: f64) {
        self.system_values[parameter] = value;
        self.has_system_values = true;
    }

    pub fn get_sys_value(&self, parameter: usize) -> f64 {
        self.system_values[parameter]
    }

    pub fn has_sys_value(&self, _parameter: usize) -> bool {
        self.has_system_values
    }

    pub fn set_dep_value(&mut self, renderer_type: impl Into<String>, value: impl Into<String>) {
        self.dependent_values
            .insert(renderer_type.into(), value.into());
    }

    pub fn get_dep_value(&self, renderer_type: &str) -> String {
        self.dependent_values
            .get(renderer_type)
            .cloned()
            .unwrap_or_default()
    }

    pub fn has_dep_value(&self, renderer_type: &str) -> bool {
        self.dependent_values.contains_key(renderer_type)
    }

    pub fn dependent_values(&self) -> &BTreeMap<String, String> {
        &self.dependent_values
    }

    /// Serialize material definitions into child `def` nodes.
    ///
    /// The C++ implementation emits only non-default ambient, diffuse and
    /// specular values. Shininess and emission are retained in memory but are
    /// not serialized there either.
    pub fn write_to(&self, node: &mut LDom2Node) {
        if self.has_system_values {
            let child = node.append_child("def");
            for (name, parameter) in [
                ("ambient", MAT_AMBIENT),
                ("diffuse", MAT_DIFFUSE),
                ("specular", MAT_SPECULAR),
            ] {
                let value = self.system_values[parameter];
                if (value - DEFAULT_VALUES[parameter]).abs() >= 1.0e-4 {
                    child.set_str_attr(name, value.to_string());
                }
            }
        }

        for (renderer_type, value) in &self.dependent_values {
            let child = node.append_child("def");
            child.set_str_attr("type", renderer_type);
            child.set_contents(value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_defaults_values_and_serialization_match_reference() {
        let mut material = Material::new();
        assert_eq!(material.get_sys_value(MAT_AMBIENT), 0.2);
        assert_eq!(material.get_sys_value(MAT_DIFFUSE), 0.8);
        assert!(!material.has_sys_value(MAT_SPECULAR));

        material.set_sys_value(MAT_AMBIENT, 0.3);
        material.set_sys_value(MAT_SHININESS, 20.0);
        material.set_dep_value("pov", "finish { phong 0.5 }");
        assert!(material.has_sys_value(MAT_EMISSION));
        assert!(material.has_dep_value("pov"));
        assert_eq!(material.get_dep_value("missing"), "");

        let mut node = LDom2Node::new();
        node.set_tag_name("material");
        material.write_to(&mut node);
        assert_eq!(node.child_count(), 2);
        assert_eq!(node.children()[0].str_attr("ambient").to_string(), "0.3");
        assert_eq!(node.children()[0].str_attr("shininess").to_string(), "");
        assert_eq!(node.children()[1].str_attr("type").to_string(), "pov");
        assert_eq!(node.children()[1].contents(), "finish { phong 0.5 }");
    }
}
