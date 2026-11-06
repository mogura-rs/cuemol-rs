use crate::AnimMotion;
use cuemol_core::{CueMolError, Result};

#[derive(Debug, Clone, Copy)]
pub struct AnimPropertySpec {
    pub name: &'static str,
    pub type_name: &'static str,
    pub read_only: bool,
    pub default: Option<&'static str>,
}

macro_rules! specs {
    ($(($name:literal, $kind:literal, $readonly:literal, $default:expr)),* $(,)?) => {
        &[$(AnimPropertySpec {name:$name,type_name:$kind,read_only:$readonly,default:$default}),*]
    };
}

const COMMON: &[AnimPropertySpec] = specs![
    ("name", "string", false, None),
    ("disabled", "boolean", false, Some("false")),
    ("timeRefName", "string", false, None),
    ("start", "object<TimeValue>", false, None),
    ("end", "object<TimeValue>", false, None),
    ("absStart", "object<TimeValue>", true, None),
    ("absEnd", "object<TimeValue>", true, None),
    ("quadric", "real", false, Some("0")),
    ("uid", "integer", true, None),
];

impl AnimMotion {
    pub fn property_specs(&self) -> impl Iterator<Item = &'static AnimPropertySpec> {
        let specific: &'static [AnimPropertySpec] = match self {
            Self::Camera(_) => specs![
                ("endcam", "string", false, None),
                ("ignorecenter", "boolean", false, Some("false")),
                ("ignorerotate", "boolean", false, Some("false")),
                ("ignorezoom", "boolean", false, Some("false")),
                ("ignoreslab", "boolean", false, Some("false")),
                ("keepquatpositive", "boolean", false, Some("true")),
            ],
            Self::RealProperty(_) => specs![
                ("startValue", "real", false, None),
                ("endValue", "real", false, None),
                ("prop", "string", false, None),
                ("rend", "string", false, None),
            ],
            Self::Molecule(_) => specs![
                ("startValue", "real", false, None),
                ("endValue", "real", false, None),
                ("prop", "string", false, None),
                ("mol", "string", false, None),
            ],
            Self::RendererTransform(_) => specs![
                ("prop", "string", false, None),
                ("rend", "string", false, None),
                ("start_tran", "object<Vector>", false, None),
                ("end_tran", "object<Vector>", false, None),
            ],
            Self::ShowHide(_) => specs![
                ("hide", "boolean", false, Some("false")),
                ("fade", "boolean", false, Some("true")),
                ("rend", "string", false, None),
                ("tgt_alpha", "real", false, Some("1")),
            ],
            Self::Spin(_) => specs![
                ("angle", "real", false, Some("0")),
                ("axis", "object<Vector>", false, Some("(0,1,0,0)")),
            ],
            Self::Slide(_) => specs![
                ("rend", "string", false, None),
                ("hide", "boolean", false, Some("false")),
                ("direction", "real", false, Some("0")),
                ("distance", "real", false, Some("1")),
            ],
            Self::Noop(_) | Self::Unknown { .. } => &[],
        };
        COMMON.iter().chain(specific)
    }

    pub fn property_spec(&self, name: &str) -> Option<&'static AnimPropertySpec> {
        self.property_specs().find(|spec| spec.name == name)
    }

    pub fn is_property_default(&self, name: &str) -> bool {
        self.property_spec(name)
            .is_some_and(|spec| spec.default.is_some())
            && !self.base().modified_defaults.contains(name)
    }

    pub fn reset_property(&mut self, name: &str) -> Result<()> {
        if self
            .property_spec(name)
            .is_none_or(|spec| spec.default.is_none())
        {
            return Err(CueMolError::invalid_argument(format!(
                "{name} has no writable default"
            )));
        }
        let mut node = self.to_ldom2_node();
        node.remove_child(name);
        let mut reset = Self::from_ldom2_node(&node)?;
        reset.base_mut().set_abs_start(self.base().abs_start());
        reset.base_mut().set_abs_end(self.base().abs_end());
        *self = reset;
        Ok(())
    }
}
