use crate::{CueMolError, Result};
use std::fmt;
use std::ops::{Index, IndexMut};
use std::ptr::NonNull;

enum ArrayStorage<T> {
    Empty,
    Owned(Vec<T>),
    Referred { pointer: NonNull<T>, len: usize },
}

type DestroyCallback<T> = Box<dyn FnMut(&mut Array<T>)>;

/// One-dimensional array corresponding to `qlib::Array`.
///
/// Owned storage is the normal Rust path. The reference implementation also
/// supports a non-owning mutable pointer through `refer`; the equivalent Rust
/// operation is available as [`Array::refer`] and is explicitly unsafe.
pub struct Array<T> {
    storage: ArrayStorage<T>,
    on_destroy: Option<DestroyCallback<T>>,
}

impl<T> Default for Array<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Array<T> {
    pub const fn new() -> Self {
        Self {
            storage: ArrayStorage::Empty,
            on_destroy: None,
        }
    }

    pub fn from_vec(values: Vec<T>) -> Self {
        Self {
            storage: ArrayStorage::Owned(values),
            on_destroy: None,
        }
    }

    pub fn from_slice(values: &[T]) -> Self
    where
        T: Clone,
    {
        Self::from_vec(values.to_vec())
    }

    pub fn with_size(size: usize) -> Self
    where
        T: Default,
    {
        Self::from_vec((0..size).map(|_| T::default()).collect())
    }

    pub fn with_value(value: T, size: usize) -> Self
    where
        T: Clone,
    {
        Self::from_vec(vec![value; size])
    }

    pub fn size(&self) -> usize {
        self.as_slice().len()
    }

    pub fn get_size(&self) -> usize {
        self.size()
    }

    pub fn is_empty(&self) -> bool {
        self.size() == 0
    }

    pub fn is_own(&self) -> bool {
        matches!(self.storage, ArrayStorage::Owned(_))
    }

    pub fn set_on_destroy(&mut self, callback: impl FnMut(&mut Array<T>) + 'static) {
        self.on_destroy = Some(Box::new(callback));
    }

    pub fn clear(&mut self) {
        self.call_on_destroy();
        self.storage = ArrayStorage::Empty;
    }

    pub fn destroy(&mut self) {
        self.clear();
    }

    pub fn resize(&mut self, size: usize)
    where
        T: Default,
    {
        self.clear();
        if size > 0 {
            self.storage = ArrayStorage::Owned((0..size).map(|_| T::default()).collect());
        }
    }

    pub fn allocate(&mut self, size: usize)
    where
        T: Default,
    {
        self.resize(size);
    }

    /// Install mutable storage that is owned by the caller.
    ///
    /// # Safety
    ///
    /// `values` must remain alive, at the same address, and exclusively
    /// accessible through this `Array` until `clear`, `resize`, `destroy`, or
    /// drop. This mirrors the lifetime contract of C++ `Array::refer`.
    pub unsafe fn refer(&mut self, values: &mut [T]) {
        self.clear();
        if values.is_empty() {
            return;
        }
        self.storage = ArrayStorage::Referred {
            pointer: NonNull::new(values.as_mut_ptr()).expect("slice pointer is non-null"),
            len: values.len(),
        };
    }

    pub fn at(&self, index: usize) -> Result<&T> {
        self.as_slice()
            .get(index)
            .ok_or_else(|| CueMolError::invalid_argument("array index out of range"))
    }

    pub fn at_mut(&mut self, index: usize) -> Result<&mut T> {
        self.as_mut_slice()
            .get_mut(index)
            .ok_or_else(|| CueMolError::invalid_argument("array index out of range"))
    }

    pub fn data(&self) -> &[T] {
        self.as_slice()
    }

    pub fn data_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }

    pub fn assign(&mut self, values: impl IntoIterator<Item = T>) -> Result<()> {
        let values = values.into_iter().collect::<Vec<_>>();
        if values.len() > self.size() {
            return Err(CueMolError::invalid_argument(
                "initializer exceeds array size",
            ));
        }
        for (slot, value) in self.as_mut_slice().iter_mut().zip(values) {
            *slot = value;
        }
        Ok(())
    }

    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.as_slice().iter()
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, T> {
        self.as_mut_slice().iter_mut()
    }

    fn as_slice(&self) -> &[T] {
        match &self.storage {
            ArrayStorage::Empty => &[],
            ArrayStorage::Owned(values) => values,
            ArrayStorage::Referred { pointer, len } => {
                // SAFETY: The caller of `refer` promises that the pointer is
                // valid for `len` elements for the lifetime of this storage.
                unsafe { std::slice::from_raw_parts(pointer.as_ptr(), *len) }
            }
        }
    }

    fn as_mut_slice(&mut self) -> &mut [T] {
        match &mut self.storage {
            ArrayStorage::Empty => &mut [],
            ArrayStorage::Owned(values) => values,
            ArrayStorage::Referred { pointer, len } => {
                // SAFETY: `refer` additionally requires exclusive access.
                unsafe { std::slice::from_raw_parts_mut(pointer.as_ptr(), *len) }
            }
        }
    }

    fn call_on_destroy(&mut self) {
        if let Some(mut callback) = self.on_destroy.take() {
            callback(self);
            self.on_destroy = Some(callback);
        }
    }
}

impl<T> Drop for Array<T> {
    fn drop(&mut self) {
        self.call_on_destroy();
    }
}

impl<T: Clone> Clone for Array<T> {
    fn clone(&self) -> Self {
        Self::from_slice(self.as_slice())
    }
}

impl<T: fmt::Debug> fmt::Debug for Array<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Array")
            .field("values", &self.as_slice())
            .field("owned", &self.is_own())
            .finish()
    }
}

impl<T> Index<usize> for Array<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        self.at(index).expect("array index out of range")
    }
}

impl<T> IndexMut<usize> for Array<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.at_mut(index).expect("array index out of range")
    }
}

/// Column-fastest three-dimensional array corresponding to `qlib::Array3D`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Array3D<T> {
    values: Vec<T>,
    columns: usize,
    rows: usize,
    sections: usize,
}

impl<T> Default for Array3D<T> {
    fn default() -> Self {
        Self::empty()
    }
}

impl<T> Array3D<T> {
    pub const fn empty() -> Self {
        Self {
            values: Vec::new(),
            columns: 0,
            rows: 0,
            sections: 0,
        }
    }

    pub fn from_vec(columns: usize, rows: usize, sections: usize, values: Vec<T>) -> Result<Self> {
        let size = checked_volume(columns, rows, sections)?;
        if values.len() != size {
            return Err(CueMolError::invalid_argument(format!(
                "3D array size mismatch: expected {size}, got {}",
                values.len()
            )));
        }
        Ok(Self {
            values,
            columns,
            rows,
            sections,
        })
    }

    pub fn from_slice(columns: usize, rows: usize, sections: usize, values: &[T]) -> Result<Self>
    where
        T: Clone,
    {
        Self::from_vec(columns, rows, sections, values.to_vec())
    }

    pub fn size(&self) -> usize {
        self.values.len()
    }

    pub fn get_size(&self) -> usize {
        self.size()
    }

    pub fn columns(&self) -> usize {
        self.columns
    }

    pub fn get_columns(&self) -> usize {
        self.columns()
    }

    pub fn cols(&self) -> usize {
        self.columns()
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn get_rows(&self) -> usize {
        self.rows()
    }

    pub fn sections(&self) -> usize {
        self.sections
    }

    pub fn get_sections(&self) -> usize {
        self.sections()
    }

    pub fn secs(&self) -> usize {
        self.sections()
    }

    pub fn shape(&self) -> [usize; 3] {
        [self.columns, self.rows, self.sections]
    }

    pub fn clear(&mut self) {
        self.values.clear();
        self.columns = 0;
        self.rows = 0;
        self.sections = 0;
    }

    pub fn at(&self, column: usize, row: usize, section: usize) -> Result<&T> {
        let index = self.offset(column, row, section)?;
        Ok(&self.values[index])
    }

    pub fn at_mut(&mut self, column: usize, row: usize, section: usize) -> Result<&mut T> {
        let index = self.offset(column, row, section)?;
        Ok(&mut self.values[index])
    }

    pub fn at_2d(&self, column: usize, row: usize) -> Result<&T> {
        self.at(column, row, 0)
    }

    pub fn at_2d_mut(&mut self, column: usize, row: usize) -> Result<&mut T> {
        self.at_mut(column, row, 0)
    }

    pub fn data(&self) -> &[T] {
        &self.values
    }

    pub fn data_mut(&mut self) -> &mut [T] {
        &mut self.values
    }

    fn offset(&self, column: usize, row: usize, section: usize) -> Result<usize> {
        if column >= self.columns || row >= self.rows || section >= self.sections {
            return Err(CueMolError::invalid_argument("3D array index out of range"));
        }
        Ok(column + (row + section * self.rows) * self.columns)
    }
}

impl<T: Default> Array3D<T> {
    pub fn new(columns: usize, rows: usize, sections: usize) -> Result<Self> {
        let size = checked_volume(columns, rows, sections)?;
        Ok(Self {
            values: (0..size).map(|_| T::default()).collect(),
            columns,
            rows,
            sections,
        })
    }

    pub fn resize(&mut self, columns: usize, rows: usize, sections: usize) -> Result<()> {
        *self = Self::new(columns, rows, sections)?;
        Ok(())
    }
}

fn checked_volume(columns: usize, rows: usize, sections: usize) -> Result<usize> {
    columns
        .checked_mul(rows)
        .and_then(|value| value.checked_mul(sections))
        .ok_or_else(|| CueMolError::invalid_argument("3D array size overflow"))
}

pub type ByteMap = Array3D<u8>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn array_owned_copy_resize_and_callback_match_reference() {
        let callback_count = Rc::new(Cell::new(0));
        let callback_count_copy = callback_count.clone();
        {
            let mut array = Array::with_value(3_i32, 4);
            array.set_on_destroy(move |_| {
                callback_count_copy.set(callback_count_copy.get() + 1);
            });
            array[2] = 9;
            let copy = array.clone();
            assert_eq!(copy.data(), &[3, 3, 9, 3]);
            assert!(copy.is_own());
            array.resize(2);
            assert_eq!(callback_count.get(), 1);
            assert_eq!(array.data(), &[0, 0]);
            array.clear();
            assert_eq!(callback_count.get(), 2);
        }
        assert_eq!(callback_count.get(), 3);
    }

    #[test]
    fn array_refer_mutates_non_owned_storage_without_freeing_it() {
        let mut source = [1_i32, 2, 3];
        {
            let mut array = Array::new();
            // SAFETY: `source` outlives `array` and is only accessed through it
            // until `array` is dropped.
            unsafe { array.refer(&mut source) };
            assert!(!array.is_own());
            array[1] = 8;
            assert_eq!(array.data(), &[1, 8, 3]);
        }
        assert_eq!(source, [1, 8, 3]);
    }

    #[test]
    fn array3d_uses_reference_column_fastest_layout() {
        let mut array = Array3D::<u8>::new(2, 3, 2).unwrap();
        for section in 0..2 {
            for row in 0..3 {
                for column in 0..2 {
                    *array.at_mut(column, row, section).unwrap() =
                        (column + (row + section * 3) * 2) as u8;
                }
            }
        }
        assert_eq!(array.shape(), [2, 3, 2]);
        assert_eq!(array.data(), &(0_u8..12).collect::<Vec<_>>());
        assert_eq!(*array.at(1, 2, 1).unwrap(), 11);
        assert!(array.at(2, 0, 0).is_err());
        let copy = array.clone();
        array.clear();
        assert_eq!(copy.shape(), [2, 3, 2]);
        assert_eq!(array.shape(), [0, 0, 0]);
    }
}
