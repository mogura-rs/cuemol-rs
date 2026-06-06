use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SysConfigSection {
    name: String,
    persistent: bool,
    const_flag: bool,
    raw_data: Option<String>,
    children: Vec<SysConfigSection>,
}

impl Default for SysConfigSection {
    fn default() -> Self {
        Self::new("")
    }
}

impl SysConfigSection {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            persistent: true,
            const_flag: false,
            raw_data: None,
            children: Vec::new(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, value: impl Into<String>) {
        self.name = value.into();
    }

    pub fn is_persistent(&self) -> bool {
        self.persistent
    }

    pub fn set_persistent(&mut self, value: bool) {
        self.persistent = value;
    }

    pub fn is_const(&self) -> bool {
        self.const_flag
    }

    pub fn set_const(&mut self, value: bool) {
        self.const_flag = value;
    }

    pub fn has_data(&self) -> bool {
        self.raw_data.is_some()
    }

    pub fn raw_data(&self) -> Option<&str> {
        self.raw_data.as_deref()
    }

    pub fn set_raw_data(&mut self, value: impl Into<String>) {
        self.raw_data = Some(value.into());
    }

    pub fn string_data(&self) -> &str {
        self.raw_data.as_deref().unwrap_or("")
    }

    pub fn size(&self) -> usize {
        self.children.len()
    }

    pub fn push_child(&mut self, child: SysConfigSection) {
        self.children.push(child);
    }

    pub fn clear(&mut self) {
        self.children.clear();
    }

    pub fn find_name(&self, start: usize, name: &str) -> Option<usize> {
        self.children
            .iter()
            .enumerate()
            .skip(start)
            .find_map(|(index, child)| (child.name == name).then_some(index))
    }

    pub fn child(&self, index: usize) -> Option<&SysConfigSection> {
        self.children.get(index)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SysConfig {
    root: SysConfigSection,
}

impl SysConfig {
    pub const DELIMITER: char = ':';

    pub fn new() -> Self {
        Self::default()
    }

    pub fn global() -> &'static Mutex<SysConfig> {
        static INSTANCE: OnceLock<Mutex<SysConfig>> = OnceLock::new();
        INSTANCE.get_or_init(|| Mutex::new(SysConfig::new()))
    }

    pub fn root(&self) -> &SysConfigSection {
        &self.root
    }

    pub fn root_mut(&mut self) -> &mut SysConfigSection {
        &mut self.root
    }

    pub fn section(&self, key: &str) -> Option<&SysConfigSection> {
        let parts = section_parts(key);
        if parts.is_empty() {
            return None;
        }
        let mut current = &self.root;
        for part in parts {
            let index = current.find_name(0, part)?;
            current = current.child(index)?;
        }
        Some(current)
    }

    pub fn section_mut(&mut self, key: &str, create: bool) -> Option<&mut SysConfigSection> {
        let parts = section_parts(key);
        if parts.is_empty() {
            return None;
        }
        let mut current = &mut self.root;
        for part in parts {
            let index = match current.children.iter().position(|child| child.name == part) {
                Some(index) => index,
                None if create => {
                    current.children.push(SysConfigSection::new(part));
                    current.children.len() - 1
                }
                None => return None,
            };
            current = current.children.get_mut(index).unwrap();
        }
        Some(current)
    }

    pub fn get(&self, key: &str) -> &str {
        self.section(key)
            .and_then(SysConfigSection::raw_data)
            .unwrap_or("")
    }

    pub fn put(&mut self, key: &str, value: impl Into<String>) {
        if let Some(section) = self.section_mut(key, true) {
            section.set_raw_data(value);
        }
    }

    pub fn conv_path_name(&self, path: &str) -> String {
        if let Some(rest) = path.strip_prefix("%%CONFDIR%%") {
            let conf_dir = self.get("config_dir");
            if conf_dir.is_empty() {
                return path.to_string();
            }
            return format!("{conf_dir}{rest}");
        }
        if let Some(rest) = path.strip_prefix("%%QSDIR%%") {
            let qs_dir = self.conv_path_name(self.get("qscript_dir"));
            if qs_dir.is_empty() {
                return path.to_string();
            }
            return format!("{qs_dir}{rest}");
        }
        path.to_string()
    }
}

fn section_parts(key: &str) -> Vec<&str> {
    key.split(SysConfig::DELIMITER)
        .take_while(|part| !part.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sys_config_section_matches_reference_properties() {
        let mut section = SysConfigSection::default();
        assert!(section.is_persistent());
        assert!(!section.is_const());
        assert!(!section.has_data());
        assert_eq!(section.size(), 0);

        let named = SysConfigSection::new("mySec");
        assert_eq!(named.name(), "mySec");

        section.set_persistent(false);
        assert!(!section.is_persistent());
        section.set_const(true);
        assert!(section.is_const());

        section.set_raw_data("hello");
        assert!(section.has_data());
        assert_eq!(section.string_data(), "hello");

        let mut parent = SysConfigSection::default();
        parent.push_child(SysConfigSection::new("alpha"));
        parent.push_child(SysConfigSection::new("beta"));
        assert_eq!(parent.size(), 2);
        assert_eq!(parent.find_name(0, "alpha"), Some(0));
        assert_eq!(parent.find_name(0, "gamma"), None);
    }

    #[test]
    fn sys_config_put_get_sections_and_path_conversion_match_reference() {
        let mut config = SysConfig::new();
        config.put("test_key", "test_value");
        assert_eq!(config.get("test_key"), "test_value");
        assert_eq!(config.get("no_such_key_xyz"), "");

        assert!(config.section_mut("a:b:c", true).is_some());
        assert!(config.section("a:b:c").is_some());
        assert!(config.section("no:such:path").is_none());

        assert_eq!(config.conv_path_name("/usr/local/bin"), "/usr/local/bin");
        config.put("config_dir", "/etc/cuemol");
        assert_eq!(
            config.conv_path_name("%%CONFDIR%%/data"),
            "/etc/cuemol/data"
        );
        assert_eq!(config.conv_path_name("%%CONFDIR%%"), "/etc/cuemol");
    }
}
