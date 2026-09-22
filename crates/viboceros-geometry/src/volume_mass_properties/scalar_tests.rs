use super::*;
use crate::{Frame3, MeshFace, NurbsSurface, Tolerance, TriangleMesh};

#[test]
fn scalar_collection_cancels_unrepresentable_mesh_volumes_before_rounding() {
    let make = |scale| {
        TriangleMesh::try_new_faces(
            [
                [0., 0., 0.],
                [3. * scale, 0., 0.],
                [0., 4. * scale, 0.],
                [0., 0., 5. * scale],
            ]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec(),
            [[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]]
                .map(MeshFace::Triangle)
                .to_vec(),
            Tolerance::DEFAULT,
        )
        .unwrap()
    };
    let large = make(1e200);
    let reverse = large.reversed();
    let small = make(1.);
    assert!(
        large
            .volume_mass_properties()
            .unwrap()
            .signed_volume()
            .is_err()
    );
    for meshes in [
        [&large, &small, &reverse],
        [&small, &reverse, &large],
        [&large, &reverse, &small],
    ] {
        assert_eq!(
            VolumeMassProperties::signed_volume_from_boundaries(
                &meshes.map(VolumeBoundary::Mesh),
                Tolerance::DEFAULT
            )
            .unwrap(),
            10.
        );
    }
}

#[test]
fn scalar_surface_and_brep_paths_agree_with_independent_analytic_integrals() {
    let s = NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        [[0., 0., 0.], [4., 0., 0.], [0., 3., 0.], [4., 3., 2.]]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec(),
        vec![0., 0., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let b = crate::Brep::try_surface_face(s.clone(), Tolerance::DEFAULT).unwrap();
    for boundary in [VolumeBoundary::Surface(&s), VolumeBoundary::Brep(&b)] {
        let scalar =
            VolumeMassProperties::signed_volume_from_boundaries(&[boundary], Tolerance::DEFAULT)
                .unwrap();
        assert!((scalar + 2.).abs() < 1e-12);
        assert_eq!(
            scalar,
            VolumeMassProperties::from_boundaries(&[boundary], Tolerance::DEFAULT)
                .unwrap()
                .signed_volume()
                .unwrap()
        );
    }
    let frame = Frame3::try_from_points(
        Point3::try_from([0.; 3]).unwrap(),
        Point3::try_from([1., 0., 0.]).unwrap(),
        Point3::try_from([0., 1., 0.]).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let solid =
        crate::Brep::try_box(frame, [[0., 4.], [0., 3.], [0., 2.]], Tolerance::DEFAULT).unwrap();
    assert!(
        (VolumeMassProperties::signed_volume_from_boundaries(
            &[VolumeBoundary::Brep(&solid)],
            Tolerance::DEFAULT
        )
        .unwrap()
            - 24.)
            .abs()
            < 1e-12
    );
    assert!(VolumeMassProperties::signed_volume_from_boundaries(&[], Tolerance::DEFAULT).is_err());
}
