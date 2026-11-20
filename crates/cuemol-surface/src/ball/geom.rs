//! BALL `MATHS` primitives (`TVector3`, `TSphere3`, `TCircle3`, `TPlane3`, `TLine3`,
//! `TAngle`, `analyticalGeometry.h`) restricted to what the SES pipeline uses.

use super::BallResult;
use std::cell::Cell;
use std::f64::consts::PI as STD_PI;
use std::ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub};

pub(super) const PI: f64 = STD_PI;

thread_local! {
    static EPSILON: Cell<f64> = const { Cell::new(1e-6) };
}

/// Current value of BALL's mutable `Constants::EPSILON`.
#[inline]
pub(super) fn eps() -> f64 {
    EPSILON.with(Cell::get)
}

/// Sets `Constants::EPSILON` and returns the previous value.
#[inline]
pub(super) fn set_eps(value: f64) -> f64 {
    EPSILON.with(|e| e.replace(value))
}

/// `BALL_ABS`, which keeps the sign of negative zero.
#[inline]
pub(super) fn babs(x: f64) -> f64 {
    if x >= 0.0 {
        x
    } else {
        -x
    }
}

#[inline]
pub(super) fn is_zero(t: f64) -> bool {
    babs(t) < eps()
}
#[inline]
pub(super) fn is_not_zero(t: f64) -> bool {
    babs(t) >= eps()
}
#[inline]
pub(super) fn is_equal(a: f64, b: f64) -> bool {
    babs(a - b) < eps()
}
#[inline]
pub(super) fn is_not_equal(a: f64, b: f64) -> bool {
    babs(a - b) >= eps()
}
#[inline]
pub(super) fn is_less(a: f64, b: f64) -> bool {
    (a - b) <= -eps()
}
#[inline]
pub(super) fn is_less_or_equal(a: f64, b: f64) -> bool {
    (a - b) < eps()
}
#[inline]
pub(super) fn is_greater_or_equal(a: f64, b: f64) -> bool {
    (a - b) > -eps()
}
#[inline]
pub(super) fn is_greater(a: f64, b: f64) -> bool {
    a - b >= eps()
}

/// `Maths::round` followed by the conversion to `Size`.
pub(super) fn round_size(t: f64) -> u32 {
    let r = if t > 0.0 {
        (t + 0.5) as i64
    } else {
        (t - 0.5) as i64
    };
    r as f64 as u32
}

pub(super) fn division_by_zero() -> String {
    "BALL: division by zero".to_string()
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    pub const ZERO: Vec3 = Vec3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    #[inline]
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
    #[inline]
    pub fn dot(self, v: Vec3) -> f64 {
        self.x * v.x + self.y * v.y + self.z * v.z
    }
    #[inline]
    pub fn cross(self, v: Vec3) -> Vec3 {
        Vec3::new(
            self.y * v.z - self.z * v.y,
            self.z * v.x - self.x * v.z,
            self.x * v.y - self.y * v.x,
        )
    }
    #[inline]
    pub fn length(self) -> f64 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }
    #[inline]
    pub fn square_length(self) -> f64 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }
    pub fn normalize(&mut self) -> BallResult<()> {
        let len = (self.x * self.x + self.y * self.y + self.z * self.z).sqrt();
        if is_zero(len) {
            return Err(division_by_zero());
        }
        self.x /= len;
        self.y /= len;
        self.z /= len;
        Ok(())
    }
    pub fn normalized(mut self) -> BallResult<Vec3> {
        self.normalize()?;
        Ok(self)
    }
    #[inline]
    pub fn negate(&mut self) {
        self.x *= -1.0;
        self.y *= -1.0;
        self.z *= -1.0;
    }

    #[inline]
    pub fn distance(self, v: Vec3) -> f64 {
        let dx = self.x - v.x;
        let dy = self.y - v.y;
        let dz = self.z - v.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
    #[inline]
    pub fn square_distance(self, v: Vec3) -> f64 {
        let dx = self.x - v.x;
        let dy = self.y - v.y;
        let dz = self.z - v.z;
        dx * dx + dy * dy + dz * dz
    }
    /// BALL `operator ==` (component-wise within EPSILON).
    #[inline]
    pub fn eq_eps(self, v: Vec3) -> bool {
        is_equal(self.x, v.x) && is_equal(self.y, v.y) && is_equal(self.z, v.z)
    }
    /// BALL `operator !=`.
    #[inline]
    pub fn ne_eps(self, v: Vec3) -> bool {
        is_not_equal(self.x, v.x) || is_not_equal(self.y, v.y) || is_not_equal(self.z, v.z)
    }
    #[inline]
    pub fn get(self, i: usize) -> f64 {
        match i {
            0 => self.x,
            1 => self.y,
            _ => self.z,
        }
    }
    #[inline]
    pub fn to_f32(self) -> [f32; 3] {
        [self.x as f32, self.y as f32, self.z as f32]
    }
}

impl Add for Vec3 {
    type Output = Vec3;
    #[inline]
    fn add(self, b: Vec3) -> Vec3 {
        Vec3::new(self.x + b.x, self.y + b.y, self.z + b.z)
    }
}
impl Sub for Vec3 {
    type Output = Vec3;
    #[inline]
    fn sub(self, b: Vec3) -> Vec3 {
        Vec3::new(self.x - b.x, self.y - b.y, self.z - b.z)
    }
}
impl Neg for Vec3 {
    type Output = Vec3;
    #[inline]
    fn neg(self) -> Vec3 {
        Vec3::new(-self.x, -self.y, -self.z)
    }
}
impl Mul<f64> for Vec3 {
    type Output = Vec3;
    #[inline]
    fn mul(self, s: f64) -> Vec3 {
        Vec3::new(self.x * s, self.y * s, self.z * s)
    }
}
impl Mul<Vec3> for f64 {
    type Output = Vec3;
    #[inline]
    fn mul(self, v: Vec3) -> Vec3 {
        Vec3::new(self * v.x, self * v.y, self * v.z)
    }
}
impl AddAssign for Vec3 {
    #[inline]
    fn add_assign(&mut self, v: Vec3) {
        self.x += v.x;
        self.y += v.y;
        self.z += v.z;
    }
}
impl MulAssign<f64> for Vec3 {
    #[inline]
    fn mul_assign(&mut self, s: f64) {
        self.x *= s;
        self.y *= s;
        self.z *= s;
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Vec4 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub h: f64,
}

impl Vec4 {
    pub fn normalize(&mut self) -> BallResult<()> {
        let len = (self.x * self.x + self.y * self.y + self.z * self.z + self.h * self.h).sqrt();
        if is_zero(len) {
            return Err(division_by_zero());
        }
        self.x /= len;
        self.y /= len;
        self.z /= len;
        self.h /= len;
        Ok(())
    }
    pub fn is_zero_eps(self) -> bool {
        is_equal(self.x, 0.0)
            && is_equal(self.y, 0.0)
            && is_equal(self.z, 0.0)
            && is_equal(self.h, 0.0)
    }
}

/// Row-major 4x4 matrix (`TMatrix4x4`).
#[derive(Clone, Copy, Debug)]
pub(super) struct Mat4(pub [[f64; 4]; 4]);

impl Mat4 {
    pub fn mul_vec(&self, v: Vec4) -> Vec4 {
        let m = &self.0;
        Vec4 {
            x: m[0][0] * v.x + m[0][1] * v.y + m[0][2] * v.z + m[0][3] * v.h,
            y: m[1][0] * v.x + m[1][1] * v.y + m[1][2] * v.z + m[1][3] * v.h,
            z: m[2][0] * v.x + m[2][1] * v.y + m[2][2] * v.z + m[2][3] * v.h,
            h: m[3][0] * v.x + m[3][1] * v.y + m[3][2] * v.z + m[3][3] * v.h,
        }
    }
}

/// `TQuaternion(axis, angle).getRotationMatrix()`.
pub(super) fn rotation_matrix(axis: Vec3, angle: f64) -> Mat4 {
    let length = axis.length();
    let (a, b, c, d);
    if is_equal(length, 0.0) {
        a = 1.0;
        b = 0.0;
        c = 0.0;
        d = 0.0;
    } else {
        let omega = angle * 0.5;
        let sin_omega = omega.sin();
        a = omega.cos();
        b = axis.x * sin_omega / length;
        c = axis.y * sin_omega / length;
        d = axis.z * sin_omega / length;
    }
    let s = 2.0 / (a * a + b * b + c * c + d * d).sqrt();
    Mat4([
        [
            1.0 - s * (c * c + d * d),
            s * (b * c - d * a),
            s * (d * b + c * a),
            0.0,
        ],
        [
            s * (b * c + d * a),
            1.0 - s * (d * d + b * b),
            s * (c * d - b * a),
            0.0,
        ],
        [
            s * (d * b - c * a),
            s * (c * d + b * a),
            1.0 - s * (c * c + b * b),
            0.0,
        ],
        [0.0, 0.0, 0.0, 1.0],
    ])
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Sphere {
    pub p: Vec3,
    pub radius: f64,
}

impl Sphere {
    pub const fn new(p: Vec3, radius: f64) -> Self {
        Self { p, radius }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Circle {
    pub p: Vec3,
    pub n: Vec3,
    pub radius: f64,
}

impl Circle {
    pub const fn new(p: Vec3, n: Vec3, radius: f64) -> Self {
        Self { p, n, radius }
    }
    /// BALL `TCircle3::operator !=`.
    pub fn ne_eps(&self, c: &Circle) -> bool {
        self.p.ne_eps(c.p) || self.n.ne_eps(c.n) || is_not_equal(self.radius, c.radius)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Plane {
    pub p: Vec3,
    pub n: Vec3,
}

impl Plane {
    pub const fn new(p: Vec3, n: Vec3) -> Self {
        Self { p, n }
    }
    pub fn from_points(a: Vec3, b: Vec3, c: Vec3) -> Self {
        Self {
            p: a,
            n: (a - b).cross(b - c),
        }
    }
    /// `TPlane3(a, b, c, d)`; `None` stands for the thrown `DivisionByZero`.
    fn from_coefficients(a: f64, b: f64, c: f64, d: f64) -> Option<Self> {
        let n = Vec3::new(a, b, c);
        if a == 0.0 && b == 0.0 && c == 0.0 {
            return None;
        }
        let mut p = Vec3::ZERO;
        if !is_zero(a) {
            p = Vec3::new(-d / a, 0.0, 0.0);
        } else if !is_zero(b) {
            p = Vec3::new(0.0, -d / b, 0.0);
        } else if !is_zero(c) {
            p = Vec3::new(0.0, 0.0, -d / c);
        }
        Some(Self { p, n })
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Line {
    pub p: Vec3,
    pub d: Vec3,
}

impl Line {
    pub fn from_two_points(a: Vec3, b: Vec3) -> Self {
        Self { p: a, d: b - a }
    }
}

/// `GetDistance(point, plane)`.
pub(super) fn distance_point_plane(point: Vec3, plane: &Plane) -> BallResult<f64> {
    let length = plane.n.length();
    if length == 0.0 {
        return Err(division_by_zero());
    }
    Ok(babs(plane.n.dot(point - plane.p)) / length)
}

/// `SolveQuadraticEquation`.
fn solve_quadratic(a: f64, b: f64, c: f64) -> (u32, f64, f64) {
    if a == 0.0 {
        if b == 0.0 {
            return (0, 0.0, 0.0);
        }
        let x = c / b;
        return (1, x, x);
    }
    let discriminant = b * b - 4.0 * a * c;
    if is_less(discriminant, 0.0) {
        return (0, 0.0, 0.0);
    }
    let sqrt_discriminant = discriminant.sqrt();
    if is_zero(sqrt_discriminant) {
        let x = -b / (2.0 * a);
        (1, x, x)
    } else {
        (
            2,
            (-b + sqrt_discriminant) / (2.0 * a),
            (-b - sqrt_discriminant) / (2.0 * a),
        )
    }
}

/// `GetIntersection(sphere, sphere, circle)`.
pub(super) fn intersect_spheres(a: &Sphere, b: &Sphere) -> Option<Circle> {
    let norm = b.p - a.p;
    let square_dist = norm.dot(norm);
    if is_zero(square_dist) {
        return None;
    }
    let dist = square_dist.sqrt();
    if is_less(a.radius + b.radius, dist) {
        return None;
    }
    if is_greater_or_equal(babs(a.radius - b.radius), dist) {
        return None;
    }
    let radius1_square = a.radius * a.radius;
    let radius2_square = b.radius * b.radius;
    let u = radius1_square - radius2_square + square_dist;
    let length = u / (2.0 * square_dist);
    let square_radius = radius1_square - u * length / 2.0;
    if square_radius < 0.0 {
        return None;
    }
    Some(Circle {
        p: a.p + (norm * length),
        radius: square_radius.sqrt(),
        n: Vec3::new(norm.x / dist, norm.y / dist, norm.z / dist),
    })
}

/// `GetIntersection(sphere, plane, circle)`.
pub(super) fn intersect_sphere_plane(sphere: &Sphere, plane: &Plane) -> BallResult<Option<Circle>> {
    let distance = distance_point_plane(sphere.p, plane)?;
    if is_greater(distance, sphere.radius) {
        return Ok(None);
    }
    let unit = plane.n.normalized()?;
    if is_equal(distance, sphere.radius) {
        Ok(Some(Circle::new(
            sphere.p + sphere.radius * unit,
            plane.n,
            0.0,
        )))
    } else {
        Ok(Some(Circle::new(
            sphere.p + distance * unit,
            plane.n,
            (sphere.radius * sphere.radius - distance * distance).sqrt(),
        )))
    }
}

/// `GetIntersection(sphere, line, p1, p2)`; the points are only written on success.
pub(super) fn intersect_sphere_line(
    sphere: &Sphere,
    line: &Line,
    p1: &mut Vec3,
    p2: &mut Vec3,
) -> bool {
    let diff = line.p - sphere.p;
    let (count, x1, x2) = solve_quadratic(
        line.d.dot(line.d),
        diff.dot(line.d) * 2.0,
        diff.dot(diff) - sphere.radius * sphere.radius,
    );
    if count == 0 {
        return false;
    }
    *p1 = line.p + x1 * line.d;
    *p2 = line.p + x2 * line.d;
    true
}

/// `GetIntersection(plane, plane, line)`.
fn intersect_planes(plane1: &Plane, plane2: &Plane) -> Option<Line> {
    let u = plane1.p.dot(plane1.n);
    let v = plane2.p.dot(plane2.n);
    let mut line = Line::default();
    let mut det = plane1.n.x * plane2.n.y - plane1.n.y * plane2.n.x;
    if is_zero(det) {
        det = plane1.n.x * plane2.n.z - plane1.n.z * plane2.n.x;
        if is_zero(det) {
            det = plane1.n.y * plane2.n.z - plane1.n.z * plane2.n.y;
            if is_zero(det) {
                return None;
            }
            let a = plane2.n.z / det;
            let b = -plane1.n.z / det;
            let c = -plane2.n.y / det;
            let d = plane1.n.y / det;
            line.p.x = 0.0;
            line.p.y = a * u + b * v;
            line.p.z = c * u + d * v;
            line.d.x = -1.0;
            line.d.y = a * plane1.n.x + b * plane2.n.x;
            line.d.z = c * plane1.n.x + d * plane2.n.x;
        } else {
            let a = plane2.n.z / det;
            let b = -plane1.n.z / det;
            let c = -plane2.n.x / det;
            let d = plane1.n.x / det;
            line.p.x = a * u + b * v;
            line.p.y = 0.0;
            line.p.z = c * u + d * v;
            line.d.x = a * plane1.n.y + b * plane2.n.y;
            line.d.y = -1.0;
            line.d.z = c * plane1.n.y + d * plane2.n.y;
        }
    } else {
        let a = plane2.n.y / det;
        let b = -plane1.n.y / det;
        let c = -plane2.n.x / det;
        let d = plane1.n.x / det;
        line.p.x = a * u + b * v;
        line.p.y = c * u + d * v;
        line.p.z = 0.0;
        line.d.x = a * plane1.n.z + b * plane2.n.z;
        line.d.y = c * plane1.n.z + d * plane2.n.z;
        line.d.z = -1.0;
    }
    Some(line)
}

/// `GetIntersection(s1, s2, s3, p1, p2, test)`; the points are written whenever the
/// quadratic has a solution, even if the optional test rejects them afterwards.
pub(super) fn intersect_three_spheres(
    s1: &Sphere,
    s2: &Sphere,
    s3: &Sphere,
    p1: &mut Vec3,
    p2: &mut Vec3,
    test: bool,
) -> bool {
    let r1_square = s1.radius * s1.radius;
    let r2_square = s2.radius * s2.radius;
    let r3_square = s3.radius * s3.radius;
    let p1_square_length = s1.p.dot(s1.p);
    let p2_square_length = s2.p.dot(s2.p);
    let p3_square_length = s3.p.dot(s3.p);
    let u = (r2_square - r1_square - p2_square_length + p1_square_length) / 2.0;
    let v = (r3_square - r1_square - p3_square_length + p1_square_length) / 2.0;
    let Some(plane1) =
        Plane::from_coefficients(s2.p.x - s1.p.x, s2.p.y - s1.p.y, s2.p.z - s1.p.z, u)
    else {
        return false;
    };
    let Some(plane2) =
        Plane::from_coefficients(s3.p.x - s1.p.x, s3.p.y - s1.p.y, s3.p.z - s1.p.z, v)
    else {
        return false;
    };
    let Some(line) = intersect_planes(&plane1, &plane2) else {
        return false;
    };
    let diff = s1.p - line.p;
    let (count, x1, x2) = solve_quadratic(
        line.d.dot(line.d),
        (-diff).dot(line.d) * 2.0,
        diff.dot(diff) - r1_square,
    );
    if count == 0 {
        return false;
    }
    *p1 = line.p + x1 * line.d;
    *p2 = line.p + x2 * line.d;
    if test {
        for (center, square) in [(s1.p, r1_square), (s2.p, r2_square), (s3.p, r3_square)] {
            for point in [*p1, *p2] {
                let t = center - point;
                if is_not_equal(t.dot(t), square) {
                    return false;
                }
            }
        }
    }
    true
}

/// `getOrientedAngle(a, b, normal)` in radians.
pub(super) fn oriented_angle(a: Vec3, b: Vec3, n: Vec3) -> BallResult<f64> {
    let bl = (a.x * a.x + a.y * a.y + a.z * a.z).sqrt();
    let el = (b.x * b.x + b.y * b.y + b.z * b.z).sqrt();
    let mut bel = a.x * b.x + a.y * b.y + a.z * b.z;
    if bl * el == 0.0 {
        return Err(division_by_zero());
    }
    bel /= bl * el;
    if bel > 1.0 {
        bel = 1.0;
    } else if bel < -1.0 {
        bel = -1.0;
    }
    let mut acosbel = bel.acos();
    if (n.x * (a.z * b.y - a.y * b.z)
        + n.y * (a.x * b.z - a.z * b.x)
        + n.z * (a.y * b.x - a.x * b.y))
        > 0.0
    {
        acosbel = PI + PI - acosbel;
    }
    Ok(acosbel)
}

/// `GetAngle(a, b, angle)`; `None` when either vector is (nearly) zero.
pub(super) fn angle_between(a: Vec3, b: Vec3) -> BallResult<Option<f64>> {
    let length_product = a.square_length() * b.square_length();
    if is_zero(length_product) {
        return Ok(None);
    }
    let length_product = a.square_length() * b.square_length();
    if length_product == 0.0 {
        return Err(division_by_zero());
    }
    let acos_arg = a.dot(b) / length_product.sqrt();
    if acos_arg.abs() > 1.0 {
        return Ok(Some(0.0));
    }
    Ok(Some(acos_arg.acos()))
}

/// `SESTriangulator::partitionOfCircle`.
pub(super) fn partition_of_circle(
    circle: &Circle,
    p0: Vec3,
    phi: f64,
    number_of_segments: u32,
    on_surface: bool,
) -> BallResult<Vec<Vec3>> {
    if number_of_segments > 48_000_000 {
        return Err("BALL circle exceeds the vertex budget".into());
    }
    let mut p;
    if on_surface {
        let d = p0 - circle.p;
        p = Vec4 {
            x: d.x,
            y: d.y,
            z: d.z,
            h: 0.0,
        };
    } else {
        p = Vec4 {
            x: circle.n.y,
            y: -circle.n.x,
            z: 0.0,
            h: 0.0,
        };
        if p.is_zero_eps() {
            p = Vec4 {
                x: circle.n.z,
                y: 0.0,
                z: -circle.n.x,
                h: 0.0,
            };
        }
        p.normalize()?;
        p.x *= circle.radius;
        p.y *= circle.radius;
        p.z *= circle.radius;
        p.h *= circle.radius;
    }
    let rotation = rotation_matrix(circle.n, phi);
    let mut partition = Vec::with_capacity(number_of_segments as usize + 1);
    partition.push(Vec3::new(p.x, p.y, p.z) + circle.p);
    for _ in 0..number_of_segments {
        p = rotation.mul_vec(p);
        partition.push(Vec3::new(p.x, p.y, p.z) + circle.p);
    }
    Ok(partition)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comparisons_follow_the_current_epsilon() {
        assert!(is_equal(1.0, 1.0 + 5e-7));
        let saved = set_eps(1e-8);
        assert!(!is_equal(1.0, 1.0 + 5e-7));
        assert!(is_less(1.0, 1.0 + 5e-7));
        set_eps(saved);
        assert!(!is_less(1.0, 1.0 + 5e-7));
        assert!(is_less_or_equal(1.0 + 5e-7, 1.0));
        assert!(babs(-0.0).is_sign_negative());
    }

    #[test]
    fn sphere_intersections() {
        let a = Sphere::new(Vec3::ZERO, 2.0);
        let b = Sphere::new(Vec3::new(2.0, 0.0, 0.0), 2.0);
        let c = intersect_spheres(&a, &b).unwrap();
        assert_eq!(c.p, Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(c.n, Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(c.radius, 3f64.sqrt());
        assert!(intersect_spheres(&a, &Sphere::new(Vec3::new(5.0, 0.0, 0.0), 1.0)).is_none());

        let s3 = Sphere::new(Vec3::new(1.0, 1.0, 0.0), 2.0);
        let (mut p1, mut p2) = (Vec3::ZERO, Vec3::ZERO);
        assert!(intersect_three_spheres(&a, &b, &s3, &mut p1, &mut p2, true));
        for p in [p1, p2] {
            assert!((p.length() - 2.0).abs() < 1e-12);
            assert!((p.distance(b.p) - 2.0).abs() < 1e-12);
            assert!((p.distance(s3.p) - 2.0).abs() < 1e-12);
        }
        assert!(p1.z * p2.z < 0.0);
    }

    #[test]
    fn oriented_angles_and_rotations() {
        let n = Vec3::new(0.0, 0.0, 1.0);
        let x = Vec3::new(1.0, 0.0, 0.0);
        let y = Vec3::new(0.0, 1.0, 0.0);
        assert!((oriented_angle(x, y, n).unwrap() - 0.5 * PI).abs() < 1e-12);
        assert!((oriented_angle(y, x, n).unwrap() - 1.5 * PI).abs() < 1e-12);
        assert!(oriented_angle(Vec3::ZERO, x, n).is_err());

        let circle = Circle::new(Vec3::new(1.0, 2.0, 3.0), n, 2.0);
        let points =
            partition_of_circle(&circle, Vec3::new(3.0, 2.0, 3.0), PI / 2.0, 4, true).unwrap();
        assert_eq!(points.len(), 5);
        assert!((points[1] - Vec3::new(1.0, 4.0, 3.0)).length() < 1e-12);
        assert!((points[4] - points[0]).length() < 1e-12);
    }

    #[test]
    fn plane_distance_and_quadratic() {
        let plane = Plane::from_points(
            Vec3::ZERO,
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        );
        assert_eq!(
            distance_point_plane(Vec3::new(0.0, 0.0, -3.0), &plane).unwrap(),
            3.0
        );
        assert!(distance_point_plane(Vec3::ZERO, &Plane::default()).is_err());
        let line = Line::from_two_points(Vec3::new(-5.0, 0.0, 0.0), Vec3::new(5.0, 0.0, 0.0));
        let (mut p1, mut p2) = (Vec3::ZERO, Vec3::ZERO);
        assert!(intersect_sphere_line(
            &Sphere::new(Vec3::ZERO, 1.0),
            &line,
            &mut p1,
            &mut p2
        ));
        assert_eq!((p1.x, p2.x), (1.0, -1.0));
        assert_eq!(round_size(2.5), 3);
        assert_eq!(round_size(2.49), 2);
    }
}
