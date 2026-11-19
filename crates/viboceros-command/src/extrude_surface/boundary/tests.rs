use super::*;

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

#[test]
fn surface_extrusion_boundary_matches_observed_sections() {
    let t = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    let face = |corners: [[f64; 3]; 4]| {
        Brep::try_surface_face(
            NurbsSurface::try_bilinear(corners.map(|p| Point3::try_from(p).unwrap())).unwrap(),
            t,
        )
        .unwrap()
    };
    let base = face([[0., 0., 2.], [2., 0., 2.], [2., 3., 2.], [0., 3., 2.]]);
    for (corners, dir, volume) in [
        (
            [[-2., -2., 6.], [4., -2., 6.], [4., 5., 6.], [-2., 5., 6.]],
            [0., 0., 1.],
            24.,
        ),
        (
            [
                [-2., -2., -4.],
                [4., -2., -4.],
                [4., 5., -4.],
                [-2., 5., -4.],
            ],
            [0., 0., 1.],
            36.,
        ),
        (
            [[-2., -2., 3.], [4., -2., 6.], [4., 5., 6.], [-2., 5., 3.]],
            [0., 0., 1.],
            15.,
        ),
        (
            [[-2., -2., 3.], [4., -2., 6.], [4., 5., 9.], [-2., 5., 4.]],
            [0., 0., 1.],
            21.,
        ),
        (
            [[-2., -2., 6.], [6., -2., 6.], [6., 5., 6.], [-2., 5., 6.]],
            [1., 0., 2.],
            24.,
        ),
    ] {
        let cutter = face(corners);
        let vector = Vector3::try_from(dir)
            .unwrap()
            .normalized_nonzero()
            .unwrap()
            .as_vector();
        let result = sweep(&base, &cutter, vector, t);
        let BoundarySweep::Finished(result) = result else {
            panic!("{corners:?} {dir:?}: {result:?}")
        };
        assert!(result.is_solid());
        assert_eq!((result.faces().len(), result.edges().len()), (6, 12));
        assert!((result.signed_volume(t).unwrap().abs() - volume).abs() < 1e-5);
    }
}

#[test]
fn surface_extrusion_boundary_leaves_cutter_holes_open() {
    let t = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    let source_face = Brep::try_surface_face(
        NurbsSurface::try_bilinear([p(0., 0., 2.), p(2., 0., 2.), p(2., 3., 2.), p(0., 3., 2.)])
            .unwrap(),
        t,
    )
    .unwrap();
    let corners = [
        p(-2., -2., 6.),
        p(4., -2., 6.),
        p(4., 5., 6.),
        p(-2., 5., 6.),
        p(-2., -2., 6.),
    ];
    let outline = Polyline3::try_new(corners.to_vec(), t)
        .unwrap()
        .to_nurbs()
        .unwrap();
    let normal = UnitVector3::try_new(0., 0., 1., t).unwrap();
    let hole = Circle3::try_new(p(1., 1.5, 6.), 0.3, normal, t)
        .unwrap()
        .to_nurbs()
        .unwrap();
    let cutter = Brep::try_planar_face_with_holes(&outline, &[hole], t).unwrap();
    let result = sweep(&source_face, &cutter, normal.as_vector(), t);
    let BoundarySweep::Finished(result) = result else {
        panic!("expected complete section, got {result:?}")
    };
    assert!(!result.is_solid());
    assert_eq!(
        (
            result.faces().len(),
            result.edges().len(),
            result.vertices().len()
        ),
        (6, 13, 9)
    );
    assert!(result.faces().iter().any(|face| face.loops().len() == 2));
    assert!(
        result
            .vertices()
            .iter()
            .all(|vertex| vertex.point().z() <= 6. + t.absolute())
    );
}

fn sweep(source: &Brep, cutter: &Brep, direction: Vector3, tolerance: Tolerance) -> BoundarySweep {
    let bounds = source
        .padded_trim_bounds()
        .unwrap()
        .union(cutter.padded_trim_bounds().unwrap())
        .unwrap();
    let length = bounds.min().distance_to(bounds.max()).unwrap();
    extrude(source, cutter, direction, length, false, tolerance).unwrap()
}

fn horizontal_face([x0, x1, y0, y1, z]: [f64; 5], tolerance: Tolerance) -> Brep {
    Brep::try_surface_face(
        NurbsSurface::try_bilinear([p(x0, y0, z), p(x1, y0, z), p(x1, y1, z), p(x0, y1, z)])
            .unwrap(),
        tolerance,
    )
    .unwrap()
}

#[test]
fn surface_extrusion_rejected_boundary_uses_real_wall_contact() {
    let tol = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    let base = horizontal_face([0., 2., 0., 3., 2.], tol);
    let axis = Vector3::try_new(0., 0., 1.).unwrap();
    for (recipe, retained_extent) in [
        ([-0.2, 1., -0.2, 2., 6.], Some([2., 7.574943945906542])),
        ([-0.2, 0.8, 3., 5., 6.], Some([2., 8.770524351924303])),
        ([0.75, 1.25, 1.25, 1.75, 6.], None),
        ([4., 5., 4., 5., 6.], None),
        ([-0.2, 1., -0.2, 2., -4.], Some([-5.1470273540822555, 2.])),
        ([-0.2, 0.3, 1.3, 1.7, -4.], None),
        ([-0.2, 1.01, 1.4, 1.6, -4.], Some([-5.059745037889117, 2.])),
    ] {
        let cutter = horizontal_face(recipe, tol);
        let result = sweep(&base, &cutter, axis, tol);
        match (result, retained_extent) {
            (BoundarySweep::Untrimmed(solid), Some([min, max])) => {
                assert!(solid.is_solid());
                assert_eq!(solid.faces().len(), 6);
                assert!((solid.bounds().min().z() - min).abs() <= tol.absolute());
                assert!((solid.bounds().max().z() - max).abs() <= tol.absolute());
            }
            (BoundarySweep::Miss, None) => {}
            (result, expect) => panic!("{recipe:?}: expected {expect:?}, got {result:?}"),
        }
    }
}

#[test]
fn surface_extrusion_coincident_boundary_uses_the_existing_base_cap() {
    let tol = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    let source = horizontal_face([0., 2., 0., 3., 2.], tol);
    let axis = Vector3::try_new(0., 0., -1.).unwrap();
    let crossing = horizontal_face([-0.2, 1., -0.2, 2., 2.], tol);
    let BoundarySweep::Finished(solid) = sweep(&source, &crossing, axis, tol) else {
        panic!("expected the existing base to cap the sweep")
    };
    assert!(solid.is_solid());
    assert!((solid.bounds().min().z() - -1.88329756778952).abs() < 1e-6);
    assert!((solid.bounds().max().z() - 2.).abs() < 1e-6);
    let interior = horizontal_face([0.25, 0.75, 0.25, 0.75, 2.], tol);
    assert!(matches!(
        sweep(&source, &interior, axis, tol),
        BoundarySweep::Miss
    ));
}
