use super::*;
use crate::Circle3;

#[test]
fn point_closed_solids_face_outward_for_both_profile_traversals() {
    let original = circle(1., 1.);
    for profile in [original.clone(), original.reversed().unwrap()] {
        for style in [
            LoftStyle::Normal,
            LoftStyle::Loose,
            LoftStyle::Tight,
            LoftStyle::Straight,
        ] {
            let brep = Brep::try_loft_with_end_points(
                std::slice::from_ref(&profile),
                style,
                LoftEndPoints {
                    start: Some(Point3::try_new(0., 0., -1.).unwrap()),
                    end: Some(Point3::try_new(0., 0., 3.).unwrap()),
                },
                LoftOptions::default(),
                Tolerance::DEFAULT,
            )
            .unwrap();
            assert!(brep.is_solid());
            for face in brep.faces() {
                let surface = face.surface();
                let u = surface.parameter_at_u(0.5).unwrap();
                let v = surface.parameter_at_v(0.125).unwrap();
                let point = surface.evaluate(u, v).unwrap();
                let normal = surface.normal_at(u, v).unwrap().as_vector();
                let outward = normal
                    .dot(Vector3::try_new(point.x(), point.y(), 0.).unwrap())
                    .unwrap();
                assert!(
                    if face.is_reversed() {
                        outward < 0.
                    } else {
                        outward > 0.
                    },
                    "{style:?}: pole solid must face outward"
                );
            }
        }
    }
}

fn circle(z: Real, radius: Real) -> NurbsCurve {
    Circle3::try_new(
        Point3::try_new(0.0, 0.0, z).unwrap(),
        radius,
        UnitVector3::try_new(0.0, 0.0, 1.0, Tolerance::DEFAULT).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
    .to_nurbs()
    .unwrap()
}

#[test]
fn parallel_closed_profiles_sort_align_and_form_an_outward_open_shell() {
    let ordered = [circle(0.0, 1.0), circle(1.0, 2.0), circle(3.0, 1.0)];
    let shuffled = [
        ordered[2].clone(),
        ordered[0].clone(),
        ordered[1].reversed().unwrap(),
    ];
    let brep = Brep::try_loft(&shuffled, LoftStyle::Normal, false, Tolerance::DEFAULT).unwrap();
    let expected = try_loft_nurbs_curves(&ordered, LoftStyle::Normal, false)
        .unwrap()
        .try_reversed_v()
        .unwrap();
    assert_eq!(brep.faces()[0].surface(), &expected);
    assert_eq!((brep.vertices().len(), brep.edges().len()), (2, 3));
    let (p, du, dv) = expected
        .evaluate_with_derivatives(
            *expected.domain_u().start(),
            expected.parameter_at_v(0.125).unwrap(),
        )
        .unwrap();
    assert!(
        du.cross(dv)
            .unwrap()
            .dot(Vector3::try_new(p.x(), p.y(), 0.0).unwrap())
            .unwrap()
            > 0.0
    );
    let mesh = brep
        .polygon_mesh(0.0, false, false, Tolerance::DEFAULT)
        .unwrap();
    assert!(mesh.topology().is_manifold());
    assert!(!mesh.topology().is_closed());
}

#[test]
fn natural_retains_offset_seams_while_automatic_alignment_avoids_a_twist() {
    let first = circle(0., 1.);
    let second = circle(2., 1.);
    let second = second
        .try_change_closed_seam(second.parameter_at(0.25).unwrap())
        .unwrap();
    let source = [first, second];
    let before = source.clone();
    for (natural, expected) in [(false, [1., 0., 1.]), (true, [0.5, 0.5, 1.])] {
        let brep = Brep::try_loft_with_surface_options(
            &source,
            LoftStyle::Normal,
            false,
            LoftOptions {
                natural_seams: natural,
                ..LoftOptions::default()
            },
            Tolerance::DEFAULT,
            &mut try_loft_nurbs_curves,
        )
        .unwrap();
        assert_eq!(brep.faces().len(), 1);
        let surface = brep.faces()[0].surface();
        let middle = surface
            .evaluate(
                surface.parameter_at_u(0.5).unwrap(),
                *surface.domain_v().start(),
            )
            .unwrap();
        assert!(
            middle
                .distance_to(Point3::try_from(expected).unwrap())
                .unwrap()
                < 1e-12,
            "Natural={natural}: {middle:?}"
        );
        assert_eq!(source, before);
    }
}

#[test]
fn straight_polyline_loft_sews_both_kink_directions_and_the_profile_seam() {
    let source = [(0.0, 1.0), (2.0, 2.0), (4.0, 1.0)]
        .into_iter()
        .map(|(z, r)| {
            NurbsCurve::try_new(
                1,
                [[0.0, 0.0], [r, 0.0], [r, r], [0.0, r], [0.0, 0.0]]
                    .into_iter()
                    .map(|[x, y]| Point3::try_new(x, y, z).unwrap())
                    .collect(),
                vec![0.0, 0.0, 0.25, 0.5, 0.75, 1.0, 1.0],
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let brep = Brep::try_loft(&source, LoftStyle::Straight, false, Tolerance::DEFAULT).unwrap();
    assert_eq!(
        (
            brep.vertices().len(),
            brep.edges().len(),
            brep.faces().len()
        ),
        (12, 20, 8)
    );
    assert!(
        brep.faces()
            .iter()
            .all(|f| *f.surface().domain_v().start() >= 0.0 && !f.is_reversed())
    );
    let mesh = brep
        .polygon_mesh(0.0, false, false, Tolerance::DEFAULT)
        .unwrap();
    assert!(mesh.topology().is_manifold());
    assert!(!mesh.topology().is_closed());
}

#[test]
fn slightly_tilted_profiles_are_not_mistaken_for_parallel_planes() {
    let c = circle(3.0, 1.0);
    let (sine, cosine) = 1e-6_f64.sin_cos();
    let tilted = NurbsCurve::try_new_rational(
        c.degree(),
        c.control_points()
            .iter()
            .map(|c| {
                let p = c.point();
                WeightedPoint3::try_new(
                    Point3::try_new(p.x(), p.y() * cosine, 3.0 + p.y() * sine).unwrap(),
                    c.weight(),
                )
                .unwrap()
            })
            .collect(),
        c.knots().to_vec(),
    )
    .unwrap();
    let mut curves = [circle(0.0, 1.0), tilted];
    let original = curves.clone();
    assert!(!sort_parallel_profiles(&mut curves, Tolerance::DEFAULT).unwrap());
    assert_eq!(curves, original);
}
