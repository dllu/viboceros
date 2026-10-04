use super::*;

fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

#[test]
fn scale_by_plane_signed_axes_zero_components_and_normal_are_independent() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let id = document
        .add_geometry(Geometry::Point(p(2., 3., 4.)))
        .unwrap();
    document.select_all();
    for (reference, target, expected) in [
        ("3,5,8", "5,11,1", p(3., 5., 4.)),
        ("3,5,3", "-3,11,3", p(-1., 5., 4.)),
        ("3,5,3", "5,-7,3", p(3., -1., 4.)),
        ("3,5,3", "1,11,3", p(2., 5., 4.)),
        ("1,5,3", "5,11,3", p(2., 5., 4.)),
    ] {
        registry
            .execute(
                &mut document,
                &format!("ScaleByPlane Plane=WorldTop 1,2,3 {reference} {target}"),
            )
            .unwrap();
        assert_eq!(
            document.object(id).unwrap().geometry(),
            &Geometry::Point(expected)
        );
        document.undo().unwrap();
    }
    let before = document.object(id).unwrap().clone();
    for invalid in [
        "ScaleByPlane Plane=Unknown 1,2,3 3,5,3 5,11,3",
        "ScaleByPlane Plane=3Point 0,0,0 0,0,0 0,1,0 1,2,3 3,5,3 5,11,3",
        "ScaleByPlane 1,2,3 3,5,3 NaN,11,3 Rigid=Yes",
        "ScaleByPlane 1,2,3 3,5,3 5,11,3 garbage",
    ] {
        assert!(
            registry.execute(&mut document, invalid).is_err(),
            "{invalid}"
        );
        assert_eq!(document.object(id).unwrap(), &before);
        assert_eq!(registry.rigid_option_default("ScaleByPlane"), Some(false));
    }
}

#[test]
fn scale_by_plane_copy_repeat_memory_and_rigid_group_centers() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let a = document
        .add_geometry(Geometry::Point(p(2., 3., 4.)))
        .unwrap();
    let b = document
        .add_geometry(Geometry::Point(p(4., 5., 6.)))
        .unwrap();
    document.add_group(None, [a, b]).unwrap();
    document.select_all();
    registry
        .execute(
            &mut document,
            "ScaleByPlane 1,2,3 3,5,3 5,11,3 Rigid=Yes Copy=Yes",
        )
        .unwrap();
    let outputs = document.objects().skip(2).collect::<Vec<_>>();
    assert_eq!(outputs.len(), 2);
    assert_eq!(outputs[0].geometry(), &Geometry::Point(p(4., 7., 4.)));
    assert_eq!(outputs[1].geometry(), &Geometry::Point(p(6., 9., 6.)));
    assert_eq!(registry.rigid_option_default("ScaleByPlane"), Some(true));
    assert_eq!(registry.copy_default("ScaleByPlane"), Some(true));
    document.undo().unwrap();
    assert_eq!(document.objects().count(), 2);
    document.redo().unwrap();
    assert_eq!(document.objects().count(), 4);
}
