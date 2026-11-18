use super::*;
use viboceros_document::SelectionMode;
use viboceros_geometry::Point3;

#[test]
fn torus_matches_native_control_nets_and_parameter_domains() {
    let captures: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("rhino_surfaces.json")).unwrap();
    for capture in captures {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        if let Some(setup) = capture["setup"].as_str() {
            registry.execute(&mut document, setup).unwrap();
        }
        let mut command = capture["command"].as_str().unwrap().to_owned();
        if command.contains("{guide}") {
            registry
                .execute(&mut document, "Line 0,0,0 0,10,10")
                .unwrap();
            let guide = document.objects().last().unwrap().id();
            command = command.replace("{guide}", &guide.to_string());
        }
        if command.contains("{circle}") {
            registry.execute(&mut document, "Circle 0,0,0 2").unwrap();
            let circle = document.objects().last().unwrap().id();
            registry
                .execute(&mut document, "Line -10,8,0 14,8,0")
                .unwrap();
            let line = document.objects().last().unwrap().id();
            command = command
                .replace("{circle}", &circle.to_string())
                .replace("{line}", &line.to_string());
        }
        if let Some(points) = capture["points"].as_array() {
            let mut ids = Vec::new();
            for point in points {
                let coords: [f64; 3] = serde_json::from_value(point.clone()).unwrap();
                ids.push(
                    document
                        .add_geometry(Geometry::Point(Point3::try_from(coords).unwrap()))
                        .unwrap(),
                );
            }
            document
                .select_objects_direct(ids, SelectionMode::Replace)
                .unwrap();
        }
        let before = document.objects().len();
        registry
            .execute(&mut document, &command)
            .unwrap_or_else(|e| panic!("{command}: {e}"));
        let Geometry::NurbsSurface(surface) = document.objects().last().unwrap().geometry() else {
            panic!("{command}: expected a NURBS surface");
        };
        assert!(
            surface.is_closed_u().unwrap() && surface.is_closed_v().unwrap(),
            "{command}"
        );
        let native = &capture["surface"];
        let degrees: [usize; 2] = serde_json::from_value(native["degree"].clone()).unwrap();
        let counts: [usize; 2] = serde_json::from_value(native["control_count"].clone()).unwrap();
        assert_eq!(
            [surface.degree_u(), surface.degree_v()],
            degrees,
            "{command}"
        );
        assert_eq!(
            [
                surface.control_point_count_u(),
                surface.control_point_count_v()
            ],
            counts,
            "{command}"
        );
        for (key, actual) in [
            ("knots_u", surface.knots_u()),
            ("knots_v", surface.knots_v()),
        ] {
            let expected: Vec<f64> = serde_json::from_value(native[key].clone()).unwrap();
            assert_eq!(actual.len(), expected.len(), "{command} {key}");
            for (&actual, expected) in actual.iter().zip(expected) {
                assert!(
                    (actual - expected).abs() < 1e-10,
                    "{command} {key}: {actual} != {expected}"
                );
            }
        }
        let controls = native["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|control| {
                let point: [f64; 3] = serde_json::from_value(control["point"].clone()).unwrap();
                (
                    Point3::try_from(point).unwrap(),
                    control["weight"].as_f64().unwrap(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(controls.len(), counts[0] * counts[1]);
        if capture["points"].is_array() {
            assert!(
                same_fitted_torus(surface, &controls),
                "{command}: fitted surface differs"
            );
        } else {
            for (i, (point, weight)) in controls.into_iter().enumerate() {
                let uv = (i % counts[0], i / counts[0]);
                let control = surface.control_point(uv.0, uv.1).unwrap();
                assert!(
                    control.point().distance_to(point).unwrap() < 1e-9,
                    "{command} CV {uv:?}: {:?} != {point:?}",
                    control.point(),
                );
                assert!(
                    (control.weight() - weight).abs() < 1e-12,
                    "{command} CV {uv:?} weight"
                );
            }
        }
        assert_eq!(document.undo_label(), Some("Torus"));
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().len(), before);
        registry.execute(&mut document, "Redo").unwrap();
        assert_eq!(document.objects().len(), before + 1);
    }
}

// Fitted plane basis signs can shift the seam or reverse both parameters.
// Compare all weighted controls after allowing those changes of parameter.
fn same_fitted_torus(surface: &viboceros_geometry::NurbsSurface, native: &[(Point3, f64)]) -> bool {
    [0, 2, 4, 6].into_iter().any(|shift| {
        [false, true].into_iter().any(|reverse| {
            native.iter().enumerate().all(|(i, &(point, weight))| {
                let (u, v) = (i % 9, i / 9);
                let (u_mapped, v_mapped) = if reverse {
                    (8 + shift - u, 8 - v)
                } else {
                    (u + shift, v)
                };
                let actual = surface.control_point(u_mapped % 8, v_mapped % 8).unwrap();
                actual.point().distance_to(point).unwrap() < 1e-9
                    && (actual.weight() - weight).abs() < 1e-12
            })
        })
    })
}
