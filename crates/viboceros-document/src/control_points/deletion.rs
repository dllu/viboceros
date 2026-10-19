//! Stage mixed owner deletion and grip reconstruction before one history commit.
use super::*;
use viboceros_geometry::{Brep, NurbsCurve, WeightedPoint3};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GripDeletionReport {
    pub deleted_objects: usize,
    pub deleted_control_points: usize,
    pub modified_objects: usize,
}
impl Document {
    pub fn delete_objects_and_grips(
        &mut self,
        objects: impl IntoIterator<Item = ObjectId>,
        grips: impl IntoIterator<Item = ControlPointId>,
    ) -> Result<GripDeletionReport, DocumentError> {
        let mut removed = objects.into_iter().collect::<BTreeSet<_>>();
        let mut picks = BTreeMap::<ObjectId, BTreeSet<usize>>::new();
        for p in grips {
            if !self.is_control_point_selectable(p) {
                return Err(DocumentError::InvalidControlPointSelection {
                    object: p.object,
                    index: p.index,
                });
            }
            picks.entry(p.object).or_default().insert(p.index);
        }
        for id in picks.keys() {
            removed.remove(id);
        }
        self.resolve_object_indices(removed.iter().copied())?;
        let mut staged = Vec::new();
        let mut before = Vec::new();
        for (index, object) in self.objects.iter().enumerate() {
            let Some(indices) = picks.get(&object.id) else {
                continue;
            };
            self.ensure_object_editable(object)?;
            let state = &self.control_points[&object.id];
            match delete_geometry(&object.geometry, indices, state.mode, self.tolerance)? {
                None => {
                    removed.insert(object.id);
                }
                Some(g)
                    if g != *object.geometry
                        || cyclic_refresh(&object.geometry, indices, state.mode)? =>
                {
                    before.push((object.id, state.clone(), matches!(g, Geometry::Mesh(_))));
                    staged.push((index, g));
                }
                Some(_) => {}
            }
        }
        if let Some(id) = self
            .block_edit
            .as_ref()
            .and_then(|edit| removed.iter().find(|id| edit.protects(**id)).copied())
        {
            return Err(DocumentError::ObjectLocked(id));
        }
        if staged.is_empty() && removed.is_empty() {
            return Ok(GripDeletionReport::default());
        }
        let owns = self.history.active.is_none();
        if owns {
            self.begin_transaction("Delete")?;
        }
        let result = (|| {
            self.commit_object_geometries(
                staged,
                "Delete",
                "Delete control points",
                ReplacementHistory::EveryReplacement,
                true,
            )?;
            let mut report = GripDeletionReport {
                modified_objects: before.len(),
                ..Default::default()
            };
            for (id, original, mesh) in &before {
                let current = self.control_points.get_mut(id).unwrap();
                report.deleted_control_points +=
                    original.points.len().saturating_sub(current.points.len());
                if *mesh || original.points.len() != current.points.len() {
                    current.selected = Arc::default();
                } else {
                    current.selected = original.selected.clone();
                }
                let mut stored = original.clone();
                if *mesh {
                    stored.selected = Arc::default();
                }
                self.record_edit(
                    "Delete control points",
                    Edit::ControlPointsDeleted {
                        id: *id,
                        stored: Some(stored),
                        mesh: *mesh,
                    },
                );
            }
            self.move_objects_to_end_in_order(before.iter().map(|(id, _, _)| *id))?;
            report.deleted_objects = self.delete_objects(removed)?;
            Ok(report)
        })();
        match result {
            Ok(report) => {
                if owns {
                    self.commit_transaction()?;
                }
                Ok(report)
            }
            Err(e) => {
                if owns {
                    self.rollback_transaction()?;
                }
                for (id, state, _) in before {
                    self.control_points.insert(id, state);
                }
                Err(e)
            }
        }
    }
}

fn cyclic_refresh(
    g: &Geometry,
    indices: &BTreeSet<usize>,
    mode: GripMode,
) -> Result<bool, GeometryError> {
    let Some(curve) = g.curve_ref() else {
        return Ok(false);
    };
    let count = g.grip_locations(mode)?.len();
    Ok(count == 3 && indices.len() == 1 && curve.is_closed()?)
}

fn delete_geometry(
    g: &Geometry,
    indices: &BTreeSet<usize>,
    mode: GripMode,
    tolerance: Tolerance,
) -> Result<Option<Geometry>, GeometryError> {
    let count = g.grip_locations(mode)?.len();
    if indices.len() == count {
        return Ok(None);
    }
    Ok(match g {
        Geometry::Mesh(m) => m
            .dissolved_vertices(&indices.iter().copied().collect::<Vec<_>>())?
            .map(Geometry::Mesh),
        Geometry::PolyCurve(c) => {
            let result = if mode == GripMode::Edit {
                c.try_delete_edit_grips(indices)
            } else {
                c.try_delete_grips(indices)
            }?;
            result.map(Geometry::PolyCurve)
        }
        Geometry::Polyline(c) => {
            if count - indices.len() < 2 {
                return Ok(Some(g.clone()));
            }
            let mut points = c.vertices().to_vec();
            let mut changed = false;
            for &i in indices.iter().rev() {
                let closed = points.first() == points.last();
                let n = points.len() - usize::from(closed);
                if closed && n <= 3 {
                    continue;
                }
                if closed && i == 0 {
                    let mut unique = points[1..n].to_vec();
                    unique.rotate_right(1);
                    unique.push(unique[0]);
                    points = unique;
                } else {
                    points.remove(i);
                }
                changed = true;
            }
            if !changed {
                return Ok(Some(g.clone()));
            }
            let knots = (0..points.len() + 2)
                .map(|i| i.saturating_sub(1).min(points.len() - 1) as f64)
                .collect();
            Some(Geometry::NurbsCurve(NurbsCurve::try_new_rational(
                1,
                points
                    .into_iter()
                    .map(|p| WeightedPoint3::try_new(p, 1.))
                    .collect::<Result<_, _>>()?,
                knots,
            )?))
        }
        Geometry::NurbsSurface(s) => s.try_delete_grips(indices)?.map(Geometry::NurbsSurface),
        Geometry::Brep(b) if b.faces().len() == 1 => {
            let face = &b.faces()[0];
            let old = face.surface();
            let Some(surface) = old.try_delete_grips(indices)? else {
                return Ok(None);
            };
            if surface == *old {
                return Ok(Some(g.clone()));
            }
            let full = face.rectangular_trim_bounds(tolerance)?.is_some_and(|v| {
                v == [
                    [*old.domain_u().start(), *old.domain_u().end()],
                    [*old.domain_v().start(), *old.domain_v().end()],
                ]
            });
            Some(Geometry::Brep(if full {
                let u = surface.domain_u();
                let v = surface.domain_v();
                Brep::try_rectangular_surface_face_with_orientation(
                    surface,
                    u,
                    v,
                    face.is_reversed(),
                    tolerance,
                )?
            } else {
                b.try_with_edited_single_surface(surface, tolerance)?
            }))
        }
        _ if g.curve_ref().is_some() => {
            let c = g.curve_ref().unwrap().to_nurbs()?;
            let result = if mode == GripMode::Edit {
                c.try_delete_edit_grips(indices)
            } else {
                c.try_delete_grips(indices)
            }?;
            result.map(|next| {
                if next == c {
                    g.clone()
                } else {
                    Geometry::NurbsCurve(next)
                }
            })
        }
        _ => {
            return Err(GeometryError::UnsupportedControlPointEdit {
                context: "deleting grips on this geometry",
            });
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mixed_grip_deletion_rejects_invalid_or_hidden_indices_before_deleting_any_peer() {
        let mut doc = Document::default();
        let curve = NurbsCurve::try_clamped_uniform(
            2,
            (0..5)
                .map(|i| Point3::try_new(i as f64, (i % 2) as f64, 0.).unwrap())
                .collect(),
        )
        .unwrap();
        let id = doc.add_geometry(Geometry::NurbsCurve(curve)).unwrap();
        let peer = doc
            .add_geometry(Geometry::Point(Point3::try_new(8., 0., 0.).unwrap()))
            .unwrap();
        doc.enable_control_points([id]).unwrap();
        let grip = |index| ControlPointId { object: id, index };
        doc.set_control_points_hidden([grip(2)], true).unwrap();
        doc.select_control_points([grip(0)], SelectionMode::Add)
            .unwrap();
        doc.select_objects_direct([peer], SelectionMode::Add)
            .unwrap();
        doc.add_geometry(Geometry::Point(Point3::try_new(30., 0., 0.).unwrap()))
            .unwrap();
        doc.undo().unwrap();
        let before = format!("{doc:?}");
        for picks in [vec![grip(0), grip(99)], vec![grip(0), grip(2)]] {
            assert!(doc.delete_objects_and_grips([peer], picks).is_err());
            assert_eq!(format!("{doc:?}"), before);
            assert!(doc.can_redo());
        }
        assert!(
            doc.delete_objects_and_grips([peer, ObjectId::new()], [grip(0)])
                .is_err()
        );
        assert_eq!(format!("{doc:?}"), before);
        let report = doc
            .delete_objects_and_grips([peer, peer], [grip(0), grip(0)])
            .unwrap();
        assert_eq!(
            report,
            GripDeletionReport {
                deleted_objects: 1,
                deleted_control_points: 1,
                modified_objects: 1
            }
        );
        assert!(doc.object(peer).is_none());
        assert!(!doc.can_redo());
        doc.undo().unwrap();
        assert!(doc.object(peer).is_some());
        assert_eq!(doc.control_point_locations(id).unwrap().len(), 5);
        assert!(doc.selected_control_points().any(|(p, _)| p == grip(0)));
        assert_eq!(doc.is_control_point_hidden(grip(2)), Some(true));
        doc.redo().unwrap();
        assert!(doc.object(peer).is_none());
        assert_eq!(doc.control_point_locations(id).unwrap().len(), 4);
    }
}
