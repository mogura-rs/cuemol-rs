use crate::{CueMolError, Result, Vector4};
use std::ops::{Add, Div, Mul, Neg, Sub};

/// Two-dimensional real vector corresponding to `qlib::Vector2D`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Vector2 {
    pub x: f64,
    pub y: f64,
}

impl Vector2 {
    pub const ZERO: Self = Self::new(0.0, 0.0);

    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn set(&mut self, x: f64, y: f64) {
        self.x = x;
        self.y = y;
    }

    pub fn ai(self, index: usize) -> Result<f64> {
        match index {
            1 => Ok(self.x),
            2 => Ok(self.y),
            _ => Err(CueMolError::invalid_argument("vector index out of range")),
        }
    }

    pub fn set_ai(&mut self, index: usize, value: f64) -> Result<()> {
        match index {
            1 => self.x = value,
            2 => self.y = value,
            _ => return Err(CueMolError::invalid_argument("vector index out of range")),
        }
        Ok(())
    }

    pub fn dot(self, rhs: Self) -> f64 {
        self.x * rhs.x + self.y * rhs.y
    }

    pub fn cross(self, rhs: Self) -> f64 {
        self.x * rhs.y - self.y * rhs.x
    }

    pub fn sqlen(self) -> f64 {
        self.dot(self)
    }

    pub fn length(self) -> f64 {
        self.sqlen().sqrt()
    }

    pub fn scale(self, value: f64) -> Self {
        Self::new(self.x * value, self.y * value)
    }

    pub fn divide(self, value: f64) -> Result<Self> {
        if value == 0.0 {
            return Err(CueMolError::invalid_argument("division by zero"));
        }
        Ok(self.scale(1.0 / value))
    }

    pub fn normalize(self) -> Result<Self> {
        self.divide(self.length())
    }

    pub fn is_zero(self) -> bool {
        self.x.abs() < 1.0e-8 && self.y.abs() < 1.0e-8
    }

    pub fn equals(self, rhs: Self) -> bool {
        (self.x - rhs.x).abs() < 1.0e-8 && (self.y - rhs.y).abs() < 1.0e-8
    }
}

impl Add for Vector2 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl Sub for Vector2 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl Mul<f64> for Vector2 {
    type Output = Self;

    fn mul(self, rhs: f64) -> Self::Output {
        self.scale(rhs)
    }
}

impl Div<f64> for Vector2 {
    type Output = Self;

    fn div(self, rhs: f64) -> Self::Output {
        self.divide(rhs).expect("division by zero")
    }
}

impl Neg for Vector2 {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self::new(-self.x, -self.y)
    }
}

/// Three-dimensional integer vector corresponding to `qlib::IntVec3D`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct IntVec3 {
    pub col: i32,
    pub row: i32,
    pub sec: i32,
}

impl IntVec3 {
    pub const ZERO: Self = Self::new(0, 0, 0);

    pub const fn new(col: i32, row: i32, sec: i32) -> Self {
        Self { col, row, sec }
    }

    pub fn ai(self, index: usize) -> Result<i32> {
        match index {
            1 => Ok(self.col),
            2 => Ok(self.row),
            3 => Ok(self.sec),
            _ => Err(CueMolError::invalid_argument("vector index out of range")),
        }
    }

    pub fn set_ai(&mut self, index: usize, value: i32) -> Result<()> {
        match index {
            1 => self.col = value,
            2 => self.row = value,
            3 => self.sec = value,
            _ => return Err(CueMolError::invalid_argument("vector index out of range")),
        }
        Ok(())
    }

    pub fn vec4(self) -> Vector4 {
        Vector4::new3(self.col as f64, self.row as f64, self.sec as f64)
    }

    pub fn dot(self, rhs: Self) -> i32 {
        self.col * rhs.col + self.row * rhs.row + self.sec * rhs.sec
    }

    pub fn scale(self, value: i32) -> Self {
        Self::new(self.col * value, self.row * value, self.sec * value)
    }

    pub fn is_zero(self) -> bool {
        self == Self::ZERO
    }
}

impl Add for IntVec3 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.col + rhs.col, self.row + rhs.row, self.sec + rhs.sec)
    }
}

impl Sub for IntVec3 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.col - rhs.col, self.row - rhs.row, self.sec - rhs.sec)
    }
}

impl Mul<i32> for IntVec3 {
    type Output = Self;

    fn mul(self, rhs: i32) -> Self::Output {
        self.scale(rhs)
    }
}

impl Neg for IntVec3 {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self::new(-self.col, -self.row, -self.sec)
    }
}

/// Axis-aligned, half-open three-dimensional box corresponding to `qlib::Box3D`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Box3 {
    pub start: Vector4,
    pub end: Vector4,
}

impl Default for Box3 {
    fn default() -> Self {
        Self::EMPTY
    }
}

impl Box3 {
    pub const EMPTY: Self = Self {
        start: Vector4::new3(1.0, 1.0, 1.0),
        end: Vector4::new3(-1.0, -1.0, -1.0),
    };

    pub const fn new(start: Vector4, end: Vector4) -> Self {
        Self { start, end }
    }

    pub fn merge(&mut self, position: Vector4) {
        if self.is_empty() {
            self.start = position;
            self.end = position;
            return;
        }
        self.start.x = self.start.x.min(position.x);
        self.start.y = self.start.y.min(position.y);
        self.start.z = self.start.z.min(position.z);
        self.end.x = self.end.x.max(position.x);
        self.end.y = self.end.y.max(position.y);
        self.end.z = self.end.z.max(position.z);
    }

    pub fn merge_box(&mut self, other: Self) {
        if other.is_empty() {
            return;
        }
        self.merge(other.start);
        self.merge(other.end);
    }

    pub fn inflate(&mut self, amount: f64) {
        if self.is_empty() {
            return;
        }
        self.start.x -= amount;
        self.start.y -= amount;
        self.start.z -= amount;
        self.end.x += amount;
        self.end.y += amount;
        self.end.z += amount;
    }

    pub fn contains(self, position: Vector4) -> bool {
        self.start.x <= position.x
            && position.x < self.end.x
            && self.start.y <= position.y
            && position.y < self.end.y
            && self.start.z <= position.z
            && position.z < self.end.z
    }

    pub fn is_empty(self) -> bool {
        self.start.x > self.end.x || self.start.y > self.end.y || self.start.z > self.end.z
    }

    pub fn set_empty(&mut self) {
        *self = Self::EMPTY;
    }

    pub fn center(self) -> Vector4 {
        (self.start + self.end).scale(0.5)
    }

    pub fn size_x(self) -> f64 {
        self.end.x - self.start.x
    }

    pub fn size_y(self) -> f64 {
        self.end.y - self.start.y
    }

    pub fn size_z(self) -> f64 {
        self.end.z - self.start.z
    }

    pub fn sizex(self) -> f64 {
        self.size_x()
    }

    pub fn sizey(self) -> f64 {
        self.size_y()
    }

    pub fn sizez(self) -> f64 {
        self.size_z()
    }

    pub fn size(self) -> Vector4 {
        Vector4::new3(self.size_x(), self.size_y(), self.size_z())
    }
}

/// Three-dimensional real matrix corresponding to `qlib::Matrix3D`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Matrix3 {
    pub values: [[f64; 3]; 3],
}

impl Default for Matrix3 {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Matrix3 {
    pub const IDENTITY: Self = Self {
        values: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
    };
    pub const ZERO: Self = Self {
        values: [[0.0; 3]; 3],
    };

    pub const fn new(values: [[f64; 3]; 3]) -> Self {
        Self { values }
    }

    pub fn get_at(self, row: usize, col: usize) -> Result<f64> {
        if !(1..=3).contains(&row) || !(1..=3).contains(&col) {
            return Err(CueMolError::invalid_argument("matrix index out of range"));
        }
        Ok(self.values[row - 1][col - 1])
    }

    pub fn set_at(&mut self, row: usize, col: usize, value: f64) -> Result<()> {
        if !(1..=3).contains(&row) || !(1..=3).contains(&col) {
            return Err(CueMolError::invalid_argument("matrix index out of range"));
        }
        self.values[row - 1][col - 1] = value;
        Ok(())
    }

    pub fn mul_vec(self, value: Vector4) -> Vector4 {
        Vector4::new3(
            self.values[0][0] * value.x + self.values[0][1] * value.y + self.values[0][2] * value.z,
            self.values[1][0] * value.x + self.values[1][1] * value.y + self.values[1][2] * value.z,
            self.values[2][0] * value.x + self.values[2][1] * value.y + self.values[2][2] * value.z,
        )
    }

    pub fn mulvec(self, value: Vector4) -> Vector4 {
        self.mul_vec(value)
    }

    pub fn xform(self, value: &mut Vector4) {
        *value = self.mul_vec(*value);
    }

    pub fn mul_mat(self, rhs: Self) -> Self {
        let mut result = Self::ZERO;
        for row in 0..3 {
            for col in 0..3 {
                result.values[row][col] = (0..3)
                    .map(|index| self.values[row][index] * rhs.values[index][col])
                    .sum();
            }
        }
        result
    }

    pub fn determinant(self) -> f64 {
        let m = self.values;
        (m[0][0] * m[1][1] - m[0][1] * m[1][0]) * m[2][2]
            + (m[1][0] * m[2][1] - m[1][1] * m[2][0]) * m[0][2]
            + (m[2][0] * m[0][1] - m[2][1] * m[0][0]) * m[1][2]
    }

    pub fn deter(self) -> f64 {
        self.determinant()
    }

    pub fn invert(self) -> Result<Self> {
        let determinant = self.determinant();
        if determinant.abs() < 1.0e-15 {
            return Err(CueMolError::invalid_argument("singular matrix"));
        }
        let m = self.values;
        Ok(Self::new([
            [
                (m[1][1] * m[2][2] - m[1][2] * m[2][1]) / determinant,
                (m[0][2] * m[2][1] - m[0][1] * m[2][2]) / determinant,
                (m[0][1] * m[1][2] - m[0][2] * m[1][1]) / determinant,
            ],
            [
                (m[1][2] * m[2][0] - m[1][0] * m[2][2]) / determinant,
                (m[0][0] * m[2][2] - m[0][2] * m[2][0]) / determinant,
                (m[0][2] * m[1][0] - m[0][0] * m[1][2]) / determinant,
            ],
            [
                (m[1][0] * m[2][1] - m[1][1] * m[2][0]) / determinant,
                (m[0][1] * m[2][0] - m[0][0] * m[2][1]) / determinant,
                (m[0][0] * m[1][1] - m[0][1] * m[1][0]) / determinant,
            ],
        ]))
    }

    /// Construct the basis matrix whose columns are `e2`, `e1 × e2`, and `e1`.
    pub fn make_basis_rot_mat(e1: Vector4, e2: Vector4) -> Self {
        let e3 = e1.cross(e2);
        Self::new([[e2.x, e3.x, e1.x], [e2.y, e3.y, e1.y], [e2.z, e3.z, e1.z]])
    }

    /// Construct CueMol's clockwise axis-angle rotation matrix.
    pub fn make_rot_mat(axis: Vector4, theta: f64) -> Self {
        Self::make_rot_mat_cs(axis, theta.cos(), theta.sin())
    }

    pub fn make_rot_mat_cs(axis: Vector4, cos_theta: f64, sin_theta: f64) -> Self {
        let t = 1.0 - cos_theta;
        Self::new([
            [
                t * axis.x * axis.x + cos_theta,
                t * axis.y * axis.x + sin_theta * axis.z,
                t * axis.z * axis.x - sin_theta * axis.y,
            ],
            [
                t * axis.x * axis.y - sin_theta * axis.z,
                t * axis.y * axis.y + cos_theta,
                t * axis.z * axis.y + sin_theta * axis.x,
            ],
            [
                t * axis.x * axis.z + sin_theta * axis.y,
                t * axis.y * axis.z - sin_theta * axis.x,
                t * axis.z * axis.z + cos_theta,
            ],
        ])
    }

    pub fn diagonalize(self) -> (Self, Vector4) {
        let mut vectors = Self::IDENTITY;
        let mut values = Vector4::ZERO;
        self.diag(&mut vectors, &mut values);
        (vectors, values)
    }

    pub fn diag(self, eigenvectors: &mut Self, eigenvalues: &mut Vector4) -> bool {
        let mut matrix = self.values;
        if matrix.iter().flatten().any(|value| !value.is_finite()) {
            return false;
        }
        let mut vectors = Self::IDENTITY;
        for _ in 0..50 {
            let off_diagonal = matrix[0][1].abs()
                + matrix[0][2].abs()
                + matrix[1][0].abs()
                + matrix[1][2].abs()
                + matrix[2][0].abs()
                + matrix[2][1].abs();
            if off_diagonal <= 0.0 {
                *eigenvectors = vectors;
                *eigenvalues = Vector4::new3(matrix[0][0], matrix[1][1], matrix[2][2]);
                return true;
            }
            // CueMol uses cyclic Jacobi rotations in this fixed order.
            for (p, q, r) in [(0, 1, 2), (0, 2, 1), (1, 2, 0)] {
                let pq = matrix[p][q];
                let t = if pq.abs() < 1.0e-8 {
                    0.0
                } else {
                    let theta = (matrix[q][q] - matrix[p][p]) / (2.0 * pq);
                    if theta >= 0.0 {
                        1.0 / (theta + (theta * theta + 1.0).sqrt())
                    } else {
                        -1.0 / (-theta + (theta * theta + 1.0).sqrt())
                    }
                };
                let c = 1.0 / (t * t + 1.0).sqrt();
                let s = c * t;
                let tau = s / (1.0 + c);
                let rp = matrix[r][p];
                let rq = matrix[r][q];
                matrix[p][p] -= t * pq;
                matrix[q][q] += t * pq;
                matrix[p][q] = 0.0;
                matrix[q][p] = 0.0;
                matrix[r][p] = rp - s * (rq + tau * rp);
                matrix[p][r] = matrix[r][p];
                matrix[r][q] = rq + s * (rp - tau * rq);
                matrix[q][r] = matrix[r][q];
                let mut rotation = Self::IDENTITY;
                rotation.values[p][p] = c;
                rotation.values[p][q] = s;
                rotation.values[q][p] = -s;
                rotation.values[q][q] = c;
                vectors = vectors * rotation;
            }
        }
        false
    }
}

impl Mul for Matrix3 {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        self.mul_mat(rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vector2_and_intvec3_match_reference_operations() {
        let a = Vector2::new(2.0, 3.0);
        let b = Vector2::new(-1.0, 4.0);
        assert_eq!(a + b, Vector2::new(1.0, 7.0));
        assert_eq!(a.cross(b), 11.0);
        assert_eq!(a.dot(b), 10.0);
        assert!(Vector2::ZERO.normalize().is_err());

        let i = IntVec3::new(1, 2, 3);
        assert_eq!(i + IntVec3::new(3, 2, 1), IntVec3::new(4, 4, 4));
        assert_eq!(i.vec4(), Vector4::new3(1.0, 2.0, 3.0));
        assert_eq!(i.ai(3).unwrap(), 3);
    }

    #[test]
    fn box3_matches_reference_half_open_bounds() {
        let mut bounds = Box3::default();
        assert!(bounds.is_empty());
        bounds.merge(Vector4::new3(1.0, 2.0, 3.0));
        bounds.merge(Vector4::new3(5.0, 6.0, 7.0));
        assert_eq!(bounds.start, Vector4::new3(1.0, 2.0, 3.0));
        assert_eq!(bounds.end, Vector4::new3(5.0, 6.0, 7.0));
        assert!(bounds.contains(Vector4::new3(1.0, 2.0, 3.0)));
        assert!(!bounds.contains(Vector4::new3(5.0, 6.0, 7.0)));
        assert_eq!(bounds.center(), Vector4::new3(3.0, 4.0, 5.0));
        assert_eq!(bounds.size(), Vector4::new3(4.0, 4.0, 4.0));
        bounds.inflate(1.0);
        assert_eq!(bounds.start, Vector4::new3(0.0, 1.0, 2.0));
        assert_eq!(bounds.end, Vector4::new3(6.0, 7.0, 8.0));
    }

    #[test]
    fn matrix3_inversion_rotation_and_diagonalization_match_reference() {
        let matrix = Matrix3::new([[3.0, 0.0, 2.0], [2.0, 0.0, -2.0], [0.0, 1.0, 1.0]]);
        let product = matrix * matrix.invert().unwrap();
        for row in 0..3 {
            for col in 0..3 {
                let expected = if row == col { 1.0 } else { 0.0 };
                assert!((product.values[row][col] - expected).abs() < 1.0e-12);
            }
        }

        let rotation =
            Matrix3::make_rot_mat(Vector4::new3(0.0, 0.0, 1.0), std::f64::consts::FRAC_PI_2);
        let rotated = rotation.mul_vec(Vector4::new3(1.0, 0.0, 0.0));
        assert!(rotated.x.abs() < 1.0e-12);
        assert!((rotated.y + 1.0).abs() < 1.0e-12);

        let symmetric = Matrix3::new([[3.0, 1.0, 0.0], [1.0, 3.0, 0.0], [0.0, 0.0, 1.0]]);
        let (vectors, values) = symmetric.diagonalize();
        for col in 0..3 {
            let eigenvector = Vector4::new3(
                vectors.values[0][col],
                vectors.values[1][col],
                vectors.values[2][col],
            );
            let lhs = symmetric.mul_vec(eigenvector);
            let rhs = eigenvector.scale([values.x, values.y, values.z][col]);
            assert!((lhs - rhs).length3d() < 1.0e-8);
        }
    }
}
