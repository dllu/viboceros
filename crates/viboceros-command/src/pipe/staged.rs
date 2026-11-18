use super::*;
use serde_json::Value;
use viboceros_geometry::Point3;

#[test]
fn staged_pipe_matches_rhino_wall_sections_and_volumes() {
    // Rhino 8.32 captures from a vertical length-12 rail. Wall samples
    // evaluate the native NURBS seam at matching model-space heights.
    let cases: Vec<Value> = serde_json::from_str(include_str!("staged/rhino_pipe.json")).unwrap();
    for row in cases {
        let case = row["case"].as_str().unwrap();
        let mut doc = Document::default();
        let tol = doc.tolerance();
        let line = LineSegment::try_new(point([0., 0., 0.]), point([0., 0., 12.]), tol).unwrap();
        let id = doc.add_geometry(Geometry::Line(line)).unwrap();
        let rail = Rail::capture(&doc, id, None).unwrap();
        let mut input = Input::new(Preferences::default());
        input.start_sizes(false);
        let mut complete = false;
        for token in row["inputs"].as_array().unwrap() {
            assert!(!complete, "{case}: finished early");
            let result = if let Some(text) = token.as_str() {
                if let Some(result) = input.option(text).unwrap() {
                    Ok(result)
                } else if text == "Enter" {
                    input.enter()
                } else {
                    input.number(text.parse().unwrap())
                }
            } else {
                input.point(
                    &rail,
                    point(serde_json::from_value(token.clone()).unwrap()),
                    tol,
                )
            };
            complete = result.unwrap_or_else(|error| panic!("{case} input {token}: {error}"));
        }
        assert!(complete, "{case}");
        let objects = input
            .geometries(&[rail], true, tol)
            .unwrap_or_else(|error| panic!("{case}: {error}"));
        let expected = row["outputs"].as_array().unwrap();
        assert_eq!(objects.len(), expected.len(), "{case}");
        for (object, expected) in objects.into_iter().zip(expected) {
            let Geometry::Brep(brep) = object else {
                panic!("{case}: expected a B-rep");
            };
            assert_eq!(
                brep.is_solid(),
                expected["solid"].as_bool().unwrap(),
                "{case}"
            );
            assert_eq!(
                brep.is_extrusion(),
                expected["extrusion"].as_bool().unwrap(),
                "{case}"
            );
            assert_eq!(
                brep.faces().len(),
                expected["faces"].as_u64().unwrap() as usize,
                "{case}"
            );
            let walls: Vec<_> = brep
                .faces()
                .iter()
                .filter_map(|f| axial_wall(f.surface()))
                .collect();
            let expected_walls = expected["walls"].as_array().unwrap();
            assert_eq!(walls.len(), expected_walls.len(), "{case}");
            for ((evaluate, bounds), expected) in walls.into_iter().zip(expected_walls) {
                for point in expected.as_array().unwrap() {
                    let [z, radius]: [f64; 2] = serde_json::from_value(point.clone()).unwrap();
                    let [mut a, mut b] = bounds;
                    for _ in 0..50 {
                        let mid = a.midpoint(b);
                        if evaluate(mid).z() < z {
                            a = mid;
                        } else {
                            b = mid;
                        }
                    }
                    let value = evaluate(a.midpoint(b));
                    let actual = value.x().hypot(value.y());
                    assert!(
                        (actual - radius).abs() < 1e-6,
                        "{case} z={z}: {actual} != {radius}"
                    );
                }
            }
            if let Some(volume) = expected["volume"].as_f64() {
                let measured = brep.signed_volume(tol).unwrap();
                assert!(
                    (measured - volume).abs() < 5e-6,
                    "{case}: {measured} != {volume}"
                );
            }
        }
    }
}

fn point(xyz: [f64; 3]) -> Point3 {
    Point3::try_from(xyz).unwrap()
}

fn axial_wall(surface: &NurbsSurface) -> Option<(impl Fn(f64) -> Point3 + '_, [f64; 2])> {
    let du = surface.domain_u();
    let dv = surface.domain_v();
    let [u0, u1, v0, v1] = [*du.start(), *du.end(), *dv.start(), *dv.end()];
    let start = surface.evaluate(u0, v0).ok()?.z();
    let mut end = surface.evaluate(u1, v0).ok()?.z();
    let along_u = (start - end).abs() > 10.;
    let mut bounds = if along_u {
        [u0, u1]
    } else {
        end = surface.evaluate(u0, v1).ok()?.z();
        if (start - end).abs() < 10. {
            return None;
        }
        [v0, v1]
    };
    if start > end {
        bounds.swap(0, 1);
    }
    Some((
        move |t| {
            if along_u {
                surface.evaluate(t, v0)
            } else {
                surface.evaluate(u0, t)
            }
            .unwrap()
        },
        bounds,
    ))
}
