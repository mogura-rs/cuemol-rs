use cuemol_core::{CueMolError, Result};

pub const STYLES: [&str; 9] = [
    "richardson",
    "ink-cross",
    "manga",
    "pen-cross",
    "pencil",
    "engraving",
    "stipple",
    "screentone-60",
    "manga-square",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Line,
    Dot,
    Stipple,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    pub kind: Kind,
    pub angle: f32,
    pub spacing: f32,
    pub subdiv: i32,
    pub width: f32,
    pub dotscale: f32,
    pub tonehi: f32,
    pub tonelo: f32,
    pub fade: f32,
    pub opacity: f32,
    pub inkscale: f32,
    pub soft: f32,
    pub seed: u32,
    pub shape: f32,
    pub aspect: f32,
    pub dotangle: f32,
    pub jitter: f32,
    pub invert: bool,
    pub wobble: f32,
    pub wobwave: f32,
    pub wjitter: f32,
    pub slen: f32,
    pub sgap: f32,
    pub taper: f32,
    pub anglejitter: f32,
    pub lenjitter: f32,
    pub tooth: f32,
    pub toothscale: f32,
}
impl Default for Layer {
    fn default() -> Self {
        Self {
            kind: Kind::Line,
            angle: 45.,
            spacing: 10.,
            subdiv: 2,
            width: 1.1,
            dotscale: 1.,
            tonehi: 0.95,
            tonelo: 0.55,
            fade: 16.,
            opacity: 1.,
            inkscale: 1.,
            soft: 0.5,
            seed: 0,
            shape: 2.,
            aspect: 1.,
            dotangle: 0.,
            jitter: 0.,
            invert: true,
            wobble: 0.,
            wobwave: 40.,
            wjitter: 0.,
            slen: 0.,
            sgap: 0.,
            taper: 0.3,
            anglejitter: 0.,
            lenjitter: 0.,
            tooth: 0.,
            toothscale: 3.,
        }
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct Tone {
    pub diffuse: f32,
    pub ambient: f32,
    pub wrap: f32,
    pub rim: f32,
    pub rimpow: f32,
    pub rimbias: f32,
    pub contact: f32,
    pub shape: f32,
    pub black: f32,
    pub white: f32,
    pub hl: f32,
    pub hlsoft: f32,
    pub gamma: f32,
    pub speccut: f32,
    pub strength: f32,
    pub curve: f32,
}
impl Default for Tone {
    fn default() -> Self {
        Self {
            diffuse: 1.,
            ambient: 0.12,
            wrap: 0.,
            rim: 0.,
            rimpow: 1.,
            rimbias: 0.6,
            contact: 1.,
            shape: 0.6,
            black: 0.,
            white: 1.,
            hl: 1.,
            hlsoft: 0.06,
            gamma: 1.,
            speccut: 0.,
            strength: 1.,
            curve: 1.,
        }
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct Look {
    pub layers: Vec<Layer>,
    pub tone: Tone,
    pub over: bool,
    pub albedo_base: bool,
    pub albedo_ink: bool,
    pub ink: [f32; 3],
    pub paper: [f32; 3],
    pub mincontrast: f32,
    pub inkshade: f32,
    pub tonefog: bool,
    pub levels: i32,
    pub albedoquant: i32,
}
impl Default for Look {
    fn default() -> Self {
        Self {
            layers: Vec::new(),
            tone: Tone::default(),
            over: false,
            albedo_base: false,
            albedo_ink: false,
            ink: [0.; 3],
            paper: [1.; 3],
            mincontrast: 0.25,
            inkshade: 1.,
            tonefog: true,
            levels: 0,
            albedoquant: 0,
        }
    }
}
fn invalid(value: &str) -> CueMolError {
    CueMolError::invalid_argument(format!("invalid hatch spec: {value}"))
}
fn number(value: &str) -> Result<f32> {
    value
        .parse::<f32>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| invalid(value))
}
fn boolean(value: &str) -> Result<bool> {
    match value {
        "on" | "1" | "true" => Ok(true),
        "off" | "0" | "false" => Ok(false),
        _ => Err(invalid(value)),
    }
}
fn integer(value: &str) -> Result<i32> {
    value.parse().map_err(|_| invalid(value))
}
fn color(value: &str) -> Result<[f32; 3]> {
    let value = value.strip_prefix('#').ok_or_else(|| invalid(value))?;
    if value.len() != 6 {
        return Err(invalid(value));
    }
    let v = u32::from_str_radix(value, 16).map_err(|_| invalid(value))?;
    Ok([
        ((v >> 16) & 255) as f32 / 255.,
        ((v >> 8) & 255) as f32 / 255.,
        (v & 255) as f32 / 255.,
    ])
}
macro_rules! numeric_keys {($this:expr,$key:expr,$value:expr,$($field:ident),+)=>{{
    match $key {$(stringify!($field)=>$this.$field=number($value)?,)+_=>return Err(invalid($key))} Ok(())
}}}
impl Layer {
    fn set(&mut self, key: &str, value: &str) -> Result<()> {
        match key {
            "kind" => {
                self.kind = match value {
                    "line" => Kind::Line,
                    "dot" => Kind::Dot,
                    "stipple" => Kind::Stipple,
                    _ => return Err(invalid(value)),
                }
            }
            "subdiv" => self.subdiv = integer(value)?,
            "seed" => self.seed = value.parse().map_err(|_| invalid(value))?,
            "invert" => self.invert = boolean(value)?,
            _ => {
                return numeric_keys!(
                    self,
                    key,
                    value,
                    angle,
                    spacing,
                    width,
                    dotscale,
                    tonehi,
                    tonelo,
                    fade,
                    opacity,
                    inkscale,
                    soft,
                    shape,
                    aspect,
                    dotangle,
                    jitter,
                    wobble,
                    wobwave,
                    wjitter,
                    slen,
                    sgap,
                    taper,
                    anglejitter,
                    lenjitter,
                    tooth,
                    toothscale
                )
            }
        }
        Ok(())
    }
}
impl Tone {
    fn set(&mut self, key: &str, value: &str) -> Result<()> {
        numeric_keys!(
            self, key, value, diffuse, ambient, wrap, rim, rimpow, rimbias, contact, shape, black,
            white, hl, hlsoft, gamma, speccut, strength, curve
        )
    }
}
impl Look {
    fn set_ink(&mut self, key: &str, value: &str) -> Result<()> {
        match key {
            "mode" => {
                self.over = match value {
                    "ink" => false,
                    "over" => true,
                    _ => return Err(invalid(value)),
                }
            }
            "base" => {
                self.albedo_base = match value {
                    "paper" => false,
                    "albedo" => true,
                    _ => return Err(invalid(value)),
                }
            }
            "ink" => {
                self.albedo_ink = match value {
                    "fixed" => false,
                    "albedo" => true,
                    _ => return Err(invalid(value)),
                }
            }
            "inkcolor" => self.ink = color(value)?,
            "papercolor" => self.paper = color(value)?,
            "tonefog" => self.tonefog = boolean(value)?,
            "levels" => self.levels = integer(value)?,
            "albedoquant" => self.albedoquant = integer(value)?,
            _ => return numeric_keys!(self, key, value, mincontrast, inkshade),
        }
        Ok(())
    }
    pub fn apply_spec(&mut self, spec: &str, sections: u8) -> Result<()> {
        let mut next = self.clone();
        let mut layers = Vec::new();
        for (line_number, line) in spec.split(['\n', ';']).enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (section, entries) = line
                .split_once(':')
                .ok_or_else(|| invalid(&format!("line {}: expected section:", line_number + 1)))?;
            let section = section.trim();
            let bit = match section {
                "layer" => 1,
                "tone" => 2,
                "ink" => 4,
                _ => return Err(invalid(section)),
            };
            if bit & sections == 0 {
                continue;
            }
            let mut layer = Layer::default();
            for entry in entries.split(',').map(str::trim).filter(|v| !v.is_empty()) {
                let (key, value) = entry.split_once('=').ok_or_else(|| invalid(entry))?;
                let (key, value) = (key.trim(), value.trim());
                let result = match section {
                    "layer" => layer.set(key, value),
                    "tone" if key != "levels" => next.tone.set(key, value),
                    _ => next.set_ink(key, value),
                };
                result.map_err(|e| invalid(&format!("line {}: {e}", line_number + 1)))?;
            }
            if section == "layer" {
                layers.push(layer);
            }
            if layers.len() > 64 {
                return Err(invalid("at most 64 layers are supported"));
            }
        }
        if !layers.is_empty() {
            next.layers = layers;
        }
        *self = next;
        Ok(())
    }
    pub fn style(name: &str) -> Result<Self> {
        let preset = match name {
            "richardson" => "pencil",
            "ink-cross" => "pen-cross",
            "manga" => "screentone-60",
            s => s,
        };
        let mut look = Self::default();
        let spec=match preset {
            "pen-cross"=>"layer:angle=45,spacing=10,subdiv=2,width=1.1,tonehi=0.95,tonelo=0.55,fade=0\nlayer:angle=-45,spacing=10,subdiv=2,width=1.1,tonehi=0.62,tonelo=0.30,fade=0\nlayer:angle=0,spacing=10,subdiv=2,width=1.1,tonehi=0.32,tonelo=0.10,fade=0",
            "pencil"=>"layer:angle=55,spacing=9,subdiv=2,width=3.6,tonehi=0.92,tonelo=0.55,fade=0,opacity=0.85,soft=0.8,tooth=0.15,toothscale=3,jitter=0.22,wobble=1.2,wobwave=60,wjitter=0.45,slen=50,sgap=5,taper=0.35,anglejitter=5,lenjitter=0.5",
            "engraving"=>"layer:angle=0,spacing=16,subdiv=3,width=2.4,tonehi=0.97,tonelo=0.12,fade=0,wjitter=0.5,wobble=0.5,wobwave=64",
            "stipple"=>"layer:kind=stipple,angle=0,spacing=3,subdiv=0,dotscale=0.9,tonehi=1,tonelo=0,fade=32,soft=0.6,shape=2,jitter=0.45",
            "screentone-60"=>"layer:kind=dot,angle=45,spacing=5,subdiv=0,tonehi=1,tonelo=1,fade=32,shape=2,invert=on",
            "manga-square"=>"layer:kind=dot,angle=45,spacing=6,subdiv=0,tonehi=1,tonelo=1,fade=32,shape=16,invert=on",
            _=>return Err(CueMolError::invalid_argument(format!("unknown hatch style: {name}"))),
        };
        look.apply_spec(spec, 7)?;
        look.apply_spec("tone:diffuse=0.85,ambient=0.05,wrap=0.5,rim=1,rimpow=3.5,rimbias=0.35,contact=1,shape=0.6,white=1.2,gamma=2.4,hl=0.86,hlsoft=0.05",7)?;
        if preset == "pencil" {
            let mut l = look.layers[0].clone();
            l.angle = -35.;
            l.tonehi = 0.62;
            l.tonelo = 0.30;
            l.inkscale = 0.62;
            l.seed = 1;
            look.layers.push(l.clone());
            l.angle = 80.;
            l.tonehi = 0.34;
            l.tonelo = 0.12;
            l.inkscale = 0.38;
            l.seed = 2;
            look.layers.push(l);
        }
        match name {
            "richardson" => {
                look.albedo_ink = true;
                look.paper = [0.941, 0.925, 0.867];
                look.mincontrast = 0.15;
                look.inkshade = 0.4;
                for l in &mut look.layers {
                    l.spacing = 0.5;
                    l.width = 0.45;
                    l.soft = 0.55;
                    l.opacity = 1.;
                    l.fade = 10.;
                }
                look.layers[1].inkscale = 0.74;
            }
            "manga" => {
                look.albedo_base = true;
                look.mincontrast = 0.35;
            }
            _ => {}
        }
        Ok(look)
    }
    pub fn spec(&self) -> String {
        let mut lines = Vec::new();
        for l in &self.layers {
            let kind = match l.kind {
                Kind::Line => "line",
                Kind::Dot => "dot",
                Kind::Stipple => "stipple",
            };
            let mut s = format!(
                "layer:kind={kind},subdiv={},seed={},invert={}",
                l.subdiv, l.seed, l.invert
            );
            macro_rules! fields {($($f:ident),+)=>{$(s.push_str(&format!(",{}={}",stringify!($f),l.$f));)+};}
            fields!(
                angle,
                spacing,
                width,
                dotscale,
                tonehi,
                tonelo,
                fade,
                opacity,
                inkscale,
                soft,
                shape,
                aspect,
                dotangle,
                jitter,
                wobble,
                wobwave,
                wjitter,
                slen,
                sgap,
                taper,
                anglejitter,
                lenjitter,
                tooth,
                toothscale
            );
            lines.push(s);
        }
        let t = &self.tone;
        let mut s = String::from("tone:");
        macro_rules! tones {($($f:ident),+)=>{$(s.push_str(&format!("{}={},",stringify!($f),t.$f));)+};}
        tones!(
            diffuse, ambient, wrap, rim, rimpow, rimbias, contact, shape, black, white, hl, hlsoft,
            gamma, speccut, strength, curve
        );
        s.pop();
        lines.push(s);
        let c = |rgb: [f32; 3]| {
            format!(
                "#{:02x}{:02x}{:02x}",
                (rgb[0].clamp(0., 1.) * 255.).round() as u8,
                (rgb[1].clamp(0., 1.) * 255.).round() as u8,
                (rgb[2].clamp(0., 1.) * 255.).round() as u8
            )
        };
        lines.push(format!("ink:mode={},base={},ink={},inkcolor={},papercolor={},mincontrast={},inkshade={},tonefog={},levels={},albedoquant={}",
            if self.over{"over"}else{"ink"},if self.albedo_base{"albedo"}else{"paper"},if self.albedo_ink{"albedo"}else{"fixed"},c(self.ink),c(self.paper),self.mincontrast,self.inkshade,self.tonefog,self.levels,self.albedoquant));
        lines.join("\n") + "\n"
    }
}
