//! Atomic control-net edits, retaining identities, metadata, and native grip rules.
use super::*;
use viboceros_geometry::{SmoothingCoordinates, SmoothingOptions};

impl Geometry {
    pub fn supports_smoothing(&self) -> bool {
        self.curve_ref().is_some()
            || matches!(self, Self::NurbsSurface(_) | Self::Mesh(_))
            || matches!(self, Self::Brep(b) if b.faces().len() == 1)
    }

    /// Curve primitives promote to NURBS even when all controls stay fixed.
    pub fn try_smoothed(
        &self,
        options: SmoothingOptions,
        coordinates: SmoothingCoordinates,
        selected: Option<&BTreeSet<usize>>,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        options.validate()?;
        if let Some(curve) = self.nurbs_curve_representation()? {
            return Ok(Self::NurbsCurve(curve.try_smoothed_in(
                options,
                coordinates,
                selected,
            )?));
        }
        Ok(match self {
            Self::NurbsSurface(surface) => {
                Self::NurbsSurface(surface.try_smoothed_in(options, coordinates, selected)?)
            }
            Self::Mesh(mesh) => Self::Mesh(mesh.try_smoothed_in(options, coordinates, selected)?),
            Self::Brep(brep) if brep.faces().len() == 1 => Self::Brep(
                brep.try_with_edited_single_surface(
                    brep.faces()[0]
                        .surface()
                        .try_smoothed_in(options, coordinates, selected)?,
                    tolerance,
                )?,
            ),
            _ => {
                return Err(GeometryError::UnsupportedControlPointEdit {
                    context: "Smooth requires curves, surfaces, or meshes",
                });
            }
        })
    }
}

impl Document {
    /// Stage the entire mixed object/grip edit before recording replacements.
    /// Grips take precedence over selected parents. Even unchanged geometry
    /// consumes one undoable replacement. A changed grip count clears picks.
    pub fn smooth_objects_and_grips(
        &mut self,
        objects: impl IntoIterator<Item = ObjectId>,
        grips: impl IntoIterator<Item = ControlPointId>,
        options: SmoothingOptions,
        coordinates: SmoothingCoordinates,
    ) -> Result<usize, DocumentError> {
        options.validate()?;
        let objects = objects.into_iter().collect::<BTreeSet<_>>();
        let mut picks = BTreeMap::<ObjectId, BTreeSet<usize>>::new();
        for grip in grips {
            if self
                .control_point_locations(grip.object)
                .is_none_or(|p| grip.index >= p.len())
            {
                return Err(DocumentError::InvalidControlPointSelection {
                    object: grip.object,
                    index: grip.index,
                });
            }
            picks.entry(grip.object).or_default().insert(grip.index);
        }
        let indices =
            self.resolve_object_indices(objects.iter().copied().chain(picks.keys().copied()))?;
        for &index in &indices {
            self.ensure_object_editable(&self.objects[index])?;
        }
        let order = indices
            .iter()
            .copied()
            .filter(|i| !picks.contains_key(&self.objects[*i].id))
            .chain(
                indices
                    .iter()
                    .copied()
                    .filter(|i| picks.contains_key(&self.objects[*i].id)),
            )
            .collect::<Vec<_>>();
        let owners = order
            .iter()
            .map(|&i| self.objects[i].id)
            .collect::<Vec<_>>();
        let staged = order
            .into_iter()
            .map(|index| {
                let object = &self.objects[index];
                Ok((
                    index,
                    object.geometry.try_smoothed(
                        options,
                        coordinates,
                        picks.get(&object.id),
                        self.tolerance,
                    )?,
                ))
            })
            .collect::<Result<Vec<_>, DocumentError>>()?;
        let before = owners
            .iter()
            .filter_map(|id| self.control_points.get(id).map(|s| (*id, s.clone())))
            .collect::<Vec<_>>();
        let owns_transaction = self.history.active.is_none();
        if owns_transaction {
            self.begin_transaction("Smooth")?;
        }
        let tolerance = self.tolerance;
        let result = (|| {
            let count = self.commit_object_geometries_with_text_policy(staged, "Smooth", "Smooth object", ReplacementHistory::EveryReplacement,
                |source, result| matches!(result, Geometry::Mesh(_))
                    || matches!(source.geometry(), Geometry::Brep(b) if b.faces()[0].is_untrimmed(tolerance) == Ok(false)),
            )?;
            for (id, old) in before {
                let state = self.control_points.get_mut(&id).unwrap();
                state.retain_selection_if_count_unchanged(&old);
                self.record_edit(
                    "Smooth",
                    Edit::ControlPointsChanged {
                        id,
                        stored: Some(old),
                    },
                );
            }
            self.move_objects_to_end_in_order(owners)?;
            Ok(count)
        })();
        if owns_transaction {
            if result.is_ok() {
                self.commit_transaction()?;
            } else {
                self.rollback_transaction()?;
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn curve() -> Geometry {
        Geometry::NurbsCurve(
            NurbsCurve::try_clamped_uniform(
                2,
                [[2., 0., 0.], [0., 2., 0.], [-2., 0., 0.], [0., -2., 0.]]
                    .map(|p| Point3::try_from(p).unwrap())
                    .to_vec(),
            )
            .unwrap(),
        )
    }

    #[test]
    fn invalid_or_uneditable_peers_and_overflow_leave_objects_grips_and_history_unchanged() {
        let mut doc = Document::default();
        let a = doc.add_geometry(curve()).unwrap();
        let b = doc.add_geometry(curve()).unwrap();
        let point = doc
            .add_geometry(Geometry::Point(Point3::try_new(8., 0., 0.).unwrap()))
            .unwrap();
        doc.enable_control_points([a]).unwrap();
        let grip = ControlPointId {
            object: a,
            index: 1,
        };
        doc.select_control_points([grip], SelectionMode::Add)
            .unwrap();
        doc.set_objects_locked([b], true).unwrap();
        doc.clear_history().unwrap();
        doc.add_geometry(curve()).unwrap();
        doc.undo().unwrap();
        let before = format!("{doc:?}");
        for (objects, grips, options) in [
            (vec![a, b], vec![], SmoothingOptions::default()),
            (vec![a, point], vec![], SmoothingOptions::default()),
            (
                vec![a, ObjectId::new()],
                vec![],
                SmoothingOptions::default(),
            ),
            (
                vec![a],
                vec![ControlPointId { index: 99, ..grip }],
                SmoothingOptions::default(),
            ),
            (
                vec![a],
                vec![],
                SmoothingOptions {
                    factor: f64::MAX,
                    fix_boundaries: false,
                    ..Default::default()
                },
            ),
            (
                vec![a],
                vec![],
                SmoothingOptions {
                    steps: 0,
                    ..Default::default()
                },
            ),
        ] {
            assert!(
                doc.smooth_objects_and_grips(objects, grips, options, SmoothingCoordinates::World)
                    .is_err()
            );
            assert_eq!(format!("{doc:?}"), before);
        }
        assert!(doc.can_redo());
    }

    #[test]
    fn grip_precedence_preserves_identity_attributes_groups_and_single_undo_even_for_noop() {
        let mut doc = Document::default();
        let id = doc.add_geometry(curve()).unwrap();
        let group = doc.add_group(Some("controls".into()), [id]).unwrap();
        doc.set_object_names([(id, Some("source".into()))]).unwrap();
        doc.set_object_user_text([id], "Code", Some("attribute"))
            .unwrap();
        doc.set_object_geometry_user_text([id], "Code", Some("geometry"))
            .unwrap();
        doc.enable_control_points([id]).unwrap();
        let grip = ControlPointId {
            object: id,
            index: 1,
        };
        doc.select_control_points([grip], SelectionMode::Add)
            .unwrap();
        doc.clear_history().unwrap();
        let before = doc.object(id).unwrap().clone();
        assert_eq!(
            doc.smooth_objects_and_grips(
                [id],
                [grip],
                SmoothingOptions::default(),
                SmoothingCoordinates::World
            )
            .unwrap(),
            1
        );
        let after = doc.object(id).unwrap().clone();
        assert_eq!(after.attributes(), before.attributes());
        assert_eq!(after.group_ids(), &[group]);
        assert!(after.geometry_user_text().is_empty());
        let Geometry::NurbsCurve(c) = after.geometry() else {
            panic!()
        };
        assert_eq!(
            c.control_points()[2].point(),
            Point3::try_new(-2., 0., 0.).unwrap()
        );
        assert_eq!(doc.selected_control_points().count(), 1);
        doc.undo().unwrap();
        assert_eq!(doc.object(id).unwrap(), &before);
        assert!(!doc.can_undo());
        doc.redo().unwrap();
        assert_eq!(doc.object(id).unwrap(), &after);
        doc.clear_history().unwrap();
        doc.smooth_objects_and_grips(
            [id],
            [],
            SmoothingOptions {
                factor: 0.,
                ..Default::default()
            },
            SmoothingCoordinates::World,
        )
        .unwrap();
        assert_eq!(doc.undo_label(), Some("Smooth"));
        doc.undo().unwrap();
        assert_eq!(doc.object(id).unwrap(), &after);
    }
}
