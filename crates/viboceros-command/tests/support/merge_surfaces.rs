//! Authored merge operands and independent captured NURBS expectations.
use serde_json::Value;
use std::{fs::File, path::Path};
use viboceros_document::Geometry;
use viboceros_geometry::*;
pub const EPSILON: f64 = 1e-9;

pub fn read(family: &str) -> Value {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|p| p.join("tools/rhino_oracle/observations").is_dir())
        .unwrap();
    let p = root.join(format!("tools/rhino_oracle/observations/{family}.json.gz"));
    if p.exists() {
        serde_json::from_reader(flate2::read::GzDecoder::new(File::open(p).unwrap())).unwrap()
    } else {
        serde_json::from_reader(File::open(p.with_extension("")).unwrap()).unwrap()
    }
}

pub fn sdk_rows() -> Vec<Value> {
    [
        "merge_surfaces_pilot",
        "merge_surfaces_holdouts",
        "merge_surfaces_final",
        "merge_surface_orientation",
    ]
    .into_iter()
    .flat_map(|f| read(f)["results"].as_array().unwrap().clone())
    .collect()
}

fn numbers(v: &Value) -> Vec<f64> {
    serde_json::from_value(v.clone()).unwrap()
}
pub fn point(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
}
fn controls(v: &Value) -> Vec<WeightedPoint3> {
    v["control_points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            WeightedPoint3::try_new(point(&c["point"]), c["weight"].as_f64().unwrap()).unwrap()
        })
        .collect()
}
pub fn surface(v: &Value) -> NurbsSurface {
    let native = v.get("degree").is_some();
    let degree = if native {
        numbers(&v["degree"])
    } else {
        vec![
            v["degree_u"].as_f64().unwrap(),
            v["degree_v"].as_f64().unwrap(),
        ]
    };
    let count = if native {
        numbers(&v["control_count"])
    } else {
        vec![
            v["control_point_count_u"].as_f64().unwrap(),
            v["control_point_count_v"].as_f64().unwrap(),
        ]
    };
    let s = NurbsSurface::try_new_rational(
        degree[0] as usize,
        degree[1] as usize,
        count[0] as usize,
        count[1] as usize,
        controls(v),
        numbers(&v["knots_u"]),
        numbers(&v["knots_v"]),
    )
    .unwrap();
    if !native && v.get("domain_u").is_some() {
        let u = numbers(&v["domain_u"]);
        let w = numbers(&v["domain_v"]);
        s.try_reparameterized(u[0]..=u[1], w[0]..=w[1]).unwrap()
    } else {
        s
    }
}
pub fn input(v: &Value, model: Tolerance) -> Geometry {
    if v["type"] == "untrimmed_brep" {
        let mut b = Brep::try_surface_face(surface(&v["surface"]), model).unwrap();
        if v["reversed"] == true {
            b.reverse_orientation();
        }
        Geometry::Brep(b)
    } else {
        Geometry::NurbsSurface(surface(v))
    }
}
pub fn options(v: &Value) -> viboceros_command::merge_surfaces::Options {
    viboceros_command::merge_surfaces::Options {
        smooth: v["smooth"].as_bool().unwrap(),
        roundness: v["roundness"].as_f64().unwrap(),
        tolerance: Some(v["merge_tolerance"].as_f64().unwrap()),
    }
}
pub fn model_tolerance() -> Tolerance {
    Tolerance::try_new(1e-6, 1e-9, 1e-6).unwrap()
}

pub fn compare(actual: &Brep, native: &Value) -> f64 {
    assert_eq!(
        actual.faces().len(),
        native["surfaces"].as_array().unwrap().len()
    );
    assert_eq!(
        actual.edges().len(),
        native["edges"].as_array().unwrap().len()
    );
    assert_eq!(
        actual.vertices().len(),
        native["vertices"].as_array().unwrap().len()
    );
    let mut maximum = 0_f64;
    for (index, face) in actual.faces().iter().enumerate() {
        assert_eq!(
            face.is_reversed(),
            native["face_reversed"][index].as_bool().unwrap()
        );
        let s = face.surface();
        let target = surface(&native["surfaces"][index]);
        assert_eq!(
            (s.degree_u(), s.degree_v()),
            (target.degree_u(), target.degree_v())
        );
        assert_eq!(
            (s.control_point_count_u(), s.control_point_count_v()),
            (
                target.control_point_count_u(),
                target.control_point_count_v()
            )
        );
        for (a, b) in [
            (s.knots_u(), target.knots_u()),
            (s.knots_v(), target.knots_v()),
        ] {
            assert_eq!(a.len(), b.len());
            for (&a, &b) in a.iter().zip(b) {
                assert!((a - b).abs() <= 1e-12);
            }
        }
        for i in 0..=16 {
            for j in 0..=16 {
                let u = target.parameter_at_u(i as f64 / 16.).unwrap();
                let v = target.parameter_at_v(j as f64 / 16.).unwrap();
                maximum = maximum.max(
                    s.evaluate(u, v)
                        .unwrap()
                        .distance_to(target.evaluate(u, v).unwrap())
                        .unwrap(),
                );
            }
        }
    }
    let curves = native["edges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            assert_eq!(e["curve"]["type"], "nurbs");
            let c = &e["curve"]["definition"];
            NurbsCurve::try_new_rational(
                c["degree"].as_u64().unwrap() as usize,
                controls(c),
                numbers(&c["knots"]),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let mut used = vec![false; curves.len()];
    let mut edge_map = Vec::new();
    for edge in actual.edges() {
        let mut matched = None;
        for (i, curve) in curves.iter().enumerate().filter(|(i, _)| !used[*i]) {
            for reverse in [false, true] {
                let target = if reverse {
                    curve.reversed().unwrap()
                } else {
                    curve.clone()
                };
                if let Some(bound) = edge
                    .curve()
                    .parameter_curve_deviation_bound(&target, EPSILON)
                    .unwrap()
                {
                    matched = Some((i, reverse, bound));
                    break;
                }
            }
            if matched.is_some() {
                break;
            }
        }
        let (index, reverse, bound) = matched.expect("complete native boundary counterpart");
        edge_map.push((index, reverse));
        let vertices = &native["edges"][index]["vertices"];
        for (i, v) in edge.vertices().into_iter().enumerate() {
            let target = vertices[if reverse { 1 - i } else { i }].as_u64().unwrap() as usize;
            let error = actual.vertices()[v]
                .point()
                .distance_to(point(&native["vertices"][target]))
                .unwrap();
            assert!(error <= EPSILON);
            maximum = maximum.max(error);
        }
        used[index] = true;
        maximum = maximum.max(bound);
    }
    for (index, face) in actual.faces().iter().enumerate() {
        let loops = native["faces"][index][0].as_array().unwrap();
        assert_eq!(face.loops().len(), loops.len());
        for (actual, target) in face.loops().iter().zip(loops) {
            assert_eq!(
                format!("{:?}", actual.loop_type()),
                target[0].as_str().unwrap()
            );
            let mut expected = target[1]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| (t[0].as_u64().unwrap() as usize, t[1].as_bool().unwrap()))
                .collect::<Vec<_>>();
            let mut actual = actual
                .trims()
                .iter()
                .map(|t| {
                    let (edge, reverse) = edge_map[t.edge().unwrap()];
                    (edge, reverse ^ t.is_reversed_3d() ^ face.is_reversed())
                })
                .collect::<Vec<_>>();
            // The SDK producer stores sorted incidence rather than loop order.
            // Each curve's oriented endpoints were checked above.
            actual.sort_unstable();
            expected.sort_unstable();
            assert_eq!(actual, expected, "oriented face-edge incidence");
        }
    }
    for vertex in actual.vertices() {
        let distance = native["vertices"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| vertex.point().distance_to(point(p)).unwrap())
            .fold(f64::INFINITY, f64::min);
        maximum = maximum.max(distance);
    }
    assert!(maximum <= EPSILON, "native merge difference {maximum}");
    maximum
}

pub fn archive(brep: &Brep) -> Brep {
    use viboceros_io::*;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("merge.3dm");
    let mut model = ThreeDmModel::new(
        vec![ThreeDmLayer {
            name: "Default".into(),
            color: [0, 0, 0],
            visible: true,
            locked: false,
        }],
        vec![],
        vec![ThreeDmObject::new(ThreeDmGeometry::Brep(brep.clone()), 0)],
    );
    model.tolerance = model_tolerance();
    write_3dm_file(&path, &model).unwrap();
    let loaded = read_3dm_file(&path, model_tolerance()).unwrap();
    let ThreeDmGeometry::Brep(b) = loaded.objects.into_iter().next().unwrap().geometry else {
        panic!("B-rep archive")
    };
    b
}
