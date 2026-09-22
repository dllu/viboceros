use super::*;
use crate::{Brep, Frame3, MeshFace, NurbsSurface, Tolerance, TriangleMesh, Vector3};

fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn tetrahedron(offset: Real, scale: Real) -> TriangleMesh {
    TriangleMesh::try_new(
        vec![
            p(offset, 0., 0.),
            p(offset + 3. * scale, 0., 0.),
            p(offset, 4. * scale, 0.),
            p(offset, 0., 5. * scale),
            p(1e308, 1e308, 1e308),
        ],
        vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
        Tolerance::try_new(1e-250, 1e-12, 1e-10).unwrap(),
    )
    .unwrap()
}

#[test]
fn exact_mesh_moments_survive_translation_winding_and_unrepresentable_volume() {
    for (offset, scale) in [
        (0., 1.),
        (1e12, 1.),
        (1e100, 1e85),
        (0., 1e-200),
        (0., 1e200),
    ] {
        let mesh = tetrahedron(offset, scale);
        let before = mesh.clone();
        let points = &mesh.vertices()[..4];
        let expected: [Rational; 3] = std::array::from_fn(|i| {
            points
                .iter()
                .map(|p| rational(p.to_array()[i]))
                .sum::<Rational>()
                / Rational::from_integer(4.into())
        });
        let volume = (rational(points[1].x()) - rational(points[0].x()))
            * rational(points[2].y())
            * rational(points[3].z())
            / Rational::from_integer(6.into());
        for (m, sign) in [(&mesh, 1.), (&mesh.reversed(), -1.)] {
            let mass = m.volume_mass_properties().unwrap();
            assert_eq!(mass.volume, &volume * rational(sign));
            assert_eq!(
                mass.centroid().unwrap().to_array(),
                expected.clone().map(|v| scalar(&v).unwrap())
            );
        }
        assert_eq!(mesh, before);
    }
}

#[test]
fn signed_aggregation_keeps_negative_weights_and_zero_volume_has_no_centroid() {
    let mut mass = tetrahedron(0., 1.).volume_mass_properties().unwrap();
    mass.add(
        &tetrahedron(10., 2.)
            .reversed()
            .volume_mass_properties()
            .unwrap(),
    );
    assert_eq!(mass.signed_volume().unwrap(), -70.);
    assert_eq!(
        mass.centroid().unwrap().to_array(),
        [365. / 28., 15. / 7., 75. / 28.]
    );
    let mut zero = tetrahedron(0., 1.).volume_mass_properties().unwrap();
    zero.add(
        &tetrahedron(10., 1.)
            .reversed()
            .volume_mass_properties()
            .unwrap(),
    );
    assert!(zero.is_zero());
    assert!(zero.centroid().is_err());
}

#[test]
fn open_and_inconsistently_oriented_meshes_are_not_closed_volume_distributions() {
    let mesh = tetrahedron(0., 1.);
    let mut faces = mesh.triangles().to_vec();
    faces[0].swap(0, 1);
    let bad = TriangleMesh::try_new(mesh.vertices().to_vec(), faces, Tolerance::DEFAULT).unwrap();
    assert_eq!(
        bad.volume_mass_properties().unwrap_err(),
        GeometryError::InvalidVolumeMesh
    );
    let open = TriangleMesh::try_new(
        mesh.vertices().to_vec(),
        mesh.triangles()[1..].to_vec(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(
        open.volume_mass_properties().unwrap_err(),
        GeometryError::InvalidVolumeMesh
    );
}

#[test]
fn warped_quad_volume_and_centroid_use_the_shorter_diagonal_in_all_orders() {
    let vertices = [
        [0., 0., 0.],
        [4., 0., 0.],
        [4., 3., 0.],
        [0., 3., 0.],
        [0., 0., 2.],
        [4., 0., 2.],
        [4., 3., 3.],
        [0., 3., 2.],
    ]
    .map(|v| Point3::try_from(v).unwrap());
    let faces = [
        [0, 3, 2, 1],
        [4, 5, 6, 7],
        [0, 1, 5, 4],
        [1, 2, 6, 5],
        [2, 3, 7, 6],
        [3, 0, 4, 7],
    ];
    for rotation in 0..4 {
        for reversed in [false, true] {
            let faces = faces.map(|mut f| {
                f.rotate_left(rotation);
                if reversed {
                    f.reverse();
                }
                MeshFace::Quad(f)
            });
            let mesh =
                TriangleMesh::try_new_faces(vertices.to_vec(), faces.to_vec(), Tolerance::DEFAULT)
                    .unwrap();
            let mass = mesh.volume_mass_properties().unwrap();
            let sign = if reversed { -1. } else { 1. };
            assert_eq!(mass.signed_volume().unwrap(), sign * 26.);
            // A 24-unit rectangular box plus a 2-unit corner tetrahedron:
            // the added tetrahedron has centroid (3,9/4,9/4).
            assert_eq!(
                mass.centroid().unwrap().to_array(),
                [27. / 13., 81. / 52., 57. / 52.]
            );
            assert!((mesh.signed_volume().unwrap() - sign * 26.).abs() < 1e-12);
        }
    }
}

#[test]
fn closed_sphere_and_cylinder_integrate_spatial_moments_not_control_net_averages() {
    let frame = Frame3::try_from_normal(
        p(8., -4., 16.),
        Vector3::try_new(0., 1., 0.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let sphere = NurbsSurface::try_sphere(frame, 2.).unwrap();
    let mass = sphere.volume_mass_properties(Tolerance::DEFAULT).unwrap();
    assert!((mass.signed_volume().unwrap() - 32. * std::f64::consts::PI / 3.).abs() < 1e-10);
    assert!(
        mass.centroid()
            .unwrap()
            .distance_to(frame.origin())
            .unwrap()
            < 1e-12
    );
    let cylinder = Brep::try_cylinder(frame, 2., 0., 5., Tolerance::DEFAULT).unwrap();
    for (b, sign) in [(&cylinder, 1.), (&cylinder.reversed(), -1.)] {
        let mass = b.volume_mass_properties(Tolerance::DEFAULT).unwrap();
        assert!((mass.signed_volume().unwrap() - sign * 20. * std::f64::consts::PI).abs() < 1e-10);
        assert!(
            mass.centroid()
                .unwrap()
                .distance_to(p(8., -1.5, 16.))
                .unwrap()
                < 1e-12
        );
    }
}

#[test]
fn offcenter_cavity_subtracts_first_moments_in_one_common_spatial_frame() {
    let frame = |x| {
        Frame3::try_from_normal(
            p(x, 0., 0.),
            Vector3::try_new(0., 0., 1.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap()
    };
    let outer = Brep::try_box(frame(0.), [[-5., 5.]; 3], Tolerance::DEFAULT).unwrap();
    let inner = Brep::try_box(frame(2.), [[-1., 1.]; 3], Tolerance::DEFAULT)
        .unwrap()
        .reversed();
    let hollow = Brep::try_combine(vec![outer, inner], Tolerance::DEFAULT).unwrap();
    let mass = hollow.volume_mass_properties(Tolerance::DEFAULT).unwrap();
    assert!((mass.signed_volume().unwrap() - 992.).abs() < 1e-10);
    assert!(
        mass.centroid()
            .unwrap()
            .distance_to(p(-16. / 992., 0., 0.))
            .unwrap()
            < 1e-12
    );
    for translation in [0., 1e12] {
        let transformed = hollow
            .transformed(
                crate::AffineTransform3::from_translation(
                    Vector3::try_new(translation, 2. * translation, -translation).unwrap(),
                ),
                Tolerance::DEFAULT,
            )
            .unwrap();
        let mass = transformed
            .volume_mass_properties(Tolerance::DEFAULT)
            .unwrap();
        assert!((mass.signed_volume().unwrap() - 992.).abs() < 1e-10);
        assert!(
            mass.centroid()
                .unwrap()
                .distance_to(p(translation - 16. / 992., 2. * translation, -translation))
                .unwrap()
                < 1e-12
        );
    }
}
