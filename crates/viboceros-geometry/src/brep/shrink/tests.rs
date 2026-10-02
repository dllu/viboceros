use super::*;

fn surface() -> NurbsSurface {
    NurbsSurface::try_bilinear([
        Point3::try_new(0., 0., 0.).unwrap(),
        Point3::try_new(10., 0., 0.).unwrap(),
        Point3::try_new(0., 10., 0.).unwrap(),
        Point3::try_new(10., 10., 4.).unwrap(),
    ])
    .unwrap()
    .try_reparameterized(0.0..=10., 0.0..=10.)
    .unwrap()
}

#[test]
fn exact_crop_preserves_boundary_geometry_indices_and_orientation() {
    for reversed in [false, true] {
        let original = Brep::try_rectangular_surface_face_with_orientation(
            surface(),
            2.0..=8.,
            1.0..=7.,
            reversed,
            Tolerance::DEFAULT,
        )
        .unwrap();
        for mode in [
            BrepSurfaceShrinkMode::Standard,
            BrepSurfaceShrinkMode::ToEdge,
        ] {
            let result = original
                .try_shrunk_surfaces(mode, Tolerance::DEFAULT)
                .unwrap();
            assert_eq!(result.vertices, original.vertices);
            assert_eq!(result.edges, original.edges);
            assert_eq!(result.faces[0].reversed, reversed);
            assert_eq!(result.faces[0].surface.domain_u(), 2.0..=8.);
            assert_eq!(result.faces[0].surface.domain_v(), 1.0..=7.);
            for (side, trim) in result.faces[0].loops[0].trims.iter().enumerate() {
                assert_eq!(
                    trim.iso,
                    [
                        SurfaceIso::South,
                        SurfaceIso::East,
                        SurfaceIso::North,
                        SurfaceIso::West
                    ][side]
                );
                assert_eq!(trim.curve, original.faces[0].loops[0].trims[side].curve);
                for t in 0..=8 {
                    let p = trim
                        .curve
                        .evaluate(trim.curve.parameter_at(t as Real / 8.).unwrap())
                        .unwrap();
                    let a = original.faces[0].surface.evaluate(p.x(), p.y()).unwrap();
                    let b = result.faces[0].surface.evaluate(p.x(), p.y()).unwrap();
                    assert!(a.distance_to(b).unwrap() < 1e-12);
                }
            }
            assert_eq!(
                result
                    .try_shrunk_surfaces(mode, Tolerance::DEFAULT)
                    .unwrap(),
                result
            );
        }
    }
}

#[test]
fn natural_seams_and_singularities_are_noops() {
    let frame = Frame3::try_from_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    for surface in [
        surface(),
        NurbsSurface::try_cylinder(frame, 2., 0., 5.).unwrap(),
        NurbsSurface::try_sphere(frame, 2.).unwrap(),
    ] {
        let original = Brep::try_surface_face(surface, Tolerance::DEFAULT).unwrap();
        for mode in [
            BrepSurfaceShrinkMode::Standard,
            BrepSurfaceShrinkMode::ToEdge,
        ] {
            assert_eq!(
                original
                    .try_shrunk_surfaces(mode, Tolerance::DEFAULT)
                    .unwrap(),
                original
            );
        }
    }
}
