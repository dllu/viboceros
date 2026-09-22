use super::*;
use crate::{
    Brep, Circle3, CurveRef, NurbsCurve, NurbsSurface, Polyline3, Tolerance, TriangleMesh,
    UnitVector3,
};

fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn check(mass: &AreaMassProperties, area: Real, centroid: Point3, epsilon: Real) {
    assert!(
        (mass.area().unwrap() - area).abs() < epsilon,
        "area: {:?}",
        mass.area()
    );
    assert!(
        mass.centroid().unwrap().distance_to(centroid).unwrap() < epsilon,
        "centroid: {:?}",
        mass.centroid()
    );
}

#[test]
fn exact_accumulation_retains_subnormal_weights_and_overflowing_sums() {
    let mut mass = AreaMassProperties::default();
    assert!(mass.centroid().is_err());
    assert_eq!(mass.area().unwrap(), 0.);
    let huge = rational(Real::MAX) * rational(Real::MAX);
    mass.add(&AreaMassProperties::at_point(
        huge.clone(),
        p(Real::MAX, 1., 0.),
    ));
    mass.add(&AreaMassProperties::at_point(huge, p(-Real::MAX, 3., 0.)));
    assert_eq!(mass.centroid().unwrap(), p(0., 2., 0.));
    assert!(mass.area().is_err());
    let tiny = rational(Real::from_bits(1));
    let a = AreaMassProperties::at_point(&tiny * &tiny, p(1., 0., 0.));
    let mut b = a.clone();
    b.add(&a);
    assert_eq!(b.area().unwrap(), 0.);
    assert_eq!(b.centroid().unwrap(), p(1., 0., 0.));
}

#[test]
fn meshes_use_triangle_area_not_vertex_average_and_ignore_unused_vertices() {
    let mesh = TriangleMesh::try_new(
        vec![
            p(0., 0., 0.),
            p(6., 0., 0.),
            p(0., 3., 0.),
            p(20., 0., 0.),
            p(22., 0., 0.),
            p(20., 2., 0.),
            p(1e100, 1e100, 1e100),
        ],
        vec![[0, 1, 2], [3, 4, 5]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    for mesh in [mesh.clone(), mesh.reversed()] {
        check(
            &mesh.area_mass_properties().unwrap(),
            11.,
            p(178. / 33., 31. / 33., 0.),
            1e-14,
        );
    }
}

#[test]
fn mesh_centroids_remain_finite_when_area_underflows_or_overflows() {
    for scale in [1e-200, 1., 1e200] {
        let mesh = TriangleMesh::try_new(
            vec![p(0., 0., 0.), p(3. * scale, 0., 0.), p(0., 3. * scale, 0.)],
            vec![[0, 1, 2]],
            Tolerance::try_new(1e-250, 1e-12, 1e-10).unwrap(),
        )
        .unwrap();
        let mass = mesh.area_mass_properties().unwrap();
        let centroid = mass.centroid().unwrap();
        assert!((centroid.x() / scale - 1.).abs() < 1e-15);
        assert!((centroid.y() / scale - 1.).abs() < 1e-15);
    }
}

#[test]
fn nonrectangular_closed_curves_integrate_the_enclosed_region() {
    let polygon = Polyline3::try_new(
        vec![
            p(0., 0., 0.),
            p(6., 0., 0.),
            p(6., 1., 0.),
            p(1., 1., 0.),
            p(1., 4., 0.),
            p(0., 4., 0.),
            p(0., 0., 0.),
        ],
        Tolerance::DEFAULT,
    )
    .unwrap();
    check(
        &CurveRef::Polyline(&polygon)
            .planar_area_mass_properties(Tolerance::DEFAULT)
            .unwrap(),
        9.,
        p(13. / 6., 7. / 6., 0.),
        1e-10,
    );
    let curve = NurbsCurve::try_new(
        3,
        vec![p(0., 0., 0.), p(1., 0., 0.), p(0., 1., 0.), p(0., 0., 0.)],
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap();
    for c in [curve.clone(), curve.reversed().unwrap()] {
        // Green-theorem polynomial integration: area=3/20, Mx=My=9/280.
        check(
            &CurveRef::NurbsCurve(&c)
                .planar_area_mass_properties(Tolerance::DEFAULT)
                .unwrap(),
            3. / 20.,
            p(3. / 14., 3. / 14., 0.),
            1e-10,
        );
    }
}

#[test]
fn offcenter_hole_subtracts_area_and_first_moments_independently_of_face_sense() {
    let normal = UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap();
    let outer = Circle3::try_new(p(0., 0., 0.), 3., normal, Tolerance::DEFAULT)
        .unwrap()
        .to_nurbs()
        .unwrap();
    let inner = Circle3::try_new(p(1., 0., 0.), 1., normal, Tolerance::DEFAULT)
        .unwrap()
        .to_nurbs()
        .unwrap();
    let brep = Brep::try_planar_face_with_holes(&outer, &[inner], Tolerance::DEFAULT).unwrap();
    for b in [brep.clone(), brep.reversed()] {
        check(
            &b.area_mass_properties(Tolerance::DEFAULT).unwrap(),
            8. * std::f64::consts::PI,
            p(-0.125, 0., 0.),
            1e-10,
        );
    }
}

#[test]
fn nonplanar_surface_centroid_matches_analytic_integrals_not_its_bounds_center() {
    let surface = NurbsSurface::try_new(
        2,
        1,
        3,
        2,
        vec![
            p(0., 0., 0.),
            p(0.5, 0., 0.),
            p(1., 0., 1.),
            p(0., 1., 0.),
            p(0.5, 1., 0.),
            p(1., 1., 1.),
        ],
        vec![0., 0., 0., 1., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let area = 5_f64.sqrt() / 2. + 2_f64.asinh() / 4.;
    let x = (5. * 5_f64.sqrt() - 1.) / 12. / area;
    let z = (18. * 5_f64.sqrt() - 2_f64.asinh()) / 64. / area;
    check(
        &surface.area_mass_properties(Tolerance::DEFAULT).unwrap(),
        area,
        p(x, 0.5, z),
        1e-11,
    );
    let brep = Brep::try_surface_face(surface, Tolerance::DEFAULT).unwrap();
    check(
        &brep.area_mass_properties(Tolerance::DEFAULT).unwrap(),
        area,
        p(x, 0.5, z),
        1e-11,
    );
}

#[test]
fn normalized_surface_integration_preserves_large_translations_and_model_scales() {
    for (offset, scale) in [(0., 1.), (1e12, 4.), (0., 1e-100), (0., 1e100)] {
        let surface = NurbsSurface::try_new(
            1,
            1,
            2,
            2,
            vec![
                p(offset, 0., 0.),
                p(offset + 2. * scale, 0., 0.),
                p(offset, 4. * scale, 0.),
                p(offset + 2. * scale, 4. * scale, 0.),
            ],
            vec![0., 0., 1., 1.],
            vec![0., 0., 1., 1.],
        )
        .unwrap();
        let mass = surface.area_mass_properties(Tolerance::DEFAULT).unwrap();
        let center = mass.centroid().unwrap();
        assert_eq!(center.x(), offset + scale);
        assert!((center.y() / scale - 2.).abs() < 1e-14);
        assert!((mass.area().unwrap() / (scale * scale) - 8.).abs() < 1e-13);
    }
}

#[test]
fn open_and_nonplanar_boundaries_do_not_invent_area_centroids() {
    for vertices in [
        vec![p(0., 0., 0.), p(1., 0., 0.), p(1., 1., 0.)],
        vec![
            p(0., 0., 0.),
            p(1., 0., 0.),
            p(1., 1., 1.),
            p(0., 1., 0.),
            p(0., 0., 0.),
        ],
    ] {
        let curve = Polyline3::try_new(vertices, Tolerance::DEFAULT).unwrap();
        assert!(
            CurveRef::Polyline(&curve)
                .planar_area_mass_properties(Tolerance::DEFAULT)
                .is_err()
        );
    }
}

#[test]
fn warped_quad_mass_uses_short_diagonal_without_rewriting_display_or_source_faces() {
    use crate::MeshFace;
    let vertices = vec![p(0., 0., 0.), p(4., 0., 0.), p(4., 3., 2.), p(0., 3., 0.)];
    let area = 6. + 61_f64.sqrt();
    let centroid = p(
        (6. * (4. / 3.) + 61_f64.sqrt() * (8. / 3.)) / area,
        (6. + 61_f64.sqrt() * 2.) / area,
        61_f64.sqrt() * (2. / 3.) / area,
    );
    for reverse in [false, true] {
        for rotation in 0..4 {
            let mut order = vec![0, 1, 2, 3];
            if reverse {
                order.reverse();
            }
            order.rotate_left(rotation);
            let mesh = TriangleMesh::try_new_faces(
                vertices.clone(),
                vec![MeshFace::Quad(order.clone().try_into().unwrap())],
                Tolerance::DEFAULT,
            )
            .unwrap();
            let before = mesh.clone();
            let mass = mesh.area_mass_properties().unwrap();
            check(&mass, area, centroid, 1e-13);
            assert!((mesh.area().unwrap() - area).abs() < 1e-13);
            assert_eq!(mesh, before);
            assert_eq!(mesh.triangles()[0], [order[0], order[1], order[2]]);
        }
    }
}
