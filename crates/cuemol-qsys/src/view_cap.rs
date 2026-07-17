/// Rendering capabilities exposed by a view backend.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ViewCap {
    vbo: bool,
    fbo: bool,
    vertex_shader: bool,
    fragment_shader: bool,
    geometry_shader: bool,
}

impl ViewCap {
    pub const fn new(
        vbo: bool,
        fbo: bool,
        vertex_shader: bool,
        fragment_shader: bool,
        geometry_shader: bool,
    ) -> Self {
        Self {
            vbo,
            fbo,
            vertex_shader,
            fragment_shader,
            geometry_shader,
        }
    }

    pub const fn has_vbo(self) -> bool {
        self.vbo
    }

    pub const fn has_fbo(self) -> bool {
        self.fbo
    }

    pub const fn has_vertex_shader(self) -> bool {
        self.vertex_shader
    }

    pub const fn has_fragment_shader(self) -> bool {
        self.fragment_shader
    }

    pub const fn has_geometry_shader(self) -> bool {
        self.geometry_shader
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_view_has_no_gpu_capabilities() {
        let capabilities = ViewCap::default();
        assert!(!capabilities.has_vbo());
        assert!(!capabilities.has_fbo());
        assert!(!capabilities.has_vertex_shader());
        assert!(!capabilities.has_fragment_shader());
        assert!(!capabilities.has_geometry_shader());
    }

    #[test]
    fn backend_can_report_individual_capabilities() {
        let capabilities = ViewCap::new(true, true, true, true, false);
        assert!(capabilities.has_vbo());
        assert!(capabilities.has_fbo());
        assert!(capabilities.has_vertex_shader());
        assert!(capabilities.has_fragment_shader());
        assert!(!capabilities.has_geometry_shader());
    }
}
