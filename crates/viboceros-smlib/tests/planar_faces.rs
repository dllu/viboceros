#![cfg(feature = "native")]
use viboceros_geometry::{NurbsCurve, Point3, Tolerance};
use viboceros_smlib::{KernelCurve, Shape};

fn rectangle(x0: f64, y0: f64, x1: f64, y1: f64) -> NurbsCurve {
    NurbsCurve::try_new(
        1,
        [[x0, y0], [x1, y0], [x1, y1], [x0, y1], [x0, y0]]
            .into_iter()
            .map(|p| Point3::try_new(p[0], p[1], 0.).unwrap())
            .collect(),
        vec![0., 0., 1., 2., 3., 4., 4.],
    )
    .unwrap()
}
#[test]
fn native_regions_preserve_holes_nested_islands_and_profile_handles() {
    let profiles = [
        rectangle(0., 0., 10., 10.),
        rectangle(2., 2., 8., 8.),
        rectangle(4., 4., 6., 6.),
    ];
    let native = profiles
        .iter()
        .map(|p| KernelCurve::from_nurbs(p).unwrap())
        .collect::<Vec<_>>();
    let result = Shape::planar_faces(&native.iter().collect::<Vec<_>>(), Tolerance::DEFAULT)
        .unwrap()
        .to_brep(Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(result.faces().len(), 2);
    assert!((result.area(Tolerance::DEFAULT).unwrap() - 68.).abs() < 1e-10);
    assert!(result.faces().iter().any(|f| f.loops().len() == 2));
    for (curve, original) in native.iter().zip(&profiles) {
        assert!(
            curve
                .evaluate(0.5)
                .unwrap()
                .distance_to(original.evaluate(0.5).unwrap())
                .unwrap()
                < 1e-12
        );
    }
}
#[test]
fn native_overlap_regions_match_the_recorded_rhino_partial_hole_policy() {
    let profiles = [
        rectangle(0., 0., 10., 10.),
        rectangle(2., 2., 6., 6.),
        rectangle(4., 4., 8., 8.),
        rectangle(0.5, 0.5, 1.5, 1.5),
    ];
    let native = profiles
        .iter()
        .map(|p| KernelCurve::from_nurbs(p).unwrap())
        .collect::<Vec<_>>();
    let result = Shape::planar_faces(&native.iter().collect::<Vec<_>>(), Tolerance::DEFAULT)
        .unwrap()
        .to_brep(Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(result.faces().len(), 2);
    let mut areas = result
        .faces()
        .iter()
        .enumerate()
        .map(|(i, _)| {
            result
                .sub_brep(&[i], Tolerance::DEFAULT)
                .unwrap()
                .area(Tolerance::DEFAULT)
                .unwrap()
        })
        .collect::<Vec<_>>();
    areas.sort_by(f64::total_cmp);
    assert!((areas[0] - 16.).abs() < 1e-12);
    assert!((areas[1] - 83.).abs() < 1e-12);
}

#[test]
fn explicit_plane_tolerance_and_failed_creation_preserve_input_handles() {
    let a = KernelCurve::from_nurbs(&rectangle(0., 0., 2., 2.)).unwrap();
    let near = rectangle(0., 0., 2., 2.)
        .transformed(viboceros_geometry::AffineTransform3::from_translation(
            viboceros_geometry::Vector3::try_new(0., 0., 1e-5).unwrap(),
        ))
        .unwrap();
    let b = KernelCurve::from_nurbs(&near).unwrap();
    assert!(!KernelCurve::are_coplanar(&[&a, &b], Tolerance::DEFAULT).unwrap());
    let before = a.evaluate(0.5).unwrap();
    let shifted = rectangle(0., 0., 2., 2.)
        .transformed(viboceros_geometry::AffineTransform3::from_translation(
            viboceros_geometry::Vector3::try_new(0., 0., 3.).unwrap(),
        ))
        .unwrap();
    let other = KernelCurve::from_nurbs(&shifted).unwrap();
    assert!(!KernelCurve::are_coplanar(&[&a, &other], Tolerance::DEFAULT).unwrap());
    assert!(Shape::planar_faces(&[&a, &other], Tolerance::DEFAULT).is_err());
    assert_eq!(a.evaluate(0.5).unwrap(), before);
}
