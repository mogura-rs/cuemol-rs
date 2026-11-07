use crate::DesktopAppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinPlugin {
    pub id: &'static str,
    pub name: &'static str,
    pub default_enabled: bool,
    pub always_enabled: bool,
}

pub const BUILTIN_PLUGINS: &[BuiltinPlugin] = &[
    BuiltinPlugin {
        id: "getpdb",
        name: "Get PDB",
        default_enabled: true,
        always_enabled: true,
    },
    BuiltinPlugin {
        id: "sequence",
        name: "Sequence Panel",
        default_enabled: true,
        always_enabled: true,
    },
    BuiltinPlugin {
        id: "catalog",
        name: "Component Catalog",
        default_enabled: false,
        always_enabled: false,
    },
    BuiltinPlugin {
        id: "agent",
        name: "AI Agent",
        default_enabled: false,
        always_enabled: false,
    },
    BuiltinPlugin {
        id: "mdtools",
        name: "MD Tools",
        default_enabled: false,
        always_enabled: false,
    },
    BuiltinPlugin {
        id: "console",
        name: "Console",
        default_enabled: true,
        always_enabled: false,
    },
    BuiltinPlugin {
        id: "pymol-console",
        name: "PyMOL console",
        default_enabled: true,
        always_enabled: false,
    },
    BuiltinPlugin {
        id: "chimerax-console",
        name: "ChimeraX console (ChX)",
        default_enabled: true,
        always_enabled: false,
    },
    BuiltinPlugin {
        id: "mcp",
        name: "MCP Server",
        default_enabled: false,
        always_enabled: false,
    },
];

impl DesktopAppState {
    pub fn mcp_server_enabled(&self) -> bool {
        self.plugin_enabled("mcp") && self.config_value("mcp:accept-connections") != "false"
    }

    pub fn mcp_server_port(&self) -> crate::Result<u16> {
        let value = self.config_value("mcp:port");
        if value.is_empty() {
            return Ok(27182);
        }
        value
            .parse()
            .map_err(|_| crate::CueMolError::parse("MCP port must be between 0 and 65535"))
    }

    pub fn plugin_enabled(&self, id: &str) -> bool {
        let Some(plugin) = BUILTIN_PLUGINS.iter().find(|plugin| plugin.id == id) else {
            return false;
        };
        if plugin.always_enabled {
            return true;
        }
        let value = self.config_value(&format!("plugins:{id}:enabled"));
        // Retain the explicit preference used before the plugin registry.
        let value = if value.is_empty() && id == "mcp" {
            self.config_value("mcp:enabled")
        } else {
            value
        };
        match value.as_str() {
            "true" => true,
            "false" => false,
            _ => plugin.default_enabled,
        }
    }

    pub fn set_plugin_enabled(&mut self, id: &str, enabled: bool) -> crate::Result<()> {
        let plugin = BUILTIN_PLUGINS
            .iter()
            .find(|plugin| plugin.id == id)
            .ok_or_else(|| crate::CueMolError::parse(format!("unknown plugin: {id}")))?;
        if plugin.always_enabled && !enabled {
            return Err(crate::CueMolError::parse(format!(
                "{} is always enabled",
                plugin.name
            )));
        }
        self.put_config_value(&format!("plugins:{id}:enabled"), enabled.to_string());
        Ok(())
    }

    pub fn set_plugin_states(
        &mut self,
        values: &std::collections::BTreeMap<String, bool>,
    ) -> crate::Result<()> {
        for (id, enabled) in values {
            let plugin = BUILTIN_PLUGINS
                .iter()
                .find(|plugin| plugin.id == id)
                .ok_or_else(|| crate::CueMolError::parse(format!("unknown plugin: {id}")))?;
            if plugin.always_enabled && !enabled {
                return Err(crate::CueMolError::parse(format!(
                    "{} is always enabled",
                    plugin.name
                )));
            }
        }
        for (id, enabled) in values {
            self.set_plugin_enabled(id, *enabled)?;
        }
        Ok(())
    }

    pub fn plugin_states(&self) -> std::collections::BTreeMap<String, bool> {
        BUILTIN_PLUGINS
            .iter()
            .map(|plugin| (plugin.id.into(), self.plugin_enabled(plugin.id)))
            .collect()
    }

    pub fn plugin_tab_enabled(&self, tab: &str) -> bool {
        match tab.to_ascii_lowercase().as_str() {
            "console" => self.plugin_enabled("console"),
            "trajectory" | "md trajectory" => self.plugin_enabled("mdtools"),
            _ => true,
        }
    }

    pub fn console_dialect_enabled(&self, dialect: &str) -> bool {
        self.plugin_enabled("console")
            && match dialect.to_ascii_lowercase().as_str() {
                "pymol" => self.plugin_enabled("pymol-console"),
                "chimerax" | "chx" => self.plugin_enabled("chimerax-console"),
                _ => true,
            }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_defaults_and_switches_preserve_required_contributions() {
        let mut state = DesktopAppState::new();
        for plugin in BUILTIN_PLUGINS {
            assert_eq!(
                state.plugin_enabled(plugin.id),
                plugin.default_enabled,
                "{}",
                plugin.id
            );
        }
        assert!(state.set_plugin_enabled("getpdb", false).is_err());
        assert!(state.set_plugin_enabled("sequence", false).is_err());
        assert!(state.set_plugin_enabled("missing", true).is_err());
        let before = state.config_text();
        assert!(state
            .set_plugin_states(
                &[("agent".into(), true), ("missing".into(), true)]
                    .into_iter()
                    .collect()
            )
            .is_err());
        assert_eq!(state.config_text(), before);
        assert!(!state.mcp_server_enabled());
        state.put_config_value("mcp:enabled", "true");
        assert!(state.mcp_server_enabled());
        state.set_plugin_enabled("mcp", false).unwrap();
        assert!(!state.mcp_server_enabled());
        state.set_plugin_enabled("pymol-console", false).unwrap();
        assert!(!state.console_dialect_enabled("pymol"));
        assert!(state.console_dialect_enabled("chimerax"));
        state.set_plugin_enabled("console", false).unwrap();
        assert!(!state.console_dialect_enabled("chimerax"));
        state.set_plugin_enabled("console", true).unwrap();
        assert!(!state.console_dialect_enabled("pymol"));
        state.set_plugin_enabled("mdtools", true).unwrap();
        let mut reloaded = DesktopAppState::new();
        reloaded.load_config_text(&state.config_text()).unwrap();
        assert!(reloaded.plugin_tab_enabled("MD Trajectory"));
        assert!(!reloaded.console_dialect_enabled("pymol"));
        reloaded.put_config_value("plugins:sequence:enabled", "false");
        assert!(reloaded.plugin_enabled("sequence"));
    }
}
