use crate::{CueMolError, Result};
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

pub trait NdScalar:
    Copy
    + PartialEq
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Div<Output = Self>
    + AddAssign
    + SubAssign
    + MulAssign
    + DivAssign
    + Neg<Output = Self>
{
    fn zero() -> Self;
    fn one() -> Self;
    fn abs_diff(self, other: Self) -> f64;
    fn to_f64(self) -> f64;
}

macro_rules! impl_scalar {
    ($($type:ty),+ $(,)?) => {$(
        impl NdScalar for $type {
            fn zero() -> Self { 0 as $type }
            fn one() -> Self { 1 as $type }
            fn abs_diff(self, other: Self) -> f64 { ((self as f64) - (other as f64)).abs() }
            fn to_f64(self) -> f64 { self as f64 }
        }
    )+};
}
impl_scalar!(f32, f64, i8, i16, i32, i64, isize);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VectorN<const N: usize, T: NdScalar = f64> {
    values: [T; N],
}

impl<const N: usize, T: NdScalar> Default for VectorN<N, T> {
    fn default() -> Self {
        Self::zero()
    }
}

impl<const N: usize, T: NdScalar> VectorN<N, T> {
    pub const DIMENSION: usize = N;
    pub fn new(values: [T; N]) -> Self {
        Self { values }
    }
    pub fn zero() -> Self {
        Self {
            values: [T::zero(); N],
        }
    }
    pub fn ai(&self, index: usize) -> T {
        assert!((1..=N).contains(&index));
        self.values[index - 1]
    }
    pub fn ai_mut(&mut self, index: usize) -> &mut T {
        assert!((1..=N).contains(&index));
        &mut self.values[index - 1]
    }
    pub fn as_slice(&self) -> &[T] {
        &self.values
    }
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.values
    }
    pub fn set(&mut self, values: [T; N]) {
        self.values = values;
    }
    pub fn equals(&self, other: &Self, tolerance: f64) -> bool {
        self.values
            .iter()
            .zip(other.values)
            .all(|(left, right)| left.abs_diff(right) < tolerance)
    }
    pub fn is_zero(&self, tolerance: f64) -> bool {
        self.values
            .iter()
            .all(|value| value.abs_diff(T::zero()) < tolerance)
    }
    pub fn sqlen(&self) -> T {
        self.dot(self)
    }
    pub fn length(&self) -> f64 {
        self.values
            .iter()
            .map(|value| {
                let value = value.to_f64();
                value * value
            })
            .sum::<f64>()
            .sqrt()
    }
    pub fn scale(&self, scalar: T) -> Self {
        Self::new(self.values.map(|value| value * scalar))
    }
    pub fn scale_vector(&self, other: &Self) -> Self {
        Self::new(std::array::from_fn(|i| self.values[i] * other.values[i]))
    }
    pub fn divide(&self, scalar: T) -> Self {
        Self::new(self.values.map(|value| value / scalar))
    }
    pub fn divide_checked(&self, scalar: T) -> Result<Self> {
        if scalar.abs_diff(T::zero()) < 1.0e-8 {
            Err(CueMolError::invalid_argument("Vector: zero division error"))
        } else {
            Ok(self.divide(scalar))
        }
    }
    pub fn add(&self, other: &Self) -> Self {
        Self::new(std::array::from_fn(|i| self.values[i] + other.values[i]))
    }
    pub fn add_scalar(&self, scalar: T) -> Self {
        Self::new(self.values.map(|value| value + scalar))
    }
    pub fn sub(&self, other: &Self) -> Self {
        Self::new(std::array::from_fn(|i| self.values[i] - other.values[i]))
    }
    pub fn dot(&self, other: &Self) -> T {
        self.values
            .iter()
            .zip(other.values)
            .fold(T::zero(), |sum, (left, right)| sum + *left * right)
    }
}

impl<const N: usize> VectorN<N, f64> {
    pub fn normalize(&self) -> Self {
        self.divide(self.length())
    }
    pub fn normalize_checked(&self) -> Result<Self> {
        self.divide_checked(self.length())
    }
}

impl<const N: usize, T: NdScalar> Add for VectorN<N, T> {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        VectorN::add(&self, &rhs)
    }
}
impl<const N: usize, T: NdScalar> Sub for VectorN<N, T> {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        VectorN::sub(&self, &rhs)
    }
}
impl<const N: usize, T: NdScalar> Neg for VectorN<N, T> {
    type Output = Self;
    fn neg(self) -> Self {
        Self::new(self.values.map(Neg::neg))
    }
}
impl<const N: usize, T: NdScalar> AddAssign for VectorN<N, T> {
    fn add_assign(&mut self, rhs: Self) {
        for i in 0..N {
            self.values[i] += rhs.values[i];
        }
    }
}
impl<const N: usize, T: NdScalar> SubAssign for VectorN<N, T> {
    fn sub_assign(&mut self, rhs: Self) {
        for i in 0..N {
            self.values[i] -= rhs.values[i];
        }
    }
}
impl<const N: usize, T: NdScalar> MulAssign<T> for VectorN<N, T> {
    fn mul_assign(&mut self, rhs: T) {
        for value in &mut self.values {
            *value *= rhs;
        }
    }
}
impl<const N: usize, T: NdScalar> DivAssign<T> for VectorN<N, T> {
    fn div_assign(&mut self, rhs: T) {
        for value in &mut self.values {
            *value /= rhs;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MatrixN<const N: usize, T: NdScalar = f64> {
    columns: [[T; N]; N],
}

impl<const N: usize, T: NdScalar> Default for MatrixN<N, T> {
    fn default() -> Self {
        Self::identity()
    }
}
impl<const N: usize, T: NdScalar> MatrixN<N, T> {
    pub const DIMENSION: usize = N;
    pub fn identity() -> Self {
        Self {
            columns: std::array::from_fn(|column| {
                std::array::from_fn(|row| if row == column { T::one() } else { T::zero() })
            }),
        }
    }
    pub fn zero() -> Self {
        Self {
            columns: [[T::zero(); N]; N],
        }
    }
    pub fn from_columns(columns: [[T; N]; N]) -> Self {
        Self { columns }
    }
    pub fn ai(&self, index: usize) -> T {
        assert!((1..=N * N).contains(&index));
        let flat = index - 1;
        self.columns[flat / N][flat % N]
    }
    pub fn aij(&self, row: usize, column: usize) -> T {
        assert!((1..=N).contains(&row) && (1..=N).contains(&column));
        self.columns[column - 1][row - 1]
    }
    pub fn aij_mut(&mut self, row: usize, column: usize) -> &mut T {
        assert!((1..=N).contains(&row) && (1..=N).contains(&column));
        &mut self.columns[column - 1][row - 1]
    }
    pub fn scale(&self, scalar: T) -> Self {
        Self::from_columns(
            self.columns
                .map(|column| column.map(|value| value * scalar)),
        )
    }
    pub fn divide(&self, scalar: T) -> Self {
        Self::from_columns(
            self.columns
                .map(|column| column.map(|value| value / scalar)),
        )
    }
    pub fn add(&self, other: &Self) -> Self {
        Self::from_columns(std::array::from_fn(|c| {
            std::array::from_fn(|r| self.columns[c][r] + other.columns[c][r])
        }))
    }
    pub fn sub(&self, other: &Self) -> Self {
        Self::from_columns(std::array::from_fn(|c| {
            std::array::from_fn(|r| self.columns[c][r] - other.columns[c][r])
        }))
    }
    pub fn mul(&self, other: &Self) -> Self {
        let mut result = Self::zero();
        for row in 1..=N {
            for column in 1..=N {
                let mut sum = T::zero();
                for k in 1..=N {
                    sum += self.aij(row, k) * other.aij(k, column);
                }
                *result.aij_mut(row, column) = sum;
            }
        }
        result
    }
    pub fn matprod(&mut self, other: &Self) {
        *self = MatrixN::mul(self, other);
    }
    pub fn transpose(&self) -> Self {
        Self::from_columns(std::array::from_fn(|column| {
            std::array::from_fn(|row| self.aij(column + 1, row + 1))
        }))
    }
    pub fn equals(&self, other: &Self, tolerance: f64) -> bool {
        (0..N).all(|c| (0..N).all(|r| self.columns[c][r].abs_diff(other.columns[c][r]) < tolerance))
    }
    pub fn is_zero(&self, tolerance: f64) -> bool {
        self.equals(&Self::zero(), tolerance)
    }
    pub fn is_identity(&self, tolerance: f64) -> bool {
        self.equals(&Self::identity(), tolerance)
    }
    pub fn resized<const M: usize>(&self) -> MatrixN<M, T> {
        let mut result = MatrixN::identity();
        let count = N.min(M);
        for row in 1..=count {
            for column in 1..=count {
                *result.aij_mut(row, column) = self.aij(row, column);
            }
        }
        result
    }
}
impl<const N: usize, T: NdScalar> Add for MatrixN<N, T> {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        MatrixN::add(&self, &rhs)
    }
}
impl<const N: usize, T: NdScalar> Sub for MatrixN<N, T> {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        MatrixN::sub(&self, &rhs)
    }
}
impl<const N: usize, T: NdScalar> Mul for MatrixN<N, T> {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        MatrixN::mul(&self, &rhs)
    }
}
impl<const N: usize, T: NdScalar> Neg for MatrixN<N, T> {
    type Output = Self;
    fn neg(self) -> Self {
        self.scale(-T::one())
    }
}
impl<const N: usize, T: NdScalar> AddAssign for MatrixN<N, T> {
    fn add_assign(&mut self, rhs: Self) {
        *self = MatrixN::add(self, &rhs);
    }
}
impl<const N: usize, T: NdScalar> SubAssign for MatrixN<N, T> {
    fn sub_assign(&mut self, rhs: Self) {
        *self = MatrixN::sub(self, &rhs);
    }
}
impl<const N: usize, T: NdScalar> MulAssign<T> for MatrixN<N, T> {
    fn mul_assign(&mut self, rhs: T) {
        *self = self.scale(rhs);
    }
}
impl<const N: usize, T: NdScalar> DivAssign<T> for MatrixN<N, T> {
    fn div_assign(&mut self, rhs: T) {
        *self = self.divide(rhs);
    }
}

pub type VectorND<const N: usize, T = f64> = VectorN<N, T>;
pub type MatrixND<const N: usize, T = f64> = MatrixN<N, T>;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vector_nd_matches_reference_operations() {
        let vector = VectorN::new([3.0, 4.0, 0.0]);
        assert_eq!(vector.ai(1), 3.0);
        assert_eq!(vector.length(), 5.0);
        assert_eq!(vector.dot(&VectorN::new([1.0, 2.0, 3.0])), 11.0);
        assert!(vector
            .normalize_checked()
            .unwrap()
            .equals(&VectorN::new([0.6, 0.8, 0.0]), 1.0e-12));
        assert!(VectorN::<3>::zero().normalize_checked().is_err());
        assert_eq!(
            VectorN::new([1_i32, 2, 3])
                .scale_vector(&VectorN::new([4, 5, 6]))
                .as_slice(),
            &[4, 10, 18]
        );
    }
    #[test]
    fn matrix_nd_uses_one_based_column_major_access_and_products() {
        let mut matrix = MatrixN::<3>::identity();
        *matrix.aij_mut(1, 2) = 2.0;
        *matrix.aij_mut(2, 3) = 3.0;
        assert_eq!(matrix.ai(4), 2.0);
        assert_eq!(matrix.transpose().aij(2, 1), 2.0);
        assert!(MatrixN::mul(&matrix, &MatrixN::identity()).equals(&matrix, 1.0e-12));
        let resized: MatrixN<4> = matrix.resized();
        assert_eq!(resized.aij(1, 2), 2.0);
        assert_eq!(resized.aij(4, 4), 1.0);
    }
    #[test]
    #[should_panic]
    fn one_based_access_rejects_zero() {
        let _ = VectorN::<2>::zero().ai(0);
    }
}
