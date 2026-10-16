use super::*;
use viboceros_geometry::{NurbsCurve, PointMorph, WeightedPoint3};
struct Warp;
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
