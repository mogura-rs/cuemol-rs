use cuemol_core::{type_consts, Vector4};
use std::cell::Cell;

use crate::{
    draw_attr::{AbstDrawElem, DrawAttrArray, DrawAttrElems},
    AbstractColor, DisplayContext, PixelBuffer,
};

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SphElem {
    pub cenx: f32,
    pub ceny: f32,
    pub cenz: f32,
    pub dspx: f32,
    pub dspy: f32,
    pub rad: f32,
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CylElem {
    pub cenx: f32,
    pub ceny: f32,
    pub cenz: f32,
    pub dirx: f32,
    pub diry: f32,
    pub dirz: f32,
    pub dspx: f32,
    pub dspy: f32,
    pub rad: f32,
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TrigVertAttr {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub nx: f32,
    pub ny: f32,
    pub nz: f32,
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LineElem {
    pub x1: f32,
    pub y1: f32,
    pub z1: f32,
    pub x2: f32,
    pub y2: f32,
    pub z2: f32,
    pub r1: u8,
    pub g1: u8,
    pub b1: u8,
    pub a1: u8,
    pub r2: u8,
    pub g2: u8,
    pub b2: u8,
    pub a2: u8,
}

pub type SphElemAry32 = DrawAttrElems<u32, SphElem>;
pub type CylElemAry32 = DrawAttrElems<u32, CylElem>;
pub type TrigMesh = DrawAttrElems<u32, TrigVertAttr>;
pub type LineArray = DrawAttrElems<u32, LineElem>;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PixVertAttr {
    pub x: f32,
    pub y: f32,
    pub tx: f32,
    pub ty: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PixDrawParams {
    pub frag_alpha: f32,
    pub padding1: [f32; 3],
    pub position: [f32; 3],
    pub padding2: f32,
    pub size: [f32; 2],
    pub viewport_size: [f32; 2],
    pub color_bias: [f32; 3],
    pub padding3: f32,
}

pub type PixQuad = DrawAttrArray<PixVertAttr>;

const DSPS: [[f32; 2]; 4] = [[-1.0, -1.0], [1.0, -1.0], [-1.0, 1.0], [1.0, 1.0]];

#[derive(Debug, Default)]
pub struct SphereGpuPrim {
    initialized: bool,
    draw_elem: Option<SphElemAry32>,
    draw_count: Cell<usize>,
}

impl SphereGpuPrim {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn init(&mut self) -> bool {
        self.initialized = true;
        true
    }

    pub fn alloc(&mut self, nsph: usize) {
        self.initialized = true;
        let mut data = SphElemAry32::new();
        data.base_mut().set_attr_size(4);
        data.base_mut().set_attr_info(
            0,
            0,
            3,
            type_consts::QTC_FLOAT32,
            std::mem::offset_of!(SphElem, cenx) as i32,
        );
        data.base_mut().set_attr_info(
            1,
            1,
            2,
            type_consts::QTC_FLOAT32,
            std::mem::offset_of!(SphElem, dspx) as i32,
        );
        data.base_mut().set_attr_info(
            2,
            2,
            1,
            type_consts::QTC_FLOAT32,
            std::mem::offset_of!(SphElem, rad) as i32,
        );
        data.base_mut().set_attr_info(
            3,
            3,
            4,
            type_consts::QTC_UINT8,
            std::mem::offset_of!(SphElem, r) as i32,
        );
        data.alloc(nsph * 4);
        data.alloc_ind(nsph * 6);
        data.base_mut()
            .draw_elem_mut()
            .set_draw_mode(AbstDrawElem::DRAW_TRIANGLES);
        self.draw_elem = Some(data);
    }

    pub fn is_valid(&self) -> bool {
        self.initialized && self.draw_elem.is_some()
    }

    pub fn size(&self) -> usize {
        self.draw_elem.as_ref().map_or(0, |data| data.size() / 4)
    }

    pub fn data(&self) -> Option<&SphElemAry32> {
        self.draw_elem.as_ref()
    }

    pub fn set_data(&mut self, index: usize, pos: Vector4, rad: f32, devcode: u32) {
        let data = self
            .draw_elem
            .as_mut()
            .expect("sphere primitive is not allocated");
        let base = index * 4;
        let ind = index * 6;
        set_quad_indices(data, base, ind);

        let (r, g, b, a) = split_devcode(devcode);
        let common = SphElem {
            cenx: pos.x as f32,
            ceny: pos.y as f32,
            cenz: pos.z as f32,
            rad,
            r,
            g,
            b,
            a,
            ..SphElem::default()
        };

        for (offset, dsp) in DSPS.iter().enumerate() {
            *data.at_mut(base + offset) = SphElem {
                dspx: dsp[0],
                dspy: dsp[1],
                ..common
            };
        }
    }

    pub fn draw(&self) {
        if self.is_valid() {
            self.draw_count.set(self.draw_count.get() + 1);
        }
    }

    pub fn draw_count(&self) -> usize {
        self.draw_count.get()
    }

    pub fn invalidate(&mut self) {
        self.draw_elem = None;
    }
}

#[derive(Debug, Default)]
pub struct CylinderGpuPrim {
    initialized: bool,
    draw_elem: Option<CylElemAry32>,
    draw_count: Cell<usize>,
}

impl CylinderGpuPrim {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn init(&mut self) -> bool {
        self.initialized = true;
        true
    }

    pub fn alloc(&mut self, ncyl: usize) {
        self.initialized = true;
        let mut data = CylElemAry32::new();
        data.base_mut().set_attr_size(5);
        data.base_mut().set_attr_info(
            0,
            0,
            3,
            type_consts::QTC_FLOAT32,
            std::mem::offset_of!(CylElem, cenx) as i32,
        );
        data.base_mut().set_attr_info(
            1,
            1,
            3,
            type_consts::QTC_FLOAT32,
            std::mem::offset_of!(CylElem, dirx) as i32,
        );
        data.base_mut().set_attr_info(
            2,
            2,
            2,
            type_consts::QTC_FLOAT32,
            std::mem::offset_of!(CylElem, dspx) as i32,
        );
        data.base_mut().set_attr_info(
            3,
            3,
            1,
            type_consts::QTC_FLOAT32,
            std::mem::offset_of!(CylElem, rad) as i32,
        );
        data.base_mut().set_attr_info(
            4,
            4,
            4,
            type_consts::QTC_UINT8,
            std::mem::offset_of!(CylElem, r) as i32,
        );
        data.alloc(ncyl * 4);
        data.alloc_ind(ncyl * 6);
        data.base_mut()
            .draw_elem_mut()
            .set_draw_mode(AbstDrawElem::DRAW_TRIANGLES);
        self.draw_elem = Some(data);
    }

    pub fn is_valid(&self) -> bool {
        self.initialized && self.draw_elem.is_some()
    }

    pub fn size(&self) -> usize {
        self.draw_elem.as_ref().map_or(0, |data| data.size() / 4)
    }

    pub fn data(&self) -> Option<&CylElemAry32> {
        self.draw_elem.as_ref()
    }

    pub fn set_data(&mut self, index: usize, pos1: Vector4, pos2: Vector4, rad: f32, devcode: u32) {
        let data = self
            .draw_elem
            .as_mut()
            .expect("cylinder primitive is not allocated");
        let base = index * 4;
        let ind = index * 6;
        set_quad_indices(data, base, ind);

        let dir = pos2.sub_vec(pos1);
        let (r, g, b, a) = split_devcode(devcode);
        let mut common = CylElem {
            cenx: pos1.x as f32,
            ceny: pos1.y as f32,
            cenz: pos1.z as f32,
            dirx: dir.x as f32,
            diry: dir.y as f32,
            dirz: dir.z as f32,
            rad,
            r,
            g,
            b,
            a,
            ..CylElem::default()
        };

        for (offset, dsp) in DSPS.iter().take(2).enumerate() {
            *data.at_mut(base + offset) = CylElem {
                dspx: dsp[0],
                dspy: dsp[1],
                ..common
            };
        }

        common.cenx = pos2.x as f32;
        common.ceny = pos2.y as f32;
        common.cenz = pos2.z as f32;
        common.dirx = -common.dirx;
        common.diry = -common.diry;
        common.dirz = -common.dirz;

        for (offset, dsp) in DSPS.iter().skip(2).enumerate() {
            *data.at_mut(base + 2 + offset) = CylElem {
                dspx: dsp[0],
                dspy: dsp[1],
                ..common
            };
        }
    }

    pub fn draw(&self) {
        if self.is_valid() {
            self.draw_count.set(self.draw_count.get() + 1);
        }
    }

    pub fn draw_count(&self) -> usize {
        self.draw_count.get()
    }

    pub fn invalidate(&mut self) {
        self.draw_elem = None;
    }
}

#[derive(Debug)]
pub struct TrigGpuPrim {
    initialized: bool,
    draw_elem: Option<TrigMesh>,
    edge_line_type: i32,
    polygon_mode: i32,
    face_count: usize,
    no_depth: bool,
    draw_count: Cell<usize>,
}

impl Default for TrigGpuPrim {
    fn default() -> Self {
        Self {
            initialized: false,
            draw_elem: None,
            edge_line_type: 0,
            polygon_mode: 2,
            face_count: 0,
            no_depth: false,
            draw_count: Cell::new(0),
        }
    }
}

impl TrigGpuPrim {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn init(&mut self) -> bool {
        self.initialized = true;
        true
    }

    pub fn alloc(&mut self, nverts: usize, nfaces: usize) {
        self.initialized = true;
        let mut data = TrigMesh::new();
        data.alloc(nverts);
        let wireframe = self.polygon_mode == 1;
        data.alloc_ind(nfaces * if wireframe { 6 } else { 3 });
        data.base_mut().draw_elem_mut().set_draw_mode(if wireframe {
            AbstDrawElem::DRAW_LINES
        } else {
            AbstDrawElem::DRAW_TRIANGLES
        });
        setup_trig_attrs(&mut data);
        self.face_count = nfaces;
        self.draw_elem = Some(data);
    }

    pub fn is_valid(&self) -> bool {
        self.initialized && self.draw_elem.is_some()
    }

    pub fn vertex_size(&self) -> usize {
        self.draw_elem.as_ref().map_or(0, DrawAttrElems::size)
    }

    pub fn face_size(&self) -> usize {
        self.draw_elem.as_ref().map_or(0, |_| self.face_count)
    }

    pub fn data(&self) -> Option<&TrigMesh> {
        self.draw_elem.as_ref()
    }

    pub fn set_vertex(&mut self, index: usize, value: Vector4) {
        let elem = self
            .draw_elem
            .as_mut()
            .expect("triangle primitive is not allocated")
            .at_mut(index);
        elem.x = value.x as f32;
        elem.y = value.y as f32;
        elem.z = value.z as f32;
    }

    pub fn set_normal(&mut self, index: usize, value: Vector4) {
        let elem = self
            .draw_elem
            .as_mut()
            .expect("triangle primitive is not allocated")
            .at_mut(index);
        elem.nx = value.x as f32;
        elem.ny = value.y as f32;
        elem.nz = value.z as f32;
    }

    pub fn set_color(&mut self, index: usize, devcode: u32) {
        let (r, g, b, a) = split_devcode(devcode);
        let elem = self
            .draw_elem
            .as_mut()
            .expect("triangle primitive is not allocated")
            .at_mut(index);
        elem.r = r;
        elem.g = g;
        elem.b = b;
        elem.a = a;
    }

    pub fn set_face(&mut self, index: usize, v1: u32, v2: u32, v3: u32) {
        let data = self
            .draw_elem
            .as_mut()
            .expect("triangle primitive is not allocated");
        if self.polygon_mode == 1 {
            let offset = index * 6;
            *data.at_ind_mut(offset) = v1;
            *data.at_ind_mut(offset + 1) = v2;
            *data.at_ind_mut(offset + 2) = v2;
            *data.at_ind_mut(offset + 3) = v3;
            *data.at_ind_mut(offset + 4) = v3;
            *data.at_ind_mut(offset + 5) = v1;
            return;
        }
        *data.at_ind_mut(index * 3) = v1;
        *data.at_ind_mut(index * 3 + 1) = v2;
        *data.at_ind_mut(index * 3 + 2) = v3;
    }

    pub fn edge_line_type(&self) -> i32 {
        self.edge_line_type
    }

    pub fn set_edge_line_type(&mut self, value: i32) {
        self.edge_line_type = value;
    }

    pub fn polygon_mode(&self) -> i32 {
        self.polygon_mode
    }

    pub fn set_polygon_mode(&mut self, value: i32) {
        self.polygon_mode = value;
    }

    pub fn is_no_depth(&self) -> bool {
        self.no_depth
    }

    pub fn set_no_depth(&mut self, value: bool) {
        self.no_depth = value;
    }

    pub fn set_updated(&mut self, value: bool) {
        if let Some(data) = self.draw_elem.as_mut() {
            data.base_mut().draw_elem_mut().set_updated(value);
        }
    }

    pub fn draw(&self) {
        if self.is_valid() {
            self.draw_count.set(self.draw_count.get() + 1);
        }
    }

    pub fn draw_count(&self) -> usize {
        self.draw_count.get()
    }

    pub fn invalidate(&mut self) {
        self.draw_elem = None;
        self.face_count = 0;
    }
}

#[derive(Debug)]
pub struct LineGpuPrim {
    initialized: bool,
    draw_elem: Option<LineArray>,
    line_width: f32,
    stipple: bool,
    no_depth: bool,
    use_vert_color: bool,
    draw_count: Cell<usize>,
}

impl Default for LineGpuPrim {
    fn default() -> Self {
        Self {
            initialized: false,
            draw_elem: None,
            line_width: 1.0,
            stipple: false,
            no_depth: false,
            use_vert_color: true,
            draw_count: Cell::new(0),
        }
    }
}

impl LineGpuPrim {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn init(&mut self) -> bool {
        self.initialized = true;
        true
    }

    pub fn alloc(&mut self, nlines: usize) {
        self.initialized = true;
        let mut data = LineArray::new();
        data.alloc(nlines);
        data.alloc_ind(6);
        data.assign_inds([0, 1, 2, 2, 1, 3]);
        data.base_mut()
            .draw_elem_mut()
            .set_draw_mode(AbstDrawElem::DRAW_TRIANGLES);
        data.base_mut().set_num_instances(nlines);
        setup_line_attrs(&mut data);
        self.draw_elem = Some(data);
    }

    pub fn is_valid(&self) -> bool {
        self.initialized && self.draw_elem.is_some()
    }

    pub fn size(&self) -> usize {
        self.draw_elem.as_ref().map_or(0, DrawAttrElems::size)
    }

    pub fn data(&self) -> Option<&LineArray> {
        self.draw_elem.as_ref()
    }

    pub fn set_line(
        &mut self,
        index: usize,
        v1: Vector4,
        devcode1: u32,
        v2: Vector4,
        devcode2: u32,
    ) {
        let elem = self
            .draw_elem
            .as_mut()
            .expect("line primitive is not allocated")
            .at_mut(index);

        elem.x1 = v1.x as f32;
        elem.y1 = v1.y as f32;
        elem.z1 = v1.z as f32;
        (elem.r1, elem.g1, elem.b1, elem.a1) = split_devcode(devcode1);

        elem.x2 = v2.x as f32;
        elem.y2 = v2.y as f32;
        elem.z2 = v2.z as f32;
        (elem.r2, elem.g2, elem.b2, elem.a2) = split_devcode(devcode2);
    }

    pub fn line_width(&self) -> f32 {
        self.line_width
    }

    pub fn set_line_width(&mut self, value: f32) {
        self.line_width = value;
    }

    pub fn is_stipple(&self) -> bool {
        self.stipple
    }

    pub fn set_stipple(&mut self, value: bool) {
        self.stipple = value;
    }

    pub fn is_no_depth(&self) -> bool {
        self.no_depth
    }

    pub fn set_no_depth(&mut self, value: bool) {
        self.no_depth = value;
    }

    pub fn is_use_vert_color(&self) -> bool {
        self.use_vert_color
    }

    pub fn set_use_vert_color(&mut self, value: bool) {
        self.use_vert_color = value;
    }

    pub fn set_updated(&mut self, value: bool) {
        if let Some(data) = self.draw_elem.as_mut() {
            data.base_mut().draw_elem_mut().set_updated(value);
        }
    }

    pub fn draw(&self) {
        if self.is_valid() {
            self.draw_count.set(self.draw_count.get() + 1);
        }
    }

    pub fn draw_count(&self) -> usize {
        self.draw_count.get()
    }

    pub fn invalidate(&mut self) {
        self.draw_elem = None;
    }
}

#[derive(Debug, Default)]
pub struct PixGpuPrim {
    initialized: bool,
    draw_elem: Option<PixQuad>,
    last_draw_params: Option<PixDrawParams>,
    draw_count: Cell<usize>,
}

impl PixGpuPrim {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn init(&mut self) -> bool {
        if self.initialized {
            return true;
        }
        let mut data = PixQuad::new();
        data.base_mut().set_attr_size(2);
        data.base_mut().set_attr_info(
            0,
            0,
            2,
            type_consts::QTC_FLOAT32,
            std::mem::offset_of!(PixVertAttr, x) as i32,
        );
        data.base_mut().set_attr_info(
            1,
            1,
            2,
            type_consts::QTC_FLOAT32,
            std::mem::offset_of!(PixVertAttr, tx) as i32,
        );
        data.alloc(4);
        data.base_mut()
            .draw_elem_mut()
            .set_draw_mode(AbstDrawElem::DRAW_TRIANGLE_STRIP);
        data.assign_elems([
            PixVertAttr {
                x: 0.0,
                y: 1.0,
                tx: 0.0,
                ty: 1.0,
            },
            PixVertAttr {
                x: 0.0,
                y: 0.0,
                tx: 0.0,
                ty: 0.0,
            },
            PixVertAttr {
                x: 1.0,
                y: 1.0,
                tx: 1.0,
                ty: 1.0,
            },
            PixVertAttr {
                x: 1.0,
                y: 0.0,
                tx: 1.0,
                ty: 0.0,
            },
        ]);
        self.draw_elem = Some(data);
        self.initialized = true;
        true
    }

    pub fn draw_pixels(
        &mut self,
        context: &DisplayContext,
        position: Vector4,
        pixel_buffer: &PixelBuffer,
        color: Option<&dyn AbstractColor>,
    ) -> bool {
        let viewport = context.viewport();
        self.draw_pixels_with_viewport(
            context,
            position,
            pixel_buffer,
            color,
            [viewport.z as f32, viewport.w as f32],
        )
    }

    pub fn draw_pixels_with_viewport(
        &mut self,
        context: &DisplayContext,
        position: Vector4,
        pixel_buffer: &PixelBuffer,
        color: Option<&dyn AbstractColor>,
        viewport_size: [f32; 2],
    ) -> bool {
        if !self.initialized {
            self.init();
        }
        if pixel_buffer.rep_handle().is_none() || viewport_size[0] <= 0.0 || viewport_size[1] <= 0.0
        {
            return false;
        }
        let color = color.map_or([1.0; 3], |value| {
            [value.fr() as f32, value.fg() as f32, value.fb() as f32]
        });
        self.last_draw_params = Some(PixDrawParams {
            frag_alpha: context.alpha() as f32,
            padding1: [0.0; 3],
            position: [position.x as f32, position.y as f32, position.z as f32],
            padding2: 0.0,
            size: [pixel_buffer.width() as f32, pixel_buffer.height() as f32],
            viewport_size,
            color_bias: color,
            padding3: 0.0,
        });
        self.draw_count.set(self.draw_count.get() + 1);
        true
    }

    pub fn is_valid(&self) -> bool {
        self.initialized && self.draw_elem.is_some()
    }

    pub fn data(&self) -> Option<&PixQuad> {
        self.draw_elem.as_ref()
    }

    pub fn last_draw_params(&self) -> Option<&PixDrawParams> {
        self.last_draw_params.as_ref()
    }

    pub fn draw_count(&self) -> usize {
        self.draw_count.get()
    }

    pub fn invalidate(&mut self) {
        self.initialized = false;
        self.draw_elem = None;
        self.last_draw_params = None;
    }
}

fn set_quad_indices<T: Default + Clone>(data: &mut DrawAttrElems<u32, T>, base: usize, ind: usize) {
    *data.at_ind_mut(ind) = base as u32;
    *data.at_ind_mut(ind + 1) = (base + 1) as u32;
    *data.at_ind_mut(ind + 2) = (base + 2) as u32;
    *data.at_ind_mut(ind + 3) = (base + 2) as u32;
    *data.at_ind_mut(ind + 4) = (base + 1) as u32;
    *data.at_ind_mut(ind + 5) = (base + 3) as u32;
}

fn setup_trig_attrs(data: &mut TrigMesh) {
    if data.base().attr_size() > 0 {
        return;
    }
    data.base_mut().set_attr_size(3);
    data.base_mut().set_attr_info(
        0,
        0,
        3,
        type_consts::QTC_FLOAT32,
        std::mem::offset_of!(TrigVertAttr, x) as i32,
    );
    data.base_mut().set_attr_info(
        1,
        1,
        3,
        type_consts::QTC_FLOAT32,
        std::mem::offset_of!(TrigVertAttr, nx) as i32,
    );
    data.base_mut().set_attr_info(
        2,
        2,
        4,
        type_consts::QTC_UINT8,
        std::mem::offset_of!(TrigVertAttr, r) as i32,
    );
}

fn setup_line_attrs(data: &mut LineArray) {
    if data.base().attr_size() > 0 {
        return;
    }
    data.base_mut().set_attr_size(4);
    data.base_mut().set_attr_info(
        0,
        0,
        3,
        type_consts::QTC_FLOAT32,
        std::mem::offset_of!(LineElem, x1) as i32,
    );
    data.base_mut().set_attr_info(
        1,
        1,
        3,
        type_consts::QTC_FLOAT32,
        std::mem::offset_of!(LineElem, x2) as i32,
    );
    data.base_mut().set_attr_info(
        2,
        2,
        4,
        type_consts::QTC_UINT8,
        std::mem::offset_of!(LineElem, r1) as i32,
    );
    data.base_mut().set_attr_info(
        3,
        3,
        4,
        type_consts::QTC_UINT8,
        std::mem::offset_of!(LineElem, r2) as i32,
    );
    for index in 0..4 {
        data.base_mut().set_attr_divisor(index, 1);
    }
}

fn split_devcode(devcode: u32) -> (u8, u8, u8, u8) {
    (
        ((devcode >> 16) & 0xff) as u8,
        ((devcode >> 8) & 0xff) as u8,
        (devcode & 0xff) as u8,
        ((devcode >> 24) & 0xff) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SolidColor;

    #[test]
    fn pixel_primitive_builds_reference_quad_and_draw_parameters() {
        assert_eq!(std::mem::size_of::<PixDrawParams>(), 64);
        let mut primitive = PixGpuPrim::new();
        assert!(primitive.init());
        let quad = primitive.data().unwrap();
        assert_eq!(quad.size(), 4);
        assert_eq!(
            quad.base().draw_elem().draw_mode(),
            AbstDrawElem::DRAW_TRIANGLE_STRIP
        );
        assert_eq!(
            *quad.at(0),
            PixVertAttr {
                x: 0.0,
                y: 1.0,
                tx: 0.0,
                ty: 1.0,
            }
        );

        let mut context = DisplayContext::new();
        context.set_alpha(0.75);
        context.set_viewport(Vector4::new4(0.0, 0.0, 800.0, 600.0));
        let mut buffer = PixelBuffer::new();
        buffer.set_width(10);
        buffer.set_height(4);
        assert!(!primitive.draw_pixels(&context, Vector4::ZERO, &buffer, None));
        buffer.set_rep_handle(Some(1));
        let color = SolidColor::from_rgb_u8(64, 128, 255);
        assert!(primitive.draw_pixels(
            &context,
            Vector4::new3(1.0, 2.0, 3.0),
            &buffer,
            Some(&color),
        ));
        let params = primitive.last_draw_params().unwrap();
        assert_eq!(params.frag_alpha, 0.75);
        assert_eq!(params.position, [1.0, 2.0, 3.0]);
        assert_eq!(params.size, [10.0, 4.0]);
        assert_eq!(params.viewport_size, [800.0, 600.0]);
        assert_eq!(params.color_bias, [64.0 / 255.0, 128.0 / 255.0, 1.0]);
        assert_eq!(primitive.draw_count(), 1);
    }
}
