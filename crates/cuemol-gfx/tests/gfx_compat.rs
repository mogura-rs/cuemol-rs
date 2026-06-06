use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use cuemol_core::Vector4;
use cuemol_gfx::{
    type_consts, AbstDrawElem, CylinderGpuPrim, DrawAttrArray, DrawAttrElems, LineGpuPrim,
    SolidColor, SphereGpuPrim, TrigGpuPrim,
};

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct TestElem {
    x: f32,
    y: f32,
    z: f32,
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

#[test]
fn solid_color_matches_reference_constructors_and_setters() {
    let c = SolidColor::new();
    assert_eq!((c.r(), c.g(), c.b(), c.a(), c.code()), (0, 0, 0, 0, 0));

    let c = SolidColor::from_code(0xff80_4020);
    assert_eq!((c.r(), c.g(), c.b(), c.a()), (0x80, 0x40, 0x20, 0xff));

    let c = SolidColor::from_rgb_u8(100, 150, 200);
    assert_eq!((c.r(), c.g(), c.b(), c.a()), (100, 150, 200, 255));

    let c = SolidColor::from_rgba(1.0, 0.5, 0.0, 0.0);
    assert_eq!((c.r(), c.g(), c.b(), c.a()), (255, 128, 0, 0));

    let c = SolidColor::from_rgba(-0.5, 0.5, 2.0, 1.0);
    assert_eq!((c.r(), c.g(), c.b(), c.a()), (0, 128, 255, 255));

    let c = SolidColor::from_vector(Vector4::new4(1.0, 0.0, 0.5, 1.0));
    assert_eq!((c.r(), c.g(), c.b(), c.a()), (255, 0, 128, 255));

    let mut c = SolidColor::from_rgba_u8(0, 0, 0, 255);
    c.set_r(10);
    c.set_g(20);
    c.set_b(30);
    c.set_a(128);
    assert_eq!((c.r(), c.g(), c.b(), c.a()), (10, 20, 30, 128));

    c.set_code(0xff11_2233);
    assert_eq!(
        (c.code(), c.r(), c.g(), c.b(), c.a()),
        (0xff11_2233, 0x11, 0x22, 0x33, 0xff)
    );
}

#[test]
fn solid_color_hsb_material_and_script_string() {
    let c = SolidColor::create_hsb(120.0 / 360.0, 1.0, 1.0, 1.0);
    assert!((c.fr() - 0.0).abs() < 2.0 / 255.0);
    assert!((c.fg() - 1.0).abs() < 2.0 / 255.0);
    assert!((c.fb() - 0.0).abs() < 2.0 / 255.0);

    let mut c = SolidColor::from_rgba_u8(255, 0, 0, 255);
    assert_eq!(c.script_string(), "#FF0000");
    c.set_alpha(0.5);
    assert!(c.script_string().starts_with("rgba("));

    let c = SolidColor::create_raw_hsb(0.0, 1.0, 1.0, 1.0);
    assert!(c.script_string().starts_with("hsb("));
    let c = SolidColor::create_raw_hsb(0.0, 1.0, 1.0, 0.5);
    assert!(c.script_string().starts_with("hsba("));

    let mut c = SolidColor::new();
    assert_eq!(c.material(), "");
    c.set_material("glossy");
    assert_eq!(c.material(), "glossy");
    assert!(SolidColor::from_rgb_u8(1, 2, 3).equals(&SolidColor::from_rgb_u8(1, 2, 3)));
}

#[test]
fn draw_attr_array_wire_form_matches_reference_contract() {
    let mut arr = DrawAttrArray::<TestElem>::new();
    arr.alloc(7);
    assert_eq!(arr.size(), 7);
    assert!(!arr.data().is_null());
    assert_eq!(arr.elem_size(), std::mem::size_of::<TestElem>());
    assert_eq!(arr.data_size(), 7 * std::mem::size_of::<TestElem>());

    arr.at_mut(1).x = 1.25;
    arr.at_mut(1).y = 2.5;
    arr.at_mut(1).r = 0x12;
    assert_eq!(arr.at(1).x, 1.25);
    assert_eq!(arr.at(1).y, 2.5);
    assert_eq!(arr.at(1).r, 0x12);

    arr.base_mut().set_attr_size(2);
    arr.base_mut().set_attr_info(
        0,
        3,
        3,
        type_consts::QTC_FLOAT32,
        std::mem::offset_of!(TestElem, x) as i32,
    );
    arr.base_mut().set_attr_info(
        1,
        5,
        4,
        type_consts::QTC_UINT8,
        std::mem::offset_of!(TestElem, r) as i32,
    );
    arr.base_mut().set_attr_divisor(1, 1);

    assert_eq!(arr.base().attr_size(), 2);
    assert_eq!(arr.base().attr_loc(0), 3);
    assert_eq!(arr.base().attr_elem_size(0), 3);
    assert_eq!(arr.base().attr_type_id(0), type_consts::QTC_FLOAT32);
    assert_eq!(
        arr.base().attr_pos(0),
        std::mem::offset_of!(TestElem, x) as i32
    );
    assert_eq!(arr.base().attr_divisor(0), 0);
    assert_eq!(arr.base().attr_loc(1), 5);
    assert_eq!(arr.base().attr_elem_size(1), 4);
    assert_eq!(arr.base().attr_type_id(1), type_consts::QTC_UINT8);
    assert_eq!(
        arr.base().attr_pos(1),
        std::mem::offset_of!(TestElem, r) as i32
    );
    assert_eq!(arr.base().attr_divisor(1), 1);
}

#[test]
fn draw_attr_elems_and_storage_hooks_match_reference_contract() {
    let mut arr = DrawAttrElems::<u32, TestElem>::new();
    arr.alloc(4);
    arr.alloc_ind(6);
    assert_eq!(arr.size(), 4);
    assert_eq!(arr.ind_size(), 6);
    assert!(!arr.ind_data().is_null());
    assert_eq!(arr.ind_elem_size(), std::mem::size_of::<u32>());
    assert_eq!(arr.ind_data_size(), 6 * std::mem::size_of::<u32>());

    *arr.at_ind_mut(0) = 0;
    *arr.at_ind_mut(1) = 2;
    *arr.at_ind_mut(2) = 1;
    assert_eq!((*arr.at_ind(0), *arr.at_ind(1), *arr.at_ind(2)), (0, 2, 1));

    arr.base_mut().set_num_instances(10);
    assert_eq!(arr.base().num_instances(), 10);
    arr.base_mut()
        .draw_elem_mut()
        .set_draw_mode(AbstDrawElem::DRAW_TRIANGLES);
    assert_eq!(
        arr.base().draw_elem().draw_mode(),
        AbstDrawElem::DRAW_TRIANGLES
    );

    let mut elems = [TestElem::default(); 3];
    let elem_ptr = elems.as_mut_ptr();
    let mut inds = [0_u32, 1, 2, 2, 1, 3];
    let ind_ptr = inds.as_mut_ptr();
    unsafe {
        arr.set_data_ref(elem_ptr, elems.len());
        arr.set_ind_data_ref(ind_ptr, inds.len());
    }
    arr.at_mut(1).x = 3.5;
    *arr.at_ind_mut(0) = 9;
    drop(arr);
    assert_eq!(elems[1].x, 3.5);
    assert_eq!(inds[0], 9);

    let called_data = Arc::new(AtomicUsize::new(0));
    let called_ind = Arc::new(AtomicUsize::new(0));
    {
        let mut arr = DrawAttrElems::<u32, TestElem>::new();
        let called_data = Arc::clone(&called_data);
        arr.base_mut().set_data_finalizer(move || {
            called_data.fetch_add(1, Ordering::SeqCst);
        });
        let called_ind = Arc::clone(&called_ind);
        arr.base_mut().set_ind_data_finalizer(move || {
            called_ind.fetch_add(1, Ordering::SeqCst);
        });
    }
    assert_eq!(called_data.load(Ordering::SeqCst), 1);
    assert_eq!(called_ind.load(Ordering::SeqCst), 1);
}

#[test]
fn gpu_primitives_match_reference_state_contracts() {
    let mut sphere = SphereGpuPrim::new();
    assert!(sphere.init());
    assert!(!sphere.is_valid());
    sphere.alloc(2);
    assert!(sphere.is_valid());
    assert_eq!(sphere.size(), 2);
    sphere.set_data(0, Vector4::new3(1.0, 2.0, 3.0), 0.5, 0xff10_2030);
    assert_eq!(sphere.data().unwrap().size(), 8);
    assert_eq!(sphere.data().unwrap().ind_size(), 12);
    sphere.draw();
    assert_eq!(sphere.draw_count(), 1);
    sphere.invalidate();
    sphere.draw();
    assert_eq!(sphere.draw_count(), 1);
    assert!(!sphere.is_valid());

    let mut cylinder = CylinderGpuPrim::new();
    assert!(cylinder.init());
    cylinder.alloc(2);
    cylinder.set_data(
        0,
        Vector4::new3(0.0, 0.0, 0.0),
        Vector4::new3(1.0, 0.0, 0.0),
        0.3,
        0xff00_00ff,
    );
    assert!(cylinder.is_valid());
    assert_eq!(cylinder.size(), 2);
    cylinder.draw();
    assert_eq!(cylinder.draw_count(), 1);
    cylinder.invalidate();
    assert!(!cylinder.is_valid());

    let mut trig = TrigGpuPrim::new();
    assert!(trig.init());
    trig.alloc(3, 1);
    assert_eq!(trig.vertex_size(), 3);
    assert_eq!(trig.face_size(), 1);
    trig.set_vertex(0, Vector4::new3(0.0, 0.0, 0.0));
    trig.set_normal(0, Vector4::new3(0.0, 0.0, 1.0));
    trig.set_color(0, 0xff00_00ff);
    trig.set_face(0, 0, 1, 2);
    assert!(trig.is_valid());
    trig.draw();
    assert_eq!(trig.draw_count(), 1);
    trig.invalidate();
    assert!(!trig.is_valid());

    let mut line = LineGpuPrim::new();
    assert!(line.init());
    line.alloc(3);
    assert_eq!(line.size(), 3);
    line.set_line(
        0,
        Vector4::new3(0.0, 0.0, 0.0),
        0xff00_00ff,
        Vector4::new3(1.0, 0.0, 0.0),
        0xff00_ff00,
    );
    line.set_line_width(2.5);
    line.set_stipple(true);
    line.set_no_depth(true);
    assert_eq!(line.line_width(), 2.5);
    assert!(line.is_stipple());
    assert!(line.is_no_depth());
    line.draw();
    assert_eq!(line.draw_count(), 1);
    line.invalidate();
    assert!(!line.is_valid());
}
