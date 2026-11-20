//! BALL `SolventExcludedSurface`, `SESComputer` and `SESSingularityCleaner`
//! (`solventExcludedSurface.cpp`, `SESEdge`, `SESFace`, `SESVertex`).

use super::geom::*;
use super::rs::{ReducedSurface, RE, RF, RV};
use super::util::{define_id, impl_arena_id, Arena, BallIntSet, HashGrid, OrderedSet};
use super::BallResult;
use std::collections::HashMap;

define_id!(SV, SE, SF);
impl_arena_id!(SV, SE, SF);

pub(super) struct SesVertex {
    pub point: Vec3,
    pub normal: Vec3,
    pub atom: i32,
    pub index: i32,
    pub edges: OrderedSet<SE>,
    pub faces: OrderedSet<SF>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum EdgeType {
    Concave,
    Convex,
    Singular,
}

#[derive(Clone)]
pub(super) struct SesEdge {
    pub vertex: [Option<SV>; 2],
    pub face: [Option<SF>; 2],
    pub circle: Circle,
    pub rsedge: Option<RE>,
    pub ty: EdgeType,
    pub index: i32,
}

impl SesEdge {
    pub fn other_face(&self, face: SF) -> BallResult<Option<SF>> {
        if self.face[0] == Some(face) {
            Ok(self.face[1])
        } else if self.face[1] == Some(face) {
            Ok(self.face[0])
        } else {
            Err("BALL: face is not incident to SES edge".to_string())
        }
    }
    pub fn other_vertex(&self, vertex: Option<SV>) -> BallResult<Option<SV>> {
        if self.vertex[0] == vertex {
            Ok(self.vertex[1])
        } else if self.vertex[1] == vertex {
            Ok(self.vertex[0])
        } else {
            Err("BALL: vertex is not incident to SES edge".to_string())
        }
    }
    fn revert(&mut self) {
        self.vertex.swap(0, 1);
    }
    fn substitute_vertex(&mut self, old: SV, new: SV) {
        if self.vertex[0] == Some(old) {
            self.vertex[0] = Some(new);
        } else if self.vertex[1] == Some(old) {
            self.vertex[1] = Some(new);
        }
    }
    fn substitute_face(&mut self, old: SF, new: SF) {
        if self.face[0] == Some(old) {
            self.face[0] = Some(new);
        } else if self.face[1] == Some(old) {
            self.face[1] = Some(new);
        }
    }
    /// `SESEdge::operator ==` (same end points).
    fn same_ends(&self, other: &SesEdge) -> bool {
        (self.vertex[0] == other.vertex[0] && self.vertex[1] == other.vertex[1])
            || (self.vertex[1] == other.vertex[0] && self.vertex[0] == other.vertex[1])
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum FaceType {
    Spheric,
    Toric,
    Contact,
    ToricSingular,
}

#[derive(Clone)]
pub(super) struct SesFace {
    pub ty: FaceType,
    pub rsvertex: Option<RV>,
    pub rsedge: Option<RE>,
    pub rsface: Option<RF>,
    pub vertex: Vec<SV>,
    pub edge: Vec<SE>,
    pub index: i32,
}

impl SesFace {
    fn new(ty: FaceType, index: i32) -> Self {
        Self {
            ty,
            rsvertex: None,
            rsedge: None,
            rsface: None,
            vertex: Vec::new(),
            edge: Vec::new(),
            index,
        }
    }
    /// `GraphFace::insert(vertex)`.
    fn insert_vertex(&mut self, v: SV) {
        if !self.vertex.contains(&v) {
            self.vertex.push(v);
        }
    }
    fn remove_vertex(&mut self, v: SV) {
        self.vertex.retain(|&x| x != v);
    }
    fn remove_edge(&mut self, e: SE) {
        self.edge.retain(|&x| x != e);
    }
    fn substitute_vertex(&mut self, old: SV, new: SV) {
        if let Some(v) = self.vertex.iter_mut().find(|v| **v == old) {
            *v = new;
        }
    }
    fn substitute_edge(&mut self, old: SE, new: SE) {
        if let Some(e) = self.edge.iter_mut().find(|e| **e == old) {
            *e = new;
        }
    }
    pub fn rsface(&self) -> RF {
        self.rsface.expect("spheric face without RS face")
    }
    pub fn rsedge(&self) -> RE {
        self.rsedge.expect("toric face without RS edge")
    }
}

pub(super) struct Ses {
    pub vertices: Vec<Option<SV>>,
    pub edges: Vec<Option<SE>>,
    pub singular_edges: Vec<SE>,
    pub number_of_singular_edges: u32,
    pub contact_faces: Vec<Option<SF>>,
    pub toric_faces: Vec<Option<SF>>,
    pub spheric_faces: Vec<Option<SF>>,
    pub va: Arena<SV, SesVertex>,
    pub ea: Arena<SE, SesEdge>,
    pub fa: Arena<SF, SesFace>,
    singular_cursor: usize,
}

impl Ses {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            edges: Vec::new(),
            singular_edges: Vec::new(),
            number_of_singular_edges: 0,
            contact_faces: Vec::new(),
            toric_faces: Vec::new(),
            spheric_faces: Vec::new(),
            va: Arena::new(),
            ea: Arena::new(),
            fa: Arena::new(),
            singular_cursor: 0,
        }
    }

    fn clear(&mut self) {
        self.vertices.clear();
        self.edges.clear();
        self.singular_edges.clear();
        self.contact_faces.clear();
        self.toric_faces.clear();
        self.spheric_faces.clear();
        self.number_of_singular_edges = 0;
    }

    pub fn vertex(&self, i: usize) -> SV {
        self.vertices[i].expect("compacted SES vertex")
    }
    pub fn toric(&self, i: usize) -> SF {
        self.toric_faces[i].expect("compacted toric face")
    }
    pub fn spheric(&self, i: usize) -> SF {
        self.spheric_faces[i].expect("compacted spheric face")
    }
    pub fn contact(&self, i: usize) -> SF {
        self.contact_faces[i].expect("compacted contact face")
    }

    fn push_vertex(&mut self, vertex: SesVertex) -> SV {
        let v = self.va.push(vertex);
        self.vertices.push(Some(v));
        v
    }

    fn push_edge(&mut self, mut edge: SesEdge) -> SE {
        edge.index = self.edges.len() as i32;
        let e = self.ea.push(edge);
        self.edges.push(Some(e));
        e
    }

    fn push_front_singular(&mut self, e: SE) {
        self.singular_edges.insert(0, e);
        self.singular_cursor += 1;
    }

    fn remove_singular(&mut self, e: SE) {
        self.singular_edges.retain(|&x| x != e);
    }

    /// `GraphVertex::join` + `substitute` for SES vertices (`operator *=` is always true).

    /// `GraphFace::getEdges(vertex, edge1, edge2)`; outputs are only written when found.
    fn face_edges_of(
        &self,
        face: SF,
        vertex: Option<SV>,
        edge1: &mut Option<SE>,
        edge2: &mut Option<SE>,
    ) -> bool {
        let edges = &self.fa[face].edge;
        let touches = |e: SE| self.ea[e].vertex[0] == vertex || self.ea[e].vertex[1] == vertex;
        let Some(first) = edges.iter().position(|&e| touches(e)) else {
            return false;
        };
        *edge1 = Some(edges[first]);
        if let Some(second) = edges[first + 1..].iter().find(|&&e| touches(e)) {
            *edge2 = Some(*second);
            return true;
        }
        false
    }

    /// `GraphFace::getEdge(v1, v2, edge)`.
    fn face_edge_between(
        &self,
        face: SF,
        v1: Option<SV>,
        v2: Option<SV>,
        edge: &mut Option<SE>,
    ) -> bool {
        for &e in &self.fa[face].edge {
            let ev = self.ea[e].vertex;
            if (ev[0] == v1 && ev[1] == v2) || (ev[0] == v2 && ev[1] == v1) {
                *edge = Some(e);
                return true;
            }
        }
        false
    }

    fn is_neighboured_to(&self, face: SF, other: SF) -> bool {
        self.fa[face]
            .edge
            .iter()
            .any(|&e| self.ea[e].face[0] == Some(other) || self.ea[e].face[1] == Some(other))
    }

    /// `SESFace::isFree`.
    pub fn face_is_free(&self, rs: &ReducedSurface, face: SF) -> bool {
        let f = &self.fa[face];
        f.ty == FaceType::Toric && rs.ea[f.rsedge()].is_free()
    }

    /// `SESFace::normalize(singular)`.
    pub fn normalize_face(
        &mut self,
        rs: &ReducedSurface,
        face: SF,
        singular: bool,
    ) -> BallResult<()> {
        let f = &self.fa[face];
        if f.ty != FaceType::Toric && f.ty != FaceType::ToricSingular {
            return Ok(());
        }
        if rs.ea[f.rsedge()].is_free() {
            return Ok(());
        }
        if singular {
            self.normalize_singular_toric_face(face)
        } else {
            self.normalize_non_singular_toric_face(face)
        }
    }

    fn normalize_non_singular_toric_face(&mut self, face: SF) -> BallResult<()> {
        let f = &self.fa[face];
        let mut points: Vec<SV> = Vec::new();
        for &v in &f.vertex {
            if !points.contains(&v) {
                points.push(v);
            }
        }
        let missing = || "BALL: malformed toric face".to_string();
        let mut concave = f
            .edge
            .iter()
            .copied()
            .filter(|&e| self.ea[e].ty == EdgeType::Concave);
        let edge0 = concave.next().ok_or_else(missing)?;
        let edge2 = concave.next().ok_or_else(missing)?;
        let p0 = self.ea[edge0].vertex[0];
        let p1 = self.ea[edge0].vertex[1];
        let e2v = self.ea[edge2].vertex;
        let (p2, p3);
        let mut edge1 = None;
        let mut edge3 = None;
        if points.len() == 4 {
            if self.face_edge_between(face, p1, e2v[0], &mut edge1) {
                p2 = e2v[0];
                p3 = e2v[1];
            } else {
                self.face_edge_between(face, p1, e2v[1], &mut edge1);
                p2 = e2v[1];
                p3 = e2v[0];
            }
            self.face_edge_between(face, p0, p3, &mut edge3);
        } else {
            if p0 == e2v[0] || p1 == e2v[1] {
                p2 = e2v[1];
                p3 = e2v[0];
            } else {
                p2 = e2v[0];
                p3 = e2v[1];
            }
            self.face_edge_between(face, p1, p2, &mut edge1);
            self.face_edge_between(face, p0, p3, &mut edge3);
        }
        let f = &mut self.fa[face];
        f.edge = vec![
            edge0,
            edge1.ok_or_else(missing)?,
            edge2,
            edge3.ok_or_else(missing)?,
        ];
        f.vertex = vec![
            p0.ok_or_else(missing)?,
            p1.ok_or_else(missing)?,
            p2.ok_or_else(missing)?,
            p3.ok_or_else(missing)?,
        ];
        Ok(())
    }

    fn normalize_singular_toric_face(&mut self, face: SF) -> BallResult<()> {
        let (edge0, mut edge1, mut edge2, mut p0, mut p1, mut p2) =
            (None, None, None, None, None, None);
        let (mut edge3, mut edge4, mut edge5, mut p3, mut p4, mut p5) =
            (None, None, None, None, None, None);
        let edge0 = self.find_triangle(
            face, true, edge0, &mut edge1, &mut edge2, &mut p0, &mut p1, &mut p2,
        )?;
        let edge3_found = self.find_triangle(
            face, false, edge3, &mut edge4, &mut edge5, &mut p3, &mut p4, &mut p5,
        )?;
        edge3 = edge3_found;
        let missing = || "BALL: malformed singular toric face".to_string();
        let c1 = self.ea[edge1.ok_or_else(missing)?].circle;
        let c4 = self.ea[edge4.ok_or_else(missing)?].circle;
        if c1.ne_eps(&c4) {
            std::mem::swap(&mut edge4, &mut edge5);
            std::mem::swap(&mut p5, &mut p3);
        }
        let edges = [edge0, edge1, edge2, edge3, edge4, edge5];
        let vertices = [p0, p1, p2, p3, p4, p5];
        let f = &mut self.fa[face];
        f.edge = edges
            .iter()
            .map(|e| e.ok_or_else(missing))
            .collect::<BallResult<_>>()?;
        f.vertex = vertices
            .iter()
            .map(|v| v.ok_or_else(missing))
            .collect::<BallResult<_>>()?;
        Ok(())
    }

    /// `SESFace::findTriangle_`; returns `edge0`.
    #[allow(clippy::too_many_arguments)]
    fn find_triangle(
        &self,
        face: SF,
        first: bool,
        _edge0: Option<SE>,
        edge1: &mut Option<SE>,
        edge2: &mut Option<SE>,
        vertex0: &mut Option<SV>,
        vertex1: &mut Option<SV>,
        vertex2: &mut Option<SV>,
    ) -> BallResult<Option<SE>> {
        let edges = &self.fa[face].edge;
        let convex = |e: &&SE| self.ea[**e].ty == EdgeType::Convex;
        let found = if first {
            edges.iter().find(convex)
        } else {
            edges.iter().rev().find(convex)
        };
        let edge0 = *found.ok_or("BALL: singular toric face without convex edge")?;
        *vertex0 = self.ea[edge0].vertex[0];
        *vertex2 = self.ea[edge0].vertex[1];
        for &e in edges {
            if self.ea[e].vertex[0] == *vertex0 && e != edge0 {
                *edge1 = Some(e);
                *vertex1 = self.ea[e].vertex[1];
            } else if self.ea[e].vertex[1] == *vertex0 && e != edge0 {
                *edge1 = Some(e);
                *vertex1 = self.ea[e].vertex[0];
            }
        }
        for &e in edges.iter().rev() {
            let ev = self.ea[e].vertex;
            if (ev[0] == *vertex1 && ev[1] == *vertex2) || (ev[1] == *vertex1 && ev[0] == *vertex2)
            {
                *edge2 = Some(e);
            }
        }
        Ok(Some(edge0))
    }

    /// `SolventExcludedSurface::clean(density)`.
    pub fn clean(&mut self, rs: &mut ReducedSurface, density: f64) -> BallResult<()> {
        let sqrt_density = density.sqrt();
        let mut done = false;
        while !done {
            done = true;
            for i in 0..self.toric_faces.len() {
                let Some(face) = self.toric_faces[i] else {
                    continue;
                };
                if self.face_is_free(rs, face) {
                    continue;
                }
                let kept = if self.fa[face].ty == FaceType::ToricSingular {
                    self.clean_singular_toric_face(rs, face, sqrt_density)?
                } else {
                    self.clean_toric_face(rs, face, sqrt_density)?
                };
                if !kept {
                    done = false;
                }
            }
        }
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
        compact(&mut self.contact_faces, |f, i| fa[f].index = i);
        compact(&mut self.toric_faces, |f, i| fa[f].index = i);
        compact(&mut self.spheric_faces, |f, i| fa[f].index = i);
        Ok(())
    }

    fn clean_singular_toric_face(
        &mut self,
        rs: &mut ReducedSurface,
        face: SF,
        sqrt_density: f64,
    ) -> BallResult<bool> {
        self.normalize_face(rs, face, true)?;
        let f = &self.fa[face];
        let (edge0, edge3) = (f.edge[0], f.edge[3]);
        let (v0, v2, v3, v5) = (f.vertex[0], f.vertex[2], f.vertex[3], f.vertex[5]);
        let del;
        let mut set = None;
        if v0 == v2 {
            let rsedge = self.ea[edge0]
                .rsedge
                .ok_or("BALL: convex edge without RS edge")?;
            del = rs.ea[rsedge].angle < PI;
            if !del {
                set = Some(rsedge);
            }
        } else if v3 == v5 {
            let rsedge = self.ea[edge3]
                .rsedge
                .ok_or("BALL: convex edge without RS edge")?;
            del = rs.ea[rsedge].angle < PI;
            if !del {
                set = Some(rsedge);
            }
        } else {
            let exact_number_of_segments =
                rs.ea[f.rsedge()].angle * self.ea[edge3].circle.radius * sqrt_density;
            del = is_less(exact_number_of_segments, 0.1);
        }
        if del {
            self.delete_small_singular_toric_face(face)?;
        }
        if let Some(rsedge) = set {
            rs.ea[rsedge].angle = 2.0 * PI;
        }
        Ok(!del)
    }

    fn clean_toric_face(
        &mut self,
        rs: &mut ReducedSurface,
        face: SF,
        sqrt_density: f64,
    ) -> BallResult<bool> {
        self.normalize_face(rs, face, false)?;
        let f = &self.fa[face];
        let (edge1, edge3) = (f.edge[1], f.edge[3]);
        let (v0, v1, v2, v3) = (f.vertex[0], f.vertex[1], f.vertex[2], f.vertex[3]);
        let del;
        let mut set = None;
        if v0 == v3 {
            let rsedge = self.ea[edge3]
                .rsedge
                .ok_or("BALL: convex edge without RS edge")?;
            del = rs.ea[rsedge].angle < PI;
            if !del {
                set = Some(rsedge);
            }
        } else if v1 == v2 {
            let rsedge = self.ea[edge1]
                .rsedge
                .ok_or("BALL: convex edge without RS edge")?;
            del = rs.ea[rsedge].angle < PI;
            if !del {
                set = Some(rsedge);
            }
        } else {
            let exact_number_of_segments =
                rs.ea[f.rsedge()].angle * self.ea[edge3].circle.radius * sqrt_density;
            del = is_less(exact_number_of_segments, 0.1);
        }
        if del {
            self.delete_small_toric_face(face)?;
        }
        if let Some(rsedge) = set {
            rs.ea[rsedge].angle = 2.0 * PI;
        }
        Ok(!del)
    }

    fn delete_small_toric_face(&mut self, face: SF) -> BallResult<()> {
        let edge: [SE; 4] = self.fa[face].edge[..4]
            .try_into()
            .map_err(|_| "BALL: malformed toric face")?;
        let p: [SV; 4] = self.fa[face].vertex[..4]
            .try_into()
            .map_err(|_| "BALL: malformed toric face")?;
        let neighbour = |s: &Self, e: SE| -> BallResult<SF> {
            s.ea[e]
                .other_face(face)?
                .ok_or_else(|| "BALL: missing neighbour face".to_string())
        };
        let neighbour1 = neighbour(self, edge[1])?;
        let neighbour2 = neighbour(self, edge[2])?;
        let neighbour3 = neighbour(self, edge[3])?;
        if p[0] != p[3] {
            self.join_vertex_only(p[0], p[3]);
            self.fa[neighbour3].remove_vertex(p[3]);
            self.substitute_only(p[3], p[0]);
        }
        if p[1] != p[2] {
            self.join_vertex_only(p[1], p[2]);
            self.fa[neighbour1].remove_vertex(p[2]);
            self.substitute_only(p[2], p[1]);
        }
        self.va[p[0]].edges.erase(edge[2]);
        self.va[p[0]].edges.erase(edge[3]);
        self.va[p[1]].edges.erase(edge[2]);
        self.va[p[1]].edges.erase(edge[1]);
        self.va[p[0]].faces.erase(face);
        self.va[p[1]].faces.erase(face);
        self.ea[edge[0]].substitute_face(face, neighbour2);
        self.fa[neighbour2].substitute_edge(edge[2], edge[0]);
        if p[2] != p[1] {
            let index = self.va[p[2]].index as usize;
            self.vertices[index] = None;
            self.fa[neighbour1].remove_edge(edge[1]);
        } else {
            self.va[p[1]].faces.erase(neighbour1);
            let index = self.fa[neighbour1].index as usize;
            self.contact_faces[index] = None;
        }
        if p[3] != p[0] {
            let index = self.va[p[3]].index as usize;
            self.vertices[index] = None;
            self.fa[neighbour3].remove_edge(edge[3]);
        } else {
            self.va[p[0]].faces.erase(neighbour3);
            let index = self.fa[neighbour3].index as usize;
            self.contact_faces[index] = None;
        }
        for e in [edge[1], edge[2], edge[3]] {
            let index = self.ea[e].index as usize;
            self.edges[index] = None;
        }
        let index = self.fa[face].index as usize;
        self.toric_faces[index] = None;
        self.make_singular(edge[0])?;
        self.singular_edges.push(edge[0]);
        Ok(())
    }

    fn delete_small_singular_toric_face(&mut self, face: SF) -> BallResult<()> {
        let edge: [SE; 6] = self.fa[face].edge[..6]
            .try_into()
            .map_err(|_| "BALL: malformed toric face")?;
        let p: [SV; 6] = self.fa[face].vertex[..6]
            .try_into()
            .map_err(|_| "BALL: malformed toric face")?;
        let neighbour = |s: &Self, e: SE| -> BallResult<SF> {
            s.ea[e]
                .other_face(face)?
                .ok_or_else(|| "BALL: missing neighbour face".to_string())
        };
        let neighbour0 = neighbour(self, edge[0])?;
        let neighbour2 = neighbour(self, edge[2])?;
        let neighbour3 = neighbour(self, edge[3])?;
        let neighbour5 = neighbour(self, edge[5])?;
        if p[0] != p[2] {
            self.join_vertex_only(p[0], p[2]);
            self.fa[neighbour0].remove_vertex(p[2]);
            self.substitute_only(p[2], p[0]);
        }
        if p[3] != p[5] {
            self.join_vertex_only(p[3], p[5]);
            self.fa[neighbour3].remove_vertex(p[5]);
            self.substitute_only(p[5], p[3]);
        }
        self.va[p[0]].edges.erase(edge[0]);
        self.va[p[0]].edges.erase(edge[2]);
        self.va[p[1]].edges.erase(edge[2]);
        self.va[p[3]].edges.erase(edge[3]);
        self.va[p[3]].edges.erase(edge[5]);
        self.va[p[4]].edges.erase(edge[5]);
        for v in [p[0], p[1], p[3], p[4]] {
            self.va[v].faces.erase(face);
        }
        self.ea[edge[1]].substitute_face(face, neighbour2);
        self.ea[edge[4]].substitute_face(face, neighbour5);
        self.fa[neighbour2].substitute_edge(edge[2], edge[1]);
        self.fa[neighbour5].substitute_edge(edge[5], edge[4]);
        if p[2] != p[0] {
            let index = self.va[p[2]].index as usize;
            self.vertices[index] = None;
            self.fa[neighbour0].remove_edge(edge[0]);
        } else {
            self.va[p[0]].faces.erase(neighbour0);
            let index = self.fa[neighbour0].index as usize;
            self.contact_faces[index] = None;
        }
        if p[3] != p[5] {
            let index = self.va[p[5]].index as usize;
            self.vertices[index] = None;
            self.fa[neighbour3].remove_edge(edge[3]);
        } else {
            self.va[p[3]].faces.erase(neighbour3);
            let index = self.fa[neighbour3].index as usize;
            self.contact_faces[index] = None;
        }
        for e in [edge[0], edge[2], edge[3], edge[5]] {
            let index = self.ea[e].index as usize;
            self.edges[index] = None;
        }
        let index = self.fa[face].index as usize;
        self.toric_faces[index] = None;
        self.make_singular(edge[1])?;
        self.make_singular(edge[4])?;
        self.singular_edges.push(edge[1]);
        self.singular_edges.push(edge[4]);
        Ok(())
    }

    fn join_vertex_only(&mut self, keep: SV, drop: SV) {
        for e in self.va[drop].edges.items() {
            self.va[keep].edges.insert(e);
        }
        for f in self.va[drop].faces.items() {
            self.va[keep].faces.insert(f);
        }
    }

    fn substitute_only(&mut self, old: SV, new: SV) {
        for e in self.va[old].edges.items() {
            self.ea[e].substitute_vertex(old, new);
        }
        for f in self.va[old].faces.items() {
            self.fa[f].substitute_vertex(old, new);
        }
    }

    fn make_singular(&mut self, e: SE) -> BallResult<()> {
        self.ea[e].ty = EdgeType::Singular;
        let edge = &self.ea[e];
        let v0 = self.va[edge.vertex[0].ok_or("BALL: singular edge without vertex")?].point;
        let v1 = self.va[edge.vertex[1].ok_or("BALL: singular edge without vertex")?].point;
        let phi = oriented_angle(v0 - edge.circle.p, v1 - edge.circle.p, edge.circle.n)?;
        if phi > PI {
            self.ea[e].circle.n.negate();
        }
        Ok(())
    }

    /// `SolventExcludedSurface::splitSphericFaces`.
    pub fn split_spheric_faces(&mut self) -> BallResult<()> {
        let mut i = 0;
        while i < self.spheric_faces.len() {
            self.split_spheric_face(i)?;
            i += 1;
        }
        Ok(())
    }

    fn split_spheric_face(&mut self, i: usize) -> BallResult<()> {
        let face = self.spheric(i);
        let edges = self.fa[face].edge.clone();
        if edges.iter().any(|&e| self.ea[e].vertex[0].is_none()) {
            return Ok(());
        }
        let first = *edges.first().ok_or("BALL: spheric face without edges")?;
        let start = self.ea[first].vertex[0];
        let mut next = self.ea[first].vertex[1];
        let mut current = first;
        let mut contour = vec![current];
        let mut vertices = vec![next.expect("checked above")];
        let mut rounds = 0;
        while next != start {
            rounds += 1;
            if rounds > edges.len() + 1 {
                return Err("BALL: open contour in spheric face".to_string());
            }
            for &e in &edges {
                if e != current {
                    let ev = self.ea[e].vertex;
                    if ev[0] == next {
                        contour.push(e);
                        next = ev[1];
                        vertices.push(next.expect("checked above"));
                        current = e;
                    } else if ev[1] == next {
                        contour.push(e);
                        next = ev[0];
                        vertices.push(next.expect("checked above"));
                        current = e;
                    }
                }
            }
        }
        if contour.len() != edges.len() {
            let mut new_face = self.fa[face].clone();
            for &e in &contour {
                new_face.remove_edge(e);
            }
            for &v in &vertices {
                new_face.remove_vertex(v);
            }
            new_face.index = self.spheric_faces.len() as i32;
            let nf = self.fa.push(new_face);
            self.spheric_faces.push(Some(nf));
            self.fa[face].edge = contour;
            self.fa[face].vertex = vertices;
        }
        Ok(())
    }

    /// `SolventExcludedSurface::check`.
    pub fn check(&self) -> bool {
        for v in &self.vertices {
            let v = &self.va[v.expect("SES vertex")];
            if v.edges.len() != v.faces.len() {
                return false;
            }
        }
        for f in &self.spheric_faces {
            let f = &self.fa[f.expect("spheric face")];
            if f.edge.len() != f.vertex.len() {
                let mut test = f.edge.len() as i64 - f.vertex.len() as i64;
                for &e in &f.edge {
                    if self.ea[e].vertex[0].is_none() {
                        test -= 1;
                    }
                }
                if test != 0 {
                    return false;
                }
            }
        }
        true
    }
}

/// `SESComputer` together with the `SESSingularityCleaner` state it owns.
pub(super) struct SesComputer<'a> {
    rs: &'a mut ReducedSurface,
    ses: &'a mut Ses,
    vertex_grid: HashGrid<i32>,
    probe_intersections: HashMap<(u32, u32, u32), Option<(Vec3, Vec3)>>,
}

type Intersection = (f64, i32, Vec3);

impl<'a> SesComputer<'a> {
    pub fn new(rs: &'a mut ReducedSurface, ses: &'a mut Ses) -> Self {
        Self {
            rs,
            ses,
            vertex_grid: HashGrid::new([0.0; 3], [0.0; 3], [0, 0, 0]),
            probe_intersections: HashMap::new(),
        }
    }

    pub fn run(&mut self) -> BallResult<()> {
        self.pre_processing();
        self.get()?;
        while !self.cleaner_run()? {
            self.ses.clear();
            self.pre_processing();
            self.get()?;
        }
        Ok(())
    }

    fn pre_processing(&mut self) {
        self.ses.clear();
        let rs = &*self.rs;
        for i in 0..rs.vertices.len() {
            let mut face = SesFace::new(FaceType::Contact, i as i32);
            face.rsvertex = Some(rs.vertex(i));
            let f = self.ses.fa.push(face);
            self.ses.contact_faces.push(Some(f));
        }
        for i in 0..rs.edges.len() {
            let mut face = SesFace::new(FaceType::Toric, i as i32);
            face.rsedge = Some(rs.edge(i));
            let f = self.ses.fa.push(face);
            self.ses.toric_faces.push(Some(f));
        }
        for i in 0..rs.faces.len() {
            let mut face = SesFace::new(FaceType::Spheric, i as i32);
            face.rsface = Some(rs.face(i));
            let f = self.ses.fa.push(face);
            self.ses.spheric_faces.push(Some(f));
        }
        let (min, max) = (rs.bbox_min, rs.bbox_max);
        let dist = rs.r_max;
        let nx = ((max.x - min.x) / dist + 5.0) as u32;
        let ny = ((max.y - min.y) / dist + 5.0) as u32;
        let nz = ((max.z - min.z) / dist + 5.0) as u32;
        let origin = [
            (min.x - 2.0 * dist) as f32,
            (min.y - 2.0 * dist) as f32,
            (min.z - 2.0 * dist) as f32,
        ];
        self.vertex_grid = HashGrid::new(origin, [dist as f32; 3], [nx, ny, nz]);
    }

    fn get(&mut self) -> BallResult<()> {
        for i in 0..self.ses.spheric_faces.len() {
            self.create_spheric_face(i)?;
        }
        for i in 0..self.ses.toric_faces.len() {
            self.create_toric_face(i)?;
        }
        Ok(())
    }

    fn create_spheric_face(&mut self, j: usize) -> BallResult<()> {
        let face = self.ses.spheric(j);
        let rsface = self.ses.fa[face].rsface();
        let probe_radius = self.rs.probe_radius;
        let probe = Sphere::new(self.rs.fa[rsface].center, probe_radius);
        let rsv = self.rs.fa[rsface].vertex;
        for v in rsv {
            self.push_vertex(face, &probe, v)?;
        }
        self.push_concave_edge(face, 0, 1, probe_radius);
        self.push_concave_edge(face, 1, 2, probe_radius);
        self.push_concave_edge(face, 2, 0, probe_radius);
        Ok(())
    }

    fn push_vertex(&mut self, face: SF, probe: &Sphere, rsvertex: RV) -> BallResult<()> {
        let atom_index = self.rs.va[rsvertex].atom;
        let atom = self.rs.atom[atom_index as usize];
        let mut point = probe.p - atom.p;
        point.normalize()?;
        point *= atom.radius;
        point += atom.p;
        let vertex = self.ses.push_vertex(SesVertex {
            point,
            normal: probe.p - point,
            atom: atom_index,
            index: self.ses.vertices.len() as i32,
            edges: OrderedSet::new(),
            faces: OrderedSet::new(),
        });
        self.ses.fa[face].vertex.push(vertex);
        self.ses.va[vertex].faces.insert(face);
        let rsface = self.ses.fa[face].rsface();
        let (tf1, tf2) = self.rs.face_edges_of(rsface, rsvertex);
        for tf in [tf1, tf2] {
            let tf = tf.ok_or("BALL: RS vertex without face edges")?;
            let toric = self.ses.toric(self.rs.ea[tf].index as usize);
            self.ses.fa[toric].vertex.push(vertex);
            self.ses.va[vertex].faces.insert(toric);
        }
        let contact = self.ses.contact(self.rs.va[rsvertex].index as usize);
        self.ses.fa[contact].vertex.push(vertex);
        self.ses.va[vertex].faces.insert(contact);
        self.vertex_grid
            .insert(point.to_f32(), self.ses.va[vertex].index);
        Ok(())
    }

    fn push_concave_edge(&mut self, face: SF, p1: usize, p2: usize, radius_of_probe: f64) {
        let rsface = self.ses.fa[face].rsface();
        let rsv = self.rs.fa[rsface].vertex;
        let rsedge = self
            .rs
            .face_edge_between(rsface, rsv[p1], rsv[p2])
            .expect("RS face edge between its vertices");
        let index = self.rs.ea[rsedge].index as usize;
        let toric = self.ses.toric(index);
        let v0 = self.ses.fa[face].vertex[p1];
        let v1 = self.ses.fa[face].vertex[p2];
        let center = self.rs.fa[rsface].center;
        let normal = (self.ses.va[v0].point - center).cross(self.ses.va[v1].point - center);
        let edge = self.ses.push_edge(SesEdge {
            vertex: [Some(v0), Some(v1)],
            face: [Some(face), Some(toric)],
            circle: Circle::new(center, normal, radius_of_probe),
            rsedge: None,
            ty: EdgeType::Concave,
            index: -1,
        });
        self.ses.fa[face].edge.push(edge);
        self.ses.fa[toric].edge.push(edge);
        self.ses.va[v0].edges.insert(edge);
        self.ses.va[v1].edges.insert(edge);
    }

    fn create_toric_face(&mut self, i: usize) -> BallResult<()> {
        let face = self.ses.toric(i);
        if self.ses.face_is_free(self.rs, face) {
            self.create_free_toric_face(i);
            return Ok(());
        }
        let rsedge = self.ses.fa[face].rsedge();
        let [rv0, rv1] = self.rs.ea[rsedge]
            .vertex
            .map(|v| v.expect("RS edge vertex"));
        let edge1 = self.create_convex_edge(face, rv0)?;
        let edge2 = self.create_convex_edge(face, rv1)?;
        if is_equal(self.rs.ea[rsedge].angle, PI) {
            let rsface1 = self.rs.ea[rsedge].face[0].expect("RS edge face");
            let rsface2 = self.rs.ea[rsedge].face[1].expect("RS edge face");
            let rsvertex3 = self.rs.fa[rsface1].third(rv0, rv1);
            let rsvertex4 = self.rs.fa[rsface2].third(rv0, rv1);
            let atom = |v: RV| self.rs.atom[self.rs.va[v].atom as usize].p;
            let (point1, point2, point3, _point4) =
                (atom(rv0), atom(rv1), atom(rsvertex3), atom(rsvertex4));
            let plane = Plane::from_points(point1, point2, self.rs.fa[rsface1].center);
            let test = plane.n.dot(point3 - plane.p);
            for edge in [edge1, edge2] {
                let e = &self.ses.ea[edge];
                let v0 = self.ses.va[e.vertex[0].expect("convex edge vertex")].point;
                let mut middle = e.circle.n.cross(v0 - e.circle.p);
                middle.normalize()?;
                middle *= e.circle.radius;
                middle += e.circle.p;
                if test * plane.n.dot(middle - plane.p) > 0.0 {
                    self.ses.ea[edge].revert();
                }
            }
        }
        if self.rs.ea[rsedge].singular {
            self.treat_singular_toric_face(i)?;
        }
        Ok(())
    }

    fn create_convex_edge(&mut self, face: SF, rsvertex: RV) -> BallResult<SE> {
        let atom = self.rs.va[rsvertex].atom;
        let index = self.rs.va[rsvertex].index;
        let vertices = &self.ses.fa[face].vertex;
        let first = vertices
            .iter()
            .position(|&v| self.ses.va[v].atom == atom)
            .ok_or("BALL: toric face misses a vertex")?;
        let second = first
            + 1
            + vertices[first + 1..]
                .iter()
                .position(|&v| self.ses.va[v].atom == atom)
                .ok_or("BALL: toric face misses a vertex")?;
        let (v0, v1) = (vertices[first], vertices[second]);
        let rsedge = self.ses.fa[face].rsedge();
        let re = &self.rs.ea[rsedge];
        let (c0, c1) = (re.circle0, re.circle1);
        let circle = if self.rs.va[re.vertex[0].expect("RS edge vertex")].index == index {
            Circle::new(c0.p, c0.p - c1.p, c0.radius)
        } else {
            Circle::new(c1.p, c1.p - c0.p, c1.radius)
        };
        let contact = self.ses.contact(index as usize);
        let mut edge = SesEdge {
            vertex: [Some(v0), Some(v1)],
            face: [Some(face), Some(contact)],
            circle,
            rsedge: Some(rsedge),
            ty: EdgeType::Convex,
            index: -1,
        };
        let a = self.ses.va[v0].point - circle.p;
        let b = self.ses.va[v1].point - circle.p;
        let test_phi = oriented_angle(a, b, circle.n)?;
        if (test_phi - PI) * (re.angle - PI) < 0.0 {
            edge.revert();
        }
        let [ev0, ev1] = edge.vertex.map(|v| v.expect("convex edge vertex"));
        let e = self.ses.push_edge(edge);
        self.ses.fa[face].edge.push(e);
        self.ses.fa[contact].edge.push(e);
        self.ses.va[ev0].edges.insert(e);
        self.ses.va[ev1].edges.insert(e);
        Ok(e)
    }

    fn treat_singular_toric_face(&mut self, i: usize) -> BallResult<()> {
        let face = self.ses.toric(i);
        self.ses.normalize_face(self.rs, face, false)?;
        let edge: [SE; 4] = self.ses.fa[face].edge[..4]
            .try_into()
            .map_err(|_| "BALL: malformed toric face")?;
        let p: [SV; 4] = self.ses.fa[face].vertex[..4]
            .try_into()
            .map_err(|_| "BALL: malformed toric face")?;
        let neighbour0 = self.ses.ea[edge[0]]
            .other_face(face)?
            .ok_or("BALL: missing neighbour face")?;
        let neighbour2 = self.ses.ea[edge[2]]
            .other_face(face)?
            .ok_or("BALL: missing neighbour face")?;
        let probe_radius = self.rs.probe_radius;
        let probe1 = Sphere::new(
            self.rs.fa[self.ses.fa[neighbour0].rsface()].center,
            probe_radius,
        );
        let probe2 = Sphere::new(
            self.rs.fa[self.ses.fa[neighbour2].rsface()].center,
            probe_radius,
        );
        let intersection_circle = intersect_spheres(&probe1, &probe2).unwrap_or_default();
        let new_edge0 = self.ses.ea.push(self.ses.ea[edge[0]].clone());
        let new_edge2 = self.ses.ea.push(self.ses.ea[edge[2]].clone());
        let rsedge = self.ses.fa[face].rsedge();
        let new_edge = self.ses.ea.push(SesEdge {
            vertex: [None, None],
            face: [Some(neighbour0), Some(neighbour2)],
            circle: intersection_circle,
            rsedge: Some(rsedge),
            ty: EdgeType::Singular,
            index: -1,
        });
        let rv0 = self.rs.ea[rsedge].vertex[0].expect("RS edge vertex");
        let ip1 = if self.ses.va[p[1]].atom == self.rs.va[rv0].atom {
            0
        } else {
            1
        };
        let ip3 = 1 - ip1;
        let new_point1 = self.create_singular_vertex(
            ip3,
            intersection_circle.p,
            [face, neighbour0, neighbour2],
            [edge[0], edge[2], new_edge],
        )?;
        let new_point3 = self.create_singular_vertex(
            ip1,
            intersection_circle.p,
            [face, neighbour0, neighbour2],
            [new_edge0, new_edge2, new_edge],
        )?;
        self.update_edge(edge[0], p[0], new_point1, false);
        self.update_edge(edge[2], p[3], new_point1, false);
        self.update_edge(new_edge0, p[1], new_point3, true);
        self.update_edge(new_edge2, p[2], new_point3, true);
        self.update_edge(new_edge, new_point3, new_point1, true);
        self.ses.singular_edges.push(new_edge);
        self.ses.number_of_singular_edges += 1;
        let phi = oriented_angle(
            self.ses.va[new_point1].point - intersection_circle.p,
            self.ses.va[new_point3].point - intersection_circle.p,
            intersection_circle.n,
        )?;
        if (self.rs.ea[rsedge].angle - PI) * (phi - PI) < 0.0 {
            self.ses.ea[new_edge].circle.n.negate();
        }
        let f0 = &mut self.ses.fa[neighbour0];
        f0.edge.extend([new_edge0, new_edge]);
        f0.vertex.extend([new_point1, new_point3]);
        let f2 = &mut self.ses.fa[neighbour2];
        f2.edge.extend([new_edge2, new_edge]);
        f2.vertex.extend([new_point1, new_point3]);
        let f = &mut self.ses.fa[face];
        f.ty = FaceType::ToricSingular;
        f.vertex.extend([new_point1, new_point3]);
        f.edge.extend([new_edge0, new_edge2]);
        self.ses.va[p[1]].edges.erase(edge[0]);
        self.ses.va[p[1]].edges.insert(new_edge0);
        self.ses.va[p[2]].edges.erase(edge[2]);
        self.ses.va[p[2]].edges.insert(new_edge2);
        Ok(())
    }

    fn create_singular_vertex(
        &mut self,
        ip: usize,
        dir: Vec3,
        faces: [SF; 3],
        edges: [SE; 3],
    ) -> BallResult<SV> {
        let rsedge = self.ses.fa[faces[0]].rsedge();
        let intersection_point = self.rs.ea[rsedge].intersection_point(ip)?;
        let test = self.vertex_exists(intersection_point);
        let vertex = if test == -1 {
            let atom = self.rs.va[self.rs.ea[rsedge].vertex[ip].expect("RS edge vertex")].atom;
            let index = self.ses.vertices.len() as i32;
            let v = self.ses.push_vertex(SesVertex {
                point: intersection_point,
                normal: dir - intersection_point,
                atom,
                index,
                edges: OrderedSet::new(),
                faces: OrderedSet::new(),
            });
            self.vertex_grid.insert(intersection_point.to_f32(), index);
            v
        } else {
            self.ses.vertex(test as usize)
        };
        for e in edges {
            self.ses.va[vertex].edges.insert(e);
        }
        for f in faces {
            self.ses.va[vertex].faces.insert(f);
        }
        Ok(vertex)
    }

    fn update_edge(&mut self, edge: SE, vertex1: SV, vertex2: SV, is_new: bool) {
        let e = &mut self.ses.ea[edge];
        if e.vertex[0] == Some(vertex1) {
            e.vertex = [Some(vertex1), Some(vertex2)];
        } else {
            e.vertex = [Some(vertex2), Some(vertex1)];
        }
        if is_new {
            e.index = self.ses.edges.len() as i32;
            self.ses.edges.push(Some(edge));
        }
    }

    fn create_free_toric_face(&mut self, i: usize) {
        let face = self.ses.toric(i);
        let rsedge = self.ses.fa[face].rsedge();
        let re = &self.rs.ea[rsedge];
        let (circle1, circle2) = (re.circle0, re.circle1);
        let index1 = self.rs.va[re.vertex[0].expect("RS edge vertex")].index as usize;
        let index2 = self.rs.va[re.vertex[1].expect("RS edge vertex")].index as usize;
        for (index, a, b) in [(index1, circle1, circle2), (index2, circle2, circle1)] {
            let contact = self.ses.contact(index);
            let edge = self.ses.push_edge(SesEdge {
                vertex: [None, None],
                face: [Some(face), Some(contact)],
                circle: Circle::new(a.p, a.p - b.p, a.radius),
                rsedge: Some(rsedge),
                ty: EdgeType::Convex,
                index: -1,
            });
            self.ses.fa[face].edge.push(edge);
            self.ses.fa[contact].edge.push(edge);
        }
    }

    fn vertex_exists(&self, point: Vec3) -> i32 {
        let epsilon = set_eps(0.001);
        if let Some(items) = self.vertex_grid.neighbourhood(point.to_f32()) {
            for d in items {
                if self.ses.va[self.ses.vertex(d as usize)].point.eq_eps(point) {
                    set_eps(epsilon);
                    return d;
                }
            }
        }
        set_eps(epsilon);
        -1
    }

    // ---- SESSingularityCleaner ----

    fn cleaner_run(&mut self) -> BallResult<bool> {
        if !self.treat_first_category()? {
            return Ok(false);
        }
        if self.ses.number_of_singular_edges > 0 {
            self.treat_second_category()?;
        }
        Ok(true)
    }

    fn treat_first_category(&mut self) -> BallResult<bool> {
        let first_category_faces = self.first_category_faces();
        let mut modified = false;
        for pair in first_category_faces.chunks(2) {
            let [face1, face2] = [pair[0], pair[1]];
            match self.ses.fa[face1].edge.len() {
                3 => {
                    self.no_cut(face1, face2);
                }

                7 => {
                    self.two_cuts(face1, face2)?;
                }
                9 => {
                    let (r1, r2) = (self.ses.fa[face1].rsface(), self.ses.fa[face2].rsface());
                    self.rs.delete_similar_faces(r1, r2)?;
                    modified = true;
                }
                _ => {}
            }
        }
        if modified {
            self.rs.clean();
            Ok(false)
        } else {
            Ok(true)
        }
    }

    fn first_category_faces(&self) -> Vec<SF> {
        let mut singular_faces: Vec<SF> = self
            .ses
            .spheric_faces
            .iter()
            .map(|f| f.expect("spheric face"))
            .filter(|&f| self.rs.fa[self.ses.fa[f].rsface()].singular)
            .collect();
        let mut result = Vec::new();
        let mut start = 0;
        while start < singular_faces.len() {
            let current = singular_faces[start];
            start += 1;
            let rc = self.ses.fa[current].rsface();
            if let Some(pos) = singular_faces[start..]
                .iter()
                .position(|&f| self.rs.faces_similar(rc, self.ses.fa[f].rsface()))
            {
                let other = singular_faces.remove(start + pos);
                result.push(current);
                result.push(other);
            }
        }
        result
    }

    fn no_cut(&mut self, face1: SF, face2: SF) {
        let probe_radius = self.rs.probe_radius;
        let rf1 = self.ses.fa[face1].rsface();
        let rf2 = self.ses.fa[face2].rsface();
        let s1 = Sphere::new(self.rs.fa[rf1].center, probe_radius);
        let s2 = Sphere::new(self.rs.fa[rf2].center, probe_radius);
        let circle = intersect_spheres(&s1, &s2).unwrap_or_default();
        let normal = self.rs.fa[rf1].normal;
        let atom = |k: usize| self.rs.atom[self.rs.va[self.rs.fa[rf1].vertex[k]].atom as usize].p;
        let (point1, point2, point3) = (atom(0), atom(1), atom(2));
        let u = normal.cross(point1 - point2);
        let v = normal.cross(point2 - point3);
        let w = normal.cross(point3 - point1);
        let diff1 = point1 - circle.p;
        let diff2 = point2 - circle.p;
        let test1 = u.dot(diff1);
        let test2 = v.dot(diff2);
        let test3 = w.dot(diff1);
        if (is_less(test1, 0.0) && is_less(test2, 0.0) && is_less(test3, 0.0))
            || (is_greater(test1, 0.0) && is_greater(test2, 0.0) && is_greater(test3, 0.0))
        {
            let edge = self.ses.push_edge(SesEdge {
                vertex: [None, None],
                face: [Some(face1), Some(face2)],
                circle,
                rsedge: None,
                ty: EdgeType::Singular,
                index: -1,
            });
            self.ses.singular_edges.push(edge);
            self.ses.fa[face1].edge.push(edge);
            self.ses.fa[face2].edge.push(edge);
        }
    }

    fn two_cuts(&mut self, face1: SF, face2: SF) -> BallResult<()> {
        let (sesedge1, sesedge2, sesvertex1, _sesvertex2) = self.sort_faces(face1, face2)?;
        let probe_radius = self.rs.probe_radius;
        let sphere1 = Sphere::new(self.rs.fa[self.ses.fa[face1].rsface()].center, probe_radius);
        let sphere2 = Sphere::new(self.rs.fa[self.ses.fa[face2].rsface()].center, probe_radius);
        let mut circle = intersect_spheres(&sphere1, &sphere2).unwrap_or_default();
        let sv = |k: usize| -> BallResult<SV> {
            sesvertex1[k].ok_or_else(|| "BALL: unsorted faces".to_string())
        };
        let phi = oriented_angle(
            self.ses.va[sv(0)?].point - circle.p,
            self.ses.va[sv(2)?].point - circle.p,
            circle.n,
        )?;
        if phi > PI {
            circle.n.negate();
        }
        for (a, b) in [(0, 2), (3, 6)] {
            let edge = self.ses.push_edge(SesEdge {
                vertex: [Some(sv(a)?), Some(sv(b)?)],
                face: [Some(face1), Some(face2)],
                circle,
                rsedge: None,
                ty: EdgeType::Singular,
                index: -1,
            });
            self.ses.singular_edges.push(edge);
            self.ses.fa[face1].edge.push(edge);
            self.ses.fa[face2].edge.push(edge);
            self.ses.va[sv(a)?].edges.insert(edge);
            self.ses.va[sv(b)?].edges.insert(edge);
            self.ses.number_of_singular_edges += 1;
        }
        for (k, a, b) in [(2, 2, 3), (6, 6, 0)] {
            if sesedge1[k] == sesedge2[k] {
                let e = sesedge1[k].ok_or("BALL: unsorted faces")?;
                let index = self.ses.ea[e].index as usize;
                self.ses.edges[index] = None;
                self.ses.remove_singular(e);
                self.ses.va[sv(a)?].edges.erase(e);
                self.ses.va[sv(b)?].edges.erase(e);
                self.ses.fa[face1].remove_edge(e);
                self.ses.fa[face2].remove_edge(e);
            }
        }
        Ok(())
    }

    /// `SESSingularityCleaner::sort(face1, face2, ...)`.
    #[allow(clippy::type_complexity)]
    fn sort_faces(
        &self,
        face1: SF,
        face2: SF,
    ) -> BallResult<(
        [Option<SE>; 7],
        [Option<SE>; 7],
        [Option<SV>; 7],
        [Option<SV>; 7],
    )> {
        let ses = &*self.ses;
        let mut se1: [Option<SE>; 7] = [None; 7];
        let mut se2: [Option<SE>; 7] = [None; 7];
        let mut sv1: [Option<SV>; 7] = [None; 7];
        let mut sv2: [Option<SV>; 7] = [None; 7];
        let common = ses.fa[face1]
            .vertex
            .iter()
            .find(|v| ses.fa[face2].vertex.contains(v))
            .ok_or("BALL: singular faces share no vertex")?;
        sv1[0] = Some(*common);
        sv2[0] = Some(*common);
        {
            let (mut a, mut b) = (se1[0], se1[1]);
            ses.face_edges_of(face1, sv1[0], &mut a, &mut b);
            se1[0] = a;
            se1[1] = b;
            let (mut a, mut b) = (se2[0], se2[1]);
            ses.face_edges_of(face2, sv2[0], &mut a, &mut b);
            se2[0] = a;
            se2[1] = b;
        }
        let edge = |e: Option<SE>| -> BallResult<&SesEdge> {
            Ok(&ses.ea[e.ok_or("BALL: unsorted faces")?])
        };
        if edge(se1[0])?.same_ends(edge(se2[1])?) {
            se1[0] = se1[1];
        } else if edge(se1[1])?.same_ends(edge(se2[0])?) {
            se2[0] = se2[1];
        } else if edge(se1[0])?.same_ends(edge(se2[0])?) {
            se1[0] = se1[1];
            se2[0] = se2[1];
        }
        sv1[1] = edge(se1[0])?.other_vertex(sv1[0])?;
        sv2[1] = edge(se2[0])?.other_vertex(sv2[0])?;
        let mut sesedge: Option<SE> = None;
        for i in 1..6 {
            let mut a = se1[i];
            ses.face_edges_of(face1, sv1[i], &mut a, &mut sesedge);
            se1[i] = a;
            if sesedge != se1[i - 1] {
                se1[i] = sesedge;
            }
            let mut a = se2[i];
            ses.face_edges_of(face2, sv2[i], &mut a, &mut sesedge);
            se2[i] = a;
            if sesedge != se2[i - 1] {
                se2[i] = sesedge;
            }
            sv1[i + 1] = edge(se1[i])?.other_vertex(sv1[i])?;
            sv2[i + 1] = edge(se2[i])?.other_vertex(sv2[i])?;
        }
        let mut e = se1[6];
        ses.face_edge_between(face1, sv1[0], sv1[6], &mut e);
        se1[6] = e;
        let mut e = se2[6];
        ses.face_edge_between(face2, sv2[0], sv2[6], &mut e);
        se2[6] = e;
        if sv1[2] != sv2[2] {
            for i in 0..3 {
                sv1.swap(i, 6 - i);
                sv2.swap(i, 6 - i);
                se1.swap(i, 5 - i);
                se2.swap(i, 5 - i);
            }
        }
        Ok((se1, se2, sv1, sv2))
    }

    fn treat_second_category(&mut self) -> BallResult<()> {
        let centers: Vec<Vec3> = (0..self.ses.spheric_faces.len())
            .map(|i| self.rs.fa[self.ses.fa[self.ses.spheric(i)].rsface()].center)
            .collect();
        let first = centers[0];
        let (mut x_min, mut y_min, mut z_min) = (first.x, first.y, first.z);
        let (mut x_max, mut y_max, mut z_max) = (first.x, first.y, first.z);
        let std_max = |a: f64, b: f64| if a < b { b } else { a };
        let std_min = |a: f64, b: f64| if b < a { b } else { a };
        for c in &centers[1..] {
            x_min = std_min(x_min, c.x);
            y_min = std_min(y_min, c.y);
            z_min = std_min(z_min, c.z);
            x_max = std_max(x_max, c.x);
            y_max = std_max(y_max, c.y);
            z_max = std_max(z_max, c.z);
        }
        let dist = 2.0 * self.rs.probe_radius;
        let nx = ((x_max - x_min) / dist + 5.0) as u32;
        let ny = ((y_max - y_min) / dist + 5.0) as u32;
        let nz = ((z_max - z_min) / dist + 5.0) as u32;
        let origin = [
            (x_min - 2.0 * dist) as f32,
            (y_min - 2.0 * dist) as f32,
            (z_min - 2.0 * dist) as f32,
        ];
        let mut grid: HashGrid<u32> = HashGrid::new(origin, [dist as f32; 3], [nx, ny, nz]);
        for (i, c) in centers.iter().enumerate() {
            grid.insert(c.to_f32(), i as u32);
        }
        let mut deletable_edges = Vec::new();
        self.ses.singular_cursor = 0;
        while self.ses.singular_cursor < self.ses.singular_edges.len() {
            let edge = self.ses.singular_edges[self.ses.singular_cursor];
            self.treat_singular_edge(edge, &grid, &mut deletable_edges)?;
            self.ses.singular_cursor += 1;
        }
        for edge in deletable_edges {
            let e = &self.ses.ea[edge];
            let (f0, f1) = (
                e.face[0].expect("singular edge face"),
                e.face[1].expect("singular edge face"),
            );
            let (v0, v1) = (
                e.vertex[0].expect("singular edge vertex"),
                e.vertex[1].expect("singular edge vertex"),
            );
            let index = e.index as usize;
            self.ses.fa[f0].remove_edge(edge);
            self.ses.fa[f1].remove_edge(edge);
            self.ses.va[v0].edges.erase(edge);
            self.ses.va[v1].edges.erase(edge);
            self.ses.edges[index] = None;
            self.ses.remove_singular(edge);
        }
        Ok(())
    }

    fn treat_singular_edge(
        &mut self,
        edge: SE,
        grid: &HashGrid<u32>,
        deletable: &mut Vec<SE>,
    ) -> BallResult<()> {
        let e = &self.ses.ea[edge];
        let Some(ev0) = e.vertex[0] else {
            return Ok(());
        };
        let ev1 = e.vertex[1].ok_or("BALL: singular edge without vertex")?;
        let phi = oriented_angle(
            self.ses.va[ev0].point - e.circle.p,
            self.ses.va[ev1].point - e.circle.p,
            e.circle.n,
        )?;
        let intersections = self.intersections_of_singular_edge(edge, phi, grid)?;
        if intersections.is_empty() {
            return Ok(());
        }

        let (min, max) = Self::extrema(&intersections);
        let mut indices = BallIntSet::new();
        for i in min.iter().chain(max.iter()) {
            indices.insert(i.1);
        }
        let mut face1 = self.ses.fa[self.ses.ea[edge].face[0].expect("singular edge face")].index;
        let mut face2 = self.ses.fa[self.ses.ea[edge].face[1].expect("singular edge face")].index;
        indices.insert(face1);
        indices.insert(face2);
        let (end_vertex1, actual_min) = self.build_end_edge(edge, &min, true)?;
        let (end_vertex2, actual_max) = self.build_end_edge(edge, &max, false)?;
        for round in 0..2 {
            if round == 1 {
                face2 = face1;
                face1 = self.ses.fa[self.ses.ea[edge].face[1].expect("singular edge face")].index;
            }
            let mut next_face = actual_min;
            let mut vertex = Some(end_vertex1);
            while next_face != face2 && vertex.is_some() {
                self.build_edge(
                    edge,
                    face1,
                    &mut next_face,
                    face2,
                    &mut vertex,
                    &indices,
                    true,
                )?;
            }
            if next_face != face2 {
                next_face = actual_max;
                vertex = Some(end_vertex2);
                while next_face != face2 && vertex.is_some() {
                    self.build_edge(
                        edge,
                        face1,
                        &mut next_face,
                        face2,
                        &mut vertex,
                        &indices,
                        false,
                    )?;
                }
            }
        }
        deletable.push(edge);
        Ok(())
    }

    fn extrema(intersections: &[Intersection]) -> (Vec<Intersection>, Vec<Intersection>) {
        let mut min = Vec::new();
        let mut max = Vec::new();
        let mut min_phi = 2.0 * PI;
        let mut max_phi = 0.0;
        let epsilon = set_eps(0.0001);
        for &i in intersections {
            if is_less_or_equal(i.0, min_phi) {
                if is_less(i.0, min_phi) {
                    min.clear();
                    min_phi = i.0;
                }
                min.push(i);
            }
            if is_greater_or_equal(i.0, max_phi) {
                if is_greater(i.0, max_phi) {
                    max.clear();
                    max_phi = i.0;
                }
                max.push(i);
            }
        }
        set_eps(epsilon);
        (min, max)
    }

    fn intersections_of_singular_edge(
        &mut self,
        edge: SE,
        phi: f64,
        grid: &HashGrid<u32>,
    ) -> BallResult<Vec<Intersection>> {
        let e = self.ses.ea[edge].clone();
        let v0 = self.ses.va[e.vertex[0].expect("singular edge vertex")].point;
        let rotation = rotation_matrix(e.circle.n, phi / 2.0);
        let middle4 = rotation.mul_vec(Vec4 {
            x: v0.x - e.circle.p.x,
            y: v0.y - e.circle.p.y,
            z: v0.z - e.circle.p.z,
            h: 0.0,
        });
        let middle = Vec3::new(
            middle4.x + e.circle.p.x,
            middle4.y + e.circle.p.y,
            middle4.z + e.circle.p.z,
        );
        let face1 = self.ses.fa[e.face[0].expect("singular edge face")].index;
        let face2 = self.ses.fa[e.face[1].expect("singular edge face")].index;
        let mut intersections = Vec::new();
        let (mut phi1, mut phi2) = (0.0, 0.0);
        let (mut point1, mut point2) = (Vec3::ZERO, Vec3::ZERO);
        let probe_radius = self.rs.probe_radius;
        if let Some(items) = grid.neighbourhood(e.circle.p.to_f32()) {
            for d in items {
                if d as i32 != face1 && d as i32 != face2 {
                    let sf = self.ses.spheric(d as usize);
                    let probe_index = self.ses.fa[sf].index as u32;
                    if self.intersection_points_and_angles(
                        &e.circle,
                        v0,
                        face1 as u32,
                        face2 as u32,
                        probe_index,
                        &mut phi1,
                        &mut point1,
                        &mut phi2,
                        &mut point2,
                    )? {
                        let probe =
                            Sphere::new(self.rs.fa[self.ses.fa[sf].rsface()].center, probe_radius);
                        if Self::is_intersection(phi1, phi2, phi, middle, &probe) {
                            intersections.push((phi1, d as i32, point1));
                            intersections.push((phi2, d as i32, point2));
                        }
                    }
                }
            }
        }
        Ok(intersections)
    }

    #[allow(clippy::too_many_arguments)]
    fn intersection_points_and_angles(
        &mut self,
        circle: &Circle,
        point: Vec3,
        index1: u32,
        index2: u32,
        probe_index: u32,
        phi1: &mut f64,
        point1: &mut Vec3,
        phi2: &mut f64,
        point2: &mut Vec3,
    ) -> BallResult<bool> {
        if !self.probe_intersection(index1, index2, probe_index, point1, point2) {
            return Ok(false);
        }
        *phi1 = oriented_angle(point - circle.p, *point1 - circle.p, circle.n)?;
        *phi2 = oriented_angle(point - circle.p, *point2 - circle.p, circle.n)?;
        let epsilon = set_eps(0.001);
        if is_equal(*phi1, 2.0 * PI) {
            *phi1 = 0.0;
        }
        if is_equal(*phi2, 2.0 * PI) {
            *phi2 = 0.0;
        }
        set_eps(epsilon);
        if is_less(*phi2, *phi1) {
            std::mem::swap(phi1, phi2);
            std::mem::swap(point1, point2);
        }
        Ok(true)
    }

    fn is_intersection(min_phi: f64, max_phi: f64, phi: f64, middle: Vec3, probe: &Sphere) -> bool {
        let epsilon = set_eps(0.001);
        let back = if is_greater(max_phi, phi) {
            false
        } else if is_not_zero(min_phi) || is_less(max_phi, phi) {
            true
        } else {
            let epsilon = set_eps(1e-6);
            let back = !is_greater(probe.p.square_distance(middle), probe.radius * probe.radius);
            set_eps(epsilon);
            back
        };
        set_eps(epsilon);
        back
    }

    fn build_end_edge(
        &mut self,
        edge: SE,
        extrema: &[Intersection],
        min: bool,
    ) -> BallResult<(SV, i32)> {
        let mut vertex = None;
        let mut actual_extremum = 0;
        for m in extrema {
            let test = self.vertex_exists(m.2);
            if test != -1 {
                vertex = Some(self.ses.vertex(test as usize));
                actual_extremum = m.1;
            }
        }
        let vertex = match vertex {
            Some(v) => v,
            None => {
                let mut absolute = *extrema.first().ok_or("BALL: no extremum")?;
                for m in extrema {
                    if (min && m.0 < absolute.0) || (!min && m.0 > absolute.0) {
                        absolute = *m;
                    }
                }
                actual_extremum = absolute.1;
                let circle_p = self.ses.ea[edge].circle.p;
                let index = self.ses.vertices.len() as i32;
                let v = self.ses.push_vertex(SesVertex {
                    point: absolute.2,
                    normal: circle_p - absolute.2,
                    atom: -2,
                    index,
                    edges: OrderedSet::new(),
                    faces: OrderedSet::new(),
                });
                self.vertex_grid.insert(absolute.2.to_f32(), index);
                v
            }
        };
        let v = if min { 0 } else { 1 };
        if Some(vertex) != self.ses.ea[edge].vertex[v] {
            let mut new_edge = self.ses.ea[edge].clone();
            new_edge.vertex[1 - v] = Some(vertex);
            new_edge.rsedge = None;
            let ne = self.ses.push_edge(new_edge);
            self.ses.push_front_singular(ne);
            self.ses.number_of_singular_edges += 1;
            let e = self.ses.ea[ne].clone();
            let [v0, v1] = e.vertex.map(|x| x.expect("singular edge vertex"));
            let [f0, f1] = e.face.map(|x| x.expect("singular edge face"));
            self.ses.va[v0].edges.insert(ne);
            self.ses.va[v1].edges.insert(ne);
            self.ses.fa[f0].edge.push(ne);
            self.ses.fa[f1].edge.push(ne);
            let other = e.vertex[1 - v].expect("singular edge vertex");
            self.ses.fa[f0].insert_vertex(other);
            self.ses.fa[f1].insert_vertex(other);
            self.ses.va[vertex].faces.insert(f0);
            self.ses.va[vertex].faces.insert(f1);
        }
        Ok((vertex, actual_extremum))
    }

    #[allow(clippy::too_many_arguments)]
    fn build_edge(
        &mut self,
        edge: SE,
        face1: i32,
        face2: &mut i32,
        end: i32,
        vertex: &mut Option<SV>,
        indices: &BallIntSet,
        minimum: bool,
    ) -> BallResult<()> {
        let spheric_face1 = self.ses.spheric(face1 as usize);
        let spheric_face2 = self.ses.spheric(*face2 as usize);
        let probe_radius = self.rs.probe_radius;
        let probe1 = Sphere::new(
            self.rs.fa[self.ses.fa[spheric_face1].rsface()].center,
            probe_radius,
        );
        let probe2 = Sphere::new(
            self.rs.fa[self.ses.fa[spheric_face2].rsface()].center,
            probe_radius,
        );
        let mut circle = intersect_spheres(&probe1, &probe2).unwrap_or_default();
        let sign: f64 = if minimum { -1.0 } else { 1.0 };
        let edge_circle = self.ses.ea[edge].circle;
        if (probe1.p - edge_circle.p).dot(edge_circle.n)
            * (probe1.p - circle.p).dot(circle.n)
            * sign
            > 0.0
        {
            circle.n.negate();
        }
        let current = vertex.expect("build_edge needs a vertex");
        let point = self.ses.va[current].point;
        let mut point1 = Vec3::ZERO;
        let mut point2 = Vec3::ZERO;
        let mut phi1 = 0.0;
        let mut phi2 = 0.0;
        let mut min: Vec<(Vec3, i32)> = Vec::new();
        let mut min_phi = 2.0 * PI;
        let epsilon = set_eps(0.001);
        for i in indices.iter() {
            if i != face1 && i != *face2 {
                let found = self.intersection_points_and_angles(
                    &circle,
                    point,
                    face1 as u32,
                    *face2 as u32,
                    i as u32,
                    &mut phi1,
                    &mut point1,
                    &mut phi2,
                    &mut point2,
                );
                let found = match found {
                    Ok(found) => found,
                    Err(e) => return Err(e),
                };
                if found {
                    if is_less_or_equal(phi1, min_phi) && is_greater(phi1, 0.0) {
                        if is_less(phi1, min_phi) {
                            min.clear();
                        }
                        min_phi = phi1;
                        min.push((point1, i));
                    }
                    if is_less_or_equal(phi2, min_phi) && is_greater(phi2, 0.0) {
                        if is_less(phi2, min_phi) {
                            min.clear();
                        }
                        min_phi = phi2;
                        min.push((point2, i));
                    }
                }
            }
        }
        set_eps(epsilon);
        let mut new_vertex = None;
        let mut not_found = true;
        for m in &min {
            if !not_found {
                break;
            }
            if m.1 == end {
                point2 = m.0;
                *face2 = end;
                let index = self.vertex_exists(point2);
                if index != -1 {
                    new_vertex = Some(self.ses.vertex(index as usize));
                }
                not_found = false;
            }
        }
        if not_found {
            let mut index = -1;
            for m in &min {
                if index != -1 {
                    break;
                }
                point2 = m.0;
                *face2 = m.1;
                index = self.vertex_exists(point2);
                if index != -1 {
                    new_vertex = Some(self.ses.vertex(index as usize));
                }
            }
        }
        if !self.ses.is_neighboured_to(spheric_face1, spheric_face2) {
            let new_vertex = match new_vertex {
                Some(v) => v,
                None => {
                    let index = self.ses.vertices.len() as i32;
                    let v = self.ses.push_vertex(SesVertex {
                        point: point2,
                        normal: circle.p - point2,
                        atom: -2,
                        index,
                        edges: OrderedSet::new(),
                        faces: OrderedSet::new(),
                    });
                    self.vertex_grid.insert(point2.to_f32(), index);
                    v
                }
            };
            let ne = self.ses.push_edge(SesEdge {
                vertex: [Some(current), Some(new_vertex)],
                face: [Some(spheric_face1), Some(spheric_face2)],
                circle,
                rsedge: None,
                ty: EdgeType::Singular,
                index: -1,
            });
            self.ses.singular_edges.push(ne);
            self.ses.number_of_singular_edges += 1;
            self.ses.fa[spheric_face1].edge.push(ne);
            self.ses.fa[spheric_face2].edge.push(ne);
            self.ses.va[current].edges.insert(ne);
            self.ses.va[new_vertex].edges.insert(ne);
            self.ses.fa[spheric_face1].insert_vertex(current);
            self.ses.fa[spheric_face2].insert_vertex(current);
            self.ses.fa[spheric_face1].insert_vertex(new_vertex);
            self.ses.fa[spheric_face2].insert_vertex(new_vertex);
            for v in [current, new_vertex] {
                self.ses.va[v].faces.insert(spheric_face1);
                self.ses.va[v].faces.insert(spheric_face2);
            }
            *vertex = Some(new_vertex);
        } else {
            *vertex = None;
        }
        Ok(())
    }

    fn probe_intersection(
        &mut self,
        face1: u32,
        face2: u32,
        face3: u32,
        point1: &mut Vec3,
        point2: &mut Vec3,
    ) -> bool {
        let mut s = [face1, face2, face3];
        if s[0] > s[1] {
            s.swap(0, 1);
        }
        if s[0] > s[2] {
            s.swap(0, 2);
        }
        if s[1] > s[2] {
            s.swap(1, 2);
        }
        let key = (s[0], s[1], s[2]);
        if let Some(entry) = self.probe_intersections.get(&key) {
            return match entry {
                Some((a, b)) => {
                    *point1 = *a;
                    *point2 = *b;
                    true
                }
                None => false,
            };
        }
        let probe_radius = self.rs.probe_radius;
        let sphere = |i: u32| {
            Sphere::new(
                self.rs.fa[self.ses.fa[self.ses.spheric(i as usize)].rsface()].center,
                probe_radius,
            )
        };
        let (s1, s2, s3) = (sphere(s[0]), sphere(s[1]), sphere(s[2]));
        if intersect_three_spheres(&s1, &s2, &s3, point1, point2, false) {
            self.probe_intersections
                .insert(key, Some((*point1, *point2)));
            true
        } else {
            self.probe_intersections.insert(key, None);
            false
        }
    }
}
