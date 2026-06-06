use std::{fmt, mem, ptr::NonNull};

pub use cuemol_core::type_consts;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttrInfo {
    pub attr_loc: i32,
    pub attr_elems: i32,
    pub attr_type: i32,
    pub start_pos: i32,
    pub divisor: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbstDrawElem {
    size: usize,
    draw_mode: i32,
    updated: bool,
}

impl Default for AbstDrawElem {
    fn default() -> Self {
        Self {
            size: 0,
            draw_mode: 0,
            updated: true,
        }
    }
}

impl AbstDrawElem {
    pub const VA_ATTRS: i32 = 9;
    pub const VA_ATTR_INDS: i32 = 10;

    pub const DRAW_POINTS: i32 = 1;
    pub const DRAW_LINE_STRIP: i32 = 2;
    pub const DRAW_LINE_LOOP: i32 = 3;
    pub const DRAW_LINES: i32 = 4;
    pub const DRAW_TRIANGLE_STRIP: i32 = 5;
    pub const DRAW_TRIANGLE_FAN: i32 = 6;
    pub const DRAW_TRIANGLES: i32 = 7;
    pub const DRAW_QUAD_STRIP: i32 = 8;
    pub const DRAW_QUADS: i32 = 9;
    pub const DRAW_POLYGON: i32 = 10;

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn set_size(&mut self, size: usize) {
        self.size = size;
    }

    pub fn draw_mode(&self) -> i32 {
        self.draw_mode
    }

    pub fn set_draw_mode(&mut self, draw_mode: i32) {
        self.draw_mode = draw_mode;
    }

    pub fn is_updated(&self) -> bool {
        self.updated
    }

    pub fn set_updated(&mut self, updated: bool) {
        self.updated = updated;
    }
}

pub struct DrawAttrsBase {
    elem: AbstDrawElem,
    attrs: Vec<AttrInfo>,
    num_instances: usize,
    ext_data_handle: Option<usize>,
    ext_ind_data_handle: Option<usize>,
    data_finalizer: Option<Box<dyn FnOnce() + Send + 'static>>,
    ind_data_finalizer: Option<Box<dyn FnOnce() + Send + 'static>>,
}

impl Default for DrawAttrsBase {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for DrawAttrsBase {
    fn drop(&mut self) {
        if let Some(finalizer) = self.data_finalizer.take() {
            finalizer();
        }
        if let Some(finalizer) = self.ind_data_finalizer.take() {
            finalizer();
        }
    }
}

impl fmt::Debug for DrawAttrsBase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DrawAttrsBase")
            .field("elem", &self.elem)
            .field("attrs", &self.attrs)
            .field("num_instances", &self.num_instances)
            .field("ext_data_handle", &self.ext_data_handle)
            .field("ext_ind_data_handle", &self.ext_ind_data_handle)
            .finish()
    }
}

impl DrawAttrsBase {
    pub fn new() -> Self {
        Self {
            elem: AbstDrawElem::default(),
            attrs: Vec::new(),
            num_instances: 0,
            ext_data_handle: None,
            ext_ind_data_handle: None,
            data_finalizer: None,
            ind_data_finalizer: None,
        }
    }

    pub fn draw_elem(&self) -> &AbstDrawElem {
        &self.elem
    }

    pub fn draw_elem_mut(&mut self) -> &mut AbstDrawElem {
        &mut self.elem
    }

    pub fn set_attr_size(&mut self, size: usize) {
        self.attrs.resize(
            size,
            AttrInfo {
                attr_loc: 0,
                attr_elems: 0,
                attr_type: 0,
                start_pos: 0,
                divisor: 0,
            },
        );
    }

    pub fn attr_size(&self) -> usize {
        self.attrs.len()
    }

    pub fn set_attr_info(
        &mut self,
        index: usize,
        attr_loc: i32,
        attr_elems: i32,
        attr_type: i32,
        start_pos: i32,
    ) {
        let attr = self
            .attrs
            .get_mut(index)
            .expect("attribute index out of bounds");
        *attr = AttrInfo {
            attr_loc,
            attr_elems,
            attr_type,
            start_pos,
            divisor: 0,
        };
    }

    pub fn set_attr_divisor(&mut self, index: usize, divisor: i32) {
        self.attrs
            .get_mut(index)
            .expect("attribute index out of bounds")
            .divisor = divisor;
    }

    pub fn attr_loc(&self, index: usize) -> i32 {
        self.attrs[index].attr_loc
    }

    pub fn attr_elem_size(&self, index: usize) -> i32 {
        self.attrs[index].attr_elems
    }

    pub fn attr_type_id(&self, index: usize) -> i32 {
        self.attrs[index].attr_type
    }

    pub fn attr_pos(&self, index: usize) -> i32 {
        self.attrs[index].start_pos
    }

    pub fn attr_divisor(&self, index: usize) -> i32 {
        self.attrs[index].divisor
    }

    pub fn set_num_instances(&mut self, value: usize) {
        self.num_instances = value;
    }

    pub fn num_instances(&self) -> usize {
        self.num_instances
    }

    pub fn ext_data_handle(&self) -> Option<usize> {
        self.ext_data_handle
    }

    pub fn set_ext_data_handle(&mut self, value: Option<usize>) {
        self.ext_data_handle = value;
    }

    pub fn ext_ind_data_handle(&self) -> Option<usize> {
        self.ext_ind_data_handle
    }

    pub fn set_ext_ind_data_handle(&mut self, value: Option<usize>) {
        self.ext_ind_data_handle = value;
    }

    pub fn set_data_finalizer(&mut self, finalizer: impl FnOnce() + Send + 'static) {
        self.data_finalizer = Some(Box::new(finalizer));
    }

    pub fn set_ind_data_finalizer(&mut self, finalizer: impl FnOnce() + Send + 'static) {
        self.ind_data_finalizer = Some(Box::new(finalizer));
    }
}

#[derive(Debug)]
enum BufferStorage<T> {
    Owned(Vec<T>),
    External { ptr: NonNull<T>, len: usize },
}

impl<T> Default for BufferStorage<T> {
    fn default() -> Self {
        Self::Owned(Vec::new())
    }
}

impl<T> BufferStorage<T> {
    fn len(&self) -> usize {
        match self {
            Self::Owned(values) => values.len(),
            Self::External { len, .. } => *len,
        }
    }

    fn as_ptr(&self) -> *const T {
        match self {
            Self::Owned(values) => values.as_ptr(),
            Self::External { ptr, .. } => ptr.as_ptr(),
        }
    }

    fn at(&self, index: usize) -> &T {
        assert!(index < self.len(), "draw attribute index out of bounds");
        match self {
            Self::Owned(values) => &values[index],
            Self::External { ptr, .. } => unsafe { &*ptr.as_ptr().add(index) },
        }
    }

    fn at_mut(&mut self, index: usize) -> &mut T {
        assert!(index < self.len(), "draw attribute index out of bounds");
        match self {
            Self::Owned(values) => &mut values[index],
            Self::External { ptr, .. } => unsafe { &mut *ptr.as_ptr().add(index) },
        }
    }
}

#[derive(Debug)]
pub struct DrawAttrArray<T> {
    base: DrawAttrsBase,
    data: BufferStorage<T>,
}

impl<T: Default + Clone> Default for DrawAttrArray<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Default + Clone> DrawAttrArray<T> {
    pub fn new() -> Self {
        Self {
            base: DrawAttrsBase::new(),
            data: BufferStorage::default(),
        }
    }

    pub fn typ(&self) -> i32 {
        AbstDrawElem::VA_ATTRS
    }

    pub fn alloc(&mut self, size: usize) {
        self.alloc_owned_data(size);
    }

    pub fn alloc_owned_data(&mut self, size: usize) {
        self.data = BufferStorage::Owned(vec![T::default(); size]);
        self.base.draw_elem_mut().set_size(size);
    }

    /// # Safety
    ///
    /// `ptr` must be valid for `size` mutable elements and outlive this array
    /// or until another allocation method replaces the external storage.
    pub unsafe fn set_data_ref(&mut self, ptr: *mut T, size: usize) {
        let ptr = NonNull::new(ptr).expect("external data pointer is null");
        self.data = BufferStorage::External { ptr, len: size };
        self.base.draw_elem_mut().set_size(size);
    }

    pub fn size(&self) -> usize {
        self.base.draw_elem().size()
    }

    pub fn elem_size(&self) -> usize {
        mem::size_of::<T>()
    }

    pub fn data(&self) -> *const T {
        self.data.as_ptr()
    }

    pub fn data_size(&self) -> usize {
        self.elem_size() * self.size()
    }

    pub fn at(&self, index: usize) -> &T {
        self.data.at(index)
    }

    pub fn at_mut(&mut self, index: usize) -> &mut T {
        self.data.at_mut(index)
    }

    pub fn assign_elems(&mut self, values: impl IntoIterator<Item = T>) {
        let values = values.into_iter().collect::<Vec<_>>();
        self.data = BufferStorage::Owned(values);
        self.base.draw_elem_mut().set_size(self.data.len());
    }

    pub fn base(&self) -> &DrawAttrsBase {
        &self.base
    }

    pub fn base_mut(&mut self) -> &mut DrawAttrsBase {
        &mut self.base
    }
}

#[derive(Debug)]
pub struct DrawAttrElems<I, T> {
    base: DrawAttrsBase,
    data: BufferStorage<T>,
    indices: BufferStorage<I>,
}

impl<I: Default + Clone, T: Default + Clone> Default for DrawAttrElems<I, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<I: Default + Clone, T: Default + Clone> DrawAttrElems<I, T> {
    pub fn new() -> Self {
        Self {
            base: DrawAttrsBase::new(),
            data: BufferStorage::default(),
            indices: BufferStorage::default(),
        }
    }

    pub fn typ(&self) -> i32 {
        AbstDrawElem::VA_ATTR_INDS
    }

    pub fn alloc(&mut self, size: usize) {
        self.alloc_owned_data(size);
    }

    pub fn alloc_owned_data(&mut self, size: usize) {
        self.data = BufferStorage::Owned(vec![T::default(); size]);
        self.base.draw_elem_mut().set_size(size);
    }

    pub fn alloc_ind(&mut self, size: usize) {
        self.alloc_owned_ind_data(size);
    }

    pub fn alloc_owned_ind_data(&mut self, size: usize) {
        self.indices = BufferStorage::Owned(vec![I::default(); size]);
    }

    /// # Safety
    ///
    /// `ptr` must be valid for `size` mutable elements and outlive this array
    /// or until another allocation method replaces the external storage.
    pub unsafe fn set_data_ref(&mut self, ptr: *mut T, size: usize) {
        let ptr = NonNull::new(ptr).expect("external data pointer is null");
        self.data = BufferStorage::External { ptr, len: size };
        self.base.draw_elem_mut().set_size(size);
    }

    /// # Safety
    ///
    /// `ptr` must be valid for `size` mutable index elements and outlive this
    /// array or until another index allocation replaces the external storage.
    pub unsafe fn set_ind_data_ref(&mut self, ptr: *mut I, size: usize) {
        let ptr = NonNull::new(ptr).expect("external index pointer is null");
        self.indices = BufferStorage::External { ptr, len: size };
    }

    pub fn size(&self) -> usize {
        self.base.draw_elem().size()
    }

    pub fn elem_size(&self) -> usize {
        mem::size_of::<T>()
    }

    pub fn data(&self) -> *const T {
        self.data.as_ptr()
    }

    pub fn data_size(&self) -> usize {
        self.elem_size() * self.size()
    }

    pub fn at(&self, index: usize) -> &T {
        self.data.at(index)
    }

    pub fn at_mut(&mut self, index: usize) -> &mut T {
        self.data.at_mut(index)
    }

    pub fn ind_data(&self) -> *const I {
        self.indices.as_ptr()
    }

    pub fn ind_elem_size(&self) -> usize {
        mem::size_of::<I>()
    }

    pub fn ind_size(&self) -> usize {
        self.indices.len()
    }

    pub fn ind_data_size(&self) -> usize {
        self.ind_elem_size() * self.ind_size()
    }

    pub fn at_ind(&self, index: usize) -> &I {
        self.indices.at(index)
    }

    pub fn at_ind_mut(&mut self, index: usize) -> &mut I {
        self.indices.at_mut(index)
    }

    pub fn assign_inds(&mut self, values: impl IntoIterator<Item = I>) {
        self.indices = BufferStorage::Owned(values.into_iter().collect());
    }

    pub fn base(&self) -> &DrawAttrsBase {
        &self.base
    }

    pub fn base_mut(&mut self) -> &mut DrawAttrsBase {
        &mut self.base
    }
}
