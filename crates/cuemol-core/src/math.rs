use crate::{geometry::Matrix3, CueMolError, Result};
use std::fmt;
use std::ops::{Add, Div, Mul, Neg, Sub};

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Vector4 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub w: f64,
}

impl Vector4 {
    pub const ZERO: Self = Self::new4(0.0, 0.0, 0.0, 0.0);

    #[inline]
    pub const fn new3(x: f64, y: f64, z: f64) -> Self {
        Self::new4(x, y, z, 0.0)
    }

    #[inline]
    pub const fn new4(x: f64, y: f64, z: f64, w: f64) -> Self {
        Self { x, y, z, w }
    }

    pub fn set3(&mut self, x: f64, y: f64, z: f64) {
        self.x = x;
        self.y = y;
        self.z = z;
        self.w = 0.0;
    }

    pub fn set4(&mut self, x: f64, y: f64, z: f64, w: f64) {
        self.x = x;
        self.y = y;
        self.z = z;
        self.w = w;
    }

    pub fn set_3(&mut self, x: f64, y: f64, z: f64) {
        self.set3(x, y, z);
    }

    pub fn set_4(&mut self, x: f64, y: f64, z: f64, w: f64) {
        self.set4(x, y, z, w);
    }

    pub fn x(&self) -> f64 {
        self.x
    }

    pub fn get_x(&self) -> f64 {
        self.x()
    }

    pub fn set_x(&mut self, value: f64) {
        self.x = value;
    }

    pub fn y(&self) -> f64 {
        self.y
    }

    pub fn get_y(&self) -> f64 {
        self.y()
    }

    pub fn set_y(&mut self, value: f64) {
        self.y = value;
    }

    pub fn z(&self) -> f64 {
        self.z
    }

    pub fn get_z(&self) -> f64 {
        self.z()
    }

    pub fn set_z(&mut self, value: f64) {
        self.z = value;
    }

    pub fn w(&self) -> f64 {
        self.w
    }

    pub fn get_w(&self) -> f64 {
        self.w()
    }

    pub fn set_w(&mut self, value: f64) {
        self.w = value;
    }

    #[inline]
    pub fn sqlen(self) -> f64 {
        self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w
    }

    #[inline]
    pub fn sqlen3d(self) -> f64 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }

    #[inline]
    pub fn length(self) -> f64 {
        self.sqlen().sqrt()
    }

    #[inline]
    pub fn length3d(self) -> f64 {
        self.sqlen3d().sqrt()
    }

    pub fn is_zero(self) -> bool {
        self.x == 0.0 && self.y == 0.0 && self.z == 0.0 && self.w == 0.0
    }

    pub fn is_zero3d(self) -> bool {
        self.is_zero3d_tol(1.0e-8)
    }

    pub fn is_zero3d_tol(self, tolerance: f64) -> bool {
        self.x.abs() < tolerance && self.y.abs() < tolerance && self.z.abs() < tolerance
    }

    #[inline]
    pub fn scale(self, value: f64) -> Self {
        Self::new4(
            self.x * value,
            self.y * value,
            self.z * value,
            self.w * value,
        )
    }

    #[inline]
    pub fn divide(self, value: f64) -> Result<Self> {
        if value == 0.0 {
            return Err(CueMolError::invalid_argument("division by zero"));
        }
        Ok(Self::new4(
            self.x / value,
            self.y / value,
            self.z / value,
            self.w / value,
        ))
    }

    pub fn normalize(self) -> Result<Self> {
        self.divide(self.length())
    }

    pub fn normalize3d(self) -> Result<Self> {
        self.divide(self.length3d())
    }

    #[inline]
    pub fn dot(self, rhs: Self) -> f64 {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z + self.w * rhs.w
    }

    #[inline]
    pub fn dot3d(self, rhs: Self) -> f64 {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z
    }

    #[inline]
    pub fn cross(self, rhs: Self) -> Self {
        Self::new3(
            self.y * rhs.z - self.z * rhs.y,
            self.z * rhs.x - self.x * rhs.z,
            self.x * rhs.y - self.y * rhs.x,
        )
    }

    pub fn angle(self, rhs: Self) -> Result<f64> {
        let denom = self.length() * rhs.length();
        if denom == 0.0 {
            return Err(CueMolError::invalid_argument("angle with zero vector"));
        }
        Ok((self.dot(rhs) / denom).clamp(-1.0, 1.0).acos())
    }

    pub fn angle_static(lhs: Self, rhs: Self) -> Result<f64> {
        lhs.angle(rhs)
    }

    pub fn torsion(i: Self, j: Self, k: Self, l: Self) -> Result<f64> {
        let vij = j.sub_vec(i);
        let vjk = k.sub_vec(j);
        let vkl = l.sub_vec(k);

        if vij.sqlen3d().abs() < 1.0e-8 {
            return Err(CueMolError::invalid_argument(
                "torsion: arg1 and arg2 is too close",
            ));
        }
        if vjk.sqlen3d().abs() < 1.0e-8 {
            return Err(CueMolError::invalid_argument(
                "torsion: arg2 and arg3 is too close",
            ));
        }
        if vkl.sqlen3d().abs() < 1.0e-8 {
            return Err(CueMolError::invalid_argument(
                "torsion: arg3 and arg4 is too close",
            ));
        }

        let a = vij.cross(vjk);
        let b = vjk.cross(vkl);
        let c = vjk.cross(a);
        if a.sqlen3d().abs() < 1.0e-8 {
            return Err(CueMolError::invalid_argument(
                "torsion: arg1-arg2-arg3 is too near to linear",
            ));
        }
        if b.sqlen3d().abs() < 1.0e-8 {
            return Err(CueMolError::invalid_argument(
                "torsion: arg2-arg3-arg4 is too near to linear",
            ));
        }
        if c.sqlen3d().abs() < 1.0e-8 {
            return Err(CueMolError::invalid_argument(
                "torsion: middle bond is too close to singular",
            ));
        }

        let a = a.normalize3d()?;
        let b = b.normalize3d()?;
        let c = c.normalize3d()?;
        let cos_phi = a.dot3d(b).clamp(-1.0, 1.0);
        let sin_phi = c.dot3d(b);
        let phi = cos_phi.acos();
        Ok(if sin_phi < 0.0 { -phi } else { phi })
    }

    pub fn dihedral(i: Self, j: Self, k: Self, l: Self) -> Result<f64> {
        Self::torsion(i, j, k, l)
    }

    #[inline]
    pub fn add_vec(self, rhs: Self) -> Self {
        Self::new4(
            self.x + rhs.x,
            self.y + rhs.y,
            self.z + rhs.z,
            self.w + rhs.w,
        )
    }

    #[inline]
    pub fn sub_vec(self, rhs: Self) -> Self {
        Self::new4(
            self.x - rhs.x,
            self.y - rhs.y,
            self.z - rhs.z,
            self.w - rhs.w,
        )
    }

    #[allow(clippy::should_implement_trait)]
    pub fn add(self, rhs: Self) -> Self {
        self.add_vec(rhs)
    }

    #[allow(clippy::should_implement_trait)]
    pub fn sub(self, rhs: Self) -> Self {
        self.sub_vec(rhs)
    }

    pub fn negate(self) -> Self {
        -self
    }

    pub fn zero(&mut self) {
        *self = Self::ZERO;
    }

    pub fn copy_from(&mut self, value: Self) {
        *self = value;
    }

    pub fn equals(self, rhs: Self) -> bool {
        self == rhs
    }

    pub fn strvalue(self) -> String {
        self.script_string()
    }

    #[allow(
        clippy::inherent_to_string,
        clippy::inherent_to_string_shadow_display,
        clippy::wrong_self_convention
    )]
    pub fn to_string(self) -> String {
        self.script_string()
    }

    pub fn set_strvalue(&mut self, value: &str) -> Result<()> {
        *self = Self::from_script_string(value)?;
        Ok(())
    }

    pub fn set_str_value(&mut self, value: &str) -> Result<()> {
        self.set_strvalue(value)
    }

    pub fn from_string(input: &str) -> Result<Self> {
        Self::from_script_string(input)
    }

    pub fn script_string(self) -> String {
        if self.w == 0.0 {
            format!(
                "({},{},{})",
                format_real(self.x),
                format_real(self.y),
                format_real(self.z)
            )
        } else {
            format!(
                "({},{},{},{})",
                format_real(self.x),
                format_real(self.y),
                format_real(self.z),
                format_real(self.w)
            )
        }
    }

    pub fn from_script_string(input: &str) -> Result<Self> {
        let trimmed = input.trim();
        let body = trimmed
            .strip_prefix('(')
            .and_then(|value| value.strip_suffix(')'))
            .ok_or_else(|| CueMolError::parse("invalid vector string"))?;
        let parts = body.split(',').map(str::trim).collect::<Vec<_>>();
        match parts.as_slice() {
            [x, y, z] => Ok(Self::new3(
                parse_vector_component(x)?,
                parse_vector_component(y)?,
                parse_vector_component(z)?,
            )),
            [x, y, z, w] => Ok(Self::new4(
                parse_vector_component(x)?,
                parse_vector_component(y)?,
                parse_vector_component(z)?,
                parse_vector_component(w)?,
            )),
            _ => Err(CueMolError::parse("invalid vector component count")),
        }
    }
}

impl fmt::Display for Vector4 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.script_string())
    }
}

impl Add for Vector4 {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        self.add_vec(rhs)
    }
}

impl Sub for Vector4 {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        self.sub_vec(rhs)
    }
}

impl Mul<f64> for Vector4 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: f64) -> Self::Output {
        self.scale(rhs)
    }
}

impl Div<f64> for Vector4 {
    type Output = Self;

    #[inline]
    fn div(self, rhs: f64) -> Self::Output {
        self.divide(rhs).expect("division by zero")
    }
}

impl Neg for Vector4 {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self::Output {
        Self::new4(-self.x, -self.y, -self.z, -self.w)
    }
}

fn parse_vector_component(value: &str) -> Result<f64> {
    value
        .parse()
        .map_err(|_| CueMolError::parse("invalid vector component"))
}

fn format_real(value: f64) -> String {
    if (value - value.round()).abs() < 1.0e-12 {
        return format!("{:.0}", value);
    }
    let text = format!("{value:.12}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Matrix4 {
    pub values: [[f64; 4]; 4],
}

impl Default for Matrix4 {
    fn default() -> Self {
        Self::new()
    }
}

impl Matrix4 {
    pub const TOLERANCE: f64 = 1.0e-8;

    pub const IDENTITY: Self = Self {
        values: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };

    pub const ZERO: Self = Self {
        values: [[0.0; 4]; 4],
    };

    pub fn new() -> Self {
        Self::IDENTITY
    }

    pub fn transform_point(self, value: Vector4) -> Vector4 {
        let v = [value.x, value.y, value.z, value.w];
        let row = |i: usize| -> f64 {
            self.values[i][0] * v[0]
                + self.values[i][1] * v[1]
                + self.values[i][2] * v[2]
                + self.values[i][3] * v[3]
        };
        Vector4::new4(row(0), row(1), row(2), row(3))
    }

    pub fn get_at(self, row: usize, col: usize) -> Result<f64> {
        if !(1..=4).contains(&row) || !(1..=4).contains(&col) {
            return Err(CueMolError::invalid_argument("matrix index out of range"));
        }
        Ok(self.values[row - 1][col - 1])
    }

    pub fn set_at(&mut self, row: usize, col: usize, value: f64) -> Result<()> {
        if !(1..=4).contains(&row) || !(1..=4).contains(&col) {
            return Err(CueMolError::invalid_argument("matrix index out of range"));
        }
        self.values[row - 1][col - 1] = value;
        Ok(())
    }

    pub fn add_at(&mut self, row: usize, col: usize, value: f64) -> Result<f64> {
        let current = self.get_at(row, col)?;
        let updated = current + value;
        self.set_at(row, col, updated)?;
        Ok(updated)
    }

    pub fn ai(self, index: usize) -> Result<f64> {
        if !(1..=16).contains(&index) {
            return Err(CueMolError::invalid_argument("matrix index out of range"));
        }
        let offset = index - 1;
        Ok(self.values[offset % 4][offset / 4])
    }

    pub fn set_ai(&mut self, index: usize, value: f64) -> Result<()> {
        if !(1..=16).contains(&index) {
            return Err(CueMolError::invalid_argument("matrix index out of range"));
        }
        let offset = index - 1;
        self.values[offset % 4][offset / 4] = value;
        Ok(())
    }

    pub fn mul_mat(self, rhs: Self) -> Self {
        let mut values = [[0.0; 4]; 4];
        for (i, row) in values.iter_mut().enumerate() {
            for (j, value) in row.iter_mut().enumerate() {
                *value = (0..4).map(|k| self.values[i][k] * rhs.values[k][j]).sum();
            }
        }
        Self { values }
    }

    pub fn mul_vec(self, value: Vector4) -> Vector4 {
        self.transform_point(value)
    }

    pub fn scale(self, value: f64) -> Self {
        let mut result = self;
        for row in &mut result.values {
            for item in row {
                *item *= value;
            }
        }
        result
    }

    pub fn divide(self, value: f64) -> Result<Self> {
        if value == 0.0 {
            return Err(CueMolError::invalid_argument("division by zero"));
        }
        Ok(self.scale(1.0 / value))
    }

    #[allow(clippy::should_implement_trait)]
    pub fn add(self, rhs: Self) -> Self {
        let mut result = self;
        for row in 0..4 {
            for col in 0..4 {
                result.values[row][col] += rhs.values[row][col];
            }
        }
        result
    }

    #[allow(clippy::should_implement_trait)]
    pub fn sub(self, rhs: Self) -> Self {
        let mut result = self;
        for row in 0..4 {
            for col in 0..4 {
                result.values[row][col] -= rhs.values[row][col];
            }
        }
        result
    }

    #[allow(clippy::should_implement_trait)]
    pub fn mul(self, rhs: Self) -> Self {
        self.mul_mat(rhs)
    }

    pub fn mulvec(self, value: Vector4) -> Vector4 {
        self.mul_vec(value)
    }

    pub fn xform3d(self, value: &mut Vector4) {
        let result = self.transform_point(Vector4::new4(value.x, value.y, value.z, 1.0));
        value.set3(result.x, result.y, result.z);
    }

    pub fn xform4d(self, value: &mut Vector4) {
        *value = self.transform_point(*value);
    }

    pub fn make_trans_mat(value: Vector4) -> Self {
        let mut matrix = Self::IDENTITY;
        matrix.values[0][3] = value.x;
        matrix.values[1][3] = value.y;
        matrix.values[2][3] = value.z;
        matrix
    }

    pub fn make_scale_mat(value: Vector4) -> Self {
        let mut matrix = Self::IDENTITY;
        matrix.values[0][0] = value.x;
        matrix.values[1][1] = value.y;
        matrix.values[2][2] = value.z;
        matrix
    }

    pub fn make_rot_mat(quat: Quat) -> Self {
        quat.to_rot_matrix()
    }

    pub fn make_rot_mat_axis(axis: Vector4, angle_rad: f64) -> Result<Self> {
        Ok(Quat::from_axis_angle(axis, angle_rad)?.to_rot_matrix())
    }

    /// Construct the basis matrix whose columns are `e2`, `e1 × e2`, and `e1`.
    pub fn make_basis_rot_mat(e1: Vector4, e2: Vector4) -> Self {
        let e3 = e1.cross(e2);
        let mut matrix = Self::IDENTITY;
        matrix.values[0][0] = e2.x;
        matrix.values[1][0] = e2.y;
        matrix.values[2][0] = e2.z;
        matrix.values[0][1] = e3.x;
        matrix.values[1][1] = e3.y;
        matrix.values[2][1] = e3.z;
        matrix.values[0][2] = e1.x;
        matrix.values[1][2] = e1.y;
        matrix.values[2][2] = e1.z;
        matrix
    }

    /// Construct a CueMol basis rotation with a translation column.
    pub fn make_rot_tran_mat(e1: Vector4, e2: Vector4, translation: Vector4) -> Self {
        let mut matrix = Self::make_basis_rot_mat(e1, e2);
        matrix.set_trans_part(translation);
        matrix
    }

    pub fn set_unit(&mut self) {
        self.set_ident();
    }

    pub fn set_ident(&mut self) {
        *self = Self::IDENTITY;
    }

    pub fn set_zero(&mut self) {
        *self = Self::ZERO;
    }

    pub fn is_zero(self) -> bool {
        self.values
            .iter()
            .flatten()
            .all(|value| value.abs() < Self::TOLERANCE)
    }

    pub fn set_translate(&mut self, shift: Vector4) {
        *self = Self::make_trans_mat(shift);
    }

    pub fn set_rotate(&mut self, center: Vector4, axis: Vector4, degree: f64) -> Result<()> {
        let rot = Self::make_rot_mat_axis(axis, degree.to_radians())?;
        let to_origin = Self::make_trans_mat(center.negate());
        let from_origin = Self::make_trans_mat(center);
        *self = from_origin.mul_mat(rot.mul_mat(to_origin));
        Ok(())
    }

    pub fn invert(self) -> Result<Self> {
        let mut aug = [[0.0; 8]; 4];
        for (row_index, row) in aug.iter_mut().enumerate() {
            row[..4].copy_from_slice(&self.values[row_index]);
            row[4 + row_index] = 1.0;
        }

        for col in 0..4 {
            let pivot = (col..4)
                .max_by(|&a, &b| {
                    aug[a][col]
                        .abs()
                        .partial_cmp(&aug[b][col].abs())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .unwrap();
            if aug[pivot][col].abs() < 1.0e-15 {
                return Err(CueMolError::invalid_argument("singular matrix"));
            }
            if pivot != col {
                aug.swap(pivot, col);
            }
            let denom = aug[col][col];
            for value in &mut aug[col] {
                *value /= denom;
            }
            for row in 0..4 {
                if row == col {
                    continue;
                }
                let factor = aug[row][col];
                let pivot_row = aug[col];
                for (value, pivot_value) in aug[row].iter_mut().zip(pivot_row) {
                    *value -= factor * pivot_value;
                }
            }
        }

        let mut values = [[0.0; 4]; 4];
        for row in 0..4 {
            values[row].copy_from_slice(&aug[row][4..8]);
        }
        Ok(Self { values })
    }

    pub fn get_trans_part(self) -> Vector4 {
        Vector4::new3(self.values[0][3], self.values[1][3], self.values[2][3])
    }

    pub fn get_matrix3d(self) -> Matrix3 {
        Matrix3::new([
            [self.values[0][0], self.values[0][1], self.values[0][2]],
            [self.values[1][0], self.values[1][1], self.values[1][2]],
            [self.values[2][0], self.values[2][1], self.values[2][2]],
        ])
    }

    pub fn set_trans_part(&mut self, value: Vector4) {
        self.values[0][3] = value.x;
        self.values[1][3] = value.y;
        self.values[2][3] = value.z;
    }

    pub fn is_ident_affine(self) -> bool {
        for row in 0..4 {
            for col in 0..4 {
                let expected = if row == col { 1.0 } else { 0.0 };
                if (self.values[row][col] - expected).abs() > 1.0e-12 {
                    return false;
                }
            }
        }
        true
    }

    pub fn is_ident(self) -> bool {
        self.equals(Self::IDENTITY)
    }

    pub fn equals(self, rhs: Self) -> bool {
        for index in 1..=16 {
            if (self.ai(index).unwrap() - rhs.ai(index).unwrap()).abs() >= Self::TOLERANCE {
                return false;
            }
        }
        true
    }

    pub fn to_script_string(self) -> String {
        if self.is_ident() {
            return "(identity)".to_string();
        }
        let values = (1..=16)
            .map(|index| format!("{:.7}", self.ai(index).unwrap()))
            .collect::<Vec<_>>()
            .join(",");
        format!("({values})")
    }

    pub fn from_script_string(src: &str) -> Result<Self> {
        if src.eq_ignore_ascii_case("(identity)") {
            return Ok(Self::IDENTITY);
        }
        let trimmed = src.trim();
        if !trimmed.starts_with('(') || !trimmed.ends_with(')') || trimmed.len() < 2 {
            return Err(CueMolError::invalid_argument(
                "cannot convert string to matrix",
            ));
        }
        let body = &trimmed[1..trimmed.len() - 1];
        let tokens = body.split(',').collect::<Vec<_>>();
        if tokens.len() != 16 {
            return Err(CueMolError::invalid_argument(
                "cannot convert string to matrix",
            ));
        }
        let mut matrix = Self::IDENTITY;
        for (offset, token) in tokens.iter().enumerate() {
            let value = token
                .trim()
                .parse::<f64>()
                .map_err(|_| CueMolError::invalid_argument("cannot convert string to matrix"))?;
            matrix.set_ai(offset + 1, value)?;
        }
        Ok(matrix)
    }

    pub fn from_string(src: &str) -> Result<Self> {
        Self::from_script_string(src)
    }

    pub fn translate(&mut self, value: Vector4) {
        *self = self.mul_mat(Self::make_trans_mat(value));
    }

    pub fn rotate(&mut self, quat: Quat) {
        *self = self.mul_mat(Self::make_rot_mat(quat));
    }

    pub fn set_row(&mut self, row: usize, value: Vector4) -> Result<()> {
        if !(1..=4).contains(&row) {
            return Err(CueMolError::invalid_argument(
                "matrix row index out of range",
            ));
        }
        self.values[row - 1] = [value.x, value.y, value.z, value.w];
        Ok(())
    }

    pub fn row(self, row: usize) -> Result<Vector4> {
        if !(1..=4).contains(&row) {
            return Err(CueMolError::invalid_argument(
                "matrix row index out of range",
            ));
        }
        let value = self.values[row - 1];
        Ok(Vector4::new4(value[0], value[1], value[2], value[3]))
    }

    pub fn set_col(&mut self, col: usize, value: Vector4) -> Result<()> {
        if !(1..=4).contains(&col) {
            return Err(CueMolError::invalid_argument(
                "matrix column index out of range",
            ));
        }
        for (row, item) in [value.x, value.y, value.z, value.w].into_iter().enumerate() {
            self.values[row][col - 1] = item;
        }
        Ok(())
    }

    pub fn col(self, col: usize) -> Result<Vector4> {
        if !(1..=4).contains(&col) {
            return Err(CueMolError::invalid_argument(
                "matrix column index out of range",
            ));
        }
        Ok(Vector4::new4(
            self.values[0][col - 1],
            self.values[1][col - 1],
            self.values[2][col - 1],
            self.values[3][col - 1],
        ))
    }

    pub fn diag3(self) -> Self {
        let (mut vectors, mut values) = jacobi_eigen3([
            [self.values[0][0], self.values[0][1], self.values[0][2]],
            [self.values[1][0], self.values[1][1], self.values[1][2]],
            [self.values[2][0], self.values[2][1], self.values[2][2]],
        ]);
        for i in 0..2 {
            for j in i + 1..3 {
                if values[i] > values[j] {
                    values.swap(i, j);
                    for row in &mut vectors {
                        row.swap(i, j);
                    }
                }
            }
        }

        let mut result = Self::IDENTITY;
        for (row, vector_row) in vectors.iter().enumerate() {
            result.values[row][..3].copy_from_slice(vector_row);
        }
        result.values[3][0] = values[0];
        result.values[3][1] = values[1];
        result.values[3][2] = values[2];
        result
    }

    pub fn diag_3(self) -> Self {
        self.diag3()
    }
}

impl fmt::Display for Matrix4 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_script_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quat {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub w: f64,
}

impl Quat {
    pub const IDENTITY: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 1.0,
    };

    pub const fn new(x: f64, y: f64, z: f64, w: f64) -> Self {
        Self { x, y, z, w }
    }

    pub const fn from_scalar_vector(w: f64, x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z, w }
    }

    pub fn from_axis_half_angle(axis: Vector4, half_angle_rad: f64) -> Result<Self> {
        let axis = axis.normalize()?;
        Ok(Self::new(
            axis.x * half_angle_rad.sin(),
            axis.y * half_angle_rad.sin(),
            axis.z * half_angle_rad.sin(),
            half_angle_rad.cos(),
        ))
    }

    pub fn from_axis_angle(axis: Vector4, angle_rad: f64) -> Result<Self> {
        Self::from_axis_half_angle(axis, angle_rad * 0.5)
    }

    pub fn norm(self) -> f64 {
        self.sqlen().sqrt()
    }

    pub fn sqlen(self) -> f64 {
        self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w
    }

    pub fn normalize(self) -> Result<Self> {
        let norm = self.norm();
        if norm == 0.0 {
            return Err(CueMolError::invalid_argument("normalize zero quaternion"));
        }
        Ok(Self::new(
            self.x / norm,
            self.y / norm,
            self.z / norm,
            self.w / norm,
        ))
    }

    pub fn scale(self, value: f64) -> Self {
        Self::new(
            self.x * value,
            self.y * value,
            self.z * value,
            self.w * value,
        )
    }

    pub fn divide(self, value: f64) -> Result<Self> {
        if value == 0.0 {
            return Err(CueMolError::invalid_argument("division by zero"));
        }
        Ok(self.scale(1.0 / value))
    }

    pub fn conj(self) -> Self {
        Self::new(-self.x, -self.y, -self.z, self.w)
    }

    pub fn conjugate(self) -> Self {
        self.conj()
    }

    pub fn inv(self) -> Result<Self> {
        self.conj().divide(self.sqlen())
    }

    pub fn mul_quat(self, rhs: Self) -> Self {
        Self::new(
            self.w * rhs.x + self.x * rhs.w + self.y * rhs.z - self.z * rhs.y,
            self.w * rhs.y - self.x * rhs.z + self.y * rhs.w + self.z * rhs.x,
            self.w * rhs.z + self.x * rhs.y - self.y * rhs.x + self.z * rhs.w,
            self.w * rhs.w - self.x * rhs.x - self.y * rhs.y - self.z * rhs.z,
        )
    }

    #[allow(clippy::should_implement_trait)]
    pub fn mul(self, rhs: Self) -> Self {
        self.mul_quat(rhs)
    }

    pub fn rotate_x(self, degrees: f64) -> Result<Self> {
        Ok(self.mul_quat(Self::from_axis_angle(
            Vector4::new3(1.0, 0.0, 0.0),
            degrees.to_radians(),
        )?))
    }

    pub fn rotate_y(self, degrees: f64) -> Result<Self> {
        Ok(self.mul_quat(Self::from_axis_angle(
            Vector4::new3(0.0, 1.0, 0.0),
            degrees.to_radians(),
        )?))
    }

    pub fn rotate_z(self, degrees: f64) -> Result<Self> {
        Ok(self.mul_quat(Self::from_axis_angle(
            Vector4::new3(0.0, 0.0, 1.0),
            degrees.to_radians(),
        )?))
    }

    pub fn to_rot_matrix(self) -> Matrix4 {
        let q = self.normalize().unwrap_or(Self::IDENTITY);
        let xx = q.x * q.x;
        let yy = q.y * q.y;
        let zz = q.z * q.z;
        let xy = q.x * q.y;
        let xz = q.x * q.z;
        let yz = q.y * q.z;
        let wx = q.w * q.x;
        let wy = q.w * q.y;
        let wz = q.w * q.z;

        Matrix4 {
            values: [
                [1.0 - 2.0 * (yy + zz), 2.0 * (xy + wz), 2.0 * (xz - wy), 0.0],
                [2.0 * (xy - wz), 1.0 - 2.0 * (xx + zz), 2.0 * (yz + wx), 0.0],
                [2.0 * (xz + wy), 2.0 * (yz - wx), 1.0 - 2.0 * (xx + yy), 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    pub fn to_matrix(self) -> Matrix4 {
        self.to_rot_matrix()
    }

    /// Convert CueMol's clockwise 3×3 rotation matrix to a quaternion.
    pub fn from_rot_matrix(matrix: Matrix3) -> Result<Self> {
        let m = matrix.values;
        let trace = m[0][0] + m[1][1] + m[2][2] + 1.0;
        let quaternion = if trace >= 1.0 {
            let s = 0.5 / trace.sqrt();
            Self::from_scalar_vector(
                0.25 / s,
                (m[1][2] - m[2][1]) * s,
                (m[2][0] - m[0][2]) * s,
                (m[0][1] - m[1][0]) * s,
            )
        } else if m[0][0] > m[1][1].max(m[2][2]) {
            let root = (m[0][0] - m[1][1] - m[2][2] + 1.0).sqrt();
            if root <= f64::EPSILON {
                return Err(CueMolError::invalid_argument("invalid rotation matrix"));
            }
            let s = 0.5 / root;
            Self::from_scalar_vector(
                (m[1][2] - m[2][1]) * s,
                root * 0.5,
                (m[0][1] + m[1][0]) * s,
                (m[2][0] + m[0][2]) * s,
            )
        } else if m[1][1] > m[2][2] {
            let root = (m[1][1] - m[2][2] - m[0][0] + 1.0).sqrt();
            if root <= f64::EPSILON {
                return Err(CueMolError::invalid_argument("invalid rotation matrix"));
            }
            let s = 0.5 / root;
            Self::from_scalar_vector(
                (m[2][0] - m[0][2]) * s,
                (m[0][1] + m[1][0]) * s,
                root * 0.5,
                (m[1][2] + m[2][1]) * s,
            )
        } else {
            let root = (m[2][2] - m[0][0] - m[1][1] + 1.0).sqrt();
            if root <= f64::EPSILON {
                return Err(CueMolError::invalid_argument("invalid rotation matrix"));
            }
            let s = 0.5 / root;
            Self::from_scalar_vector(
                (m[0][1] - m[1][0]) * s,
                (m[2][0] + m[0][2]) * s,
                (m[1][2] + m[2][1]) * s,
                root * 0.5,
            )
        };
        quaternion.normalize()
    }

    pub fn make_from_rot_mat(matrix: Matrix3) -> Result<Self> {
        Self::from_rot_matrix(matrix)
    }

    pub fn normalize_self(&mut self) -> Result<()> {
        *self = self.normalize()?;
        Ok(())
    }

    pub fn slerp(lhs: Self, rhs: Self, t: f64) -> Result<Self> {
        let lhs = lhs.normalize()?;
        let mut rhs = rhs.normalize()?;
        let mut dot = lhs.x * rhs.x + lhs.y * rhs.y + lhs.z * rhs.z + lhs.w * rhs.w;
        if dot < 0.0 {
            rhs = rhs.scale(-1.0);
            dot = -dot;
        }
        if dot > 0.9995 {
            return Self::new(
                lhs.x + t * (rhs.x - lhs.x),
                lhs.y + t * (rhs.y - lhs.y),
                lhs.z + t * (rhs.z - lhs.z),
                lhs.w + t * (rhs.w - lhs.w),
            )
            .normalize();
        }
        let theta_0 = dot.acos();
        let theta = theta_0 * t;
        let sin_theta = theta.sin();
        let sin_theta_0 = theta_0.sin();
        let s0 = theta.cos() - dot * sin_theta / sin_theta_0;
        let s1 = sin_theta / sin_theta_0;
        Ok(lhs.scale(s0).add_quat(rhs.scale(s1)))
    }

    pub fn add_quat(self, rhs: Self) -> Self {
        Self::new(
            self.x + rhs.x,
            self.y + rhs.y,
            self.z + rhs.z,
            self.w + rhs.w,
        )
    }

    pub fn from_euler(roll: f64, pitch: f64, yaw: f64) -> Self {
        let (sr, cr) = (roll * 0.5).sin_cos();
        let (sp, cp) = (pitch * 0.5).sin_cos();
        let (sy, cy) = (yaw * 0.5).sin_cos();
        Self::new(
            sr * cp * cy - cr * sp * sy,
            cr * sp * cy + sr * cp * sy,
            cr * cp * sy - sr * sp * cy,
            cr * cp * cy + sr * sp * sy,
        )
    }

    pub fn to_euler(self) -> (f64, f64, f64) {
        let q = self.normalize().unwrap_or(Self::IDENTITY);
        let sinr_cosp = 2.0 * (q.w * q.x + q.y * q.z);
        let cosr_cosp = 1.0 - 2.0 * (q.x * q.x + q.y * q.y);
        let roll = sinr_cosp.atan2(cosr_cosp);

        let sinp = 2.0 * (q.w * q.y - q.z * q.x);
        let pitch = if sinp.abs() >= 1.0 {
            sinp.signum() * std::f64::consts::FRAC_PI_2
        } else {
            sinp.asin()
        };

        let siny_cosp = 2.0 * (q.w * q.z + q.x * q.y);
        let cosy_cosp = 1.0 - 2.0 * (q.y * q.y + q.z * q.z);
        let yaw = siny_cosp.atan2(cosy_cosp);
        (roll, pitch, yaw)
    }

    pub fn a(&self) -> f64 {
        self.w
    }

    pub fn get_a(&self) -> f64 {
        self.a()
    }

    pub fn set_a(&mut self, value: f64) {
        self.w = value;
    }

    pub fn x(&self) -> f64 {
        self.x
    }

    pub fn get_x(&self) -> f64 {
        self.x()
    }

    pub fn set_x(&mut self, value: f64) {
        self.x = value;
    }

    pub fn y(&self) -> f64 {
        self.y
    }

    pub fn get_y(&self) -> f64 {
        self.y()
    }

    pub fn set_y(&mut self, value: f64) {
        self.y = value;
    }

    pub fn z(&self) -> f64 {
        self.z
    }

    pub fn get_z(&self) -> f64 {
        self.z()
    }

    pub fn set_z(&mut self, value: f64) {
        self.z = value;
    }

    pub fn equals(self, rhs: Self) -> bool {
        self == rhs
    }
}

impl fmt::Display for Quat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "({},{},{},{})",
            format_real(self.x),
            format_real(self.y),
            format_real(self.z),
            format_real(self.w)
        )
    }
}

pub(crate) fn jacobi_eigen3(mut matrix: [[f64; 3]; 3]) -> ([[f64; 3]; 3], [f64; 3]) {
    let mut vectors = [[0.0; 3]; 3];
    for (i, row) in vectors.iter_mut().enumerate() {
        row[i] = 1.0;
    }

    for _ in 0..64 {
        let mut p = 0;
        let mut q = 1;
        let mut max = matrix[0][1].abs();
        for (i, row) in matrix.iter().enumerate() {
            for (j, value) in row.iter().enumerate().skip(i + 1) {
                if value.abs() > max {
                    max = value.abs();
                    p = i;
                    q = j;
                }
            }
        }
        if max < 1.0e-12 {
            break;
        }

        let theta = (matrix[q][q] - matrix[p][p]) / (2.0 * matrix[p][q]);
        let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
        let c = 1.0 / (t * t + 1.0).sqrt();
        let s = t * c;
        let tau = s / (1.0 + c);
        let mpq = matrix[p][q];
        matrix[p][p] -= t * mpq;
        matrix[q][q] += t * mpq;
        matrix[p][q] = 0.0;
        matrix[q][p] = 0.0;

        for r in 0..3 {
            if r != p && r != q {
                let mrp = matrix[r][p];
                let mrq = matrix[r][q];
                matrix[r][p] = mrp - s * (mrq + tau * mrp);
                matrix[p][r] = matrix[r][p];
                matrix[r][q] = mrq + s * (mrp - tau * mrq);
                matrix[q][r] = matrix[r][q];
            }
            let vrp = vectors[r][p];
            let vrq = vectors[r][q];
            vectors[r][p] = vrp - s * (vrq + tau * vrp);
            vectors[r][q] = vrq + s * (vrp - tau * vrq);
        }
    }

    (vectors, [matrix[0][0], matrix[1][1], matrix[2][2]])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vector_math_matches_cuemol_basics() {
        let a = Vector4::new3(1.0, 0.0, 0.0);
        let b = Vector4::new3(0.0, 1.0, 0.0);
        assert_eq!(a.cross(b), Vector4::new3(0.0, 0.0, 1.0));
        assert_eq!(a.dot(b), 0.0);
        assert_eq!(a.add_vec(b), Vector4::new4(1.0, 1.0, 0.0, 0.0));
        assert!((a.angle(b).unwrap() - std::f64::consts::FRAC_PI_2).abs() < 1.0e-12);
    }

    #[test]
    fn torsion_is_signed_matching_cuemol_convention() {
        // Sign convention pinned against C++ qlib Vector4D::torsion (VectorHelper.cpp):
        // about the j-k axis (+x), the i arm along +y and the l arm along +z give +90.
        let i = Vector4::new3(0.0, 1.0, 0.0);
        let j = Vector4::new3(0.0, 0.0, 0.0);
        let k = Vector4::new3(1.0, 0.0, 0.0);
        let l_pos = Vector4::new3(1.0, 0.0, 1.0);
        let l_neg = Vector4::new3(1.0, 0.0, -1.0);
        assert!((Vector4::torsion(i, j, k, l_pos).unwrap().to_degrees() - 90.0).abs() < 1.0e-9);
        assert!((Vector4::torsion(i, j, k, l_neg).unwrap().to_degrees() + 90.0).abs() < 1.0e-9);
        // Trans (anti) arrangement is +-180.
        let l_trans = Vector4::new3(1.0, -1.0, 0.0);
        assert!(
            (Vector4::torsion(i, j, k, l_trans)
                .unwrap()
                .to_degrees()
                .abs()
                - 180.0)
                .abs()
                < 1.0e-9
        );
        // Degenerate (collinear i-j-k) is an error, not a silent zero.
        assert!(Vector4::torsion(j, j, k, l_pos).is_err());
    }
}
