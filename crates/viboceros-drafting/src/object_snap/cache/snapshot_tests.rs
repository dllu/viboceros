use super::*;
use viboceros_document::{ColorRgb, ReplacementHistory};
use viboceros_geometry::{Brep, PolyCurve3};

fn p(x: Real, y: Real) -> Point3 {
    Point3::try_new(x, y, 0.).unwrap()
}

fn curve() -> NurbsCurve {
    NurbsCurve::try_new(
        2,
        vec![p(2., -2.), p(3., -2.), p(8., -2.)],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap()
}

fn sources() -> [Geometry; 4] {
    let surface =
        NurbsSurface::try_bilinear([p(2., -2.), p(8., -2.), p(2., -6.), p(8., -6.)]).unwrap();
    [
        Geometry::NurbsCurve(curve()),
        Geometry::PolyCurve(PolyCurve3::try_new(vec![curve()]).unwrap()),
        Geometry::NurbsSurface(surface.clone()),
        Geometry::Brep(Brep::try_surface_face(surface, Tolerance::DEFAULT).unwrap()),
    ]
}

fn query(cache: &mut ObjectSnapCache, doc: &Document, mode: ObjectSnapModes) -> Option<ObjectSnap> {
    cache
        .nearest_projected_with_modes(doc, [5., -2.], 0.01, |p| Some([p.x(), p.y()]), mode)
        .unwrap()
}

#[test]
fn unchanged_snapshots_skip_source_scans_including_document_clones_and_style_edits() {
    for geometry in sources() {
        let mut doc = Document::default();
        let id = doc.add_geometry(geometry.clone()).unwrap();
        let mut cache = ObjectSnapCache::default();
        let expected = query(&mut cache, &doc, ObjectSnapModes::ALL).unwrap();
        assert!(expected.point().distance_to(p(5., -2.)).unwrap() < 1e-11);
        let mut clone = doc.clone();
        clone
            .set_objects_color([id], Some(ColorRgb::new(10, 20, 30)))
            .unwrap();
        for _ in 0..8 {
            assert_eq!(
                query(&mut cache, &clone, ObjectSnapModes::ALL),
                Some(expected)
            );
            assert_eq!(
                query(&mut cache, &doc, ObjectSnapModes::ALL),
                Some(expected)
            );
        }
        assert_eq!(cache.builds, 1);
        assert_eq!(cache.source_comparisons, 0);
        // An equal explicit replacement compares once, then updates the stamp
        // so later queries do not repeat the structural comparison.
        doc.replace_object_geometries_with_history(
            [(id, geometry)],
            ReplacementHistory::EveryReplacement,
        )
        .unwrap();
        for _ in 0..8 {
            assert_eq!(
                query(&mut cache, &doc, ObjectSnapModes::ALL),
                Some(expected)
            );
        }
        assert_eq!(cache.source_comparisons, 1);
        assert_eq!(cache.builds, 1);
    }
}

#[test]
fn retention_prunes_changed_types_and_removed_ids_even_in_point_only_mode() {
    let mut doc = Document::default();
    let ids = sources().map(|g| doc.add_geometry(g).unwrap());
    let mut cache = ObjectSnapCache::default();
    query(&mut cache, &doc, ObjectSnapModes::ALL);
    assert_eq!((cache.curves.len(), cache.surfaces.len()), (3, 1));
    doc.delete_objects(ids[..2].iter().copied()).unwrap();
    doc.replace_object_geometries(ids[2..].iter().map(|&id| (id, Geometry::Point(p(5., -2.)))))
        .unwrap();
    assert!(query(&mut cache, &doc, ObjectSnapModes::NONE).is_none());
    assert_eq!((cache.curves.len(), cache.surfaces.len()), (3, 1)); // Suspension stays O(1).
    assert!(
        query(
            &mut cache,
            &doc,
            ObjectSnapModes::only(ObjectSnapKind::Point)
        )
        .is_some()
    );
    assert!(cache.curves.is_empty() && cache.surfaces.is_empty() && cache.polygons.is_empty());
    // A different document must evict all old IDs, regardless of visibility.
    let mut other = Document::default();
    other.add_geometry(Geometry::NurbsCurve(curve())).unwrap();
    query(&mut cache, &other, ObjectSnapModes::ALL);
    assert_eq!(cache.curves.len(), 1);
    query(
        &mut cache,
        &Document::default(),
        ObjectSnapModes::only(ObjectSnapKind::End),
    );
    assert!(cache.curves.is_empty() && cache.polygons.is_empty());
}

#[test]
fn layer_index_preserves_visibility_locked_snaps_and_document_order_ties() {
    let mut doc = Document::default();
    let layers: Vec<_> = (0..64)
        .map(|i| {
            doc.add_layer(format!("Layer {i}"), ColorRgb::BLACK)
                .unwrap()
        })
        .collect();
    let ids = [layers[63], layers[0]].map(|layer| {
        let id = doc.add_geometry(Geometry::NurbsCurve(curve())).unwrap();
        doc.set_objects_layer([id], layer).unwrap();
        id
    });
    let mid = ObjectSnapModes::only(ObjectSnapKind::Mid);
    let mut cache = ObjectSnapCache::default();
    for _ in 0..8 {
        assert_eq!(query(&mut cache, &doc, mid).unwrap().object_id(), ids[0]);
    }
    doc.set_layer_visibility(layers[63], false).unwrap();
    assert_eq!(query(&mut cache, &doc, mid).unwrap().object_id(), ids[1]);
    doc.set_layer_visibility(layers[63], true).unwrap();
    doc.set_layer_locked(layers[63], true).unwrap();
    assert_eq!(query(&mut cache, &doc, mid).unwrap().object_id(), ids[0]);
    doc.set_layer_locked(layers[63], false).unwrap();
    doc.move_objects_to_end([ids[0]]).unwrap();
    for _ in 0..8 {
        assert_eq!(query(&mut cache, &doc, mid).unwrap().object_id(), ids[1]);
    }
    doc.undo().unwrap();
    assert_eq!(query(&mut cache, &doc, mid).unwrap().object_id(), ids[0]);
}
