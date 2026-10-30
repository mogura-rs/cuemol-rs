use super::*;
#[derive(Debug, Clone)]
pub enum GuiPastedNode {
    Camera(String),
    Style { index: usize, name: String },
    Object(Uid),
    Renderers { uids: Vec<Uid>, group: Option<Uid> },
}

impl DesktopAppState {
    pub fn set_density_map_sampling(&mut self, id: Uid, stride: usize, kind: &str) -> Result<()> {
        if !matches!(kind, "auto" | "xtal" | "em") {
            return Err(CueMolError::invalid_argument("invalid map type"));
        }
        let map = self
            .density_maps
            .get_mut(&id)
            .ok_or_else(|| CueMolError::not_found("density map"))?;
        if stride > 1 {
            map.map.subsample(stride)?;
            map.modified = true;
        }
        map.map.set_map_type(kind)?;
        map.modified = true;
        Ok(())
    }

    pub fn density_map_level_unit(&self, id: Uid) -> Result<&str> {
        let map = self
            .density_maps
            .get(&id)
            .ok_or_else(|| CueMolError::not_found("map"))?;
        Ok(if map.map.is_em() { "percent" } else { "sigma" })
    }

    pub fn paste_command_xml(
        &mut self,
        kind: &str,
        form: &str,
        xml: &str,
        target: Option<Uid>,
        target_type: Option<&str>,
    ) -> Result<GuiPastedNode> {
        self.paste_clipboard_bytes(kind, form, xml.as_bytes(), target, target_type)
    }
}
