use super::*;
use crate::{Frame3, MeshFace, NurbsSurface, Tolerance, TriangleMesh};

#[test]
fn dimensional_conversion_precedes_volume_and_conversion_cube_range_loss() {
    use crate::LengthUnitSystem;
    for exponent in [-1000, -600, 0, 600, 1000] {
        let scale = 2_f64.powi(exponent);
        let mesh = TriangleMesh::try_new(
            [
                [0., 0., 0.],
                [3. * scale, 0., 0.],
                [0., 4. * scale, 0.],
                [0., 0., 5. * scale],
            ]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec(),
            vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
            // These are already-defined facets, including features far below
            // the default document's modeling tolerance.
            Tolerance::MESH_VALIDATION,
        )
        .unwrap();
        let units = LengthUnitSystem::Custom {
            name: "scaled meter".into(),
            meters_per_unit: 2_f64.powi(-exponent),
        };
        for (m, expected) in [(&mesh, 10.), (&mesh.reversed(), -10.)] {
            assert_eq!(
                VolumeMassProperties::signed_volume_from_boundaries_in_units(
                    &[VolumeBoundary::Mesh(m)],
                    Tolerance::DEFAULT,
                    &units,
                    &LengthUnitSystem::Meters,
                )
                .unwrap(),
                expected
            );
        }
    }
}

#[test]
fn nominal_units_have_exact_cubic_ratios_before_final_rounding() {
    use crate::LengthUnitSystem::*;
    let frame = Frame3::try_from_points(
        Point3::try_from([0.; 3]).unwrap(),
        Point3::try_from([1., 0., 0.]).unwrap(),
        Point3::try_from([0., 1., 0.]).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let b = crate::Brep::try_box(frame, [[0., 1.]; 3], Tolerance::DEFAULT).unwrap();
    for (source, target, expected) in [
        (Millimeters, Microns, 1e9),
        (Inches, Microinches, 1e18),
        (Feet, Inches, 1728.),
        (Meters, Decimeters, 1000.),
        (Millimeters, Meters, 1e-9),
        (None, Meters, 1.),
    ] {
        let value = VolumeMassProperties::signed_volume_from_boundaries_in_units(
            &[VolumeBoundary::Brep(&b)],
            Tolerance::DEFAULT,
            &source,
            &target,
        )
        .unwrap();
        // The B-rep integral is numerical; compare its unchanged raw value with
        // independently specified exact conversion factors, not observed targets.
        let raw = VolumeMassProperties::signed_volume_from_boundaries(
            &[VolumeBoundary::Brep(&b)],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!((value / (raw * expected) - 1.).abs() < 3e-16);
    }
    assert!(
        VolumeMassProperties::signed_volume_from_boundaries_in_units(
            &[VolumeBoundary::Brep(&b)],
            Tolerance::DEFAULT,
            &Unset,
            &Meters,
        )
        .is_err()
    );
}

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
