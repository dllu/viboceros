use super::automatic_tests::{add_line, native_surface};
use super::curved_tests::rail;
use super::*;
use serde_json::Value;

// Preserve every captured face, including both one-sided limits at joins.
// Matching V weight rows make each patch affine in normalized V. A single
// monotone U correspondence for both boundary atlases bounds the whole surface.
fn boundary(atlas: &[&NurbsSurface], v: usize) -> NurbsCurve {
    let mut atlas = atlas.to_vec();
    atlas.sort_by(|a, b| a.domain_u().start().total_cmp(b.domain_u().start()));
    let degree = atlas[0].degree_u();
    let mut controls = vec![];
    let mut knots = vec![];
    let mut end = None;
    for s in atlas {
        assert_eq!(s.degree_u(), degree);
        assert_eq!(s.degree_v(), 1);
        assert_eq!(s.control_point_count_v(), 2);
        assert!(
            s.knots_u()[..degree + 1]
                .iter()
                .all(|t| *t == *s.domain_u().start())
        );
        assert!(
            s.knots_u()[s.knots_u().len() - degree - 1..]
                .iter()
                .all(|t| *t == *s.domain_u().end())
        );
        assert!(
            (0..s.control_point_count_u()).all(|u| s.control_point(u, 0).unwrap().weight()
                == s.control_point(u, 1).unwrap().weight())
        );
        if let Some(end) = end {
            assert_eq!(end, *s.domain_u().start());
            knots.extend_from_slice(&s.knots_u()[degree + 1..]);
        } else {
            knots.extend_from_slice(s.knots_u());
        }
        end = Some(*s.domain_u().end());
        controls.extend((0..s.control_point_count_u()).map(|u| s.control_point(u, v).unwrap()));
    }
    NurbsCurve::try_new_rational(degree, controls, knots).unwrap()
}

fn ruled_bound(a: &[&NurbsSurface], b: &[&NurbsSurface], limit: Real) -> Option<Real> {
    let curves = [0, 1].map(|v| [boundary(a, v), boundary(b, v)]);
    for segments in [1, 64, 128, 256] {
        let mut correspondence = vec![[0., 0.]];
        for i in 1..segments {
            let f = i as Real / segments as Real;
            let target = curves[0][1]
                .evaluate(curves[0][1].parameter_at(f).unwrap())
                .unwrap();
            // Only propose a nearby station. Complete exact bounds below
            // validate the shared correspondence; no local-search result is
            // treated as proof of closest-point membership or equality.
            let mut t = curves[0][0].parameter_at(f).unwrap();
            for _ in 0..8 {
                let (p, d) = curves[0][0].evaluate_with_derivative(t).unwrap();
                let step = p.vector_to(target).unwrap().dot(d).unwrap() / d.dot(d).unwrap();
                let domain = curves[0][0].domain();
                let next = (t + step).clamp(*domain.start(), *domain.end());
                if next == t {
                    break;
                }
                t = next;
            }
            let domain = curves[0][0].domain();
            correspondence.push([(t - domain.start()) / (domain.end() - domain.start()), f]);
        }
        correspondence.push([1., 1.]);
        assert!(correspondence.windows(2).all(|p| p[0][0] < p[1][0]));
        let mut maximum = 0_f64;
        let mut certified = true;
        for pair in &curves {
            let bound = match pair[0].mapped_curve_deviation_bound(&pair[1], &correspondence, limit)
            {
                Ok(bound) => bound,
                Err(GeometryError::CurveApproximationLimit) => None,
                Err(e) => panic!("invalid correspondence: {e}"),
            };
            if let Some(bound) = bound {
                maximum = maximum.max(bound);
            } else {
                certified = false;
                break;
            }
        }
        if certified {
            println!("correspondence intervals {segments}");
            return Some(maximum);
        }
    }
    None
}

#[test]
fn automatic_arc_stations_match_complete_native_ruled_surfaces() {
    let request: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/sweep1_arc_stations.json"
    ))
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/sweep1_arc_stations.json"
    ))
    .unwrap();
    run_cases(request, observed, 36);
}

#[test]
fn original_thirty_degree_arc_matches_the_complete_native_surface() {
    let mut request: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/sweep1_automatic_stations.json"
    ))
    .unwrap();
    let mut observed: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/sweep1_automatic_stations.json"
    ))
    .unwrap();
    request["operations"]
        .as_array_mut()
        .unwrap()
        .retain(|r| r["id"] == "arc-thirty");
    observed["results"]
        .as_array_mut()
        .unwrap()
        .retain(|r| r["id"] == "arc-thirty");
    run_cases(request, observed, 1);
}

fn run_cases(request: Value, observed: Value, expected_count: usize) {
    let mut gaps = vec![];
    let mut checked = 0;
    let mut maximum = 0_f64;
    let mut proofs: Vec<(Vec<NurbsSurface>, Value, Real)> = vec![];
    let mut reused = 0;
    for op in request["operations"].as_array().unwrap() {
        let tolerance = Tolerance::try_new(1e-7, 1e-12, 1e-10).unwrap();
        let mut doc = Document::new(tolerance);
        let registry = CommandRegistry::with_builtins();
        let rail = rail(&mut doc, &op["rail"]);
        let profile = add_line(&mut doc, &op["sections"][0]);
        doc.select_objects_direct([rail, profile], SelectionMode::Replace)
            .unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        let command = if op["refit_rail"] == true {
            "Sweep1 RefitRail=Yes"
        } else {
            "Sweep1"
        };
        if let Err(e) = registry.execute(&mut doc, command) {
            gaps.push(format!("{} construction: {e}", op["id"]));
            continue;
        }
        let actual = doc
            .objects()
            .find_map(|o| {
                if let Geometry::Brep(b) = o.geometry() {
                    Some(b.faces().iter().map(|f| f.surface()).collect::<Vec<_>>())
                } else {
                    None
                }
            })
            .unwrap();
        let row = observed["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == op["id"])
            .unwrap();
        let expected = row["value"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| native_surface(&v["definition"]))
            .collect::<Vec<_>>();
        let expected = expected.iter().collect::<Vec<_>>();
        let geometry = actual.iter().map(|s| (*s).clone()).collect::<Vec<_>>();
        let bound = if let Some((_, _, bound)) = proofs
            .iter()
            .find(|(a, b, _)| a == &geometry && b == &row["value"])
        {
            reused += 1;
            Some(*bound)
        } else {
            let bound = ruled_bound(&actual, &expected, 1e-6);
            if let Some(bound) = bound {
                proofs.push((geometry, row["value"].clone(), bound));
            }
            bound
        };
        println!("{} continuous surface bound {bound:?}", op["id"]);
        if let Some(bound) = bound {
            maximum = maximum.max(bound);
        } else {
            gaps.push(op["id"].as_str().unwrap().to_owned());
        }
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        checked += 1;
    }
    println!("{checked} arc station surfaces, maximum complete bound {maximum:.17e}");
    println!("Reused {reused} proofs for exactly identical actual/captured atlases");
    assert_eq!(checked, expected_count);
    assert!(gaps.is_empty(), "arc station gaps: {gaps:?}");
}
