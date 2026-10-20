//! Cached partial control edits share the same geometry operation as commits.
use super::display_cache::DisplayGeometry;
use super::object_preview::TransformedObjects;
use super::*;
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    rc::Rc,
};
use viboceros_document::{GeometrySnapshot, GripMode, Object};
use viboceros_geometry::AffineTransform3;

struct Entry {
    source: GeometrySnapshot,
    mode: GripMode,
    picks: BTreeSet<usize>,
    transform: AffineTransform3,
    tolerance: Tolerance,
    density: i32,
    geometry: Rc<DisplayGeometry>,
}

struct WholeValidation {
    source: GeometrySnapshot,
    transform: AffineTransform3,
    tolerance: Tolerance,
    valid: bool,
}

#[derive(Default)]
pub(super) struct GripPreviewCache {
    entries: BTreeMap<ObjectId, Entry>,
    whole: BTreeMap<ObjectId, WholeValidation>,
}

impl GripPreviewCache {
    pub(super) fn retain_visible(&mut self, visible: &HashSet<ObjectId>) {
        self.entries.retain(|id, _| visible.contains(id));
        self.whole.retain(|id, _| visible.contains(id));
    }
}

impl Viewport {
    /// Singular SetPt maps must pass the same geometry edit as a commit. Cache
    /// success and failure across all views without preparing display meshes.
    pub(super) fn whole_edit_preview_is_valid(
        &self,
        object: &Object,
        transform: AffineTransform3,
        tolerance: Tolerance,
    ) -> bool {
        let mut cache = self.grip_preview_cache.borrow_mut();
        let stale = cache.whole.get(&object.id()).is_none_or(|entry| {
            !entry.source.shares_storage_with(object.geometry_snapshot())
                || entry.transform != transform
                || entry.tolerance != tolerance
        });
        if stale {
            let valid = object
                .geometry()
                .transformed_for_edit(transform, tolerance)
                .is_ok();
            cache.whole.insert(
                object.id(),
                WholeValidation {
                    source: object.geometry_snapshot().clone(),
                    transform,
                    tolerance,
                    valid,
                },
            );
        }
        cache.whole[&object.id()].valid
    }

    pub(super) fn grip_preview_geometry(
        &self,
        object: &Object,
        preview: TransformedObjects<'_>,
        document: &Document,
    ) -> Option<Rc<DisplayGeometry>> {
        let tolerance = document.tolerance();
        let mode = document.grip_mode(object.id())?;
        let picks = preview
            .grips
            .iter()
            .filter(|id| id.object == object.id())
            .map(|id| id.index)
            .collect::<BTreeSet<_>>();
        if picks.is_empty() {
            return None;
        }
        let density = object.attributes().wire_density();
        let mut cache = self.grip_preview_cache.borrow_mut();
        let stale = cache.entries.get(&object.id()).is_none_or(|entry| {
            !entry.source.shares_storage_with(object.geometry_snapshot())
                || entry.mode != mode
                || entry.picks != picks
                || entry.transform != preview.transform
                || entry.tolerance != tolerance
                || entry.density != density
        });
        if stale {
            let geometry = object
                .geometry()
                .with_transformed_grips_in_mode(&picks, preview.transform, tolerance, mode)
                .ok()?;
            cache.entries.insert(
                object.id(),
                Entry {
                    source: object.geometry_snapshot().clone(),
                    mode,
                    picks,
                    transform: preview.transform,
                    tolerance,
                    density,
                    geometry: Rc::new(DisplayGeometry::new(geometry.into(), density, tolerance)),
                },
            );
        }
        Some(Rc::clone(&cache.entries[&object.id()].geometry))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_document::ControlPointId;
    use viboceros_geometry::Vector3;

    #[test]
    fn set_point_whole_validity_cache_refreshes_geometry_and_is_shared_and_pruned() {
        let mut doc = Document::default();
        let p = |x| Point3::try_new(x, 0., 0.).unwrap();
        let id = doc
            .add_geometry(Geometry::Line(
                viboceros_geometry::LineSegment::try_new(p(0.), p(1.), doc.tolerance()).unwrap(),
            ))
            .unwrap();
        let transform = viboceros_command::set_point::SetPointOptions::default()
            .transform_at(
                p(5.),
                viboceros_command::CommandContext::default().construction_plane,
            )
            .unwrap();
        let views = Viewport::standard_views();
        for view in &views {
            assert!(!view.whole_edit_preview_is_valid(
                doc.object(id).unwrap(),
                transform,
                doc.tolerance()
            ));
            assert_eq!(view.grip_preview_cache.borrow().whole.len(), 1);
        }
        doc.replace_object_geometries([(id, Geometry::Point(p(2.)))])
            .unwrap();
        assert!(views[1].whole_edit_preview_is_valid(
            doc.object(id).unwrap(),
            transform,
            doc.tolerance()
        ));
        assert!(views[0].grip_preview_cache.borrow().whole[&id].valid);
        views[2]
            .grip_preview_cache
            .borrow_mut()
            .retain_visible(&HashSet::new());
        assert!(views[0].grip_preview_cache.borrow().whole.is_empty());
    }

    #[test]
    fn partial_preview_reuses_geometry_across_views_and_invalidates_edits() {
        let mut doc = Document::default();
        let points = [(2., 0.), (0., 2.), (-2., 0.), (0., -2.)]
            .map(|(x, y)| Point3::try_new(x, y, 0.).unwrap());
        let id = doc
            .add_geometry(Geometry::NurbsCurve(
                NurbsCurve::try_clamped_uniform(2, points.to_vec()).unwrap(),
            ))
            .unwrap();
        doc.enable_control_points([id]).unwrap();
        let grips = [ControlPointId {
            object: id,
            index: 0,
        }];
        let transform = AffineTransform3::from_translation(Vector3::try_new(1., 2., 3.).unwrap());
        let preview = TransformedObjects {
            rigid_layout: None,
            source_geometry: None,
            sources: &[],
            grips: &grips,
            copy: true,
            reference_sources: false,
            draw_source_faces: false,
            reversing: false,
            reference: None,
            transform,
        };
        let before = format!("{doc:?}");
        let views = Viewport::standard_views();
        let geometry = views[0]
            .grip_preview_geometry(doc.object(id).unwrap(), preview, &doc)
            .unwrap();
        let Geometry::NurbsCurve(curve) = &*geometry.geometry else {
            panic!("expected curve")
        };
        assert_eq!(
            curve.control_points()[0].point(),
            Point3::try_new(3., 2., 3.).unwrap()
        );
        assert_eq!(
            curve.control_points()[1..]
                .iter()
                .map(|p| p.point())
                .collect::<Vec<_>>(),
            points[1..]
        );
        for view in &views {
            let reused = view
                .grip_preview_geometry(doc.object(id).unwrap(), preview, &doc)
                .unwrap();
            assert!(Rc::ptr_eq(&geometry, &reused));
        }
        assert_eq!(format!("{doc:?}"), before);
        // A display-only mode switch leaves the geometry snapshot unchanged,
        // but changes the meaning of the same selected grip index.
        doc.enable_edit_points([id]).unwrap();
        let edit_geometry = views[0]
            .grip_preview_geometry(doc.object(id).unwrap(), preview, &doc)
            .unwrap();
        assert!(!Rc::ptr_eq(&geometry, &edit_geometry));
        let Geometry::NurbsCurve(edit_curve) = &*edit_geometry.geometry else {
            panic!("expected curve")
        };
        let source = doc
            .object(id)
            .unwrap()
            .geometry()
            .curve_ref()
            .unwrap()
            .to_nurbs()
            .unwrap();
        for (index, edit) in source.edit_points().unwrap().iter().enumerate() {
            let target = if index == 0 {
                transform.transform_point(edit.point).unwrap()
            } else {
                edit.point
            };
            assert!(
                edit_curve
                    .evaluate_on_side(edit.parameter, edit.side)
                    .unwrap()
                    .distance_to(target)
                    .unwrap()
                    < 1e-8
            );
        }
        let reused = views[1]
            .grip_preview_geometry(doc.object(id).unwrap(), preview, &doc)
            .unwrap();
        assert!(Rc::ptr_eq(&edit_geometry, &reused));
        doc.enable_control_points([id]).unwrap();
        let different = views[0]
            .grip_preview_geometry(
                doc.object(id).unwrap(),
                TransformedObjects {
                    transform: AffineTransform3::identity(),
                    ..preview
                },
                &doc,
            )
            .unwrap();
        assert!(!Rc::ptr_eq(&geometry, &different));
        doc.replace_object_geometries([(
            id,
            Geometry::NurbsCurve(
                NurbsCurve::try_clamped_uniform(
                    2,
                    points
                        .iter()
                        .map(|p| transform.transform_point(*p).unwrap())
                        .collect(),
                )
                .unwrap(),
            ),
        )])
        .unwrap();
        let edited = views[1]
            .grip_preview_geometry(doc.object(id).unwrap(), preview, &doc)
            .unwrap();
        assert!(!Rc::ptr_eq(&different, &edited));
        let invalid = [ControlPointId {
            object: id,
            index: 4,
        }];
        assert!(
            views[0]
                .grip_preview_geometry(
                    doc.object(id).unwrap(),
                    TransformedObjects {
                        grips: &invalid,
                        ..preview
                    },
                    &doc
                )
                .is_none()
        );
        views[0]
            .grip_preview_cache
            .borrow_mut()
            .retain_visible(&HashSet::new());
        assert!(views[1].grip_preview_cache.borrow().entries.is_empty());
    }
}
