//! Cached partial control edits share the same geometry operation as commits.
use super::display_cache::DisplayGeometry;
use super::object_preview::TransformedObjects;
use super::*;
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    rc::Rc,
};
use viboceros_document::{GeometrySnapshot, Object};
use viboceros_geometry::AffineTransform3;

struct Entry {
    source: GeometrySnapshot,
    picks: BTreeSet<usize>,
    transform: AffineTransform3,
    tolerance: Tolerance,
    density: i32,
    geometry: Rc<DisplayGeometry>,
}

#[derive(Default)]
pub(super) struct GripPreviewCache {
    entries: BTreeMap<ObjectId, Entry>,
}

impl GripPreviewCache {
    pub(super) fn retain_visible(&mut self, visible: &HashSet<ObjectId>) {
        self.entries.retain(|id, _| visible.contains(id));
    }
}

impl Viewport {
    pub(super) fn grip_preview_geometry(
        &self,
        object: &Object,
        preview: TransformedObjects<'_>,
        tolerance: Tolerance,
    ) -> Option<Rc<DisplayGeometry>> {
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
                || entry.picks != picks
                || entry.transform != preview.transform
                || entry.tolerance != tolerance
                || entry.density != density
        });
        if stale {
            let geometry = object
                .geometry()
                .with_transformed_grips(&picks, preview.transform, tolerance)
                .ok()?;
            cache.entries.insert(
                object.id(),
                Entry {
                    source: object.geometry_snapshot().clone(),
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
            .grip_preview_geometry(doc.object(id).unwrap(), preview, doc.tolerance())
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
                .grip_preview_geometry(doc.object(id).unwrap(), preview, doc.tolerance())
                .unwrap();
            assert!(Rc::ptr_eq(&geometry, &reused));
        }
        assert_eq!(format!("{doc:?}"), before);
        let different = views[0]
            .grip_preview_geometry(
                doc.object(id).unwrap(),
                TransformedObjects {
                    transform: AffineTransform3::identity(),
                    ..preview
                },
                doc.tolerance(),
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
            .grip_preview_geometry(doc.object(id).unwrap(), preview, doc.tolerance())
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
                    doc.tolerance()
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
