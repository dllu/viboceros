use super::*;
use serde_json::Value;

fn setup() -> (Document, ObjectId, ObjectId) {
    let mut d = Document::default();
    let frame = CommandContext::default().construction_plane;
    let b = Brep::try_box(frame, [[0., 4.], [0., 6.], [0., 2.]], d.tolerance()).unwrap();
    let target = d.add_geometry(Geometry::Brep(b)).unwrap();
    let line = d
        .add_geometry(Geometry::Line(
            LineSegment::try_new(
                Point3::try_new(0., 0., 0.).unwrap(),
                Point3::try_new(1., 1., 0.).unwrap(),
                d.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    d.clear_history().unwrap();
    d.select_command_results([line]).unwrap();
    (d, target, line)
}

#[test]
fn every_box_face_is_a_readonly_apply_reference_with_original_face_domains() {
    let registry = CommandRegistry::with_builtins();
    for face in 0..6 {
        let (mut d, target, line) = setup();
        let saved = d.object(target).unwrap().geometry().clone();
        registry
            .execute(&mut d, &format!("ApplyCrv Face={face} Surface={target}"))
            .unwrap();
        assert_eq!(d.object(target).unwrap().geometry(), &saved);
        assert_eq!(d.objects().len(), 3);
        let Geometry::Brep(b) = &saved else { panic!() };
        let s = b.faces()[face].surface();
        let c = d
            .selected_objects()
            .next()
            .unwrap()
            .geometry()
            .curve_ref()
            .unwrap()
            .to_nurbs()
            .unwrap();
        for i in 0..=32 {
            let t = i as Real / 32.;
            let p = s
                .evaluate(s.parameter_at_u(t).unwrap(), s.parameter_at_v(t).unwrap())
                .unwrap();
            assert!(
                c.parameter_sampler()
                    .unwrap()
                    .evaluate(t)
                    .unwrap()
                    .distance_to(p)
                    .unwrap()
                    < 1e-8
            );
        }
        registry.execute(&mut d, "Undo").unwrap();
        assert_eq!(d.objects().len(), 2);
        assert!(d.object(line).is_some());
        assert_eq!(d.selected_object_count(), 0);
        registry.execute(&mut d, "Redo").unwrap();
        assert_eq!(d.selected_object_count(), 1);
    }
}

#[test]
fn native_uv_face_references_match_all_six_recorded_box_charts() {
    let capture: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/uv_face_reference_command.json"
    ))
    .unwrap();
    let registry = CommandRegistry::with_builtins();
    for row in capture["results"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["value"]["success"] == true)
    {
        let mut d = Document::default();
        // Reconstruct the captured source charts: BoundingBox.ToBrep has
        // different chart axes/face ordering from the kernel box constructor.
        let definitions = row["value"]["before"][0]["surfaces"].as_array().unwrap();
        let parts = definitions
            .iter()
            .map(|s| {
                let values = |v: &Value| serde_json::from_value::<Vec<Real>>(v.clone()).unwrap();
                let controls = s["control_points"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|p| {
                        WeightedPoint3::try_new(
                            Point3::try_from(
                                serde_json::from_value::<[Real; 3]>(p["point"].clone()).unwrap(),
                            )
                            .unwrap(),
                            p["weight"].as_f64().unwrap(),
                        )
                        .unwrap()
                    })
                    .collect();
                let surface = NurbsSurface::try_new_rational(
                    s["degree"][0].as_u64().unwrap() as usize,
                    s["degree"][1].as_u64().unwrap() as usize,
                    s["control_count"][0].as_u64().unwrap() as usize,
                    s["control_count"][1].as_u64().unwrap() as usize,
                    controls,
                    values(&s["knots_u"]),
                    values(&s["knots_v"]),
                )
                .unwrap();
                Brep::try_surface_face(surface, d.tolerance()).unwrap()
            })
            .collect::<Vec<_>>();
        let recorded = Brep::try_disjoint_union(parts, d.tolerance()).unwrap();
        let target = d.add_geometry(Geometry::Brep(recorded)).unwrap();
        let saved = d.object(target).unwrap().geometry().clone();
        let face = row["value"]["face"].as_u64().unwrap() as usize;
        let command = row["value"]["command"].as_str().unwrap();
        if command == "ApplyCrv" {
            let line = d
                .add_geometry(Geometry::Line(
                    LineSegment::try_new(
                        Point3::try_new(0., 0., 0.).unwrap(),
                        Point3::try_new(1., 1., 0.).unwrap(),
                        d.tolerance(),
                    )
                    .unwrap(),
                ))
                .unwrap();
            d.select_command_results([line]).unwrap();
        }
        registry
            .execute(&mut d, &format!("{command} Surface={target} Face={face}"))
            .unwrap();
        assert_eq!(d.object(target).unwrap().geometry(), &saved);
        let c = d
            .selected_objects()
            .next()
            .unwrap()
            .geometry()
            .curve_ref()
            .unwrap()
            .to_nurbs()
            .unwrap();
        let expected = row["value"]["after"]
            .as_array()
            .unwrap()
            .iter()
            .find(|o| o["source"].is_null())
            .unwrap();
        for p in expected["samples"].as_array().unwrap() {
            let p =
                Point3::try_from(serde_json::from_value::<[Real; 3]>(p.clone()).unwrap()).unwrap();
            let t = c.closest_parameter(p, d.tolerance()).unwrap();
            assert!(c.evaluate(t).unwrap().distance_to(p).unwrap() < 1e-8);
        }
    }
}

#[test]
fn missing_ambiguous_invalid_and_restricted_face_references_never_edit() {
    let registry = CommandRegistry::with_builtins();
    let (mut d, target, line) = setup();
    let before = format!("{d:?}");
    for command in ["ApplyCrv", "CreateUVCrv"] {
        for args in [
            format!("Surface={target}"),
            format!("Surface={target} Face=6"),
            format!("Surface={target} Face=1 Face=2"),
            format!("Surface={target} Face=bad"),
            format!("Surface={line} Face=0"),
        ] {
            assert!(
                registry
                    .execute(&mut d, &format!("{command} {args}"))
                    .is_err()
            );
            assert_eq!(format!("{d:?}"), before);
        }
    }
}
