#[path = "support/observation.rs"]
mod observation;
use viboceros_geometry::{CurveRef, MeshFace, NurbsCurve, Point3, SubD, Tolerance, TriangleMesh};
#[test]
fn prepared_subd_curves_match_native_edge_images_and_lengths() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let data = observation::read(&base, "component_length_discovery");
    let mut count = 0;
    for row in data["results"].as_array().unwrap() {
        let v = &row["value"];
        if v["curves"].as_array().unwrap().is_empty() {
            continue;
        }
        let input = &v["input"];
        let points = input["points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                Point3::try_new(
                    p[0].as_f64().unwrap(),
                    p[1].as_f64().unwrap(),
                    p[2].as_f64().unwrap(),
                )
                .unwrap()
            })
            .collect();
        let faces = input["faces"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| {
                let a = f
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|i| i.as_u64().unwrap() as u32)
                    .collect::<Vec<_>>();
                match a.as_slice() {
                    [a, b, c] => MeshFace::Triangle([*a, *b, *c]),
                    [a, b, c, d] => MeshFace::Quad([*a, *b, *c, *d]),
                    _ => panic!(),
                }
            })
            .collect();
        let mesh = TriangleMesh::try_from_face_records(points, faces).unwrap();
        let corners = input["corners"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|i| (i.as_u64().unwrap() as usize, 3))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let mut s = SubD::try_from_mesh(&mesh, Default::default(), Tolerance::DEFAULT)
            .unwrap()
            .try_with_vertex_tags(&corners)
            .unwrap();
        if let Some(pairs) = input["creases"].as_array() {
            let tags = s
                .edges()
                .iter()
                .enumerate()
                .filter(|(_, e)| {
                    pairs.iter().any(|p| {
                        [p[0].as_u64().unwrap() as u32, p[1].as_u64().unwrap() as u32] == **e
                            || [p[1].as_u64().unwrap() as u32, p[0].as_u64().unwrap() as u32] == **e
                    })
                })
                .map(|(i, _)| (i, 2))
                .collect::<Vec<_>>();
            s = s.try_with_edge_tags(&tags).unwrap();
        }
        let selected = v["selected"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i.as_u64().unwrap() as usize)
            .collect::<Vec<_>>();
        let curves = s.edge_nurbs_curves(&selected).unwrap();
        for (actual, native) in curves.iter().zip(v["curves"].as_array().unwrap()) {
            let d = &native["definition"];
            let controls = d["control_points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| {
                    assert_eq!(c["weight"], 1.);
                    let p = &c["point"];
                    Point3::try_new(
                        p[0].as_f64().unwrap(),
                        p[1].as_f64().unwrap(),
                        p[2].as_f64().unwrap(),
                    )
                    .unwrap()
                })
                .collect();
            let reference = NurbsCurve::try_new(
                d["degree"].as_u64().unwrap() as usize,
                controls,
                d["knots"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|k| k.as_f64().unwrap())
                    .collect(),
            )
            .unwrap();
            let length = CurveRef::NurbsCurve(actual)
                .length(Tolerance::DEFAULT)
                .unwrap();
            let expected = native["length"].as_f64().unwrap();
            let domain = actual.domain();
            let rd = reference.domain();
            let mut forward: f64 = 0.;
            let mut reverse: f64 = 0.;
            for i in 0..=64 {
                let t = i as f64 / 64.;
                let a = actual
                    .evaluate(domain.start() + (domain.end() - domain.start()) * t)
                    .unwrap();
                let b = reference
                    .evaluate(rd.start() + (rd.end() - rd.start()) * t)
                    .unwrap();
                let r = reference
                    .evaluate(rd.end() - (rd.end() - rd.start()) * t)
                    .unwrap();
                forward = forward.max(a.distance_to(b).unwrap());
                reverse = reverse.max(a.distance_to(r).unwrap());
            }
            let error = forward.min(reverse);
            eprintln!(
                "{} length delta={} station error={}",
                row["id"],
                length - expected,
                error
            );
            assert!((length - expected).abs() < 1e-6, "{} length", row["id"]);
            assert!(error < 1e-11, "{} image", row["id"]);
            count += 1;
        }
    }
    assert_eq!(count, 16);
}
