use super::*;

fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

fn support(axis: usize, warp: Real) -> NurbsSurface {
    let cross = if axis == 0 {
        [(0., 0.), (-1., -0.4), (-2., -0.2)]
    } else {
        [(4., 0.), (5., -0.3), (6., -0.2)]
    };
    let points = [0., 6.]
        .into_iter()
        .flat_map(|y| cross.map(|(x, z)| p(x, y, z * (1. + warp * y / 6.))))
        .collect();
    NurbsSurface::try_new(
        2,
        1,
        3,
        2,
        points,
        vec![0., 0., 0., 1., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap()
}

fn inputs(doc: &mut Document, guide_curve: bool, skew: bool, width: usize, warp: Real) -> String {
    let mut options = Vec::new();
    for i in 0..2 {
        if i == 1 && guide_curve {
            let line = LineSegment::try_new(p(4., 0., 0.), p(4., 6., 0.), doc.tolerance()).unwrap();
            let id = doc.add_geometry(Geometry::Line(line)).unwrap();
            doc.set_object_names([(id, Some("Loose".into()))]).unwrap();
            options.push("Rail2Name=Loose".into());
            continue;
        }
        let support = support(i, warp);
        let brep = Brep::try_surface_face(support.clone(), doc.tolerance()).unwrap();
        let x = 4. * i as Real;
        let index = brep
            .edges()
            .iter()
            .position(|e| {
                let curve = e.curve();
                curve.evaluate(*curve.domain().start()).unwrap().x() == x
                    && curve.evaluate(*curve.domain().end()).unwrap().x() == x
            })
            .unwrap();
        let id = doc.add_geometry(Geometry::NurbsSurface(support)).unwrap();
        options.push(format!("Rail{}Edge={id},{index}", i + 1));
    }
    let ids = [0., 6.]
        .into_iter()
        .zip([1., 1.4])
        .map(|(y, height)| {
            let points = (0..width)
                .map(|index| {
                    let fraction = index as Real / (width - 1) as Real;
                    let wave = (std::f64::consts::PI * fraction).sin();
                    p(
                        4. * fraction,
                        y + if skew { 0.95 * wave } else { 0. },
                        height * wave,
                    )
                })
                .collect();
            let knots = std::iter::repeat_n(0., width)
                .chain(std::iter::repeat_n(1., width))
                .collect();
            doc.add_geometry(Geometry::NurbsCurve(
                NurbsCurve::try_new(width - 1, points, knots).unwrap(),
            ))
            .unwrap()
        })
        .collect::<Vec<_>>();
    doc.select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    options.join(" ")
}

fn run<'a>(doc: &'a mut Document, options: &str) -> &'a Brep {
    let command = format!("Sweep2 {options}");
    CommandRegistry::with_builtins()
        .execute(doc, &command)
        .unwrap_or_else(|error| panic!("{command}: {error}"));
    let Geometry::Brep(brep) = doc.objects().last().unwrap().geometry() else {
        panic!("Expected Sweep2 B-rep");
    };
    brep
}

#[test]
fn sweep_two_rail_continuity_and_end_shapes_match_native_surfaces() {
    let mut doc = Document::default();
    let rails = inputs(&mut doc, false, false, 6, 0.);
    let originals = doc.objects().cloned().collect::<Vec<_>>();
    let tolerance = doc.tolerance();
    // Captured with Rhino 8.32's scripted Sweep2 after picking both real edges.
    for (a, b, start, end, area) in [
        ("Position", "Position", "Yes", "Yes", 27.118249079527505),
        ("Tangency", "Position", "Yes", "Yes", 26.734571357063068),
        ("Position", "Tangency", "No", "No", 26.797997651618882),
        ("Tangency", "Tangency", "Yes", "Yes", 26.225209650748400),
        ("Tangency", "Tangency", "No", "No", 26.481766005529067),
        ("Tangency", "Tangency", "Yes", "No", 26.405680238416110),
        ("Tangency", "Tangency", "No", "Yes", 26.299733657578518),
        ("Curvature", "Curvature", "Yes", "Yes", 25.517728838775486),
        ("Curvature", "Curvature", "No", "No", 26.025027297781762),
    ] {
        let brep = run(
            &mut doc,
            &format!(
                "{rails} EdgeContinuityA={a} EdgeContinuityB={b} StartShape={start} EndShape={end}"
            ),
        );
        assert_eq!((brep.faces().len(), brep.edges().len()), (1, 4));
        let surface = brep.faces()[0].surface();
        let degree_u = if a == "Position" && b == "Position" {
            1
        } else {
            3
        };
        assert_eq!([surface.degree_u(), surface.degree_v()], [degree_u, 5]);
        let start_y = surface.evaluate(0., 0.).unwrap().y();
        let end_y = surface.evaluate(1., 0.).unwrap().y();
        for (v, mode, slope, curvature) in [(0., a, 0.4, 0.3), (1., b, -0.3, 0.2)] {
            for u in [1. / 3., 2. / 3.] {
                let jet = surface.evaluate_with_second_derivatives(u, v).unwrap();
                let expected = p(4. * v, start_y + u * (end_y - start_y), 0.);
                assert!(
                    jet.point.distance_to(expected).unwrap() < 1e-8,
                    "{a} {b} {u} {v}: {:?} != {expected:?}",
                    jet.point
                );
                if mode != "Position" {
                    assert!((jet.derivative_v.z() - slope * jet.derivative_v.x()).abs() < 1e-8);
                }
                if mode == "Curvature" {
                    let normal_acceleration = jet.derivative_vv.z() - slope * jet.derivative_vv.x();
                    assert!(
                        (normal_acceleration - curvature * jet.derivative_v.x().powi(2)).abs()
                            < 1e-8
                    );
                }
            }
        }
        for (u, limit, height) in [(0, start, 1.), (degree_u, end, 1.4)] {
            if limit == "No" {
                let control = surface.control_point(u, 1).unwrap().point();
                assert!((control.z() - height * (std::f64::consts::PI / 5.).sin()).abs() < 1e-8);
            }
        }
        let actual = brep.area(tolerance).unwrap();
        assert!(
            (actual - area).abs() < 1e-5,
            "{a} {b} {start} {end}: {actual} != {area}"
        );
        assert_eq!(doc.undo_label(), Some("Sweep2"));
        doc.undo().unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), originals);
    }
}

#[test]
fn sweep_two_rail_edge_continuity_keeps_skew_and_handles_a_loose_opposite_rail() {
    for (guide, skew, width, mode, area) in [
        (true, false, 6, "Tangency", 26.842464696807326),
        (false, true, 6, "Tangency", 26.369272759129171),
        (false, false, 3, "Tangency", 25.393769138677843),
        (false, false, 4, "Curvature", 26.280998825323092),
    ] {
        let mut doc = Document::default();
        let rails = inputs(&mut doc, guide, skew, width, 0.);
        let tolerance = doc.tolerance();
        let brep = run(
            &mut doc,
            &format!(
                "{rails} EdgeContinuityA={mode} EdgeContinuityB={mode} StartShape=No EndShape=No"
            ),
        );
        let actual = brep.area(tolerance).unwrap();
        assert!(
            (actual - area).abs() < 1e-5,
            "guide={guide} skew={skew} count={width}: {actual} != {area}"
        );
    }
}

#[test]
fn sweep_two_rail_edge_continuity_on_varying_supports_matches_native_area() {
    let mut doc = Document::default();
    let rails = inputs(&mut doc, false, false, 6, 0.5);
    let (left, right) = rails.split_once(' ').unwrap();
    let swapped = format!(
        "{} {}",
        right.replacen("Rail2", "Rail1", 1),
        left.replacen("Rail1", "Rail2", 1)
    );
    let tolerance = doc.tolerance();
    for (axes, options, area) in [
        (
            &rails,
            "EdgeContinuityA=Tangency EdgeContinuityB=Tangency",
            26.378390539113511,
        ),
        (
            &rails,
            "EdgeContinuityA=Curvature EdgeContinuityB=Curvature StartShape=No EndShape=No",
            26.455458301178027,
        ),
        (
            &rails,
            "EdgeContinuityA=Tangency EdgeContinuityB=Curvature RefitRails=Yes",
            26.082695047623911,
        ),
        (
            &swapped,
            "EdgeContinuityA=Tangency EdgeContinuityB=Curvature StartShape=No EndShape=No",
            26.666384233881079,
        ),
    ] {
        let brep = run(&mut doc, &format!("{axes} {options}"));
        let actual = brep.area(tolerance).unwrap();
        assert_eq!((brep.faces().len(), brep.edges().len()), (1, 4));
        assert!(
            (actual - area).abs() < 1e-5,
            "{options}: {actual} != {area}"
        );
        doc.undo().unwrap();
    }
    let mut profiles = doc.selected_object_ids().collect::<Vec<_>>();
    profiles.reverse();
    doc.select_objects_direct(profiles, SelectionMode::Replace)
        .unwrap();
    let brep = run(
        &mut doc,
        &format!("{swapped} EdgeContinuityA=Tangency EdgeContinuityB=Curvature"),
    );
    let actual = brep.area(tolerance).unwrap();
    assert!((actual - 26.501626667878480).abs() < 1e-5, "{actual}");
}
