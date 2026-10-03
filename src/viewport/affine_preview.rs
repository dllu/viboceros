//! Read-only instances for point-driven Scale, Rotate, Rotate3D, and Shear.
use super::object_preview::TransformedObjects;
use super::*;
use viboceros_command::point_transform::PointTransform;
use viboceros_geometry::AffineTransform3;

#[derive(Clone, Copy, Debug)]
pub(crate) struct AffinePreview<'a> {
    pub sources: &'a [ObjectId],
    pub definition: PointTransform,
    /// Scale2D uses the destination viewport; the other planar transforms retain their start plane.
    pub frame: Option<Frame3>,
    pub copy: bool,
    pub last_transform: Option<AffineTransform3>,
}

impl<'a> AffinePreview<'a> {
    pub(super) fn resolve(
        self,
        cursor: Option<Point3>,
        frame: Frame3,
        tolerance: Tolerance,
    ) -> (
        Option<TransformedObjects<'a>>,
        Option<Option<AffineTransform3>>,
    ) {
        // Native point transforms display identity at an invalid final reference,
        // while leaving the prompt active. Leaving the viewport retains the map.
        let update = cursor.map(|point| {
            self.definition
                .transform_at(self.frame.unwrap_or(frame), point, tolerance)
                .ok()
                .filter(|map| {
                    matches!(
                        self.definition,
                        PointTransform::Scale1DDirection { factor: 0., .. }
                    ) || map.orientation_reversing().is_ok()
                })
                .unwrap_or_else(AffineTransform3::identity)
        });
        let transform = update.or(self.last_transform);
        (
            transform.map(|transform| TransformedObjects {
                sources: self.sources,
                reference_sources: !self.copy,
                draw_source_faces: false,
                reversing: !matches!(
                    self.definition,
                    PointTransform::Scale1DDirection { factor: 0., .. }
                ) && transform.orientation_reversing().unwrap_or(false),
                reference: None,
                transform,
            }),
            cursor.map(|_| transform),
        )
    }
}

impl Viewport {
    pub(super) fn resolve_affine_preview<'a>(
        &self,
        preview: AffinePreview<'a>,
        cursor: Option<Point3>,
        document: &Document,
    ) -> (
        Option<TransformedObjects<'a>>,
        Option<Option<AffineTransform3>>,
    ) {
        let resolved = preview.resolve(cursor, self.construction_plane(), document.tolerance());
        if let Some(objects) = resolved.0
            && resolved.1.is_some()
        {
            let mut cache = self.display_cache.borrow_mut();
            let valid = objects.sources.iter().all(|id| {
                let Some(object) = document.object(*id) else {
                    return false;
                };
                let bounds = cache.get(object, document.tolerance()).bounds();
                let lo = bounds.min().to_array();
                let hi = bounds.max().to_array();
                (0..8).all(|bits| {
                    objects
                        .transform
                        .transform_point(
                            Point3::try_from(std::array::from_fn(|axis| {
                                if bits & (1 << axis) == 0 {
                                    lo[axis]
                                } else {
                                    hi[axis]
                                }
                            }))
                            .unwrap(),
                        )
                        .is_ok()
                })
            });
            if !valid {
                return (
                    preview
                        .resolve(None, self.construction_plane(), document.tolerance())
                        .0,
                    Some(preview.last_transform),
                );
            }
        }
        resolved
    }

    pub(super) fn affine_drafting_cursor(
        &self,
        pointer: Pos2,
        rect: Rect,
        document: &Document,
        drafting: DraftingInput,
        input: &ViewportInput<'_>,
    ) -> Option<drafting::DraftingCursor> {
        let filter = input.point_filter;
        let constraint = input.point_constraint;
        let definition = input.affine_preview.map(|p| p.definition);
        let line = definition.and_then(|p| p.mouse_line(document.tolerance()));
        let cursor = self.translation_drafting_cursor(
            pointer,
            rect,
            document,
            drafting,
            filter,
            constraint,
            input.translation_constraint.or(line),
        );
        let Some((anchor, normal)) = definition
            .and_then(|p| p.mouse_plane(document.tolerance()))
            .or_else(|| input.angle_plane.map(|p| (p.origin(), p.z_axis())))
        else {
            return cursor;
        };
        if filter.is_some() || cursor.is_some_and(|c| c.object_snap.is_some()) {
            return cursor;
        }
        let (origin, direction) = self.drafting_view_line_relative_to(anchor, pointer, rect)?;
        let denominator = direction.dot(normal.as_vector()).ok()?;
        if denominator.abs() <= 1e-12 * direction.length().ok()? {
            return None;
        }
        let distance = -origin.dot(normal.as_vector()).ok()? / denominator;
        let point = anchor
            .translated(
                Vector3::try_from(std::array::from_fn(|axis| {
                    direction.to_array()[axis].mul_add(distance, origin.to_array()[axis])
                }))
                .ok()?,
            )
            .ok()?;
        Some(drafting::DraftingCursor {
            pointer,
            source_point: point,
            point: constraint
                .map_or(Ok(point), |s| s.apply_cursor(point))
                .ok()?,
            object_snap: None,
            track: None,
            ortho: false,
            ortho_z: false,
            grid_snapped: false,
        })
    }
}

#[cfg(test)]
pub(crate) mod tests;

#[cfg(test)]
mod angle_plane_tests {
    use super::*;
    #[test]
    fn explicit_twist_angle_plane_resolves_free_mouse_picks_without_changing_cplane() {
        let viewport = Viewport::new(ViewKind::Perspective);
        let doc = Document::default();
        let before = viewport.construction_plane();
        let plane = Frame3::try_from_normal(
            Point3::try_new(0., 0., 0.).unwrap(),
            Vector3::try_new(1., 0., 0.).unwrap(),
            doc.tolerance(),
        )
        .unwrap();
        let rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(640., 480.));
        let pointer = Pos2::new(365., 260.);
        let cursor = viewport
            .affine_drafting_cursor(
                pointer,
                rect,
                &doc,
                DraftingInput {
                    active: true,
                    ..Default::default()
                },
                &ViewportInput {
                    angle_plane: Some(plane),
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(cursor.point.x().abs() < 1e-11);
        assert_eq!(viewport.construction_plane(), before);
    }
}
