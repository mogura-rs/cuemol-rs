//! BALL `ReducedSurface` / `RSComputer` (`reducedSurface.cpp`, `RSVertex`, `RSEdge`, `RSFace`).

use super::geom::*;
use super::util::{define_id, impl_arena_id, Arena, HashGrid, OrderedSet};
use super::BallResult;
use std::collections::{BTreeSet, HashMap, VecDeque};

define_id!(RV, RE, RF);
impl_arena_id!(RV, RE, RF);

pub(super) struct RsVertex {
    pub atom: i32,
    pub index: i32,
    pub edges: OrderedSet<RE>,
    pub faces: OrderedSet<RF>,
}

impl RsVertex {
    fn new(atom: i32) -> Self {
        Self {
            atom,
            index: -1,
            edges: OrderedSet::new(),
            faces: OrderedSet::new(),
        }
    }
}

#[derive(Clone)]
pub(super) struct RsEdge {
    pub vertex: [Option<RV>; 2],
    pub face: [Option<RF>; 2],
    pub center_of_torus: Vec3,
    pub radius_of_torus: f64,
    pub angle: f64,
    pub circle0: Circle,
    pub circle1: Circle,
    pub intersection_point0: Vec3,
    pub intersection_point1: Vec3,
    pub singular: bool,
    pub index: i32,
}

impl Default for RsEdge {
    fn default() -> Self {
        Self {
            vertex: [None; 2],
            face: [None; 2],
            center_of_torus: Vec3::ZERO,
            radius_of_torus: 0.0,
            angle: 0.0,
            circle0: Circle::default(),
            circle1: Circle::default(),
            intersection_point0: Vec3::ZERO,
            intersection_point1: Vec3::ZERO,
            singular: false,
            index: -1,
        }
    }
}

impl RsEdge {
    pub fn is_free(&self) -> bool {
        self.face[0].is_none()
    }
    /// `RSEdge::getIntersectionPoint`.
    pub fn intersection_point(&self, i: usize) -> BallResult<Vec3> {
        if !self.singular {
            return Err("BALL: RSEdge has no intersection points".to_string());
        }
        Ok(if i == 0 {
            self.intersection_point0
        } else {
            self.intersection_point1
        })
    }
    /// `GraphEdge::remove(face)`.
    fn remove_face(&mut self, face: RF) -> Option<RF> {
        if self.face[1] == Some(face) {
            self.face[1] = None;
        } else if self.face[0] == Some(face) {
            self.face[0] = self.face[1];
            self.face[1] = None;
        }
        self.face[0]
    }
    fn substitute_vertex(&mut self, old: RV, new: RV) {
        if self.vertex[0] == Some(old) {
            self.vertex[0] = Some(new);
        } else if self.vertex[1] == Some(old) {
            self.vertex[1] = Some(new);
        }
    }
    pub fn other_face(&self, face: RF) -> BallResult<Option<RF>> {
        if self.face[0] == Some(face) {
            Ok(self.face[1])
        } else if self.face[1] == Some(face) {
            Ok(self.face[0])
        } else {
            Err("BALL: face is not incident to edge".to_string())
        }
    }
}

#[derive(Clone)]
pub(super) struct RsFace {
    pub vertex: [RV; 3],
    pub edge: [Option<RE>; 3],
    pub center: Vec3,
    pub normal: Vec3,
    pub singular: bool,
    pub index: i32,
}

impl RsFace {
    /// `GraphTriangle::third(v1, v2)`.
    pub fn third(&self, v1: RV, v2: RV) -> RV {
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
    fn substitute_vertex(&mut self, old: RV, new: RV) {
        for v in &mut self.vertex {
            if *v == old {
                *v = new;
                return;
            }
        }
    }
}

pub(super) struct ReducedSurface {
    pub atom: Vec<Sphere>,
    pub probe_radius: f64,
    pub vertices: Vec<Option<RV>>,
    pub edges: Vec<Option<RE>>,
    pub faces: Vec<Option<RF>>,
    pub r_max: f64,
    pub bbox_min: Vec3,
    pub bbox_max: Vec3,
    pub va: Arena<RV, RsVertex>,
    pub ea: Arena<RE, RsEdge>,
    pub fa: Arena<RF, RsFace>,
}

impl ReducedSurface {
    pub fn new(spheres: Vec<Sphere>, probe_radius: f64) -> Self {
        Self {
            atom: spheres,
            probe_radius,
            vertices: Vec::new(),
            edges: Vec::new(),
            faces: Vec::new(),
            r_max: 0.0,
            bbox_min: Vec3::ZERO,
            bbox_max: Vec3::ZERO,
            va: Arena::new(),
            ea: Arena::new(),
            fa: Arena::new(),
        }
    }

    pub fn compute(&mut self) -> BallResult<()> {
        RsComputer::new(self).run()
    }

    pub fn vertex(&self, i: usize) -> RV {
        self.vertices[i].expect("compacted RS vertex")
    }
    pub fn edge(&self, i: usize) -> RE {
        self.edges[i].expect("compacted RS edge")
    }
    pub fn face(&self, i: usize) -> RF {
        self.faces[i].expect("compacted RS face")
    }

    fn insert_vertex(&mut self, v: RV) {
        self.va[v].index = self.vertices.len() as i32;
        self.vertices.push(Some(v));
    }
    fn insert_edge(&mut self, e: RE) {
        self.ea[e].index = self.edges.len() as i32;
        self.edges.push(Some(e));
    }
    fn insert_face(&mut self, f: RF) {
        self.fa[f].index = self.faces.len() as i32;
        self.faces.push(Some(f));
    }

    /// `RSFace::RSFace(...)`, which normalises the normal.
    fn new_face(&mut self, vertex: [RV; 3], center: Vec3, normal: Vec3) -> BallResult<RF> {
        Ok(self.fa.push(RsFace {
            vertex,
            edge: [None; 3],
            center,
            normal: normal.normalized()?,
            singular: false,
            index: -1,
        }))
    }

    /// `RSVertex::operator ==`.
    fn vertices_equal(&self, a: RV, b: RV) -> bool {
        let (va, vb) = (&self.va[a], &self.va[b]);
        va.atom == vb.atom
            && va.edges.iter().all(|e| vb.edges.has(e))
            && vb.edges.iter().all(|e| va.edges.has(e))
            && va.faces.iter().all(|f| vb.faces.has(f))
            && vb.faces.iter().all(|f| va.faces.has(f))
    }

    /// `GraphVertex::join` followed by `GraphVertex::substitute` for vertices of one atom.
    fn join_and_substitute(&mut self, keep: RV, drop: RV) {
        let drop_edges = self.va[drop].edges.items();
        let drop_faces = self.va[drop].faces.items();
        if self.va[keep].atom == self.va[drop].atom {
            for &e in &drop_edges {
                self.va[keep].edges.insert(e);
            }
            for &f in &drop_faces {
                self.va[keep].faces.insert(f);
            }
        }
        if self.va[drop].atom == self.va[keep].atom {
            for &e in &drop_edges {
                self.ea[e].substitute_vertex(drop, keep);
            }
            for &f in &drop_faces {
                self.fa[f].substitute_vertex(drop, keep);
            }
        }
    }

    /// `RSFace::operator ==`.
    fn faces_equal(&self, a: RF, b: RF) -> bool {
        let (fa, fb) = (&self.fa[a], &self.fa[b]);
        fa.center.eq_eps(fb.center) && self.faces_similar(a, b)
    }

    /// `RSFace::operator *=` (same atoms in any order).
    pub fn faces_similar(&self, a: RF, b: RF) -> bool {
        let x = self.fa[a].vertex.map(|v| self.va[v].atom);
        let y = self.fa[b].vertex.map(|v| self.va[v].atom);
        (x[0] == y[0] && x[1] == y[1] && x[2] == y[2])
            || (x[0] == y[0] && x[1] == y[2] && x[2] == y[1])
            || (x[0] == y[1] && x[1] == y[0] && x[2] == y[2])
            || (x[0] == y[1] && x[1] == y[2] && x[2] == y[0])
            || (x[0] == y[2] && x[1] == y[0] && x[2] == y[1])
            || (x[0] == y[2] && x[1] == y[1] && x[2] == y[0])
    }

    /// `RSEdge::operator *=`.
    fn edges_similar(&self, a: RE, b: RE) -> bool {
        let atom = |v: Option<RV>| self.va[v.expect("RS edge vertex")].atom;
        let (ea, eb) = (&self.ea[a], &self.ea[b]);
        (atom(ea.vertex[0]) == atom(eb.vertex[0]) && atom(ea.vertex[1]) == atom(eb.vertex[1]))
            || (atom(ea.vertex[0]) == atom(eb.vertex[1])
                && atom(ea.vertex[1]) == atom(eb.vertex[0]))
    }

    /// `GraphTriangle::getEdges(vertex, edge1, edge2)`.
    pub fn face_edges_of(&self, f: RF, v: RV) -> (Option<RE>, Option<RE>) {
        let mut found = Vec::with_capacity(2);
        for e in self.fa[f].edge.into_iter().flatten() {
            if found.len() == 2 {
                break;
            }
            let edge = &self.ea[e];
            if edge.vertex[0] == Some(v) || edge.vertex[1] == Some(v) {
                found.push(e);
            }
        }
        (found.first().copied(), found.get(1).copied())
    }

    /// `GraphTriangle::getEdge(v1, v2, edge)`.
    pub fn face_edge_between(&self, f: RF, v1: RV, v2: RV) -> Option<RE> {
        self.fa[f].edge.into_iter().flatten().find(|&e| {
            let edge = &self.ea[e];
            (edge.vertex[0] == Some(v1) && edge.vertex[1] == Some(v2))
                || (edge.vertex[0] == Some(v2) && edge.vertex[1] == Some(v1))
        })
    }

    /// `ReducedSurface::clean`.
    pub fn clean(&mut self) {
        fn compact<I: Copy>(list: &mut Vec<Option<I>>, mut set_index: impl FnMut(I, i32)) {
            while matches!(list.last(), Some(None)) {
                list.pop();
            }
            let mut i = 0;
            while i < list.len() {
                if list[i].is_none() {
                    let last = list.pop().flatten();
                    list[i] = last;
                    if let Some(item) = last {
                        set_index(item, i as i32);
                    }
                    while matches!(list.last(), Some(None)) {
                        list.pop();
                    }
                }
                i += 1;
            }
        }
        let (va, ea, fa) = (&mut self.va, &mut self.ea, &mut self.fa);
        compact(&mut self.vertices, |v, i| va[v].index = i);
        compact(&mut self.edges, |e, i| ea[e].index = i);
        compact(&mut self.faces, |f, i| fa[f].index = i);
    }

    /// `ReducedSurface::deleteSimilarFaces`.
    pub fn delete_similar_faces(&mut self, face1: RF, face2: RF) -> BallResult<()> {
        if !self.faces_similar(face1, face2) {
            return Ok(());
        }
        let rsedge1 = self.fa[face1].edge;
        let mut rsedge2: [Option<RE>; 3] = [None; 3];
        for j in 0..3 {
            for i in 0..3 {
                let edge = self.fa[face2].edge[i].expect("RS face edge");
                if self.edges_similar(edge, rsedge1[j].expect("RS face edge")) {
                    rsedge2[j] = Some(edge);
                }
            }
        }
        let rsvertex1 = self.fa[face1].vertex;
        let mut rsvertex2: [Option<RV>; 3] = [None; 3];
        for j in 0..3 {
            for i in 0..3 {
                let vertex = self.fa[face2].vertex[i];
                if self.va[vertex].atom == self.va[rsvertex1[j]].atom {
                    rsvertex2[j] = Some(vertex);
                }
            }
        }
        for i in 0..3 {
            let (v1, v2) = (rsvertex1[i], rsvertex2[i].ok_or("BALL: no similar vertex")?);
            if v1 != v2 {
                self.join_and_substitute(v1, v2);
                let index = self.va[v2].index as usize;
                self.vertices[index] = None;
            }
            self.va[v1].faces.erase(face1);
            self.va[v1].faces.erase(face2);
        }
        for i in 0..3 {
            let e1 = rsedge1[i].expect("RS face edge");
            let e2 = rsedge2[i].ok_or("BALL: no similar edge")?;
            self.correct_edges(face1, face2, e1, e2)?;
        }
        let i1 = self.fa[face1].index as usize;
        let i2 = self.fa[face2].index as usize;
        self.faces[i1] = None;
        self.faces[i2] = None;
        Ok(())
    }

    fn correct_edges(&mut self, face1: RF, face2: RF, edge1: RE, edge2: RE) -> BallResult<()> {
        if edge1 == edge2 {
            if self.ea[edge1].singular {
                let [v0, v1] = self.ea[edge1].vertex;
                self.va[v0.expect("RS edge vertex")].edges.erase(edge1);
                self.va[v1.expect("RS edge vertex")].edges.erase(edge1);
                let index = self.ea[edge1].index as usize;
                self.edges[index] = None;
            } else {
                self.ea[edge1].face = [None, None];
                self.ea[edge1].angle = 2.0 * PI;
            }
        } else {
            let neighbour2 = self.ea[edge2].other_face(face2)?;
            if self.ea[edge1].face[0] == Some(face1) {
                self.ea[edge1].face[0] = neighbour2;
            } else {
                self.ea[edge1].face[1] = neighbour2;
            }
            let n2 = neighbour2.ok_or("BALL: missing neighbour face")?;
            for j in 0..3 {
                if self.fa[n2].edge[j] == Some(edge2) {
                    self.fa[n2].edge[j] = Some(edge1);
                }
            }
            let [v0, v1] = self.ea[edge2].vertex;
            self.va[v0.expect("RS edge vertex")].edges.erase(edge2);
            self.va[v1.expect("RS edge vertex")].edges.erase(edge2);
            let index = self.ea[edge2].index as usize;
            self.edges[index] = None;
            let n1 = self.ea[edge1].face[0].ok_or("BALL: missing neighbour face")?;
            let n2 = self.ea[edge1].face[1].ok_or("BALL: missing neighbour face")?;
            let [a, b] = self.ea[edge1].vertex;
            let angle = self.get_angle(
                n1,
                n2,
                a.expect("RS edge vertex"),
                b.expect("RS edge vertex"),
            )?;
            self.ea[edge1].angle = angle;
        }
        Ok(())
    }

    /// `ReducedSurface::getAngle` without the optional incidence check.
    fn get_angle(&self, face1: RF, face2: RF, vertex1: RV, vertex2: RV) -> BallResult<f64> {
        let vertex3 = self.fa[face1].third(vertex1, vertex2);
        let mut atom1 = self.atom[self.va[vertex1].atom as usize];
        let mut atom2 = self.atom[self.va[vertex2].atom as usize];
        let atom3 = self.atom[self.va[vertex3].atom as usize];
        let mut axis = atom1.p - atom2.p;
        let test = axis.cross(self.fa[face1].normal);
        if is_less(test.dot(atom1.p - atom3.p), 0.0) {
            axis.negate();
        }
        atom1.radius += self.probe_radius;
        atom2.radius += self.probe_radius;
        let circle = intersect_spheres(&atom1, &atom2).unwrap_or_default();
        let v1 = self.fa[face1].center - circle.p;
        let v2 = self.fa[face2].center - circle.p;
        oriented_angle(v1, v2, axis)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AtomStatus {
    OnSurface,
    Inside,
    Unknown,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProbeStatus {
    Ok,
    NotOk,
    NotTested,
}

#[derive(Clone, Copy)]
struct ProbePosition {
    status: [ProbeStatus; 2],
    point: [Vec3; 2],
}

type Pos2 = (u32, u32);
type Pos3 = (u32, u32, u32);

fn sorted2(a: i32, b: i32) -> Pos2 {
    let (mut a, mut b) = (a as u32, b as u32);
    if a > b {
        std::mem::swap(&mut a, &mut b);
    }
    (a, b)
}

fn sorted3(a: i32, b: i32, c: i32) -> Pos3 {
    let (mut a, mut b, mut c) = (a as u32, b as u32, c as u32);
    if a > b {
        std::mem::swap(&mut a, &mut b);
    }
    if a > c {
        std::mem::swap(&mut a, &mut c);
    }
    if b > c {
        std::mem::swap(&mut b, &mut c);
    }
    (a, b, c)
}

/// `std::set_intersection` of two ascending lists.
fn set_intersection(a: &[i32], b: &[i32]) -> Vec<i32> {
    let (mut i, mut j) = (0, 0);
    let mut out = Vec::new();
    while i < a.len() && j < b.len() {
        if a[i] < b[j] {
            i += 1;
        } else {
            if b[j] >= a[i] {
                out.push(a[i]);
                i += 1;
            }
            j += 1;
        }
    }
    out
}

struct ThirdAtom {
    atom: i32,
    probe: Sphere,
    phi: f64,
}

struct RsComputer<'a> {
    rs: &'a mut ReducedSurface,
    neighbours: Vec<Vec<i32>>,
    atom_status: Vec<AtomStatus>,
    neighbours_of_two: HashMap<Pos2, Vec<i32>>,
    probe_positions: HashMap<Pos3, Option<ProbePosition>>,
    new_vertices: BTreeSet<RV>,
    vertices: Vec<Vec<RV>>,
}

impl<'a> RsComputer<'a> {
    fn new(rs: &'a mut ReducedSurface) -> Self {
        let n = rs.atom.len();
        Self {
            rs,
            neighbours: vec![Vec::new(); n],
            atom_status: vec![AtomStatus::Unknown; n],
            neighbours_of_two: HashMap::new(),
            probe_positions: HashMap::new(),
            new_vertices: BTreeSet::new(),
            vertices: vec![Vec::new(); n],
        }
    }

    fn run(&mut self) -> BallResult<()> {
        let epsilon = set_eps(1e-4);
        self.pre_processing();
        let mut start = 1;
        while start != 0 {
            start = self.get_start_position()?;
            match start {
                2 => self.extend_component()?,
                3 => self.get_rs_component()?,
                _ => {}
            }
        }
        self.rs.clean();
        set_eps(epsilon);
        Ok(())
    }

    fn get_rs_component(&mut self) -> BallResult<()> {
        let mut i = 0;
        while i < self.rs.faces.len() {
            if let Some(face) = self.rs.faces[i] {
                if self.treat_face(face)? {
                    i += 1;
                } else {
                    i = 0;
                }
            } else {
                i += 1;
            }
        }
        self.extend_component()
    }

    fn treat_face(&mut self, face: RF) -> BallResult<bool> {
        for k in 0..3 {
            let edge = self.rs.fa[face].edge[k].expect("RS face edge");
            if self.rs.ea[edge].face[1].is_none() && !self.treat_edge(edge)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn treat_edge(&mut self, edge: RE) -> BallResult<bool> {
        let start_face = self.rs.ea[edge].face[0].expect("RS edge face");
        let vertex1 = self.rs.ea[edge].vertex[0].expect("RS edge vertex");
        let vertex2 = self.rs.ea[edge].vertex[1].expect("RS edge vertex");
        let atom1 = self.rs.va[vertex1].atom;
        let atom2 = self.rs.va[vertex2].atom;
        let Some(third) = self.third_atom(vertex1, vertex2, start_face)? else {
            return Ok(false);
        };
        let atom3 = third.atom;
        let probe = third.probe;
        let sphere1 = self.rs.atom[atom1 as usize];
        let sphere2 = self.rs.atom[atom2 as usize];
        let sphere3 = self.rs.atom[atom3 as usize];
        let vertex3 = self.rs.va.push(RsVertex::new(atom3));
        let normal = Self::face_normal(&sphere1, &sphere2, &sphere3, &probe);
        let mut new_face = self
            .rs
            .new_face([vertex1, vertex2, vertex3], probe.p, normal)?;
        let test = self.face_exists(new_face, self.rs.va[vertex1].atom);
        match test {
            None => {
                self.insert_vertex(vertex3);
                let edge1 = self.rs.ea.push(RsEdge {
                    vertex: [Some(vertex2), Some(vertex3)],
                    face: [Some(new_face), None],
                    ..RsEdge::default()
                });
                let edge2 = self.rs.ea.push(RsEdge {
                    vertex: [Some(vertex3), Some(vertex1)],
                    face: [Some(new_face), None],
                    ..RsEdge::default()
                });
                self.rs.fa[new_face].edge = [Some(edge), Some(edge1), Some(edge2)];
                let plane = Plane::from_points(sphere1.p, sphere2.p, sphere3.p);
                self.rs.fa[new_face].singular =
                    is_less(distance_point_plane(probe.p, &plane)?, self.rs.probe_radius);
                self.insert_face(new_face);
            }
            Some(test) => {
                let mut similar = None;
                for i in 0..3 {
                    let e = self.rs.fa[test].edge[i].expect("RS face edge");
                    if self.rs.edges_similar(e, edge) {
                        similar = Some((i, e));
                        break;
                    }
                }
                let (i, test_edge) = similar.ok_or("BALL: no similar RS edge")?;
                let mut test_vertex1 = self.rs.ea[test_edge].vertex[0].expect("RS edge vertex");
                let mut test_vertex2 = self.rs.ea[test_edge].vertex[1].expect("RS edge vertex");
                if self.rs.va[test_vertex1].atom == self.rs.va[vertex2].atom {
                    std::mem::swap(&mut test_vertex1, &mut test_vertex2);
                }
                for (vertex, test_vertex) in [(vertex1, test_vertex1), (vertex2, test_vertex2)] {
                    if !self.rs.vertices_equal(vertex, test_vertex) {
                        self.rs.join_and_substitute(vertex, test_vertex);
                        let index = self.rs.va[test_vertex].index as usize;
                        self.rs.vertices[index] = None;
                        self.new_vertices.remove(&test_vertex);
                        let atom = self.rs.va[test_vertex].atom as usize;
                        self.vertices[atom].retain(|&v| v != test_vertex);
                    }
                }
                self.rs.va[vertex1].edges.erase(test_edge);
                self.rs.va[vertex2].edges.erase(test_edge);
                self.rs.fa[test].edge[i] = Some(edge);
                if self.rs.ea[test_edge].index != -1 {
                    let index = self.rs.ea[test_edge].index as usize;
                    self.rs.edges[index] = None;
                }
                new_face = test;
            }
        }
        let (_, circle1, circle2, circle3) = self.get_circles(atom1, atom2);
        let mut ip1 = Vec3::ZERO;
        let mut ip2 = Vec3::ZERO;
        let line = Line::from_two_points(sphere1.p, sphere2.p);
        let singular = intersect_sphere_line(&probe, &line, &mut ip1, &mut ip2);
        if singular
            && is_less(
                ip1.square_distance(sphere2.p),
                ip2.square_distance(sphere2.p),
            )
        {
            std::mem::swap(&mut ip1, &mut ip2);
        }
        let e = &mut self.rs.ea[edge];
        e.face[1] = Some(new_face);
        e.center_of_torus = circle1.p;
        e.radius_of_torus = circle1.radius;
        e.angle = third.phi;
        e.circle0 = circle2;
        e.circle1 = circle3;
        e.intersection_point0 = ip1;
        e.intersection_point1 = ip2;
        e.singular = singular;
        if e.index == -1 {
            self.rs.insert_edge(edge);
        }
        Ok(true)
    }

    fn correct(&mut self, atom: i32) -> BallResult<()> {
        let list = atom as usize;
        let mut cursor = 0;
        while cursor < self.vertices[list].len() {
            let vertex = self.vertices[list][cursor];
            let next = self.vertices[list].get(cursor + 1).copied();
            let mut delete_edges: OrderedSet<RE> = OrderedSet::new();
            let mut test_vertices: OrderedSet<RV> = OrderedSet::new();
            let mut treat_faces: OrderedSet<RF> = OrderedSet::new();
            let faces = self.rs.va[vertex].faces.items();
            for &f in &faces {
                self.remove_face(f, &mut delete_edges, &mut test_vertices, &mut treat_faces);
            }
            for &f in &faces {
                treat_faces.erase(f);
                let index = self.rs.fa[f].index as usize;
                self.rs.faces[index] = None;
            }
            for f in treat_faces.items() {
                let index = self.rs.fa[f].index as usize;
                self.rs.faces[index] = None;
                self.rs.insert_face(f);
            }
            for e in delete_edges.items() {
                let index = self.rs.ea[e].index;
                if index != -1 {
                    self.rs.edges[index as usize] = None;
                }
            }
            test_vertices.erase(vertex);
            for t in test_vertices.items() {
                if self.rs.va[t].edges.is_empty() {
                    let index = self.rs.va[t].index as usize;
                    self.rs.vertices[index] = None;
                    let a = self.rs.va[t].atom as usize;
                    self.vertices[a].retain(|&v| v != t);
                    self.new_vertices.remove(&t);
                }
            }
            let index = self.rs.va[vertex].index as usize;
            self.rs.vertices[index] = None;
            self.vertices[list].retain(|&v| v != vertex);
            self.new_vertices.remove(&vertex);
            cursor = match next {
                Some(n) => match self.vertices[list].iter().position(|&v| v == n) {
                    Some(p) => p,
                    None => break,
                },
                None => self.vertices[list].len(),
            };
        }
        self.rs.atom[list].radius -= 10.0 * eps();
        self.atom_status[list] = AtomStatus::Unknown;
        self.correct_probe_positions_of_atom(atom as u32);
        Ok(())
    }

    /// `RSFace::remove(edges, vertices, faces)`.
    fn remove_face(
        &mut self,
        face: RF,
        edges: &mut OrderedSet<RE>,
        vertices: &mut OrderedSet<RV>,
        faces: &mut OrderedSet<RF>,
    ) {
        for v in self.rs.fa[face].vertex {
            self.rs.va[v].faces.erase(face);
        }
        for i in 0..3 {
            let Some(e) = self.rs.fa[face].edge[i] else {
                continue;
            };
            if self.rs.ea[e].face[1].is_none() {
                let [v0, v1] = self.rs.ea[e].vertex.map(|v| v.expect("RS edge vertex"));
                self.rs.va[v0].edges.erase(e);
                self.rs.va[v1].edges.erase(e);
                vertices.insert(v0);
                vertices.insert(v1);
                edges.insert(e);
                self.rs.fa[face].edge[i] = None;
            } else if let Some(other) = self.rs.ea[e].remove_face(face) {
                faces.insert(other);
            }
        }
    }

    fn extend_component(&mut self) -> BallResult<()> {
        let mut new_vertices: VecDeque<RV> = self.new_vertices.iter().copied().collect();
        while let Some(vertex1) = new_vertices.pop_front() {
            let mut face = None;
            let atom1 = self.rs.va[vertex1].atom;
            let mut i = 0;
            let mut stop = false;
            while !stop && i < self.neighbours[atom1 as usize].len() {
                let atom2 = self.neighbours[atom1 as usize][i];
                if self.atom_status[atom2 as usize] == AtomStatus::Unknown {
                    let s = self.neighbours_of_two_atoms(sorted2(atom1, atom2));
                    let candidates = self.find_third_atom(atom1, atom2, &s);
                    if candidates.is_empty() {
                        let vertex2 = self.rs.va.push(RsVertex::new(atom2));
                        if let Some(edge) = self.create_free_edge(vertex1, vertex2)? {
                            self.insert_edge(edge);
                            self.insert_vertex(vertex2);
                            new_vertices.push_back(vertex1);
                            new_vertices.push_back(vertex2);
                            break;
                        }
                    } else {
                        for &(atom3, probe) in &candidates {
                            if self.atom_status[atom3 as usize] == AtomStatus::Unknown
                                && self.check_probe(&probe, sorted3(atom1, atom2, atom3))
                            {
                                let vertex2 = self.rs.va.push(RsVertex::new(atom2));
                                let vertex3 = self.rs.va.push(RsVertex::new(atom3));
                                let f =
                                    self.update_face_and_edges(vertex1, vertex2, vertex3, &probe)?;
                                self.insert_face(f);
                                self.insert_vertex(vertex2);
                                self.insert_vertex(vertex3);
                                new_vertices.push_back(vertex1);
                                new_vertices.push_back(vertex2);
                                new_vertices.push_back(vertex3);
                                face = Some(f);
                                stop = true;
                                break;
                            }
                        }
                    }
                }
                i += 1;
            }
            if face.is_some() {
                self.get_rs_component()?;
            }
        }
        self.new_vertices.clear();
        Ok(())
    }

    fn third_atom(&mut self, vertex1: RV, vertex2: RV, face: RF) -> BallResult<Option<ThirdAtom>> {
        let atom1 = self.rs.va[vertex1].atom;
        let atom2 = self.rs.va[vertex2].atom;
        let s = self.neighbours_of_two_atoms(sorted2(atom1, atom2));
        let candidates = self.find_third_atom(atom1, atom2, &s);
        let mut old_angle = 3.0 * PI;
        let two_pi = 2.0 * PI;
        let mut axis = self.rs.atom[atom1 as usize].p - self.rs.atom[atom2 as usize].p;
        let test_vector = self.rs.fa[face].normal.cross(axis);
        let third_face_atom = self.rs.va[self.rs.fa[face].third(vertex1, vertex2)].atom;
        if is_less(
            test_vector.dot(self.rs.atom[third_face_atom as usize].p),
            test_vector.dot(self.rs.atom[atom1 as usize].p),
        ) {
            axis.negate();
        }
        let mut sphere1 = self.rs.atom[atom1 as usize];
        let mut sphere2 = self.rs.atom[atom2 as usize];
        sphere1.radius += self.rs.probe_radius;
        sphere2.radius += self.rs.probe_radius;
        let circle = intersect_spheres(&sphere1, &sphere2).unwrap_or_default();
        let start_probe = self.rs.fa[face].center;
        let v1 = start_probe - circle.p;
        let mut third: Vec<(i32, Sphere)> = Vec::new();
        for &k in &candidates {
            if k.0 != third_face_atom || k.1.p.ne_eps(start_probe) {
                let v2 = k.1.p - circle.p;
                let new_angle = oriented_angle(v1, v2, axis)?;
                if is_zero(new_angle) || is_equal(new_angle, two_pi) {
                    self.correct(k.0)?;
                    return Ok(None);
                }
                if is_less_or_equal(new_angle, old_angle) {
                    if is_less(new_angle, old_angle) {
                        old_angle = new_angle;
                        for t in &third {
                            if self.atom_status[t.0 as usize] == AtomStatus::Unknown {
                                self.atom_status[t.0 as usize] = AtomStatus::Inside;
                            }
                        }
                        third.clear();
                    }
                    third.push(k);
                } else if self.atom_status[k.0 as usize] == AtomStatus::Unknown {
                    self.atom_status[k.0 as usize] = AtomStatus::Inside;
                }
            }
        }
        if third.len() > 1 {
            for k in &third[1..] {
                self.correct(k.0)?;
            }
            return Ok(None);
        }
        let &(atom, probe) = third.first().ok_or("BALL: no third atom found")?;
        self.atom_status[atom as usize] = AtomStatus::OnSurface;
        Ok(Some(ThirdAtom {
            atom,
            probe,
            phi: old_angle,
        }))
    }

    fn get_start_position(&mut self) -> BallResult<u32> {
        if self.find_first_face()? {
            return Ok(3);
        }
        if self.find_first_edge()? {
            return Ok(2);
        }
        if self.find_first_vertex() {
            return Ok(1);
        }
        Ok(0)
    }

    fn find_first_face(&mut self) -> BallResult<bool> {
        for direction in 0..3 {
            if self.find_face(direction, 0)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn find_first_edge(&mut self) -> BallResult<bool> {
        for direction in 0..3 {
            if self.find_edge(direction, 0)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn find_first_vertex(&mut self) -> bool {
        for i in 0..self.rs.atom.len() {
            if self.atom_status[i] == AtomStatus::Unknown && self.neighbours[i].is_empty() {
                let vertex = self.rs.va.push(RsVertex::new(i as i32));
                self.insert_vertex(vertex);
                return true;
            }
        }
        false
    }

    fn find_face(&mut self, direction: usize, extrem: u32) -> BallResult<bool> {
        let Some(a1) = self.find_first_atom(direction, extrem) else {
            return Ok(false);
        };
        let Some(a2) = self.find_second_atom(a1, direction, extrem) else {
            return Ok(false);
        };
        let s = self.neighbours_of_two_atoms(sorted2(a1, a2));
        let candidates = self.find_third_atom(a1, a2, &s);
        if candidates.is_empty() {
            return Ok(false);
        }
        let mut found = None;
        for &(a3, probe) in &candidates {
            if self.atom_status[a3 as usize] == AtomStatus::Unknown
                && self.check_probe(&probe, sorted3(a1, a2, a3))
            {
                found = Some((a3, probe));
                break;
            }
        }
        if let Some((a3, probe)) = found {
            let vertex1 = self.rs.va.push(RsVertex::new(a1));
            let vertex2 = self.rs.va.push(RsVertex::new(a2));
            let vertex3 = self.rs.va.push(RsVertex::new(a3));
            let face = self.update_face_and_edges(vertex1, vertex2, vertex3, &probe)?;
            self.insert_face(face);
            self.insert_vertex(vertex1);
            self.insert_vertex(vertex2);
            self.insert_vertex(vertex3);
            Ok(true)
        } else {
            self.atom_status[a1 as usize] = AtomStatus::Inside;
            self.atom_status[a2 as usize] = AtomStatus::Inside;
            Ok(false)
        }
    }

    fn find_edge(&mut self, direction: usize, extrem: u32) -> BallResult<bool> {
        let Some(a1) = self.find_first_atom(direction, extrem) else {
            return Ok(false);
        };
        let Some(a2) = self.find_second_atom(a1, direction, extrem) else {
            return Ok(false);
        };
        let vertex1 = self.rs.va.push(RsVertex::new(a1));
        let vertex2 = self.rs.va.push(RsVertex::new(a2));
        self.neighbours_of_two_atoms(sorted2(a1, a2));
        if let Some(edge) = self.create_free_edge(vertex1, vertex2)? {
            self.insert_edge(edge);
            self.insert_vertex(vertex1);
            self.insert_vertex(vertex2);
            Ok(true)
        } else {
            self.neighbours[a1 as usize].retain(|&x| x != a2);
            self.neighbours[a2 as usize].retain(|&x| x != a1);
            Ok(false)
        }
    }

    fn find_first_atom(&self, direction: usize, extrem: u32) -> Option<i32> {
        let n = self.rs.atom.len();
        let mut i = (0..n).find(|&i| self.atom_status[i] == AtomStatus::Unknown)?;
        let extremum_of = |atom: &Sphere| {
            if extrem == 0 {
                atom.p.get(direction) - atom.radius
            } else {
                atom.p.get(direction) + atom.radius
            }
        };
        let mut extrem_atom = i;
        let mut extrem_value = extremum_of(&self.rs.atom[i]);
        i += 1;
        while i < n {
            if self.atom_status[i] == AtomStatus::Unknown {
                let extremum = extremum_of(&self.rs.atom[i]);
                if (extrem == 0 && is_less(extremum, extrem_value))
                    || (extrem != 0 && is_greater(extremum, extrem_value))
                {
                    extrem_value = extremum;
                    extrem_atom = i;
                }
            }
            i += 1;
        }
        Some(extrem_atom as i32)
    }

    fn find_second_atom(&self, atom: i32, direction: usize, extrem: u32) -> Option<i32> {
        let neighbours = &self.neighbours[atom as usize];
        let start = neighbours
            .iter()
            .position(|&n| self.atom_status[n as usize] == AtomStatus::Unknown)?;
        let mut second_atom = neighbours[start];
        let mut first_atom = self.rs.atom[atom as usize];
        first_atom.radius += self.rs.probe_radius;
        let mut extrem_value = if extrem == 0 {
            first_atom.p.get(direction) + first_atom.radius
        } else {
            first_atom.p.get(direction) - first_atom.radius
        };
        for &n in &neighbours[start..] {
            if self.atom_status[n as usize] == AtomStatus::Unknown {
                let mut next_atom = self.rs.atom[n as usize];
                next_atom.radius += self.rs.probe_radius;
                if let Some(circle) = intersect_spheres(&first_atom, &next_atom) {
                    let next_extrem = Self::circle_extremum(&circle, direction, extrem);
                    if (extrem == 0 && is_less(next_extrem, extrem_value))
                        || (extrem != 0 && is_greater(next_extrem, extrem_value))
                    {
                        extrem_value = next_extrem;
                        second_atom = n;
                    }
                }
            }
        }
        Some(second_atom)
    }

    fn find_third_atom(&mut self, atom1: i32, atom2: i32, third: &[i32]) -> Vec<(i32, Sphere)> {
        let mut atoms = Vec::new();
        let radius = self.rs.probe_radius;
        for &i in third {
            let mut center1 = Vec3::ZERO;
            let mut center2 = Vec3::ZERO;
            if self.center_of_probe(sorted3(atom1, atom2, i), &mut center1, &mut center2) {
                if !(center1.x.is_nan() || center1.y.is_nan() || center1.z.is_nan()) {
                    atoms.push((i, Sphere::new(center1, radius)));
                }
                if !(center2.x.is_nan() || center2.y.is_nan() || center2.z.is_nan()) {
                    atoms.push((i, Sphere::new(center2, radius)));
                }
            }
        }
        atoms
    }

    fn neighbours_of_two_atoms(&mut self, pos: Pos2) -> Vec<i32> {
        if let Some(list) = self.neighbours_of_two.get(&pos) {
            return list.clone();
        }
        let list = set_intersection(
            &self.neighbours[pos.0 as usize],
            &self.neighbours[pos.1 as usize],
        );
        self.neighbours_of_two.insert(pos, list.clone());
        list
    }

    fn neighbours_of_three_atoms(&mut self, atom1: i32, atom2: i32, atom3: i32) -> Vec<i32> {
        let s1 = self.neighbours_of_two_atoms(sorted2(atom1, atom2));
        let s2 = self.neighbours_of_two_atoms(sorted2(atom1, atom3));
        set_intersection(&s1, &s2)
    }

    fn circle_extremum(circle: &Circle, direction: usize, extrem: u32) -> f64 {
        let n = circle.n;
        let norm2 = Vec3::new(n.x * n.x, n.y * n.y, n.z * n.z);
        let (min, max) = match direction {
            0 => {
                if is_zero(n.y) && is_zero(n.z) {
                    (circle.p.x, circle.p.x)
                } else {
                    let mut x_norm = norm2.y + norm2.z;
                    x_norm /= norm2.x + x_norm;
                    x_norm = circle.radius * x_norm.sqrt();
                    (circle.p.x - x_norm, circle.p.x + x_norm)
                }
            }
            1 => {
                if is_zero(n.x) && is_zero(n.z) {
                    (circle.p.y, circle.p.y)
                } else {
                    let mut y_norm = norm2.x + norm2.z;
                    y_norm /= norm2.y + y_norm;
                    y_norm = circle.radius * y_norm.sqrt();
                    (circle.p.y - y_norm, circle.p.y + y_norm)
                }
            }
            _ => {
                if is_zero(n.x) && is_zero(n.y) {
                    (circle.p.z, circle.p.z)
                } else {
                    let mut z_norm = norm2.x + norm2.y;
                    z_norm /= norm2.z + z_norm;
                    z_norm = circle.radius * z_norm.sqrt();
                    (circle.p.z - z_norm, circle.p.z + z_norm)
                }
            }
        };
        if extrem == 0 {
            min
        } else {
            max
        }
    }

    fn create_free_edge(&mut self, vertex1: RV, vertex2: RV) -> BallResult<Option<RE>> {
        let atom1 = self.rs.va[vertex1].atom;
        let atom2 = self.rs.va[vertex2].atom;
        let (ok, circle1, circle2, circle3) = self.get_circles(atom1, atom2);
        if ok && is_greater(circle1.radius, self.rs.probe_radius) {
            let plane = Plane::new(circle1.p, circle1.n);
            let s = self.neighbours_of_two_atoms(sorted2(atom1, atom2));
            for &i in &s {
                let atom = self.rs.atom[i as usize];
                let sphere = Sphere::new(atom.p, atom.radius + self.rs.probe_radius);
                if let Some(test_circle) = intersect_sphere_plane(&sphere, &plane)? {
                    let radius_dist = test_circle.radius - circle1.radius;
                    let radius_sum = test_circle.radius + circle1.radius;
                    let center_dist = test_circle.p.square_distance(circle1.p);
                    if is_less_or_equal(radius_dist * radius_dist, center_dist)
                        && is_greater_or_equal(radius_sum * radius_sum, center_dist)
                    {
                        return Ok(None);
                    }
                }
            }
            let edge = self.rs.ea.push(RsEdge {
                vertex: [Some(vertex1), Some(vertex2)],
                face: [None, None],
                center_of_torus: circle1.p,
                radius_of_torus: circle1.radius,
                angle: 2.0 * PI,
                circle0: circle2,
                circle1: circle3,
                intersection_point0: Vec3::ZERO,
                intersection_point1: Vec3::ZERO,
                singular: false,
                index: -1,
            });
            return Ok(Some(edge));
        }
        Ok(None)
    }

    fn get_circles(&self, atom1: i32, atom2: i32) -> (bool, Circle, Circle, Circle) {
        let mut sphere1 = self.rs.atom[atom1 as usize];
        let mut sphere2 = self.rs.atom[atom2 as usize];
        sphere1.radius += self.rs.probe_radius;
        sphere2.radius += self.rs.probe_radius;
        let mut circle2 = Circle::default();
        let mut circle3 = Circle::default();
        if let Some(circle1) = intersect_spheres(&sphere1, &sphere2) {
            let ratio = self.rs.atom[atom1 as usize].radius / sphere1.radius;
            circle2.radius = circle1.radius * ratio;
            circle2.p = sphere1.p + (circle1.p - sphere1.p) * ratio;
            let ratio = self.rs.atom[atom2 as usize].radius / sphere2.radius;
            circle3.radius = circle1.radius * ratio;
            circle3.p = sphere2.p + (circle1.p - sphere2.p) * ratio;
            (true, circle1, circle2, circle3)
        } else {
            (false, Circle::default(), circle2, circle3)
        }
    }

    fn face_normal(atom1: &Sphere, atom2: &Sphere, atom3: &Sphere, probe: &Sphere) -> Vec3 {
        let plane = Plane::from_points(atom1.p, atom2.p, atom3.p);
        let mut norm = plane.n;
        if is_less(norm.dot(probe.p), norm.dot(atom1.p)) {
            norm.negate();
        }
        norm
    }

    fn update_face_and_edges(&mut self, v1: RV, v2: RV, v3: RV, probe: &Sphere) -> BallResult<RF> {
        let f = self.rs.fa.push(RsFace {
            vertex: [v1, v2, v3],
            edge: [None; 3],
            center: Vec3::ZERO,
            normal: Vec3::ZERO,
            singular: false,
            index: -1,
        });
        let edge = |a: RV, b: RV, rs: &mut ReducedSurface| {
            rs.ea.push(RsEdge {
                vertex: [Some(a), Some(b)],
                face: [Some(f), None],
                ..RsEdge::default()
            })
        };
        let e1 = edge(v1, v2, self.rs);
        let e2 = edge(v2, v3, self.rs);
        let e3 = edge(v3, v1, self.rs);
        let plane = Plane::from_points(
            self.rs.atom[self.rs.va[v1].atom as usize].p,
            self.rs.atom[self.rs.va[v2].atom as usize].p,
            self.rs.atom[self.rs.va[v3].atom as usize].p,
        );
        let mut normal = plane.n;
        if is_less(
            normal.dot(probe.p),
            normal.dot(self.rs.atom[self.rs.va[v1].atom as usize].p),
        ) {
            normal.negate();
        }
        let singular = is_less(distance_point_plane(probe.p, &plane)?, probe.radius);
        let face = &mut self.rs.fa[f];
        face.edge = [Some(e1), Some(e2), Some(e3)];
        face.center = probe.p;
        face.normal = normal;
        face.singular = singular;
        Ok(f)
    }

    fn face_exists(&self, face: RF, atom: i32) -> Option<RF> {
        for &v in &self.vertices[atom as usize] {
            for f in self.rs.va[v].faces.iter() {
                if self.rs.faces_equal(f, face) {
                    return Some(f);
                }
            }
        }
        None
    }

    fn center_of_probe(&mut self, pos: Pos3, c1: &mut Vec3, c2: &mut Vec3) -> bool {
        if let Some(entry) = self.probe_positions.get(&pos) {
            if let Some(position) = entry {
                *c1 = position.point[0];
                *c2 = position.point[1];
                return true;
            }
            return false;
        }
        let r = self.rs.probe_radius;
        let s = |i: u32| {
            let atom = self.rs.atom[i as usize];
            Sphere::new(atom.p, atom.radius + r)
        };
        let (s1, s2, s3) = (s(pos.0), s(pos.1), s(pos.2));
        if intersect_three_spheres(&s1, &s2, &s3, c1, c2, false) {
            self.probe_positions.insert(
                pos,
                Some(ProbePosition {
                    status: [ProbeStatus::NotTested; 2],
                    point: [*c1, *c2],
                }),
            );
            true
        } else {
            self.probe_positions.insert(pos, None);
            false
        }
    }

    fn check_probe(&mut self, probe: &Sphere, pos: Pos3) -> bool {
        let Some(Some(position)) = self.probe_positions.get(&pos).copied() else {
            return false;
        };
        let index = if probe.p.eq_eps(position.point[0]) {
            0
        } else {
            1
        };
        let mut status = position.status[index];
        if status == ProbeStatus::NotTested {
            let atom_list =
                self.neighbours_of_three_atoms(pos.0 as i32, pos.1 as i32, pos.2 as i32);
            status = ProbeStatus::Ok;
            for &i in &atom_list {
                let atom = self.rs.atom[i as usize];
                let dist = probe.radius + atom.radius;
                if is_less(probe.p.square_distance(atom.p), dist * dist) {
                    status = ProbeStatus::NotOk;
                    break;
                }
            }
            if let Some(Some(p)) = self.probe_positions.get_mut(&pos) {
                p.status[index] = status;
            }
        }
        status == ProbeStatus::Ok
    }

    fn correct_probe_positions_of_atom(&mut self, atom: u32) {
        let keys: Vec<Pos3> = self
            .probe_positions
            .keys()
            .filter(|p| p.0 == atom || p.1 == atom || p.2 == atom)
            .copied()
            .collect();
        for pos in keys {
            self.correct_probe_position(pos);
        }
    }

    fn correct_probe_position(&mut self, pos: Pos3) {
        let r = self.rs.probe_radius;
        let s = |i: u32| {
            let atom = self.rs.atom[i as usize];
            Sphere::new(atom.p, atom.radius + r)
        };
        let (s1, s2, s3) = (s(pos.0), s(pos.1), s(pos.2));
        let (mut c1, mut c2) = (Vec3::ZERO, Vec3::ZERO);
        if intersect_three_spheres(&s1, &s2, &s3, &mut c1, &mut c2, true) {
            self.probe_positions.insert(
                pos,
                Some(ProbePosition {
                    status: [ProbeStatus::NotTested; 2],
                    point: [c1, c2],
                }),
            );
        } else {
            self.probe_positions.insert(pos, None);
        }
    }

    fn pre_processing(&mut self) {
        let atoms = &mut self.rs.atom;
        let mut r_max = atoms[0].radius;
        let (mut x_min, mut y_min, mut z_min) = (atoms[0].p.x, atoms[0].p.y, atoms[0].p.z);
        let (mut x_max, mut y_max, mut z_max) = (x_min, y_min, z_min);
        let std_max = |a: f64, b: f64| if a < b { b } else { a };
        let std_min = |a: f64, b: f64| if b < a { b } else { a };
        for atom in &atoms[1..] {
            r_max = std_max(r_max, atom.radius);
            x_min = std_min(x_min, atom.p.x);
            y_min = std_min(y_min, atom.p.y);
            z_min = std_min(z_min, atom.p.z);
            x_max = std_max(x_max, atom.p.x);
            y_max = std_max(y_max, atom.p.y);
            z_max = std_max(z_max, atom.p.z);
        }
        self.rs.r_max = r_max;
        self.rs.bbox_min = Vec3::new(x_min, y_min, z_min);
        self.rs.bbox_max = Vec3::new(x_max, y_max, z_max);
        let dist = 2.0 * (r_max + self.rs.probe_radius);
        let nx = ((x_max - x_min) / dist + 5.0) as u32;
        let ny = ((y_max - y_min) / dist + 5.0) as u32;
        let nz = ((z_max - z_min) / dist + 5.0) as u32;
        let origin = [
            (x_min - 2.0 * dist) as f32,
            (y_min - 2.0 * dist) as f32,
            (z_min - 2.0 * dist) as f32,
        ];
        let spacing = dist as f32;
        let mut grid: HashGrid<u32> = HashGrid::new(origin, [spacing; 3], [nx, ny, nz]);
        let mut to_delete = Vec::new();
        let mut num_deleted = 0u32;
        let atoms = &mut self.rs.atom;
        for i in 0..atoms.len() {
            let pos = atoms[i].p.to_f32();
            let radius_i = atoms[i].radius;
            let mut too_close = false;
            for d in grid.neighbourhood(pos).unwrap_or_default() {
                let d = d as usize;
                let radius_d = atoms[d].radius;
                if is_less_or_equal(
                    atoms[i].p.distance(atoms[d].p),
                    0.05 * std_max(radius_d, radius_i),
                ) {
                    too_close = true;
                    to_delete.push(i);
                    num_deleted += 1;
                    if radius_i > radius_d {
                        atoms[d].p = atoms[i].p;
                        atoms[d].radius = atoms[i].radius;
                    }
                    break;
                }
            }
            if !too_close {
                grid.insert(pos, i as u32 - num_deleted);
            }
        }
        for &i in to_delete.iter().rev() {
            atoms.remove(i);
        }
        let n = atoms.len();
        for i in 0..n.saturating_sub(1) {
            let offset = self.rs.atom[i].radius + 2.0 * self.rs.probe_radius;
            let pos = self.rs.atom[i].p.to_f32();
            for d in grid.neighbourhood(pos).unwrap_or_default() {
                if d as usize > i {
                    let next_atom = self.rs.atom[d as usize];
                    let dist = next_atom.p.square_distance(self.rs.atom[i].p);
                    let mut max_dist = next_atom.radius + offset;
                    max_dist *= max_dist;
                    if !is_greater(dist, max_dist) {
                        self.neighbours[i].push(d as i32);
                        self.neighbours[d as usize].push(i as i32);
                    }
                }
            }
            self.neighbours[i].sort_unstable();
        }
    }

    fn insert_vertex(&mut self, vertex: RV) {
        self.rs.insert_vertex(vertex);
        self.new_vertices.insert(vertex);
        let atom = self.rs.va[vertex].atom as usize;
        self.vertices[atom].push(vertex);
        self.atom_status[atom] = AtomStatus::OnSurface;
    }

    fn insert_edge(&mut self, edge: RE) {
        self.rs.insert_edge(edge);
        let [v0, v1] = self.rs.ea[edge].vertex.map(|v| v.expect("RS edge vertex"));
        self.rs.va[v0].edges.insert(edge);
        self.rs.va[v1].edges.insert(edge);
    }

    fn insert_face(&mut self, face: RF) {
        self.rs.insert_face(face);
        for v in self.rs.fa[face].vertex {
            self.rs.va[v].faces.insert(face);
        }
        for k in 0..3 {
            let edge = self.rs.fa[face].edge[k].expect("RS face edge");
            let [v0, v1] = self.rs.ea[edge].vertex.map(|v| v.expect("RS edge vertex"));
            self.rs.va[v0].edges.insert(edge);
            self.rs.va[v1].edges.insert(edge);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_intersection_of_sorted_lists() {
        assert_eq!(
            set_intersection(&[1, 3, 5, 7], &[2, 3, 4, 7, 9]),
            vec![3, 7]
        );
        assert_eq!(sorted3(5, 1, 3), (1, 3, 5));
    }

    #[test]
    fn three_atoms_give_two_faces() {
        let spheres = vec![
            Sphere::new(Vec3::new(0.0, 0.0, 0.0), 1.7),
            Sphere::new(Vec3::new(2.2, 0.3, 0.0), 1.55),
            Sphere::new(Vec3::new(1.0, 1.9, 0.4), 1.52),
        ];
        let mut rs = ReducedSurface::new(spheres, 1.4);
        rs.compute().unwrap();
        assert_eq!(
            (rs.vertices.len(), rs.edges.len(), rs.faces.len()),
            (3, 3, 2)
        );
        assert_eq!(eps(), 1e-6);
    }
}
