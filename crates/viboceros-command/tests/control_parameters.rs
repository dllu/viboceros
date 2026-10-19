//! Model queries can agree geometrically while exact getter decisions differ.
use serde_json::Value;
use viboceros_command::open_3dm_with_views;
use viboceros_document::Geometry;
use viboceros_geometry::Point3;

#[test]
fn native_surface_and_isocurve_parameters_remain_within_geometry_epsilon() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let capture: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("observations/remove_control_point_parameters.json"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(capture["results"].as_array().unwrap().len(), 10);
    for row in capture["results"].as_array().unwrap() {
        let name = row["id"].as_str().unwrap();
        let v = &row["value"];
        let (doc, _, _, _) = open_3dm_with_views(
            root.join(format!(
                "fixtures/remove_control_point_parameters/{name}.3dm"
            ))
            .to_str()
            .unwrap(),
        )
        .unwrap();
        let Geometry::Brep(b) = doc.objects().next().unwrap().geometry() else {
            panic!()
        };
        let s = b.faces()[0].surface();
        let point = |v: &Value| {
            Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
        };
        let p = point(&v["picks"][0]);
        let (u, w) = s.closest_parameters(p, doc.tolerance()).unwrap();
        let q = &v["parameter_queries"];
        let mut gap = (u - q["surface_closest"]["parameters"][0].as_f64().unwrap())
            .abs()
            .max((w - q["surface_closest"]["parameters"][1].as_f64().unwrap()).abs());
        gap = gap.max(
            s.evaluate(u, w)
                .unwrap()
                .distance_to(point(&q["projected_point"]))
                .unwrap(),
        );
        let axis = usize::from(v["recipe"]["direction"] == "U");
        let iso = if axis == 1 {
            s.isocurve_v(u)
        } else {
            s.isocurve_u(w)
        }
        .unwrap();
        let t = iso.closest_parameter(p, doc.tolerance()).unwrap();
        let native = q["curves"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["kind"] == "iso" && c["axis"] == axis)
            .unwrap();
        gap = gap.max((t - native["parameters"][0].as_f64().unwrap()).abs());
        assert!(gap <= 1e-8, "{name}: query error {gap}");
        eprintln!(
            "CONTROL_PARAMETER_QUERY_AUDIT {}",
            serde_json::json!({"id":name,"maximum_error":gap})
        );
    }
}
