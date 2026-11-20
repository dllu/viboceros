use super::*;
use viboceros_geometry::{BrepTrimType, WeightedPoint3};

const OCTANTS: [[Real; 2]; 9] = [
    [1., 0.],
    [1., 1.],
    [0., 1.],
    [-1., 1.],
    [-1., 0.],
    [-1., -1.],
    [0., -1.],
    [1., -1.],
    [1., 0.],
];
const WEIGHT: Real = std::f64::consts::FRAC_1_SQRT_2;

fn p([x, y]: [Real; 2], radius: Real, z: Real) -> Point3 {
    Point3::try_new(radius * x, radius * y, z).unwrap()
}

fn circle(radius: Real) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        2,
        OCTANTS
            .into_iter()
            .enumerate()
            .map(|(i, xy)| {
                WeightedPoint3::try_new(p(xy, radius, 0.), if i % 2 == 0 { 1. } else { WEIGHT })
                    .unwrap()
            })
            .collect(),
        vec![0., 0., 0., 0.25, 0.25, 0.5, 0.5, 0.75, 0.75, 1., 1., 1.],
    )
    .unwrap()
}

fn rail_pair(doc: &mut Document, a: NurbsCurve, b: NurbsCurve, profiles: Vec<NurbsCurve>) {
    let ids = [a, b]
        .into_iter()
        .chain(profiles)
        .map(|c| doc.add_geometry(Geometry::NurbsCurve(c)).unwrap())
        .collect::<Vec<_>>();
    doc.select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
}

fn run<'a>(doc: &'a mut Document, options: &str) -> &'a Brep {
    CommandRegistry::with_builtins()
        .execute(doc, &format!("Sweep2 {options}"))
        .unwrap();
    assert_eq!(doc.undo_label(), Some("Sweep2"));
    let Geometry::Brep(result) = doc.objects().last().unwrap().geometry() else {
        panic!("Expected a Sweep2 B-rep");
    };
    result
}

fn profile(xy: [Real; 2], height: Real) -> NurbsCurve {
    if height == 0. {
        return NurbsCurve::try_new(1, vec![p(xy, 1., 0.), p(xy, 3., 0.)], vec![0., 0., 1., 1.])
            .unwrap();
    }
    NurbsCurve::try_new(
        2,
        vec![p(xy, 1., 0.), p(xy, 2., height), p(xy, 3., 0.)],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap()
}

#[test]
fn sweep_two_rail_closed_circles_match_native_patches_and_seams() {
    // Rhino 8.32 captures for radial profiles. The first pick sets the seam,
    // even when the first profile is away from the authored seam of both rails.
    for (start, step, profiles, options, heights, area) in [
        (
            0,
            1_i32,
            &[(0, 0.), (2, 0.)][..],
            "Closed=No",
            &[[0.; 3]][..],
            6.2831853847175783,
        ),
        (
            2,
            1,
            &[(2, 0.), (4, 0.)][..],
            "Closed=Yes",
            &[[0.; 3]; 4][..],
            25.132741538870338,
        ),
        (
            2,
            1,
            &[(2, 0.5)][..],
            "",
            &[[0.5; 3]; 4][..],
            26.143802006078587,
        ),
        (
            0,
            1,
            &[(0, 0.5), (2, 0.8)][..],
            "Closed=Yes",
            &[
                [0.5, 0.65, 0.8],
                [0.8, 0.8013480038173294, 0.7222222222222223],
                [0.7222222222222223, 0.65, 0.5777777777777778],
                [0.5777777777777778, 0.49865199618267042, 0.5],
            ][..],
            26.840998622947957,
        ),
        (
            0,
            1,
            &[(0, 0.5), (6, 0.8)][..],
            "Closed=No",
            &[
                [0.5, 0.49865199618267042, 0.5777777777777778],
                [0.5777777777777778, 0.65, 0.7222222222222223],
                [0.7222222222222223, 0.8013480038173294, 0.8],
            ][..],
            20.129799185882742,
        ),
        (
            0,
            1,
            &[(0, 0.5), (2, 0.8), (4, 0.5)][..],
            "Closed=Yes",
            &[[0.5, 0.65, 0.8], [0.8, 0.65, 0.5], [0.5; 3], [0.5; 3]][..],
            26.494299877169738,
        ),
        (
            0,
            -1,
            &[(0, 0.5), (6, 0.8), (2, 0.5)][..],
            "Closed=Yes",
            &[
                [0.5, 0.65, 0.8],
                [0.8, 0.7928997564417433, 0.65],
                [0.65, 0.5071002435582568, 0.5],
                [0.5; 3],
            ][..],
            26.668150168617203,
        ),
    ] {
        let mut doc = Document::default();
        let tolerance = doc.tolerance();
        let sections = profiles
            .iter()
            .enumerate()
            .map(|(i, &(j, height))| {
                let c = profile(OCTANTS[j], height);
                if i == 0 { c } else { c.reversed().unwrap() }
            })
            .collect();
        rail_pair(
            &mut doc,
            circle(1.),
            circle(3.).reversed().unwrap(),
            sections,
        );
        let brep = run(&mut doc, options);
        let faces = heights.len();
        let closed = faces == 4;
        assert_eq!(brep.faces().len(), faces);
        assert_eq!(brep.edges().len(), 3 * faces + usize::from(!closed));
        assert_eq!(brep.vertices().len(), 2 * (faces + usize::from(!closed)));
        for (i, (face, expected_heights)) in brep.faces().iter().zip(heights).enumerate() {
            let surface = face.surface();
            assert_eq!(surface.degree_u(), 2);
            let trims = face.loops()[0].trims();
            assert_eq!(
                trims[1].trim_type(),
                if closed || i + 1 < faces {
                    BrepTrimType::Mated
                } else {
                    BrepTrimType::Boundary
                }
            );
            assert_eq!(
                trims[3].trim_type(),
                if closed || i > 0 {
                    BrepTrimType::Mated
                } else {
                    BrepTrimType::Boundary
                }
            );
            for (u, &z) in expected_heights.iter().enumerate() {
                let xy = OCTANTS[(start + step * (2 * i + u) as i32).rem_euclid(8) as usize];
                for (v, (radius, height)) in if profiles[0].1 == 0. {
                    vec![(1., 0.), (3., 0.)]
                } else {
                    vec![(1., 0.), (2., z), (3., 0.)]
                }
                .into_iter()
                .enumerate()
                {
                    let control = surface.control_point(u, v).unwrap();
                    assert!(
                        control.point().distance_to(p(xy, radius, height)).unwrap() < 1e-8,
                        "{options}, {i}/{u}/{v}: {control:?}"
                    );
                    let weight = if u == 1 { WEIGHT } else { 1. };
                    assert!((control.weight() - weight).abs() < 1e-9);
                }
            }
        }
        let actual = brep.area(tolerance).unwrap();
        assert!(
            (actual - area).abs() < 1e-5,
            "{options}: {actual} != {area}"
        );
    }
}

#[test]
fn sweep_two_rail_closed_squares_keep_kinks_and_allow_joined_rail_pieces() {
    let mut doc = Document::default();
    let tolerance = doc.tolerance();
    let corners = [[1., 1.], [-1., 1.], [-1., -1.], [1., -1.], [1., 1.]];
    let rails = [1., 3.].map(|radius| {
        let mut chain = corners
            .windows(2)
            .map(|span| {
                let line =
                    LineSegment::try_new(p(span[0], radius, 0.), p(span[1], radius, 0.), tolerance)
                        .unwrap();
                let id = doc.add_geometry(Geometry::Line(line)).unwrap();
                (Target::Curve(id), false)
            })
            .collect::<Vec<_>>();
        if radius == 3. {
            chain.reverse();
            for piece in &mut chain {
                piece.1 = true;
            }
        }
        Rail::chain(&doc, &chain, tolerance.absolute()).unwrap()
    });
    let ids = [corners[0], corners[1]].map(|xy| {
        doc.add_geometry(Geometry::NurbsCurve(profile(xy, 0.)))
            .unwrap()
    });
    for (sections, closed, faces, edges, area) in [
        (2, false, 1, 4, 8.),
        (2, true, 4, 12, 32.),
        (1, false, 4, 12, 32.),
    ] {
        let source = Source::with_rails(&doc, rails.clone(), &ids[..sections]).unwrap();
        let brep = source
            .build(
                ShapeOptions {
                    closed,
                    ..Default::default()
                },
                &[],
                tolerance,
            )
            .unwrap();
        assert_eq!(brep.faces().len(), faces);
        assert_eq!(brep.edges().len(), edges);
        assert!(
            brep.faces()
                .iter()
                .all(|face| face.surface().degree_u() == 3)
        );
        assert!((brep.area(tolerance).unwrap() - area).abs() < 1e-8);
    }
}

#[test]
fn sweep_two_rail_closed_refit_sews_the_smooth_rail_pair() {
    let tolerance = Tolerance::try_new(0.001, 1e-12, 1e-10).unwrap();
    let mut doc = Document::new(tolerance);
    rail_pair(
        &mut doc,
        circle(1.),
        circle(3.),
        vec![profile(OCTANTS[0], 0.5), profile(OCTANTS[2], 0.8)],
    );
    let brep = run(&mut doc, "Closed=Yes RefitRails=Yes");
    assert_eq!(brep.faces().len(), 1);
    assert_eq!(brep.edges().len(), 3);
    assert_eq!(brep.faces()[0].surface().degree_u(), 3);
    assert!(brep.faces()[0].surface().is_closed_u().unwrap());
    assert!((brep.area(tolerance).unwrap() - 26.843988910923045).abs() < 0.03);
}

#[test]
fn sweep_two_rail_closed_rails_refit_profiles_between_knots() {
    let tolerance = Tolerance::try_new(0.001, 1e-12, 1e-10).unwrap();
    let mut doc = Document::new(tolerance);
    let sections = [(0.33, 0.5), (1.71, 0.8)].map(|(station, height)| {
        let theta = station * std::f64::consts::FRAC_PI_2;
        let (y, x) = theta.sin_cos();
        profile([x, y], height)
    });
    let expected_ends = sections.each_ref().map(|c| c.evaluate(0.5).unwrap());
    rail_pair(
        &mut doc,
        circle(1.),
        circle(3.).reversed().unwrap(),
        sections.into(),
    );
    let brep = run(&mut doc, "Closed=No");
    assert_eq!(brep.faces().len(), 1);
    let surface = brep.faces()[0].surface();
    assert_eq!(surface.degree_u(), 3);
    let domain = surface.domain_u();
    for (station, expected) in [*domain.start(), *domain.end()]
        .into_iter()
        .zip(expected_ends)
    {
        assert!(
            surface
                .evaluate(station, 0.5)
                .unwrap()
                .distance_to(expected)
                .unwrap()
                < 1e-8
        );
    }
}

#[test]
fn sweep_two_rail_closed_arc_direction_ignores_unequal_parameter_speeds() {
    let mut doc = Document::default();
    let tolerance = doc.tolerance();
    let regular = circle(3.);
    let uneven = NurbsCurve::try_new_rational(
        2,
        regular.control_points().to_vec(),
        vec![0., 0., 0., 0.8, 0.8, 0.9, 0.9, 0.95, 0.95, 1., 1., 1.],
    )
    .unwrap();
    rail_pair(
        &mut doc,
        circle(1.),
        uneven,
        vec![profile(OCTANTS[0], 0.), profile(OCTANTS[2], 0.)],
    );
    let brep = run(&mut doc, "Closed=No");
    assert_eq!(brep.faces().len(), 1);
    assert!((brep.area(tolerance).unwrap() - 2. * std::f64::consts::PI).abs() < 1e-8);
}
