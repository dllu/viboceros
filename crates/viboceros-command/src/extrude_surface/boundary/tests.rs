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
    let source = Shape::from_brep(&base, t).unwrap();
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
        let result = extrude(&base, &source, &cutter, vector, t)
            .unwrap_or_else(|error| panic!("{corners:?} {dir:?}: {error}"));
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
    let source = Shape::from_brep(&source_face, t).unwrap();
    let result = extrude(&source_face, &source, &cutter, normal.as_vector(), t).unwrap();
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
