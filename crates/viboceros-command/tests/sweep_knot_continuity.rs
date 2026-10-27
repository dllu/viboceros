use viboceros_command::CommandRegistry;
use viboceros_document::{Document, Geometry, SelectionMode};
use viboceros_geometry::{LineSegment, NurbsCurve, Point3, Tolerance, WeightedPoint3};

#[test]
fn smooth_join_profiles_sweep_through_automatic_and_explicit_command_paths() {
    let request: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/sweep1_refit_intervals.json"
    ))
    .unwrap();
    let r = &request["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|op| op["id"] == "greville-nurbs-0.5-refit")
        .unwrap()["rail"];
    let source = NurbsCurve::try_new_rational(
        2,
        r["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                let p: [f64; 3] = serde_json::from_value(c["point"].clone()).unwrap();
                WeightedPoint3::try_new(Point3::try_from(p).unwrap(), c["weight"].as_f64().unwrap())
                    .unwrap()
            })
            .collect(),
        serde_json::from_value(r["knots"].clone()).unwrap(),
    )
    .unwrap();
    let registry = CommandRegistry::with_builtins();
    for reversed in [false, true] {
        let rail = if reversed {
            source.reversed().unwrap()
        } else {
            source.clone()
        };
        let t = rail.knots()[3].next_down();
        let start = rail.evaluate(t).unwrap();
        let end = Point3::try_new(start.x() + 0.5, start.y(), start.z()).unwrap();
        for explicit in [false, true] {
            let tolerance = Tolerance::try_new(1e-7, 1e-12, 1e-10).unwrap();
            let mut doc = Document::new(tolerance);
            let rail_id = doc
                .add_geometry(Geometry::NurbsCurve(rail.clone()))
                .unwrap();
            let profile_id = doc
                .add_geometry(Geometry::Line(
                    LineSegment::try_new(start, end, tolerance).unwrap(),
                ))
                .unwrap();
            let selected = if explicit {
                [profile_id, rail_id]
            } else {
                [rail_id, profile_id]
            };
            doc.select_objects_direct([selected[0]], SelectionMode::Replace)
                .unwrap();
            doc.select_objects_direct([selected[1]], SelectionMode::Add)
                .unwrap();
            assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), selected);
            let before = doc.objects().cloned().collect::<Vec<_>>();
            let command = if explicit {
                format!("Sweep1 Parameters={t:.17e} RefitRail=Yes")
            } else {
                "Sweep1 RefitRail=Yes".to_owned()
            };
            registry
                .execute(&mut doc, &command)
                .unwrap_or_else(|e| panic!("reversed={reversed} {command}: {e}"));
            let output = doc
                .objects()
                .find(|o| !before.iter().any(|s| s.id() == o.id()))
                .unwrap();
            let Geometry::Brep(brep) = output.geometry() else {
                panic!("expected one B-rep")
            };
            assert!(!brep.faces().is_empty());
            for face in brep.faces() {
                let s = face.surface();
                for i in 0..=16 {
                    for v in [0., 0.5, 1.] {
                        let p = s
                            .evaluate(
                                s.parameter_at_u(i as f64 / 16.).unwrap(),
                                s.parameter_at_v(v).unwrap(),
                            )
                            .unwrap();
                        assert!((p.x() - 7. - 0.5 * v).abs() < 1e-10);
                        assert!(((p.y() + 3.).hypot(p.z() - 11.) - 0.75).abs() < 1e-7);
                    }
                }
            }
            for original in &before {
                assert_eq!(doc.object(original.id()), Some(original));
            }
            assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), selected);
            registry.execute(&mut doc, "Undo").unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), selected);
        }
    }
    println!("4 smooth-join command constructions; analytic sample and atomic undo checks passed");
}
