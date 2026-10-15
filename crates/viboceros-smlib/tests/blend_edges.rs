#![cfg(feature = "native")]
use viboceros_geometry::{Brep, Frame3, Point3, Tolerance, Vector3};
use viboceros_smlib::{EdgeBlendRailType, Solid};
fn source() -> Brep {
    let t = Tolerance::DEFAULT;
    let f = Frame3::try_from_directions(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 1., 0.).unwrap(),
        t,
    )
    .unwrap();
    Brep::try_box(f, [[0., 10.], [0., 8.], [0., 6.]], t).unwrap()
}
// A unit-setback quintic with zero second endpoint derivatives has controls
// (1,0),(1-D/5,0),(1-2D/5,0),(0,1-2D/5),(0,1-D/5),(0,1).
// Equal-spacing tangent construction gives D=3*(2-sqrt(2)). Integrating x dy
// gives (13D²-110D+210)/420 = 3/5-23sqrt(2)/70, independently of mass code.
fn missing_area() -> f64 {
    3. / 5. - 23. * 2_f64.sqrt() / 70.
}
fn rail_jets(result: &Brep, include_curved_parents: bool) -> usize {
    let t = Tolerance::DEFAULT;
    let mut checked = 0;
    for f in result.faces() {
        if f.surface().degree_u() != 5 && f.surface().degree_v() != 5 {
            continue;
        }
        for trim in f.loops().iter().flat_map(|l| l.trims()) {
            let Some(edge) = trim.edge() else { continue };
            if !include_curved_parents && !result.edges()[edge].curve().is_linear(t).unwrap() {
                continue;
            }
            let Some(parent) = result.faces().iter().find(|other| {
                !std::ptr::eq(*other, f)
                    && other
                        .loops()
                        .iter()
                        .flat_map(|l| l.trims())
                        .any(|t| t.edge() == Some(edge))
                    && (include_curved_parents || other.surface().plane(t).unwrap().is_some())
            }) else {
                continue;
            };
            for frac in [0.2, 0.5, 0.8] {
                let d = trim.curve().domain();
                let uv = trim
                    .curve()
                    .evaluate(d.start() + (d.end() - d.start()) * frac)
                    .unwrap();
                let a = f.surface().curvature_at(uv.x(), uv.y()).unwrap();
                let (u, v) = parent.surface().closest_parameters(a.point, t).unwrap();
                let b = parent.surface().curvature_at(u, v).unwrap();
                assert!(a.point.distance_to(b.point).unwrap() < 1e-7);
                assert!(
                    a.normal
                        .as_vector()
                        .dot(b.normal.as_vector())
                        .unwrap()
                        .abs()
                        > 1. - 1e-8,
                    "normals differ at {:?}",
                    a.point
                );
                let orientation = a
                    .normal
                    .as_vector()
                    .dot(b.normal.as_vector())
                    .unwrap()
                    .signum();
                assert!(
                    a.principal
                        .iter()
                        .zip(b.principal)
                        .all(|(x, y)| (x - orientation * y).abs() < 1e-6),
                    "signed curvature mismatch at {:?}: {:?} vs {:?}",
                    a.point,
                    a.principal,
                    b.principal
                );
                if (b.principal[0] - b.principal[1]).abs() > 1e-6 {
                    assert!(
                        a.directions[0]
                            .as_vector()
                            .dot(b.directions[0].as_vector())
                            .unwrap()
                            .abs()
                            > 1. - 1e-6
                    );
                }
                checked += 1;
            }
        }
    }
    checked
}
#[test]
fn every_original_box_edge_has_quintic_volume_and_g2_planar_rails() {
    let t = Tolerance::DEFAULT;
    let b = source();
    let original = b.clone();
    for rail in [
        EdgeBlendRailType::RollingBall,
        EdgeBlendRailType::DistanceBetweenRails,
    ] {
        let setback = if rail == EdgeBlendRailType::RollingBall {
            1.
        } else {
            1. / 2_f64.sqrt()
        };
        for i in 0..b.edges().len() {
            println!("original box edge {i}, {rail:?}");
            let [a, c] = b.edges()[i].vertices().map(|v| b.vertices()[v].point());
            let r = Solid::blend_brep_edges(&b, &[i, i], 1., rail, t)
                .unwrap()
                .to_brep(t)
                .unwrap();
            let want = 480. - a.distance_to(c).unwrap() * setback * setback * missing_area();
            assert!(r.is_solid());
            assert!(
                (r.signed_volume(t).unwrap() - want).abs() < 1e-6,
                "edge={i}, {rail:?}"
            );
            assert_eq!(rail_jets(&r, false), 6);
            assert_eq!(b, original);
        }
    }
}
#[test]
fn all_edges_and_invalid_sizes_preserve_source() {
    let t = Tolerance::DEFAULT;
    let b = source();
    let original = b.clone();
    let all = (0..b.edges().len()).collect::<Vec<_>>();
    let r = Solid::blend_brep_edges(&b, &all, 1., EdgeBlendRailType::RollingBall, t)
        .unwrap()
        .to_brep(t)
        .unwrap();
    println!("all edges: {} faces", r.faces().len());
    assert!(r.is_solid());
    assert!(rail_jets(&r, false) >= 12 * 6);
    for (edges, distance) in [
        (vec![], 1.),
        (vec![999], 1.),
        (vec![0], 0.),
        (vec![0], f64::NAN),
        (all, 100.),
    ] {
        assert!(
            Solid::blend_brep_edges(&b, &edges, distance, EdgeBlendRailType::RollingBall, t)
                .and_then(|s| s.to_brep(t))
                .is_err()
        );
        assert_eq!(b, original);
    }
}
#[test]
fn cylinder_rim_matches_parent_curvature_on_both_rails() {
    let t = Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap();
    let origin = Point3::try_new(0., 0., 0.).unwrap();
    let z = Vector3::try_new(0., 0., 1.).unwrap();
    let frames = [
        Frame3::try_from_normal(origin, z, t).unwrap(),
        Frame3::try_from_directions(
            origin,
            Vector3::try_new(1., 0., 0.).unwrap(),
            Vector3::try_new(0., 1., 0.).unwrap(),
            t,
        )
        .unwrap(),
    ];
    for f in frames {
        let b = Brep::try_cylinder(f, 5., 0., 8., t).unwrap();
        let original = b.clone();
        let shared = b.edges_shared_by_distinct_faces();
        let e = b
            .edges()
            .iter()
            .enumerate()
            .find(|(i, e)| {
                let d = e.curve().domain();
                shared[*i] && e.curve().evaluate((d.start() + d.end()) / 2.).unwrap().z() > 7.999
            })
            .unwrap()
            .0;
        for rail in [
            EdgeBlendRailType::RollingBall,
            EdgeBlendRailType::DistanceBetweenRails,
        ] {
            let r = Solid::blend_brep_edges(&b, &[e], 1., rail, t)
                .unwrap()
                .to_brep(t)
                .unwrap();
            let s = if rail == EdgeBlendRailType::RollingBall {
                1.
            } else {
                1. / 2_f64.sqrt()
            };
            // First inward moment of the independent planar quintic region:
            // (1/2) integral x² dy, with D=3*(2-sqrt(2)).
            let moment = -10121. / 30030. + 731. * 2_f64.sqrt() / 2860.;
            let expected = 200. * std::f64::consts::PI
                - 2. * std::f64::consts::PI * (5. * s * s * missing_area() - s * s * s * moment);
            assert!((r.signed_volume(t).unwrap() - expected).abs() < 1e-5);
            assert!(r.is_solid());
            assert_eq!(rail_jets(&r, true), 6);
            assert_eq!(b, original);
        }
    }
}
#[test]
fn concave_notch_adds_the_independent_quintic_wedge() {
    use viboceros_smlib::BooleanOperation;
    let t = Tolerance::DEFAULT;
    let p = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let frame = Frame3::try_from_directions(
        p(0., 0., 0.),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 1., 0.).unwrap(),
        t,
    )
    .unwrap();
    let cube = Brep::try_box(frame, [[0., 10.]; 3], t).unwrap();
    let tool = Brep::try_box(frame, [[5., 11.], [5., 11.], [-1., 11.]], t).unwrap();
    let outer = Solid::from_brep(&cube, t).unwrap();
    let cutter = Solid::from_brep(&tool, t).unwrap();
    let b = outer
        .boolean(&cutter, BooleanOperation::Difference)
        .unwrap()
        .to_brep(t)
        .unwrap();
    let original = b.clone();
    let edge = b
        .edges()
        .iter()
        .position(|e| {
            e.vertices().iter().all(|&v| {
                let p = b.vertices()[v].point();
                (p.x() - 5.).abs() < 1e-8 && (p.y() - 5.).abs() < 1e-8
            })
        })
        .unwrap();
    for (rail, factor) in [
        (EdgeBlendRailType::RollingBall, 10.),
        (EdgeBlendRailType::DistanceBetweenRails, 5.),
    ] {
        let r = Solid::blend_brep_edges(&b, &[edge], 1., rail, t)
            .unwrap()
            .to_brep(t)
            .unwrap();
        assert!(r.is_solid());
        assert!((r.signed_volume(t).unwrap() - (750. + factor * missing_area())).abs() < 1e-5);
        assert_eq!(rail_jets(&r, false), 6);
        assert_eq!(b, original);
    }
}

#[test]
fn native_primitive_notch_respects_declared_topology_accuracy() {
    use viboceros_smlib::BooleanOperation;
    let t = Tolerance::DEFAULT;
    let p = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let outer = Solid::box_solid(p(0., 0., 0.), [10., 10., 10.]).unwrap();
    let cutter = Solid::box_solid(p(5., 5., -1.), [6., 6., 12.]).unwrap();
    let source = outer
        .boolean(&cutter, BooleanOperation::Difference)
        .unwrap()
        .to_brep(t)
        .unwrap();
    let original = source.clone();
    let edge = source
        .edges()
        .iter()
        .position(|e| {
            e.vertices().iter().all(|&v| {
                let p = source.vertices()[v].point();
                (p.x() - 5.).abs() < 1e-8 && (p.y() - 5.).abs() < 1e-8
            })
        })
        .unwrap();
    let declared = source
        .edges()
        .iter()
        .map(|e| e.tolerance())
        .chain(source.vertices().iter().map(|v| v.tolerance()))
        .fold(t.absolute(), f64::max);
    // The native executive fits at four times the declared edge zone. Scale
    // the comparison by the ten-cube's surface area; this is a test tolerance,
    // not a continuous Hausdorff or volume-error certificate.
    let volume_tolerance = 600. * 4. * declared;
    for (rail, factor) in [
        (EdgeBlendRailType::RollingBall, 10.),
        (EdgeBlendRailType::DistanceBetweenRails, 5.),
    ] {
        let result = Solid::blend_brep_edges(&source, &[edge], 1., rail, t)
            .unwrap()
            .to_brep(t)
            .unwrap();
        let error = (result.signed_volume(t).unwrap() - (750. + factor * missing_area())).abs();
        println!(
            "coarse native zone={declared}, {rail:?}: ideal volume error={error}, threshold={volume_tolerance}"
        );
        assert!(result.is_solid());
        assert!(error < volume_tolerance);
        assert_eq!(rail_jets(&result, false), 6);
        assert_eq!(source, original);
    }
}
