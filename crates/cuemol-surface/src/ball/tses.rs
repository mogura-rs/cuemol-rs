//! BALL `TriangulatedSES`, `SESTriangulator` and `TriangulatedSphere`
//! (`triangulatedSES.cpp`, `triangulatedSurface.cpp`, `TrianglePoint`, `TriangleEdge`, `Triangle`).

use super::geom::*;
use super::rs::ReducedSurface;
use super::ses::{EdgeType, FaceType, Ses, SesEdge, SE, SF};
use super::util::{define_id, impl_arena_id, Arena};
use super::BallResult;
use std::collections::HashSet;

define_id!(TP, TE, TT);
impl_arena_id!(TP, TE, TT);

pub(super) struct TPoint {
    pub point: Vec3,
    pub normal: Vec3,
    edges: Vec<TE>,
    faces: u32,
}

impl TPoint {
    fn new(point: Vec3, normal: Vec3) -> Self {
        Self {
            point,
            normal,
            edges: Vec::new(),
            faces: 0,
        }
    }
    fn insert_edge(&mut self, e: TE) {
        if !self.edges.contains(&e) {
            self.edges.push(e);
        }
    }
}

struct TEdge {
    vertex: [TP; 2],
    face: [Option<TT>; 2],
}

pub(super) struct Triangle {
    pub vertex: [TP; 3],
}

impl Triangle {
    fn third(&self, v1: TP, v2: TP) -> TP {
        if self.vertex[0] == v1 || self.vertex[0] == v2 {
            if self.vertex[1] == v1 || self.vertex[1] == v2 {
                self.vertex[2]
            } else {
                self.vertex[1]
            }
        } else {
            self.vertex[0]
        }
    }
    fn relative_index(&self, v: TP) -> i32 {
        self.vertex
            .iter()
            .position(|&x| x == v)
            .map_or(-1, |i| i as i32)
    }
}

/// The triangulated surface: point and triangle lists over shared arenas.
pub(super) struct Mesh {
    pub pa: Arena<TP, TPoint>,
    ea: Arena<TE, TEdge>,
    pub ta: Arena<TT, Triangle>,
    pub points: Vec<TP>,
    pub triangles: Vec<TT>,
}

impl Mesh {
    fn new() -> Self {
        Self {
            pa: Arena::new(),
            ea: Arena::new(),
            ta: Arena::new(),
            points: Vec::new(),
            triangles: Vec::new(),
        }
    }

    fn new_edge(&mut self, v0: TP, v1: TP) -> TE {
        self.ea.push(TEdge {
            vertex: [v0, v1],
            face: [None, None],
        })
    }

    /// Edge registered at both end points (`v->edges_.insert(e)`).
    fn new_linked_edge(&mut self, v0: TP, v1: TP) -> TE {
        let e = self.new_edge(v0, v1);
        self.pa[v0].insert_edge(e);
        self.pa[v1].insert_edge(e);
        e
    }

    fn new_point(&mut self, point: Vec3, normal: Vec3) -> TP {
        let p = self.pa.push(TPoint::new(point, normal));
        self.points.push(p);
        p
    }

    /// `TriangleEdge::operator ==` (end points compared by coordinates).
    fn edges_equal(&self, a: [TP; 2], b: [TP; 2]) -> bool {
        let pt = |p: TP| self.pa[p].point;
        (pt(a[0]).eq_eps(pt(b[0])) && pt(a[1]).eq_eps(pt(b[1])))
            || (pt(a[0]).eq_eps(pt(b[1])) && pt(a[1]).eq_eps(pt(b[0])))
    }

    /// `GraphVertex::has(edge)` for a not yet created edge.
    fn find_edge(&self, at: TP, ends: [TP; 2]) -> Option<TE> {
        let edges = &self.pa[at].edges;
        edges
            .iter()
            .copied()
            .find(|&e| self.edges_equal(self.ea[e].vertex, ends))
    }

    fn add_triangle(&mut self, vertex: [TP; 3]) -> TT {
        let t = self.ta.push(Triangle { vertex });
        self.triangles.push(t);
        t
    }
}

/// A part triangulated separately and appended with `TriangulatedSurface::join`.
struct Part {
    points: Vec<TP>,
    triangles: Vec<TT>,
}

/// `TriangulatedSphere` built by `icosaeder` and `refine`.
struct Sphere3 {
    points: Vec<Vec3>,
    normals: Vec<Vec3>,
    edges: Vec<[usize; 2]>,
    edge_faces: Vec<[usize; 2]>,
    triangles: Vec<[usize; 3]>,
}

#[derive(Clone, Copy)]
struct RefineFace {
    p: [usize; 6],
    e: [usize; 9],
    pcount: usize,
    ecount: usize,
}

impl Sphere3 {
    fn icosaeder(out: bool) -> Self {
        let c = |x: f64, y: f64, z: f64| Vec3::new(x, y, z);
        let points = vec![
            c(0.0, 0.0, 1.0),
            c(0.894427, 0.0, 0.4472135),
            c(0.276393, 0.850651, 0.4472135),
            c(-0.723607, 0.525731, 0.4472135),
            c(-0.723607, -0.525731, 0.4472135),
            c(0.276393, -0.850651, 0.4472135),
            c(0.723607, 0.525731, -0.4472135),
            c(-0.276393, 0.850651, -0.4472135),
            c(-0.894427, 0.0, -0.4472135),
            c(-0.276393, -0.850651, -0.4472135),
            c(0.723607, -0.525731, -0.4472135),
            c(0.0, 0.0, -1.0),
        ];
        let normals = points
            .iter()
            .map(|&p| if out { p } else { -1.0 * p })
            .collect();
        let edges: Vec<[usize; 2]> = vec![
            [2, 0],
            [0, 1],
            [1, 2],
            [3, 0],
            [2, 3],
            [4, 0],
            [3, 4],
            [5, 0],
            [4, 5],
            [5, 1],
            [1, 6],
            [6, 2],
            [7, 2],
            [6, 7],
            [7, 3],
            [8, 3],
            [7, 8],
            [8, 4],
            [9, 4],
            [8, 9],
            [9, 5],
            [10, 5],
            [9, 10],
            [1, 10],
            [10, 6],
            [6, 11],
            [11, 7],
            [11, 8],
            [11, 9],
            [11, 10],
        ];
        let triangle_edges: [[usize; 3]; 20] = [
            [0, 1, 2],
            [0, 3, 4],
            [3, 5, 6],
            [5, 7, 8],
            [1, 7, 9],
            [2, 10, 11],
            [11, 12, 13],
            [4, 12, 14],
            [14, 15, 16],
            [6, 15, 17],
            [17, 18, 19],
            [8, 18, 20],
            [20, 21, 22],
            [10, 23, 24],
            [9, 21, 23],
            [13, 25, 26],
            [16, 26, 27],
            [19, 27, 28],
            [22, 28, 29],
            [24, 25, 29],
        ];
        let triangles = triangle_edges
            .iter()
            .map(|t| {
                let e1 = edges[t[0]];
                let e2 = edges[t[1]];
                let third = if e2[0] != e1[0] && e2[0] != e1[1] {
                    e2[0]
                } else {
                    e2[1]
                };
                [e1[0], e1[1], third]
            })
            .collect();
        let edge_faces = vec![
            [0, 1],
            [0, 4],
            [0, 5],
            [1, 2],
            [1, 7],
            [2, 3],
            [2, 9],
            [3, 4],
            [3, 11],
            [4, 14],
            [5, 13],
            [5, 6],
            [6, 7],
            [6, 15],
            [7, 8],
            [8, 9],
            [8, 16],
            [9, 10],
            [10, 11],
            [10, 17],
            [11, 12],
            [12, 14],
            [12, 18],
            [13, 14],
            [13, 19],
            [15, 19],
            [15, 16],
            [16, 17],
            [17, 18],
            [18, 19],
        ];
        Self {
            points,
            normals,
            edges,
            edge_faces,
            triangles,
        }
    }

    /// `TriangulatedSphere::refine(iterations, out)`.
    fn refine(&mut self, iterations: u32, out: bool) -> BallResult<()> {
        for _ in 0..iterations {
            self.refine_once(out)?;
        }
        for t in &mut self.triangles {
            let [a, b, c] = [self.points[t[0]], self.points[t[1]], self.points[t[2]]];
            let norm = (b - a).cross(c - a);
            if (is_greater(norm.dot(a), 0.0) && !out) || (is_less(norm.dot(a), 0.0) && out) {
                t.swap(1, 2);
            }
        }
        Ok(())
    }

    fn refine_once(&mut self, out: bool) -> BallResult<()> {
        const NONE: usize = usize::MAX;
        let mut faces: Vec<RefineFace> = self
            .triangles
            .iter()
            .map(|t| RefineFace {
                p: [t[0], t[1], t[2], NONE, NONE, NONE],
                e: [NONE; 9],
                pcount: 3,
                ecount: 0,
            })
            .collect();
        let old_edges = self.edges.len();
        let mut new_edges = Vec::with_capacity(old_edges);
        let mut new_edge_faces = Vec::with_capacity(old_edges);
        for e in 0..old_edges {
            let [point1, point2] = self.edges[e];
            let point = (self.points[point1] + self.points[point2]).normalized()?;
            let new_point = self.points.len();
            self.points.push(point);
            self.normals.push(if out { point } else { -point });
            self.edges[e] = [point1, new_point];
            let new_edge2 = old_edges + new_edges.len();
            new_edges.push([point2, new_point]);
            new_edge_faces.push([NONE, NONE]);
            for i in self.edge_faces[e] {
                let f = &mut faces[i];
                f.p[f.pcount] = new_point;
                f.pcount += 1;
                f.e[f.ecount] = e;
                f.e[f.ecount + 1] = new_edge2;
                f.ecount += 2;
            }
            self.edge_faces[e] = [NONE, NONE];
        }
        self.edges.extend(new_edges);
        self.edge_faces.extend(new_edge_faces);
        let old_triangles = self.triangles.len();
        let mut new_triangles = Vec::with_capacity(3 * old_triangles);
        for (i, face) in faces.iter_mut().enumerate() {
            for k in 6..9 {
                face.e[k] = self.edges.len();
                self.edges.push([face.p[k - 3], face.p[(k - 5) % 3 + 3]]);
                self.edge_faces.push([NONE, NONE]);
            }
            let base = old_triangles + new_triangles.len();
            let ids = [base, base + 1, base + 2];
            let mut tris = [[0usize; 3]; 3];
            let mut inner = [NONE; 3];
            for k in 0..3 {
                let p3 = face.p[k];
                let mut j = 0;
                let mut first = NONE;
                let mut p1 = NONE;
                while first == NONE {
                    let ev = self.edges[face.e[j]];
                    if ev[0] == p3 {
                        first = face.e[j];
                        p1 = ev[1];
                    } else if ev[1] == p3 {
                        first = face.e[j];
                        p1 = ev[0];
                    }
                    j += 1;
                }
                let mut second = NONE;
                let mut p2 = NONE;
                while second == NONE {
                    let ev = self.edges[face.e[j]];
                    if ev[0] == p3 {
                        second = face.e[j];
                        p2 = ev[1];
                    } else if ev[1] == p3 {
                        second = face.e[j];
                        p2 = ev[0];
                    }
                    j += 1;
                }
                let mut j = 6;
                while inner[k] == NONE {
                    let ev = self.edges[face.e[j]];
                    if (ev[0] == p1 && ev[1] == p2) || (ev[0] == p2 && ev[1] == p1) {
                        inner[k] = face.e[j];
                    }
                    j += 1;
                }
                tris[k] = [p1, p2, p3];
                for edge in [first, second] {
                    let ef = &mut self.edge_faces[edge];
                    if ef[0] == NONE {
                        ef[0] = ids[k];
                    } else {
                        ef[1] = ids[k];
                    }
                }
                self.edge_faces[inner[k]] = [ids[k], i];
            }
            self.triangles[i] = [face.p[3], face.p[4], face.p[5]];
            new_triangles.extend(tris);
        }
        self.triangles.extend(new_triangles);
        Ok(())
    }
}

/// `SESTriangulator::numberOfRefinements`.
fn number_of_refinements(density: f64, radius: f64) -> usize {
    let test0 = (4.0 * density * PI * radius * radius - 12.0) / 30.0;
    let mut n = 0;
    if is_greater_or_equal(test0, 0.0) {
        let mut test1 = 1.0;
        let mut test2 = 1.0;
        while is_less(test2, test0) {
            test1 = test2;
            test2 *= 4.0;
            n += 1;
        }
        if is_less(test2 - test0, test0 - test1) {
            n += 1;
        }
    }
    n.min(3)
}

/// A candidate triangle from `createTriangleAndEdges` that is only materialised when kept.
struct Candidate {
    vertex: [TP; 3],
    edge1: Result<TE, [TP; 2]>,
    edge2: Result<TE, [TP; 2]>,
}

pub(super) struct SesTriangulator<'a> {
    rs: &'a mut ReducedSurface,
    ses: &'a mut Ses,
    pub mesh: Mesh,
    point: Vec<Option<TP>>,
    edge: Vec<Vec<TE>>,
    templates: Vec<Vec<(Vec3, Vec3)>>,
    density: f64,
    sqrt_density: f64,
}

impl<'a> SesTriangulator<'a> {
    pub fn new(rs: &'a mut ReducedSurface, ses: &'a mut Ses, density: f64) -> Self {
        let nv = ses.vertices.len();
        let ne = ses.edges.len();
        Self {
            rs,
            ses,
            mesh: Mesh::new(),
            point: vec![None; nv],
            edge: vec![Vec::new(); ne],
            templates: Vec::new(),
            density,
            sqrt_density: density.sqrt(),
        }
    }

    pub fn run(&mut self) -> BallResult<()> {
        self.pre_processing()?;
        self.triangulate_toric_faces()?;
        self.partition_singular_edges()?;
        self.triangulate_contact_faces()?;
        self.triangulate_spheric_faces()
    }

    fn pt(&self, v: super::ses::SV) -> TP {
        self.point[self.ses.va[v].index as usize].expect("triangle point of SES vertex")
    }

    fn pre_processing(&mut self) -> BallResult<()> {
        self.ses.clean(self.rs, self.density)?;
        self.ses.split_spheric_faces()?;
        for i in 0..self.ses.vertices.len() {
            let v = &self.ses.va[self.ses.vertex(i)];
            let (point, normal) = (v.point, v.normal);
            self.point[i] = Some(self.mesh.new_point(point, normal));
        }
        let mut sphere = Sphere3::icosaeder(true);
        let snapshot = |s: &Sphere3| {
            s.points
                .iter()
                .copied()
                .zip(s.normals.iter().copied())
                .collect()
        };
        self.templates.push(snapshot(&sphere));
        for _ in 0..3 {
            sphere.refine(1, true)?;
            self.templates.push(snapshot(&sphere));
        }
        Ok(())
    }

    fn triangulate_toric_faces(&mut self) -> BallResult<()> {
        let epsilon = set_eps(1e-4);
        let probe_radius = self.rs.probe_radius;
        for i in 0..self.ses.toric_faces.len() {
            let face = self.ses.toric(i);
            if self.ses.face_is_free(self.rs, face) {
                self.triangulate_free_toric_face(face, probe_radius)?;
            } else if self.rs.ea[self.ses.fa[face].rsedge()].singular {
                self.triangulate_singular_toric_face(face, probe_radius)?;
            } else {
                self.triangulate_non_singular_toric_face(face, probe_radius)?;
            }
        }
        set_eps(epsilon);
        Ok(())
    }

    fn reversed_segments(segments: &mut [Vec3], n: usize) {
        for i in 0..(n + 1) / 2 {
            segments.swap(i, n - i);
        }
    }

    fn triangulate_non_singular_toric_face(
        &mut self,
        face: SF,
        probe_radius: f64,
    ) -> BallResult<()> {
        let f = &self.ses.fa[face];
        let (edge0, edge1, edge2, edge3) = (f.edge[0], f.edge[1], f.edge[2], f.edge[3]);
        let (p0, p1) = (f.vertex[0], f.vertex[1]);
        let rsedge = self.rs.ea[f.rsedge()].clone();
        let e1 = self.ses.ea[edge1].clone();
        let e3 = self.ses.ea[edge3].clone();
        let mut number_of_segments =
            round_size(rsedge.angle * e3.circle.radius * self.sqrt_density);
        if number_of_segments == 0 {
            number_of_segments += 1;
        }
        let psi = rsedge.angle / number_of_segments as f64;
        let n = number_of_segments as usize;
        let mut normal = e3.circle.n;
        let point_of = |v: Option<super::ses::SV>| self.ses.va[v.expect("toric edge vertex")].point;
        let mut edge3_segments = partition_of_circle(
            &e3.circle,
            point_of(e3.vertex[0]),
            psi,
            number_of_segments,
            true,
        )?;
        edge3_segments[n] = point_of(e3.vertex[1]);
        if e3.vertex[0] != Some(p0) {
            Self::reversed_segments(&mut edge3_segments, n);
            normal.negate();
        }
        let mut edge1_segments = partition_of_circle(
            &e1.circle,
            point_of(e1.vertex[0]),
            psi,
            number_of_segments,
            true,
        )?;
        edge1_segments[n] = point_of(e1.vertex[1]);
        if e1.vertex[0] != Some(p1) {
            Self::reversed_segments(&mut edge1_segments, n);
        }
        let center_circle = Circle::new(rsedge.center_of_torus, normal, rsedge.radius_of_torus);
        let mut centers = partition_of_circle(
            &center_circle,
            self.ses.ea[edge0].circle.p,
            psi,
            number_of_segments,
            true,
        )?;
        centers[n] = self.ses.ea[edge2].circle.p;
        let start = self.mesh.triangles.len();
        self.build_triangles(
            Some(edge0),
            Some(edge1),
            Some(edge2),
            Some(edge3),
            &centers,
            &edge1_segments,
            &edge3_segments,
            probe_radius,
        )?;
        self.orient_from(start, |t, mesh| {
            let v = t.vertex.map(|p| mesh.pa[p].point);
            let orth = (v[1] - v[0]).cross(v[2] - v[0]);
            is_greater(orth.dot(v[0]), orth.dot(centers[0]))
        });
        Ok(())
    }

    /// Swaps the first two vertices of all triangles from `start` if the test on the first holds.
    fn orient_from(&mut self, start: usize, test: impl Fn(&Triangle, &Mesh) -> bool) {
        let Some(&first) = self.mesh.triangles.get(start) else {
            return;
        };
        if test(&self.mesh.ta[first], &self.mesh) {
            for i in start..self.mesh.triangles.len() {
                let t = self.mesh.triangles[i];
                self.mesh.ta[t].vertex.swap(0, 1);
            }
        }
    }

    fn triangulate_free_toric_face(&mut self, face: SF, probe_radius: f64) -> BallResult<()> {
        let f = &self.ses.fa[face];
        let front = *f
            .edge
            .first()
            .ok_or("BALL: free toric face without edges")?;
        let back = *f.edge.last().ok_or("BALL: free toric face without edges")?;
        let rsedge = self.rs.ea[f.rsedge()].clone();
        let normal = self.ses.ea[front].circle.n;
        let circle1 = self.ses.ea[front].circle;
        let circle2 = Circle::new(
            self.ses.ea[back].circle.p,
            normal,
            self.ses.ea[back].circle.radius,
        );
        let circle3 = Circle::new(rsedge.center_of_torus, normal, rsedge.radius_of_torus);
        let mut orth = Vec3::new(normal.y, -normal.x, 0.0);
        if orth.eq_eps(Vec3::ZERO) {
            orth = Vec3::new(normal.z, 0.0, -normal.x);
        }
        orth.normalize()?;
        let p1 = circle1.p + (orth * circle1.radius);
        let p2 = circle2.p + (orth * circle2.radius);
        let p3 = circle3.p + (orth * circle3.radius);
        let mut number_of_points = round_size(2.0 * PI * circle1.radius * self.sqrt_density);
        if number_of_points == 0 {
            number_of_points += 1;
        }
        let phi = 2.0 * PI / number_of_points as f64;
        let mut points1 = partition_of_circle(&circle1, p1, phi, number_of_points, true)?;
        let mut points2 = partition_of_circle(&circle2, p2, phi, number_of_points, true)?;
        let mut centers = partition_of_circle(&circle3, p3, phi, number_of_points, true)?;
        points1.pop();
        points2.pop();
        centers.pop();
        self.build_triangles(
            None,
            Some(front),
            None,
            Some(back),
            &centers,
            &points1,
            &points2,
            probe_radius,
        )
    }

    fn triangulate_singular_toric_face(&mut self, face: SF, probe_radius: f64) -> BallResult<()> {
        self.ses.normalize_face(self.rs, face, true)?;
        let f = &self.ses.fa[face];
        let edge: [SE; 6] = f.edge[..6]
            .try_into()
            .map_err(|_| "BALL: malformed singular toric face")?;
        let vertex: [super::ses::SV; 6] = f.vertex[..6]
            .try_into()
            .map_err(|_| "BALL: malformed singular toric face")?;
        let rsedge = self.rs.ea[f.rsedge()].clone();
        let mut number_of_segments =
            round_size(rsedge.angle * self.ses.ea[edge[1]].circle.radius * self.sqrt_density);
        if number_of_segments == 0 {
            number_of_segments += 1;
        }
        let psi = rsedge.angle / number_of_segments as f64;
        let n = number_of_segments as usize;
        let mut offset = 0;
        for _ in 0..2 {
            let e0 = self.ses.ea[edge[offset]].clone();
            let point_of =
                |v: Option<super::ses::SV>| self.ses.va[v.expect("toric edge vertex")].point;
            let mut edge_segments = partition_of_circle(
                &e0.circle,
                point_of(e0.vertex[0]),
                psi,
                number_of_segments,
                true,
            )?;
            edge_segments[n] = point_of(e0.vertex[1]);
            let mut axis = e0.circle.n;
            if e0.vertex[0] != Some(vertex[offset]) {
                Self::reversed_segments(&mut edge_segments, n);
                axis.negate();
            }
            let point = vec![self.ses.va[vertex[1 + offset]].point; n + 1];
            let center_circle = Circle::new(rsedge.center_of_torus, axis, rsedge.radius_of_torus);
            let mut centers = partition_of_circle(
                &center_circle,
                self.ses.ea[edge[1 + offset]].circle.p,
                psi,
                number_of_segments,
                true,
            )?;
            centers[n] = self.ses.ea[edge[2 + offset]].circle.p;
            let start = self.mesh.triangles.len();
            self.build_triangles(
                Some(edge[1 + offset]),
                Some(edge[offset]),
                Some(edge[2 + offset]),
                None,
                &centers,
                &edge_segments,
                &point,
                probe_radius,
            )?;
            offset += 3;
            self.orient_from(start, |t, mesh| {
                let v = t.vertex.map(|p| mesh.pa[p].point);
                let orth = (v[1] - v[0]).cross(v[2] - v[0]);
                is_greater(orth.dot(v[0] - centers[0]), 0.0)
            });
        }
        Ok(())
    }

    fn partition_singular_edges(&mut self) -> BallResult<()> {
        let edges = self.ses.singular_edges.clone();
        for e in edges {
            if self.ses.ea[e].vertex[0].is_none() {
                self.partition_free_singular_edge(e)?;
            } else {
                self.partition_non_free_singular_edge(e)?;
            }
        }
        Ok(())
    }

    fn partition_free_singular_edge(&mut self, e: SE) -> BallResult<()> {
        let edge = self.ses.ea[e].clone();
        let phi = 2.0 * PI;
        let mut number_of_segments = round_size(phi * edge.circle.radius * self.sqrt_density);
        if number_of_segments == 0 {
            number_of_segments += 1;
        }
        let psi = phi / number_of_segments as f64;
        let mut points =
            partition_of_circle(&edge.circle, Vec3::ZERO, psi, number_of_segments, false)?;
        points.pop();
        let index = edge.index as usize;
        let mut p1 = self.mesh.new_point(points[0], edge.circle.p - points[0]);
        let p0 = p1;
        for &point in &points[1..] {
            let p2 = self.mesh.new_point(point, edge.circle.p - point);
            let te = self.mesh.new_linked_edge(p1, p2);
            self.edge[index].push(te);
            p1 = p2;
        }
        let te = self.mesh.new_linked_edge(p1, p0);
        self.edge[index].push(te);
        Ok(())
    }

    fn partition_non_free_singular_edge(&mut self, e: SE) -> BallResult<()> {
        let edge = self.ses.ea[e].clone();
        let v0 = edge.vertex[0].expect("checked by caller");
        let v1 = edge.vertex[1].ok_or("BALL: singular edge without end vertex")?;
        let (pv0, pv1) = (self.ses.va[v0].point, self.ses.va[v1].point);
        let phi = oriented_angle(pv0 - edge.circle.p, pv1 - edge.circle.p, edge.circle.n)?;
        let mut number_of_segments = round_size(phi * edge.circle.radius * self.sqrt_density);
        if number_of_segments == 0 {
            number_of_segments += 1;
        }
        let psi = phi / number_of_segments as f64;
        let mut points = partition_of_circle(&edge.circle, pv0, psi, number_of_segments, true)?;
        points.pop();
        points.push(pv1);
        let index = edge.index as usize;
        let mut p1 = self.pt(v0);
        self.mesh.pa[p1].normal = edge.circle.p - self.mesh.pa[p1].point;
        for &point in &points[1..points.len() - 1] {
            let p2 = self.mesh.new_point(point, edge.circle.p - point);
            let te = self.mesh.new_linked_edge(p1, p2);
            self.edge[index].push(te);
            p1 = p2;
        }
        let p2 = self.pt(v1);
        self.mesh.pa[p2].normal = edge.circle.p - self.mesh.pa[p2].point;
        let te = self.mesh.new_linked_edge(p1, p2);
        self.edge[index].push(te);
        Ok(())
    }

    fn triangulate_contact_faces(&mut self) -> BallResult<()> {
        for i in 0..self.ses.contact_faces.len() {
            let face = self.ses.contact(i);
            let atom = self.rs.va[self.ses.fa[face].rsvertex.expect("contact face RS vertex")].atom;
            let sphere = self.rs.atom[atom as usize];
            self.triangulate_contact_face(face, &sphere)?;
        }
        Ok(())
    }

    fn edge_list_len(&self, e: SE) -> usize {
        self.edge[self.ses.ea[e].index as usize].len()
    }

    fn triangulate_contact_face(&mut self, face: SF, sphere: &Sphere) -> BallResult<()> {
        let edges = self.ses.fa[face].edge.clone();
        if edges.is_empty() {
            let mut part = Sphere3::icosaeder(true);
            part.refine(
                number_of_refinements(self.density, sphere.radius) as u32,
                true,
            )?;
            let base: Vec<TP> = part
                .points
                .iter()
                .zip(&part.normals)
                .map(|(&p, &n)| {
                    let mut p = p;
                    p *= sphere.radius;
                    p += sphere.p;
                    self.mesh.new_point(p, n)
                })
                .collect();
            for t in &part.triangles {
                self.mesh.add_triangle([base[t[0]], base[t[1]], base[t[2]]]);
            }
            return Ok(());
        }
        let (front, back) = (edges[0], edges[edges.len() - 1]);
        if edges.len() == 2 && self.edge_list_len(front) == 1 && self.edge_list_len(back) == 1 {
            return Ok(());
        }
        if edges.len() == 3 && self.edge_list_len(front) == 1 && self.edge_list_len(back) == 1 {
            let mut points: Vec<TP> = Vec::new();
            for v in [
                self.ses.ea[front].vertex[0],
                self.ses.ea[front].vertex[1],
                self.ses.ea[back].vertex[0],
                self.ses.ea[back].vertex[1],
            ] {
                let p = self.pt(v.ok_or("BALL: contact edge without vertex")?);
                if !points.contains(&p) {
                    points.push(p);
                }
            }
            if points.len() < 3 {
                return Err("BALL: degenerate contact face".to_string());
            }
            let mut vertex = [points[0], points[1], points[2]];
            let pt = |p: TP| self.mesh.pa[p].point;
            let normal = (pt(vertex[0]) - pt(vertex[1])).cross(pt(vertex[0]) - pt(vertex[2]));
            if is_greater(normal.dot(sphere.p - pt(vertex[1])), 0.0) {
                vertex.swap(0, 1);
            }
            self.mesh.add_triangle(vertex);
            return Ok(());
        }
        let template = &self.templates[number_of_refinements(self.density, sphere.radius)];
        let mut candidates: Vec<(Vec3, Vec3)> = template
            .iter()
            .map(|&(p, n)| {
                let mut p = p;
                p *= sphere.radius;
                (p, n)
            })
            .collect();
        for &e in &edges {
            let circle = self.ses.ea[e].circle;
            let plane = Plane::new(circle.p - sphere.p, circle.n);
            let test_value = plane.n.dot(plane.p) + 0.05;
            candidates.retain(|(p, _)| !is_less_or_equal(plane.n.dot(*p), test_value));
        }
        let points: Vec<TP> = candidates
            .into_iter()
            .map(|(p, n)| {
                let mut p = p;
                p += sphere.p;
                self.mesh.pa.push(TPoint::new(p, n))
            })
            .collect();
        let mut part = Part {
            points,
            triangles: Vec::new(),
        };
        self.build_spheric_triangles(face, &mut part, sphere, true)?;
        part.points.retain(|&p| self.mesh.pa[p].faces != 0);
        self.mesh.points.extend(part.points);
        self.mesh.triangles.extend(part.triangles);
        Ok(())
    }

    fn triangulate_spheric_faces(&mut self) -> BallResult<()> {
        let mut sphere = Sphere::new(Vec3::ZERO, self.rs.probe_radius);
        let mut not_triangulated = std::collections::VecDeque::new();
        for i in 0..self.ses.spheric_faces.len() {
            let face = self.ses.spheric(i);
            sphere.p = self.rs.fa[self.ses.fa[face].rsface()].center;
            if !self.triangulate_spheric_face(face, &sphere)? {
                not_triangulated.push_back(face);
            }
        }
        if !not_triangulated.is_empty() {}
        let mut end = not_triangulated.len();
        let mut counter = 0;
        while counter < end {
            let Some(face) = not_triangulated.pop_front() else {
                break;
            };
            let mut ok = false;
            let edges = self.ses.fa[face].edge.clone();
            for e in edges {
                if ok {
                    break;
                }
                let list = &self.edge[self.ses.ea[e].index as usize];
                let first = *list
                    .first()
                    .ok_or("BALL: SES edge without triangle edges")?;
                if self.mesh.ea[first].face[0].is_some() {
                    sphere.p = self.rs.fa[self.ses.fa[face].rsface()].center;
                    let ty = self.ses.ea[e].ty;
                    self.ses.ea[e].ty = EdgeType::Concave;
                    let result = self.triangulate_spheric_face(face, &sphere);
                    self.ses.ea[e].ty = ty;
                    ok = result?;
                }
            }
            if ok {
                end -= 1;
                counter = 0;
            } else {
                not_triangulated.push_back(face);
                counter += 1;
            }
        }
        Ok(())
    }

    fn triangulate_spheric_face(&mut self, face: SF, sphere: &Sphere) -> BallResult<bool> {
        let mut part = Part {
            points: Vec::new(),
            triangles: Vec::new(),
        };
        let ok = self.build_spheric_triangles(face, &mut part, sphere, false)?;
        if ok {
            self.mesh.points.extend(part.points);
            self.mesh.triangles.extend(part.triangles);
        }
        Ok(ok)
    }

    fn build_spheric_triangles(
        &mut self,
        face: SF,
        part: &mut Part,
        sphere: &Sphere,
        convex: bool,
    ) -> BallResult<bool> {
        let mut points: Vec<TP> = Vec::new();
        let mut seen: HashSet<TP> = HashSet::new();
        for &p in &part.points {
            if seen.insert(p) {
                points.push(p);
            }
        }
        for &e in &self.ses.fa[face].edge {
            for &c in &self.edge[self.ses.ea[e].index as usize] {
                for v in self.mesh.ea[c].vertex {
                    if seen.insert(v) {
                        points.push(v);
                    }
                }
            }
        }
        let mut border: std::collections::VecDeque<TE> = std::collections::VecDeque::new();
        if !self.build_first_triangle(face, part, &mut border, sphere, &points, convex)? {
            return Ok(false);
        }
        while let Some(edge) = border.pop_front() {
            let start_triangle =
                self.mesh.ea[edge].face[0].ok_or("BALL: border edge without triangle")?;
            let [ev0, ev1] = self.mesh.ea[edge].vertex;
            let tri = &self.mesh.ta[start_triangle];
            let third_point = tri.third(ev0, ev1);
            let diff = tri.relative_index(ev1) - tri.relative_index(ev0);
            if (convex && (diff == -1 || diff == 2)) || (!convex && (diff == 1 || diff == -2)) {
                self.mesh.ea[edge].vertex.swap(0, 1);
            }
            let [v0, v1] = self.mesh.ea[edge].vertex;
            let (p0, p1) = (self.mesh.pa[v0].point, self.mesh.pa[v1].point);
            let Some(start) = points
                .iter()
                .position(|&p| p != v0 && p != v1 && p != third_point)
            else {
                continue;
            };
            let next = points[start];
            let mut third = vec![next];
            let q = self.mesh.pa[next].point;
            let mut normal = (q - p1).cross(q - p0);
            let mut test_value = normal.dot(p0);
            for &candidate in &points[start + 1..] {
                if candidate != v0 && candidate != v1 && candidate != third_point {
                    let q = self.mesh.pa[candidate].point;
                    let this_value = normal.dot(q);
                    if is_greater(this_value, test_value) {
                        third.clear();
                        third.push(candidate);
                        normal = (q - p1).cross(q - p0);
                        test_value = normal.dot(p0);
                    } else if is_equal(this_value, test_value) {
                        third.push(candidate);
                    }
                }
            }
            match third.len() {
                0 => {}
                1 => self.build_unambiguous_triangle(
                    edge,
                    third[0],
                    &mut border,
                    sphere,
                    part,
                    convex,
                ),
                _ => self.build_ambiguous_triangles(edge, third, &mut border, sphere, part, convex),
            }
        }
        Ok(true)
    }

    fn build_first_triangle(
        &mut self,
        face: SF,
        part: &mut Part,
        border: &mut std::collections::VecDeque<TE>,
        sphere: &Sphere,
        points: &[TP],
        convex: bool,
    ) -> BallResult<bool> {
        let Some(first_sesedge) = self.first_ses_edge(face) else {
            return Ok(false);
        };
        let list = &self.edge[self.ses.ea[first_sesedge].index as usize];
        let edge = list[0];
        let mut same_edge: Vec<TP> = Vec::new();
        for &e in list {
            for v in self.mesh.ea[e].vertex {
                if !same_edge.contains(&v) {
                    same_edge.push(v);
                }
            }
        }
        let [a, b] = self.mesh.ea[edge].vertex;
        let mut edge_vector = self.mesh.pa[a].point - self.mesh.pa[b].point;
        let triangle = self.mesh.ea[edge].face[0].ok_or("BALL: SES edge without triangle")?;
        let tri = &self.mesh.ta[triangle];
        let diff = tri.relative_index(b) - tri.relative_index(a);
        if diff == 1 || diff == -2 {
            self.mesh.ea[edge].vertex.swap(0, 1);
            edge_vector = -edge_vector;
        }
        let [v0, v1] = self.mesh.ea[edge].vertex;
        let p0 = self.mesh.pa[v0].point;
        let mut third = Vec::new();
        for &next in points {
            if same_edge.contains(&next) {
                continue;
            }
            let normal = edge_vector.cross(p0 - self.mesh.pa[next].point);
            let test_value = normal.dot(p0);
            let mut is_convex = true;
            for &test in points {
                if test != next && test != v0 && test != v1 {
                    let this_value = normal.dot(self.mesh.pa[test].point);
                    if (is_greater(this_value, test_value) && convex)
                        || (is_less(this_value, test_value) && !convex)
                    {
                        is_convex = false;
                    }
                }
            }
            if is_convex {
                third.push(next);
            }
        }
        let mut real_third = Vec::new();
        if third.len() == 1 {
            real_third.push(third[0]);
        } else {
            let t = self.mesh.ta[triangle].vertex.map(|p| self.mesh.pa[p].point);
            let normal = (t[0] - t[1]).cross(t[0] - t[2]);
            let mut min = 3.0 * PI;
            for &p in &third {
                let new_normal = edge_vector.cross(p0 - self.mesh.pa[p].point);
                let angle = oriented_angle(normal, new_normal, edge_vector)?;
                if is_less_or_equal(angle, min) {
                    if is_less(angle, min) {
                        real_third.clear();
                        min = angle;
                    }
                    real_third.push(p);
                }
            }
        }
        match real_third.len() {
            0 => {}
            1 => self.build_unambiguous_triangle(edge, real_third[0], border, sphere, part, convex),
            _ => self.build_ambiguous_triangles(edge, real_third, border, sphere, part, convex),
        }
        Ok(true)
    }

    fn first_ses_edge(&self, face: SF) -> Option<SE> {
        let edges = &self.ses.fa[face].edge;
        let mut i = 0;
        while i < edges.len() {
            let mut found = false;
            while !found && i < edges.len() {
                if self.ses.ea[edges[i]].ty == EdgeType::Singular {
                    i += 1;
                } else {
                    found = true;
                }
            }
            if found {
                let list = &self.edge[self.ses.ea[edges[i]].index as usize];
                if list.len() == 1 {
                    let [a, b] = self.mesh.ea[list[0]].vertex;
                    let diff = self.mesh.pa[a].point - self.mesh.pa[b].point;
                    if diff.dot(diff) < 0.01 {
                        i += 1;
                    } else {
                        return Some(edges[i]);
                    }
                } else {
                    return Some(edges[i]);
                }
            }
        }
        None
    }

    /// `createTriangleAndEdges` without allocating anything yet.
    fn candidate(&self, edge: TE, point: TP, sphere: &Sphere, convex: bool) -> Candidate {
        let [v0, v1] = self.mesh.ea[edge].vertex;
        let edge1 = self.mesh.find_edge(v0, [v0, point]).ok_or([v0, point]);
        let edge2 = self.mesh.find_edge(v1, [point, v1]).ok_or([point, v1]);
        let mut vertex = [v1, v0, point];
        let t = vertex.map(|p| self.mesh.pa[p].point);
        let test_vector = (t[1] - t[0]).cross(t[2] - t[0]);
        let test_value = test_vector.dot(sphere.p - t[0]);
        if (is_greater(test_value, 0.0) && convex) || (is_less(test_value, 0.0) && !convex) {
            vertex.swap(0, 1);
        }
        Candidate {
            vertex,
            edge1,
            edge2,
        }
    }

    /// Commits a candidate edge: an existing edge gets the triangle as its second face and
    /// leaves the border, a new one is linked, gets the triangle as first face and joins it.
    fn attach_edge(
        &mut self,
        edge: Result<TE, [TP; 2]>,
        triangle: TT,
        border: &mut std::collections::VecDeque<TE>,
        planar: Option<&mut std::collections::VecDeque<TE>>,
    ) {
        match edge {
            Ok(e) => {
                let te = &mut self.mesh.ea[e];
                if te.face[0].is_none() {
                    te.face[0] = Some(triangle);
                } else {
                    te.face[1] = Some(triangle);
                }
                border.retain(|&x| x != e);
                if let Some(planar) = planar {
                    planar.retain(|&x| x != e);
                }
            }
            Err([a, b]) => {
                let e = self.mesh.new_linked_edge(a, b);
                self.mesh.ea[e].face[0] = Some(triangle);
                border.push_back(e);
                if let Some(planar) = planar {
                    planar.push_back(e);
                }
            }
        }
    }

    fn build_unambiguous_triangle(
        &mut self,
        edge: TE,
        point: TP,
        border: &mut std::collections::VecDeque<TE>,
        sphere: &Sphere,
        part: &mut Part,
        convex: bool,
    ) {
        let c = self.candidate(edge, point, sphere, convex);
        let triangle = self.mesh.ta.push(Triangle { vertex: c.vertex });
        self.attach_edge(c.edge1, triangle, border, None);
        self.attach_edge(c.edge2, triangle, border, None);
        self.mesh.ea[edge].face[1] = Some(triangle);
        for v in c.vertex {
            self.mesh.pa[v].faces += 1;
        }
        part.triangles.push(triangle);
    }

    fn build_ambiguous_triangles(
        &mut self,
        edge: TE,
        mut points: Vec<TP>,
        border: &mut std::collections::VecDeque<TE>,
        sphere: &Sphere,
        part: &mut Part,
        convex: bool,
    ) {
        let [ev0, ev1] = self.mesh.ea[edge].vertex;
        points.push(ev0);
        points.push(ev1);
        let mut planar: std::collections::VecDeque<TE> = std::collections::VecDeque::new();
        planar.push_back(edge);
        while let Some(edge0) = planar.pop_front() {
            let [a, b] = self.mesh.ea[edge0].vertex;
            for &p in &points {
                if p == a || p == b {
                    continue;
                }
                let c = self.candidate(edge0, p, sphere, convex);
                let built = if edge0 == edge {
                    true
                } else {
                    let old =
                        &self.mesh.ta[self.mesh.ea[edge0].face[0].expect("planar edge triangle")];
                    let (i1, i2) = (old.relative_index(a), old.relative_index(b));
                    let back = i1 - i2 == 1 || i1 - i2 == -2;
                    let index = |v: TP| {
                        c.vertex
                            .iter()
                            .position(|&x| x == v)
                            .map_or(-1, |i| i as i32)
                    };
                    let (i1, i2) = (index(a), index(b));
                    if back {
                        i1 - i2 == -1 || i1 - i2 == 2
                    } else {
                        i1 - i2 == 1 || i1 - i2 == -2
                    }
                };
                if built {
                    let triangle = self.mesh.ta.push(Triangle { vertex: c.vertex });
                    self.mesh.ea[edge0].face[1] = Some(triangle);
                    for v in c.vertex {
                        self.mesh.pa[v].faces += 1;
                    }
                    self.attach_edge(c.edge1, triangle, border, Some(&mut planar));
                    self.attach_edge(c.edge2, triangle, border, Some(&mut planar));
                    border.retain(|&x| x != edge0);
                    part.triangles.push(triangle);
                    break;
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn build_triangles(
        &mut self,
        edge0: Option<SE>,
        edge1: Option<SE>,
        edge2: Option<SE>,
        edge3: Option<SE>,
        centers: &[Vec3],
        edge1_points: &[Vec3],
        edge3_points: &[Vec3],
        probe_radius: f64,
    ) -> BallResult<()> {
        let psi = angle_between(edge1_points[0] - centers[0], edge3_points[0] - centers[0])?
            .unwrap_or(0.0);
        let mut number_of_triangles = round_size(psi * probe_radius * self.sqrt_density);
        if number_of_triangles == 0 {
            number_of_triangles += 1;
        }
        let phi = psi / number_of_triangles as f64;
        let n_tri = number_of_triangles as usize;
        let n_seg = centers.len() - 1;
        let number_of_points = (n_seg + 1)
            .checked_mul(n_tri + usize::from(edge3.is_some()))
            .and_then(|n| n.checked_add(usize::from(edge3.is_none())))
            .filter(|&n| n <= 48_000_000usize.saturating_sub(self.mesh.points.len()))
            .ok_or("BALL surface exceeds the vertex budget")?;
        let mut points: Vec<Option<TP>> = vec![None; number_of_points];
        let ses_edge =
            |e: Option<SE>| -> SesEdge { self.ses.ea[e.expect("toric face edge")].clone() };
        if edge3.is_none() {
            let e0 = ses_edge(edge0);
            let v0 = e0.vertex[0].expect("toric edge vertex");
            let v1 = e0.vertex[1].expect("toric edge vertex");
            let index = if self.ses.va[v0].point.eq_eps(edge3_points[0]) {
                v0
            } else {
                v1
            };
            let top = self.pt(index);
            self.mesh.pa[top].normal = centers[n_seg] - self.mesh.pa[top].point;
            points[(n_seg + 1) * n_tri] = Some(top);
        }
        let end = if edge3.is_some() { n_tri + 1 } else { n_tri };
        let corner = |e: Option<SE>, at: Vec3, s: &Self| -> TP {
            let edge = &s.ses.ea[e.expect("toric face edge")];
            let v0 = edge.vertex[0].expect("toric edge vertex");
            let v1 = edge.vertex[1].expect("toric edge vertex");
            let dist1 = at.square_distance(s.ses.va[v0].point);
            let dist2 = at.square_distance(s.ses.va[v1].point);
            s.pt(if dist1 < dist2 { v0 } else { v1 })
        };
        for i in 0..=n_seg {
            let circle = Circle::new(
                centers[i],
                (edge1_points[i] - centers[i]).cross(edge3_points[i] - centers[i]),
                probe_radius,
            );
            let line =
                partition_of_circle(&circle, edge1_points[i], phi, number_of_triangles, true)?;
            for j in 0..end {
                let mut existing = None;
                if edge0.is_some() {
                    if i == 0 && j == 0 {
                        existing = Some(corner(edge0, line[0], self));
                    }
                    if i == 0 && j == n_tri && edge3.is_some() {
                        existing = Some(corner(edge0, line[n_tri], self));
                    }
                    if i == n_seg && j == 0 {
                        existing = Some(corner(edge2, line[0], self));
                    }
                    if i == n_seg && j == n_tri && edge3.is_some() {
                        existing = Some(corner(edge2, line[n_tri], self));
                    }
                }
                let point = match existing {
                    Some(p) => p,
                    None => self.mesh.new_point(line[j], centers[i] - line[j]),
                };
                points[i * end + j] = Some(point);
            }
        }
        let points: Vec<TP> = points
            .into_iter()
            .map(|p| p.expect("toric grid point"))
            .collect();
        let index_of =
            |e: Option<SE>, s: &Self| s.ses.ea[e.expect("toric face edge")].index as usize;
        let edge_number = if edge3.is_some() {
            3 * n_seg * n_tri + n_seg + n_tri
        } else {
            (3 * n_seg * n_tri + n_tri)
                .checked_sub(2 * n_seg + 1)
                .ok_or("invalid BALL torus grid")?
        };
        let mut edges: Vec<Option<TE>> = vec![None; edge_number];
        for j in 0..=n_seg {
            let offset = j * end;
            for i in 0..end - 1 {
                let offset2 = offset + i;
                let new_edge = self
                    .mesh
                    .new_linked_edge(points[offset2], points[offset2 + 1]);
                if j == 0 && edge0.is_some() {
                    let k = index_of(edge0, self);
                    self.edge[k].push(new_edge);
                } else if j == n_seg && edge0.is_some() {
                    let k = index_of(edge2, self);
                    self.edge[k].push(new_edge);
                }
                edges[offset2 - j] = Some(new_edge);
            }
        }
        let add1 = (end - 1) * (n_seg + 1);
        for j in 0..n_seg {
            let offset = j * end;
            for i in 0..end {
                let offset2 = offset + i;
                let new_edge = self
                    .mesh
                    .new_linked_edge(points[offset2], points[offset2 + end]);
                if i == 0 && edge1.is_some() {
                    let k = index_of(edge1, self);
                    self.edge[k].push(new_edge);
                } else if i == end - 1 && edge3.is_some() {
                    let k = index_of(edge3, self);
                    self.edge[k].push(new_edge);
                }
                edges[offset2 + add1] = Some(new_edge);
            }
        }
        let add2 = add1 + end * n_seg;
        for j in 0..n_seg {
            let offset = j * end - j;
            for i in 0..end - 1 {
                let offset2 = offset + i;
                let new_edge = self
                    .mesh
                    .new_linked_edge(points[offset2 + j], points[offset2 + j + end + 1]);
                edges[offset2 + add2] = Some(new_edge);
            }
        }
        let edge_at = |k: usize| edges[k].expect("toric grid edge");
        for i in 0..end - 1 {
            for j in 0..n_seg {
                let offset2 = j * end + i;
                let t1 = self.mesh.add_triangle([
                    points[offset2],
                    points[offset2 + 1],
                    points[offset2 + 1 + end],
                ]);
                for v in self.mesh.ta[t1].vertex {
                    self.mesh.pa[v].faces += 1;
                }
                for k in [offset2 - j, offset2 + add1 + 1, offset2 - j + add2] {
                    self.mesh.ea[edge_at(k)].face[0] = Some(t1);
                }
                let t2 = self.mesh.add_triangle([
                    points[offset2],
                    points[offset2 + 1 + end],
                    points[offset2 + end],
                ]);
                for v in self.mesh.ta[t2].vertex {
                    self.mesh.pa[v].faces += 1;
                }
                for k in [offset2 - j + end - 1, offset2 + add1, offset2 - j + add2] {
                    self.mesh.ea[edge_at(k)].face[0] = Some(t2);
                }
            }
        }
        if edge0.is_none() {
            let offset = n_seg * end;
            let mut edge = self.mesh.new_linked_edge(points[offset], points[0]);
            let k = index_of(edge1, self);
            self.edge[k].push(edge);
            for i in 0..n_tri {
                let offset2 = offset + i;
                let t1 = self
                    .mesh
                    .add_triangle([points[offset2], points[i + 1], points[i]]);
                let first = edge;
                edge = self.mesh.new_linked_edge(points[offset2], points[i + 1]);
                for v in self.mesh.ta[t1].vertex {
                    self.mesh.pa[v].faces += 1;
                }
                for e in [first, edge, edge_at(i)] {
                    self.mesh.ea[e].face[0] = Some(t1);
                }
                let t2 =
                    self.mesh
                        .add_triangle([points[offset2], points[offset2 + 1], points[i + 1]]);
                let first = edge;
                let second = edge_at(offset2 - n_seg);
                edge = self
                    .mesh
                    .new_linked_edge(points[offset2 + 1], points[i + 1]);
                for v in self.mesh.ta[t2].vertex {
                    self.mesh.pa[v].faces += 1;
                }
                for e in [first, second, edge] {
                    self.mesh.ea[e].face[0] = Some(t2);
                }
            }
            let k = index_of(edge3, self);
            self.edge[k].push(edge);
        }
        if edge3.is_none() {
            let last = (n_seg + 1) * n_tri;
            let offset = (end - 1) * (n_seg + 2);
            let mut edge = self.mesh.new_linked_edge(points[n_tri - 1], points[last]);
            let k = index_of(edge0, self);
            self.edge[k].push(edge);
            for i in 0..n_seg {
                let t = self.mesh.add_triangle([
                    points[n_tri * (i + 1) - 1],
                    points[last],
                    points[n_tri * (i + 2) - 1],
                ]);
                let first = edge;
                let second = edge_at(offset + i * end);
                edge = self
                    .mesh
                    .new_linked_edge(points[n_tri * (i + 2) - 1], points[last]);
                if i == n_seg - 1 {
                    let k = index_of(edge2, self);
                    self.edge[k].push(edge);
                }
                for v in self.mesh.ta[t].vertex {
                    self.mesh.pa[v].faces += 1;
                }
                for e in [first, second, edge] {
                    self.mesh.ea[e].face[0] = Some(t);
                }
            }
        }
        Ok(())
    }
}

/// Builds the triangulation and returns interleaved point/normal data and triangle indices.
pub(super) fn triangulate(
    rs: &mut ReducedSurface,
    ses: &mut Ses,
    density: f64,
) -> BallResult<(Vec<f64>, Vec<u32>)> {
    let mut triangulator = SesTriangulator::new(rs, ses, density);
    triangulator.run()?;
    let mesh = &triangulator.mesh;
    let mut index = vec![u32::MAX; 0];
    let mut vertices = Vec::with_capacity(mesh.points.len() * 6);
    let mut lookup: std::collections::HashMap<TP, u32> =
        std::collections::HashMap::with_capacity(mesh.points.len());
    for (i, &p) in mesh.points.iter().enumerate() {
        let point = &mesh.pa[p];
        vertices.extend([
            point.point.x,
            point.point.y,
            point.point.z,
            point.normal.x,
            point.normal.y,
            point.normal.z,
        ]);
        lookup.insert(p, i as u32);
    }
    index.reserve(mesh.triangles.len() * 3);
    for &t in &mesh.triangles {
        for v in mesh.ta[t].vertex {
            index.push(lookup.get(&v).copied().unwrap_or(u32::MAX));
        }
    }
    let _ = FaceType::Contact;
    Ok((vertices, index))
}
