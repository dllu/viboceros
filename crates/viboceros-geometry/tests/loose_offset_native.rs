//! Public native expectations never construct product source geometry.
use serde_json::Value;
use std::io::Read;
use viboceros_geometry::{NurbsSurface, Point3, WeightedPoint3};

fn read(family: &str) -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../tools/rhino_oracle/observations/{family}.json.gz"
    ));
    let mut text = String::new();
    flate2::read::GzDecoder::new(std::fs::File::open(path).unwrap())
        .read_to_string(&mut text)
        .unwrap();
    serde_json::from_str(&text).unwrap()
}

fn surface(v: &Value, authored: bool) -> NurbsSurface {
    let p = if authored {
        [
            v["degree_u"].as_u64().unwrap() as usize,
            v["degree_v"].as_u64().unwrap() as usize,
        ]
    } else {
        serde_json::from_value(v["degree"].clone()).unwrap()
    };
    let n = if authored {
        [
            v["control_point_count_u"].as_u64().unwrap() as usize,
            v["control_point_count_v"].as_u64().unwrap() as usize,
        ]
    } else {
        serde_json::from_value(v["control_count"].clone()).unwrap()
    };
    let points = v["control_points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            WeightedPoint3::try_new(
                Point3::try_from(serde_json::from_value::<[f64; 3]>(p["point"].clone()).unwrap())
                    .unwrap(),
                p["weight"].as_f64().unwrap(),
            )
            .unwrap()
        })
        .collect();
    NurbsSurface::try_new_rational(
        p[0],
        p[1],
        n[0],
        n[1],
        points,
        serde_json::from_value(v["knots_u"].clone()).unwrap(),
        serde_json::from_value(v["knots_v"].clone()).unwrap(),
    )
    .unwrap()
}

#[test]
fn native_loose_interpolates_normal_offsets_at_original_greville_stations() {
    let capture = read("offset_srf_loose_pilot");
    for row in capture["results"].as_array().unwrap() {
        let id = row["id"].as_str().unwrap();
        if id == "offset_loose_polysurface" {
            continue;
        }
        let v = &row["value"];
        let source = surface(&v["prescribed_case"]["curves"][0], true);
        let result = surface(&v["after"][2]["surfaces"][0], false);
        let distance = if id == "offset_loose_negative" {
            -0.5
        } else {
            0.5
        };
        let actual = source
            .try_loose_offset(distance, viboceros_geometry::Tolerance::default())
            .unwrap();
        assert_eq!(actual.knots_u(), source.knots_u());
        assert_eq!(actual.knots_v(), source.knots_v());
        let mut control_maximum = 0_f64;
        for (a, b) in actual.control_points().iter().zip(result.control_points()) {
            control_maximum = control_maximum.max(a.point().distance_to(b.point()).unwrap());
            assert!((a.weight() - b.weight()).abs() <= 1e-14);
        }
        println!("LOOSE_NATIVE_CONTROLS {id} {control_maximum:e}");
        assert!(control_maximum <= 1e-6, "{id}: {control_maximum:e}");
        let mut maximum = 0_f64;
        for j in 0..source.control_point_count_v() {
            let v = source.knots_v()[j + 1..=j + source.degree_v()]
                .iter()
                .sum::<f64>()
                / source.degree_v() as f64;
            for i in 0..source.control_point_count_u() {
                let u = source.knots_u()[i + 1..=i + source.degree_u()]
                    .iter()
                    .sum::<f64>()
                    / source.degree_u() as f64;
                let target = source
                    .evaluate(u, v)
                    .unwrap()
                    .translated(
                        source
                            .normal_at(u, v)
                            .unwrap()
                            .as_vector()
                            .scaled(distance)
                            .unwrap(),
                    )
                    .unwrap();
                maximum = maximum.max(target.distance_to(result.evaluate(u, v).unwrap()).unwrap());
            }
        }
        println!("LOOSE_NATIVE_GREVILLE {id} {maximum:e}");
        assert!(maximum <= 1e-6, "{id}: {maximum:e}");
    }
}

#[test]
fn native_loose_control_nets_include_closed_periodic_poles_and_degree_five() {
    let mut count = 0;
    for family in [
        "offset_srf_loose_pilot",
        "offset_srf_loose_holdouts",
        "offset_srf_loose_final",
    ] {
        for row in read(family)["results"].as_array().unwrap() {
            let id = row["id"].as_str().unwrap();
            let v = &row["value"];
            let authored = &v["prescribed_case"]["curves"][0];
            if authored["type"] != "surface"
                || v["history"].as_str().unwrap().contains("Unknown command:")
                || !v["events"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|e| e["name"] == "OffsetSrf" && e["result"] == "Success")
            {
                continue;
            }
            let source = surface(authored, true);
            let mut distance = 1.;
            let mut flip = false;
            let mut solid = false;
            let mut delete = false;
            for action in v["prescribed_case"]["actions"].as_array().unwrap() {
                if let Some(n) = action.as_f64() {
                    distance = n;
                }
                if let Some(s) = action.get("number").and_then(Value::as_str) {
                    distance = s.strip_suffix("cm").unwrap().parse::<f64>().unwrap() * 10.;
                }
                if action == "FlipAll" {
                    flip = !flip;
                }
                if action == "Solid=Yes" {
                    solid = true;
                } else if action == "Solid=No" {
                    solid = false;
                }
                if action == "DeleteInput=Yes" {
                    delete = true;
                } else if action == "DeleteInput=No" {
                    delete = false;
                }
            }
            if flip {
                distance = -distance;
            }
            let target = &v["after"][if solid && delete { 1 } else { 2 }]["surfaces"][0];
            let expected = surface(target, false);
            let b = viboceros_geometry::Brep::try_surface_face(
                source.clone(),
                viboceros_geometry::Tolerance::DEFAULT,
            )
            .unwrap();
            if b.is_solid()
                && b.solid_orientation().unwrap()
                    == viboceros_geometry::BrepSolidOrientation::Inward
            {
                distance = -distance;
            }
            let basis = if b.is_solid() {
                source.clone()
            } else {
                source.try_clamped_to_active_domain().unwrap()
            };
            let actual = basis
                .try_loose_offset(distance, viboceros_geometry::Tolerance::DEFAULT)
                .unwrap_or_else(|e| panic!("{id}: {e}"));
            assert_eq!(actual.degree_u(), expected.degree_u(), "{id}");
            assert_eq!(actual.degree_v(), expected.degree_v(), "{id}");
            // OpenNURBS omits the two unused outer knots; its public export
            // pads those endpoints by repetition. All active knots must match.
            let active = |k: &[f64]| k[1..k.len() - 1].to_vec();
            assert_eq!(active(actual.knots_u()), active(expected.knots_u()), "{id}");
            assert_eq!(active(actual.knots_v()), active(expected.knots_v()), "{id}");
            assert_eq!(
                actual.control_point_count_u(),
                expected.control_point_count_u(),
                "{id}"
            );
            assert_eq!(
                actual.control_point_count_v(),
                expected.control_point_count_v(),
                "{id}"
            );
            let mut maximum = 0_f64;
            for (a, b) in actual
                .control_points()
                .iter()
                .zip(expected.control_points())
            {
                maximum = maximum.max(a.point().distance_to(b.point()).unwrap());
                assert!(
                    (a.weight() - b.weight()).abs() <= 64. * f64::EPSILON * a.weight().abs(),
                    "{id}"
                );
            }
            println!("LOOSE_CONTROL_NET {id} {maximum:e}");
            assert!(maximum <= 1e-6, "{id}: {maximum:e}");
            count += 1;
        }
    }
    println!("LOOSE_CONTROL_NET_COUNT {count}");
}
