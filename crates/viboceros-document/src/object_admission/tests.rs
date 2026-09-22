use crate::*;
use viboceros_geometry::{
    BrepFace, BrepLoop, BrepSolidOrientation, BrepTrim, Frame3, NurbsCurve2, Point2, Vector3,
    WeightedPoint2,
};

fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

fn frame() -> Frame3 {
    Frame3::try_from_normal(
        point(0., 0., 0.),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}

fn shell(x: f64, radius: f64) -> Brep {
    Brep::try_box(
        frame(),
        [
            [x - radius, x + radius],
            [-radius, radius],
            [-radius, radius],
        ],
        Tolerance::DEFAULT,
    )
    .unwrap()
}

#[test]
fn admission_normalizes_whole_solids_without_volume_sign_or_representable_volume() {
    let mut sources = vec![shell(0., 1.), higher_degree_inward_box(false).reversed()];
    for (outer, inner) in [
        (shell(-5., 1.), shell(5., 2.)),
        (shell(-3., 1.), shell(3., 1.)),
        (shell(0., 4.), shell(0., 1.)),
    ] {
        sources.push(Brep::try_combine(vec![outer, inner.reversed()], Tolerance::DEFAULT).unwrap());
    }
    for outward in sources {
        assert_eq!(
            outward.solid_orientation().unwrap(),
            BrepSolidOrientation::Outward
        );
        for reversed in [false, true] {
            let source = if reversed {
                outward.reversed()
            } else {
                outward.clone()
            };
            let mut document = Document::default();
            let id = document
                .add_geometry(Geometry::Brep(source.clone()))
                .unwrap();
            assert_eq!(
                document.object(id).unwrap().geometry(),
                &Geometry::Brep(outward.clone())
            );
            // Replacement stages the same global normalization before equality.
            assert_eq!(
                document
                    .replace_object_geometries([(id, Geometry::Brep(source.clone()))])
                    .unwrap(),
                0
            );
            assert_eq!(
                source,
                if reversed {
                    outward.reversed()
                } else {
                    outward.clone()
                }
            );
        }
    }
    for exponent in [-360, 360] {
        let scale = 2_f64.powi(exponent);
        let tolerance = Tolerance::try_new(scale * 1e-9, 1e-12, 1e-10).unwrap();
        let outward = Brep::try_box(
            frame(),
            [[0., scale], [0., 2. * scale], [0., 4. * scale]],
            tolerance,
        )
        .unwrap();
        let mut doc = Document::new(tolerance);
        let id = doc
            .add_geometry(Geometry::Brep(outward.reversed()))
            .unwrap();
        assert_eq!(doc.object(id).unwrap().geometry(), &Geometry::Brep(outward));
    }
}

#[test]
fn admission_normalization_precedes_noop_and_explicit_replacement_history() {
    for history in [
        ReplacementHistory::ChangesOnly,
        ReplacementHistory::EveryReplacement,
    ] {
        for active in [false, true] {
            let outward = shell(0., 1.);
            let mut doc = Document::default();
            let id = doc.add_geometry(Geometry::Brep(outward.clone())).unwrap();
            doc.add_group(Some("solid".into()), [id]).unwrap();
            doc.set_object_names([(id, Some("Source".into()))]).unwrap();
            doc.select_objects_direct([id], SelectionMode::Replace)
                .unwrap();
            doc.add_geometry(Geometry::Point(point(5., 5., 5.)))
                .unwrap();
            doc.undo().unwrap();
            if active {
                doc.begin_transaction("caller").unwrap();
            }
            let before = doc.clone();
            let snapshot = doc.object(id).unwrap().geometry_snapshot().clone();
            let count = doc
                .replace_object_geometries_with_history(
                    [
                        (id, Geometry::Brep(shell(5., 1.))),
                        (id, Geometry::Brep(outward.reversed())),
                    ],
                    history,
                )
                .unwrap();
            assert_eq!(doc.objects, before.objects);
            assert_eq!(doc.groups, before.groups);
            assert_eq!(doc.selection_order, before.selection_order);
            if history == ReplacementHistory::ChangesOnly {
                assert_eq!(count, 0);
                assert_eq!(format!("{doc:?}"), format!("{before:?}"));
                assert!(snapshot.shares_storage_with(doc.object(id).unwrap().geometry_snapshot()));
                if active {
                    doc.rollback_transaction().unwrap();
                }
            } else {
                assert_eq!(count, 1);
                if active {
                    doc.commit_transaction().unwrap();
                }
                assert_eq!(doc.redo_label(), None);
                assert!(!snapshot.shares_storage_with(doc.object(id).unwrap().geometry_snapshot()));
                let after = doc.object(id).unwrap().geometry_snapshot().clone();
                doc.undo().unwrap();
                assert!(snapshot.shares_storage_with(doc.object(id).unwrap().geometry_snapshot()));
                doc.redo().unwrap();
                assert!(after.shares_storage_with(doc.object(id).unwrap().geometry_snapshot()));
            }
        }
    }
}

fn higher_degree_inward_box(mixed: bool) -> Brep {
    let solid = shell(0., 1.);
    let faces = solid
        .reversed()
        .faces()
        .iter()
        .map(|face| {
            let loops = face
                .loops()
                .iter()
                .map(|boundary| {
                    let trims = boundary
                        .trims()
                        .iter()
                        .map(|trim| {
                            let cp = trim.curve().control_points();
                            assert_eq!(cp.len(), 2);
                            let (a, b) = (cp[0].point(), cp[1].point());
                            let degree = if mixed { 4 } else { 2 };
                            let controls = (0..=degree)
                                .map(|i| {
                                    let t = i as f64 / degree as f64;
                                    WeightedPoint2::try_new(
                                        Point2::try_new(
                                            a.x() + t * (b.x() - a.x()),
                                            a.y() + t * (b.y() - a.y()),
                                        )
                                        .unwrap(),
                                        if mixed && i == 2 { -0.125 } else { 1. },
                                    )
                                    .unwrap()
                                })
                                .collect();
                            let mut knots = vec![0.; degree + 1];
                            knots.extend(vec![1.; degree + 1]);
                            let curve =
                                NurbsCurve2::try_new_rational(degree, controls, knots).unwrap();
                            BrepTrim::try_new(
                                trim.vertices(),
                                trim.edge(),
                                trim.is_reversed_3d(),
                                curve,
                                trim.trim_type(),
                                trim.iso(),
                                trim.tolerance(),
                            )
                            .unwrap()
                        })
                        .collect();
                    BrepLoop::try_new(boundary.loop_type(), trims).unwrap()
                })
                .collect();
            BrepFace::try_new(face.surface().clone(), face.is_reversed(), loops).unwrap()
        })
        .collect();
    Brep::try_new(
        solid.vertices().to_vec(),
        solid.edges().to_vec(),
        faces,
        Tolerance::DEFAULT,
    )
    .unwrap()
}

#[test]
fn admission_preserves_nonsolids_unknown_solids_and_mesh_winding() {
    let solid = shell(0., 1.);
    let opened = solid
        .sub_brep(&[0, 1, 2, 3, 4], Tolerance::DEFAULT)
        .unwrap()
        .reversed();
    let mut faces = solid.faces().to_vec();
    faces[0] = BrepFace::try_new(
        faces[0].surface().clone(),
        !faces[0].is_reversed(),
        faces[0].loops().to_vec(),
    )
    .unwrap();
    let inconsistent = Brep::try_new(
        solid.vertices().to_vec(),
        solid.edges().to_vec(),
        faces,
        Tolerance::DEFAULT,
    )
    .unwrap();
    let coincident =
        Brep::try_combine(vec![solid.clone(), solid.reversed()], Tolerance::DEFAULT).unwrap();
    assert_eq!(
        coincident.solid_orientation().unwrap(),
        BrepSolidOrientation::Unknown
    );
    // The mixed-weight quartic is pole-free with the original segment image
    // (proved in the kernel trim tests), but has no same-sign hull certificate.
    // Its volume is negative; Unknown must still be preserved on admission.
    let unsupported = higher_degree_inward_box(true);
    assert_eq!(
        unsupported.solid_orientation().unwrap(),
        BrepSolidOrientation::Unknown
    );
    assert!(unsupported.signed_volume(Tolerance::DEFAULT).unwrap() < 0.);
    for source in [opened, inconsistent, coincident, unsupported] {
        let mut doc = Document::default();
        let id = doc.add_geometry(Geometry::Brep(source.clone())).unwrap();
        assert_eq!(
            doc.object(id).unwrap().geometry(),
            &Geometry::Brep(source.clone())
        );
        doc.replace_object_geometries([(id, Geometry::Brep(source.reversed()))])
            .unwrap();
        assert_eq!(
            doc.object(id).unwrap().geometry(),
            &Geometry::Brep(source.reversed())
        );
    }
    let mesh = TriangleMesh::try_new(
        vec![
            point(0., 0., 0.),
            point(3., 0., 0.),
            point(0., 4., 0.),
            point(0., 0., 5.),
        ],
        vec![[0, 1, 2], [0, 3, 1], [0, 2, 3], [1, 3, 2]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let mut doc = Document::default();
    let id = doc.add_geometry(Geometry::Mesh(mesh.clone())).unwrap();
    assert_eq!(doc.object(id).unwrap().geometry(), &Geometry::Mesh(mesh));
}

#[test]
fn admission_curved_solids_and_undo_redo_keep_exact_recorded_snapshots() {
    let sphere = Brep::try_surface_face(
        NurbsSurface::try_sphere(frame(), 2.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let mut doc = Document::default();
    let id = doc.add_geometry(Geometry::Brep(sphere.reversed())).unwrap();
    assert_eq!(doc.object(id).unwrap().geometry(), &Geometry::Brep(sphere));
    let inserted = doc.object(id).unwrap().geometry_snapshot().clone();
    doc.undo().unwrap();
    assert!(doc.object(id).is_none());
    doc.redo().unwrap();
    assert!(inserted.shares_storage_with(doc.object(id).unwrap().geometry_snapshot()));

    // A material-side-preserving transform can change which mixed shell is
    // spatially first. Transform/copy is not a new explicit geometry admission.
    let source = Brep::try_combine(
        vec![shell(-5., 1.), shell(5., 2.).reversed()],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let mirror = AffineTransform3::try_uniform_scale(point(0., 0., 0.), -1.).unwrap();
    let transformed = source.transformed(mirror, Tolerance::DEFAULT).unwrap();
    assert_eq!(
        transformed.solid_orientation().unwrap(),
        BrepSolidOrientation::Inward
    );
    let id = doc.add_geometry(Geometry::Brep(source.clone())).unwrap();
    let copies = doc.copy_objects_transformed([id], mirror).unwrap();
    assert_eq!(
        doc.object(copies[0]).unwrap().geometry(),
        &Geometry::Brep(transformed.clone())
    );
    doc.transform_objects([id], mirror).unwrap();
    assert_eq!(
        doc.object(id).unwrap().geometry(),
        &Geometry::Brep(transformed.clone())
    );
    let before = doc.object(id).unwrap().geometry_snapshot().clone();
    assert_eq!(
        doc.replace_object_geometries([(id, Geometry::Brep(transformed.clone()))])
            .unwrap(),
        1
    );
    assert_eq!(
        doc.object(id).unwrap().geometry(),
        &Geometry::Brep(transformed.reversed())
    );
    let after = doc.object(id).unwrap().geometry_snapshot().clone();
    doc.undo().unwrap();
    assert!(before.shares_storage_with(doc.object(id).unwrap().geometry_snapshot()));
    doc.redo().unwrap();
    assert!(after.shares_storage_with(doc.object(id).unwrap().geometry_snapshot()));
}

#[test]
fn admission_of_explicit_copy_geometry_preserves_source_metadata_and_group_history() {
    let outward = Brep::try_combine(
        vec![shell(-5., 1.), shell(5., 2.).reversed()],
        Tolerance::DEFAULT,
    )
    .unwrap();
    for kind in 0..3 {
        let mut doc = Document::default();
        let id = doc
            .add_geometry_with_attributes(
                Geometry::Point(point(1., 2., 3.)),
                ObjectAttributes::on_layer(doc.current_layer_id())
                    .with_name("Source")
                    .with_object_color(ColorRgb::new(10, 20, 30)),
            )
            .unwrap();
        let group = doc.add_group(Some("copies".into()), [id]).unwrap();
        doc.select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        let before = doc.object(id).unwrap().clone();
        let pieces = [
            (id, Geometry::Brep(outward.reversed())),
            (id, Geometry::Brep(outward.reversed())),
        ];
        let copies = match kind {
            0 => doc.copy_object_geometries_into_source_groups(pieces),
            1 => doc.copy_object_geometries_into_source_groups_in_order(pieces),
            _ => doc.copy_object_pieces_into_source_groups(pieces),
        }
        .unwrap();
        assert_eq!(copies.len(), if kind == 2 { 2 } else { 1 });
        assert_eq!(doc.object(id), Some(&before));
        let snapshots = copies
            .iter()
            .map(|&copy| {
                let object = doc.object(copy).unwrap();
                assert_eq!(object.geometry(), &Geometry::Brep(outward.clone()));
                assert_eq!(object.attributes(), before.attributes());
                assert_eq!(object.group_ids(), &[group]);
                assert!(!doc.is_selected(copy));
                object.geometry_snapshot().clone()
            })
            .collect::<Vec<_>>();
        doc.undo().unwrap();
        assert_eq!(doc.objects().len(), 1);
        assert_eq!(doc.object(id), Some(&before));
        doc.redo().unwrap();
        for (copy, snapshot) in copies.iter().zip(&snapshots) {
            assert!(snapshot.shares_storage_with(doc.object(*copy).unwrap().geometry_snapshot()));
        }
    }
}

#[test]
fn admission_validates_all_targets_before_geometry_or_history_mutation() {
    let input = Geometry::Brep(shell(0., 1.).reversed());
    for active in [false, true] {
        for missing in [false, true] {
            let mut doc = Document::default();
            let id = doc
                .add_geometry(Geometry::Point(point(0., 0., 0.)))
                .unwrap();
            let target = if missing {
                ObjectId::new()
            } else {
                let target = doc
                    .add_geometry(Geometry::Point(point(1., 0., 0.)))
                    .unwrap();
                doc.set_objects_locked([target], true).unwrap();
                target
            };
            let layer = if missing {
                LayerId::new()
            } else {
                let layer = doc.add_layer("locked", ColorRgb::BLACK).unwrap();
                doc.set_layer_locked(layer, true).unwrap();
                layer
            };
            if active {
                doc.begin_transaction("caller").unwrap();
            }
            let before = format!("{doc:?}");
            assert!(
                doc.add_geometry_with_attributes(input.clone(), ObjectAttributes::on_layer(layer))
                    .is_err()
            );
            assert_eq!(format!("{doc:?}"), before);
            for history in [
                ReplacementHistory::ChangesOnly,
                ReplacementHistory::EveryReplacement,
            ] {
                assert!(
                    doc.replace_object_geometries_with_history(
                        [(id, input.clone()), (target, input.clone())],
                        history
                    )
                    .is_err()
                );
                assert_eq!(format!("{doc:?}"), before);
            }
            assert!(
                doc.copy_object_geometries_into_source_groups([
                    (id, input.clone()),
                    (target, input.clone())
                ])
                .is_err()
            );
            assert_eq!(format!("{doc:?}"), before);
            assert!(
                doc.copy_object_pieces_into_source_groups([
                    (id, input.clone()),
                    (target, input.clone())
                ])
                .is_err()
            );
            assert_eq!(format!("{doc:?}"), before);
        }
    }
}
