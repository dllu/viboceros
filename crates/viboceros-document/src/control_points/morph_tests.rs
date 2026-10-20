use super::*;
use viboceros_geometry::{NurbsCurve, PointMorph, WeightedPoint3};
struct Warp;

#[test]
fn control_grips_on_an_axially_reparameterized_face_rebuild_boundaries_and_history() {
    use viboceros_geometry::{AffineTransform3, Brep, NurbsSurface, StretchPointMorph, Vector3};
    let t = Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap();
    let p = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let surface = NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        vec![
            p(1., -1., 0.),
            p(3., -1., 0.),
            p(1., -1., 10.),
            p(3., -1., 10.),
        ],
        vec![0., 0., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let source = Brep::try_surface_face(surface, t).unwrap();
    let a = p(1., 2., 3.);
    let b = p(4., -2., 11.);
    let morph = StretchPointMorph::try_new(a, b, a.distance_to(b).unwrap() * 0.5).unwrap();
    let shape = morph
        .morph_brep(&source, Tolerance::try_new(4e-5, 1e-12, 1e-10).unwrap())
        .unwrap();
    assert!(
        shape.faces()[0]
            .rectangular_trim_bounds(t)
            .unwrap()
            .is_none()
    );
    let mut doc = Document::new(t);
    let id = doc.add_geometry(Geometry::Brep(shape)).unwrap();
    doc.enable_control_points([id]).unwrap();
    let grip = ControlPointId {
        object: id,
        index: 0,
    };
    doc.select_control_points([grip], SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    let original = doc.object(id).unwrap().geometry().clone();
    let locations = original.grip_locations(GripMode::Control).unwrap();
    let offset = Vector3::try_new(0., 0.25, 0.).unwrap();
    doc.transform_objects_and_grips(
        [],
        [grip],
        AffineTransform3::from_translation(offset),
        false,
        CopyGroupPolicy::Preserve,
    )
    .unwrap();
    let edited = doc.object(id).unwrap().geometry().clone();
    let changed = edited.grip_locations(GripMode::Control).unwrap();
    assert_eq!(changed[0], locations[0].translated(offset).unwrap());
    assert_eq!(changed[1..], locations[1..]);
    assert!(doc.undo().unwrap().is_some());
    assert_eq!(doc.object(id).unwrap().geometry(), &original);
    assert!(doc.redo().unwrap().is_some());
    assert_eq!(doc.object(id).unwrap().geometry(), &edited);
}
impl PointMorph for Warp {
    fn morph_point(&self, p: Point3) -> Result<Point3, GeometryError> {
        Point3::try_new(p.x(), p.y(), p.z() + p.x() * p.y() + 1.)
    }
}
fn curve() -> Geometry {
    let pts = [
        [2., 0., 0.],
        [0., 2., 0.],
        [-2., 0., 0.],
        [0., -2., 0.],
        [2., 0., 0.],
        [0., 2., 0.],
    ];
    Geometry::NurbsCurve(
        NurbsCurve::try_new_rational(
            2,
            pts.into_iter()
                .map(|p| WeightedPoint3::try_new(Point3::try_from(p).unwrap(), 1.).unwrap())
                .collect(),
            (-2..=6).map(|i| i as f64).collect(),
        )
        .unwrap(),
    )
}
#[test]
fn nonlinear_grips_keep_periodic_seams_knots_and_weights_and_undo_as_one_edit() {
    let mut d = Document::default();
    let original = curve();
    let id = d.add_geometry(original.clone()).unwrap();
    d.enable_control_points([id]).unwrap();
    let grip = ControlPointId {
        object: id,
        index: 0,
    };
    d.select_control_points([grip], SelectionMode::Add).unwrap();
    d.clear_history().unwrap();
    d.morph_objects_and_grips([id], [grip], &Warp, false, CopyGroupPolicy::Preserve)
        .unwrap();
    let (Geometry::NurbsCurve(before), Geometry::NurbsCurve(after)) =
        (&original, d.object(id).unwrap().geometry())
    else {
        panic!("curve")
    };
    assert_eq!(before.knots(), after.knots());
    assert_eq!(before.grip_count().unwrap(), after.grip_count().unwrap());
    for (i, (a, b)) in before
        .control_points()
        .iter()
        .zip(after.control_points())
        .enumerate()
    {
        assert_eq!(a.weight(), b.weight());
        assert_eq!(
            b.point(),
            if i % 4 == 0 {
                Warp.morph_point(a.point()).unwrap()
            } else {
                a.point()
            }
        );
    }
    d.undo().unwrap();
    assert_eq!(d.object(id).unwrap().geometry(), &original);
    d.redo().unwrap();
    assert_eq!(d.selected_control_points().count(), 1);
}
#[test]
fn nonlinear_copy_grips_reuse_groups_and_leave_new_grips_unselected() {
    let mut d = Document::default();
    let id = d.add_geometry(curve()).unwrap();
    let p = d
        .add_geometry(Geometry::Point(Point3::try_new(1., 1., 0.).unwrap()))
        .unwrap();
    let group = d.add_group(None, [id, p]).unwrap();
    d.enable_control_points([id]).unwrap();
    let grip = ControlPointId {
        object: id,
        index: 2,
    };
    d.select_control_points([grip], SelectionMode::Add).unwrap();
    d.clear_history().unwrap();
    let (owners, copies) = d
        .morph_objects_and_grips([p], [grip], &Warp, true, CopyGroupPolicy::Preserve)
        .unwrap();
    let copied_owner = copies[owners.iter().position(|o| *o == id).unwrap()];
    let copied_peer = copies[owners.iter().position(|o| *o == p).unwrap()];
    assert_eq!(d.object(copied_owner).unwrap().group_ids(), &[group]);
    assert_ne!(d.object(copied_peer).unwrap().group_ids(), &[group]);
    assert_eq!(d.groups().len(), 2);
    assert!(d.grip_mode(copied_owner).is_some());
    assert_eq!(
        d.selected_control_points()
            .map(|(g, _)| g)
            .collect::<Vec<_>>(),
        vec![grip]
    );
    d.undo().unwrap();
    assert_eq!(d.objects().count(), 2);
    d.redo().unwrap();
    assert_eq!(d.objects().count(), 4);
    assert_eq!(d.grip_mode(copied_owner), Some(GripMode::Control));
    assert_eq!(
        d.selected_control_points()
            .map(|(g, _)| g)
            .collect::<Vec<_>>(),
        vec![grip]
    );
}
#[test]
fn failed_mixed_nonlinear_grips_preserve_geometry_selection_groups_history_and_redo() {
    struct Failing;
    impl PointMorph for Failing {
        fn morph_point(&self, p: Point3) -> Result<Point3, GeometryError> {
            if p.x() < 0. {
                Err(GeometryError::Degenerate {
                    context: "injected point map failure",
                })
            } else {
                Warp.morph_point(p)
            }
        }
    }
    let mut d = Document::default();
    let id = d.add_geometry(curve()).unwrap();
    let p = d
        .add_geometry(Geometry::Point(Point3::try_new(1., 0., 0.).unwrap()))
        .unwrap();
    d.enable_control_points([id]).unwrap();
    let grip = ControlPointId {
        object: id,
        index: 2,
    };
    d.select_control_points([grip], SelectionMode::Add).unwrap();
    d.select_objects_direct([p], SelectionMode::Add).unwrap();
    d.add_group(None, [id, p]).unwrap();
    d.add_geometry(Geometry::Point(Point3::try_new(9., 0., 0.).unwrap()))
        .unwrap();
    d.undo().unwrap();
    let before = format!("{d:?}");
    for copy in [false, true] {
        assert!(
            d.morph_objects_and_grips([p], [grip], &Failing, copy, CopyGroupPolicy::Preserve)
                .is_err()
        );
        assert_eq!(format!("{d:?}"), before);
    }
}

#[test]
fn staged_geometry_and_grip_reconstruction_commit_atomically_without_remapping_objects() {
    struct Failing;
    impl PointMorph for Failing {
        fn morph_point(&self, _: Point3) -> Result<Point3, GeometryError> {
            Err(GeometryError::Degenerate {
                context: "injected grip reconstruction failure",
            })
        }
    }
    for copy in [false, true] {
        let mut d = Document::default();
        let id = d.add_geometry(curve()).unwrap();
        let p = d
            .add_geometry(Geometry::Point(Point3::try_new(1., 0., 0.).unwrap()))
            .unwrap();
        d.enable_control_points([id]).unwrap();
        let grip = ControlPointId {
            object: id,
            index: 2,
        };
        d.select_control_points([grip], SelectionMode::Add).unwrap();
        d.add_group(None, [id, p]).unwrap();
        d.add_geometry(Geometry::Point(Point3::try_new(100., 0., 0.).unwrap()))
            .unwrap();
        d.undo().unwrap();
        let before = format!("{d:?}");
        let mapped = Geometry::Point(Point3::try_new(20., 2., 3.).unwrap());
        assert!(
            d.morph_grips_with_object_geometries(
                [(p, mapped.clone())],
                [grip],
                &Failing,
                copy,
                CopyGroupPolicy::Preserve
            )
            .is_err()
        );
        assert_eq!(format!("{d:?}"), before);
        let original = d.object(id).unwrap().geometry().clone();
        let (owners, copies) = d
            .morph_grips_with_object_geometries(
                [
                    (p, mapped.clone()),
                    (id, Geometry::Point(Point3::try_new(999., 0., 0.).unwrap())),
                ],
                [grip],
                &Warp,
                copy,
                CopyGroupPolicy::Preserve,
            )
            .unwrap();
        let result = |id| {
            if copy {
                copies[owners.iter().position(|o| *o == id).unwrap()]
            } else {
                id
            }
        };
        assert_eq!(d.object(result(p)).unwrap().geometry(), &mapped);
        assert_eq!(
            d.object(result(id)).unwrap().geometry(),
            &original
                .with_morphed_grips_in_mode(
                    &BTreeSet::from([2]),
                    &Warp,
                    d.tolerance(),
                    GripMode::Control
                )
                .unwrap()
        );
        if copy {
            assert_eq!(d.object(id).unwrap().geometry(), &original);
        }
        d.undo().unwrap();
        assert_eq!(d.object(id).unwrap().geometry(), &original);
        d.redo().unwrap();
        assert_eq!(d.object(result(p)).unwrap().geometry(), &mapped);
    }
}

#[test]
fn nonlinear_copy_display_uses_source_basis_and_survives_copy_history_only() {
    let mut d = Document::default();
    let original = curve();
    let id = d.add_geometry(original.clone()).unwrap();
    d.enable_control_points([id]).unwrap();
    let grip = ControlPointId {
        object: id,
        index: 0,
    };
    d.select_control_points([grip], SelectionMode::Add).unwrap();
    d.clear_history().unwrap();
    let initial = d.control_point_locations(id).unwrap()[0];
    let mapped = Warp.morph_point(initial).unwrap();
    d.morph_objects_and_grips([], [grip], &Warp, true, CopyGroupPolicy::Preserve)
        .unwrap();
    d.morph_copy_grip_display([grip], &Warp).unwrap();
    assert_eq!(d.object(id).unwrap().geometry(), &original);
    assert_eq!(d.control_point_locations(id).unwrap()[0], mapped);
    d.morph_copy_grip_display([grip], &Warp).unwrap();
    assert_eq!(
        d.control_point_locations(id).unwrap()[0],
        mapped,
        "copies must not accumulate the old display displacement"
    );
    d.undo().unwrap();
    assert_eq!(d.objects().count(), 1);
    assert_eq!(d.control_point_locations(id).unwrap()[0], mapped);
    d.redo().unwrap();
    assert_eq!(d.objects().count(), 2);
    assert_eq!(d.control_point_locations(id).unwrap()[0], mapped);
    let display = d.control_point_locations(id).unwrap().to_vec();
    let label = d.undo_label().map(str::to_owned);
    assert!(
        d.morph_copy_grip_display(
            [
                grip,
                ControlPointId {
                    object: id,
                    index: usize::MAX
                }
            ],
            &Warp
        )
        .is_err()
    );
    assert_eq!(d.control_point_locations(id).unwrap(), display);
    assert_eq!(d.undo_label(), label.as_deref());
    d.replace_object_geometries([(
        id,
        Geometry::Line(
            viboceros_geometry::LineSegment::try_new(
                Point3::try_new(0., 0., 0.).unwrap(),
                Point3::try_new(2., 0., 0.).unwrap(),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ),
    )])
    .unwrap();
    assert_ne!(
        d.control_point_locations(id).unwrap()[0],
        mapped,
        "replacement must refresh the new owner's grip basis"
    );
}
