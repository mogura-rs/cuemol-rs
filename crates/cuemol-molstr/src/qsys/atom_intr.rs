use super::*;
use std::borrow::Cow;

impl AtomIntrLine {
    pub fn with_objects(mut self, owner: Uid, objects: Vec<(Uid, String)>) -> Self {
        if objects.iter().any(|(uid, _)| *uid != owner) {
            self.object_refs = objects
                .into_iter()
                .map(|(uid, name)| {
                    if uid == owner {
                        (None, String::new())
                    } else {
                        (Some(uid), name)
                    }
                })
                .collect();
        }
        self
    }
}

impl MolScene {
    /// Resolve live measurement references into world coordinates without
    /// cloning molecule storage or replacing the saved atom references.
    pub fn resolve_atom_intr_renderer<'a>(
        &self,
        owner: &MolObject,
        renderer: &'a MolRenderer,
    ) -> Cow<'a, MolRenderer> {
        if renderer.renderer_type() != "atomintr"
            || (owner.xform_matrix() == Matrix4::IDENTITY
                && renderer
                    .atom_intr_lines
                    .iter()
                    .all(|line| line.object_refs.is_empty()))
        {
            return Cow::Borrowed(renderer);
        }
        let mut resolved = renderer.clone();
        for line in &mut resolved.atom_intr_lines {
            let points = if !line.positions.is_empty() {
                Some(line.positions.clone())
            } else if line.object_refs.is_empty() {
                owner.atom_intr_points(line).map(|points| {
                    points
                        .into_iter()
                        .map(|mut p| {
                            owner.xform_matrix().xform3d(&mut p);
                            p
                        })
                        .collect()
                })
            } else {
                (0..line.kind.atom_count())
                    .map(|index| {
                        let (uid, name) = line.object_refs.get(index)?;
                        let object = match uid {
                            Some(uid) => self.object(*uid)?,
                            None if name.is_empty() => owner,
                            None => self.object_by_name(name)?,
                        };
                        let point = if let Some(aid) = line.aids.get(index) {
                            object.resolve_atom(aid)?.position
                        } else if let Some(selection) = line.selectors.get(index) {
                            let selection = crate::parse_selector(selection).ok()?;
                            let indices = object.mol.select_indices(&selection);
                            if indices.is_empty() {
                                return None;
                            }
                            indices
                                .iter()
                                .map(|i| object.mol.atoms()[*i].position)
                                .fold(Vector4::ZERO, |sum, p| sum + p)
                                / indices.len() as f64
                        } else {
                            *line.positions.get(index)?
                        };
                        let mut point = point;
                        object.xform_matrix().xform3d(&mut point);
                        Some(point)
                    })
                    .collect::<Option<Vec<_>>>()
            };
            match points {
                Some(points) => line.positions = points,
                None => line.active = false,
            }
            line.object_refs.clear();
        }
        Cow::Owned(resolved)
    }

    pub(super) fn bind_atom_intr_objects(&mut self) {
        let mut ids = BTreeMap::new();
        for object in &self.objects {
            ids.entry(object.name.clone()).or_insert(object.uid);
        }
        for object in &mut self.objects {
            for renderer in &mut object.renderers {
                for line in &mut renderer.atom_intr_lines {
                    for (uid, name) in &mut line.object_refs {
                        if uid.is_none() && !name.is_empty() {
                            *uid = ids.get(name).copied();
                        }
                    }
                }
            }
        }
    }
}
