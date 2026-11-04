//! Imported native soft-crease archives, complete images and B-rep incidence.
use serde_json::Value;
use std::io::Read;
use viboceros_geometry::{SubDNurbsContinuity, Tolerance};
use viboceros_io::{ThreeDmGeometry, read_3dm_file};
#[path = "../../viboceros-geometry/tests/support/nurbs_common_basis.rs"]
mod basis;
#[path = "../../viboceros-geometry/tests/support/positive_tensor.rs"]
mod bound;
#[path = "../../viboceros-command/tests/support/subd_nurbs_images.rs"]
mod complete;
#[path = "../../viboceros-geometry/tests/support/subd_packed_images.rs"]
mod images;
#[path = "../../viboceros-geometry/tests/support/subd_to_nurbs.rs"]
#[allow(dead_code)]
mod native;
#[path = "../../viboceros-geometry/tests/support/nurbs_surface.rs"]
mod surface;
#[path = "../../viboceros-geometry/tests/support/subd_packed_topology.rs"]
mod topology;

fn mode(id: &str) -> SubDNurbsContinuity {
    match id.rsplit_once('_').unwrap().1 {
        "0" => SubDNurbsContinuity::G0,
        "1" => SubDNurbsContinuity::G1,
        "2" => SubDNurbsContinuity::G2,
        "3" => SubDNurbsContinuity::G1x,
        "4" => SubDNurbsContinuity::G1xx,
        _ => unreachable!(),
    }
}
fn supported(id: &str) -> bool {
    !(id.starts_with("variable_")
        || id.starts_with("hard_soft_")
        || id.starts_with("even_")
            && (id.contains("_90.0_") || id.contains("_100.0_"))
            && matches!(
                mode(id),
                SubDNurbsContinuity::G1x | SubDNurbsContinuity::G1xx
            ))
}
fn for_cases(mut check: impl FnMut(&str, &Value, &viboceros_geometry::SubD)) {
    let temp = tempfile::tempdir().unwrap();
    for bytes in [
        include_bytes!("../../../tools/rhino_oracle/observations/subd_soft_nurbs.json.gz")
            .as_slice(),
        include_bytes!("../../../tools/rhino_oracle/observations/subd_soft_nurbs_holdouts.json.gz")
            .as_slice(),
        include_bytes!("../../../tools/rhino_oracle/observations/subd_soft_nurbs_even.json.gz")
            .as_slice(),
    ] {
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(bytes)
            .read_to_end(&mut raw)
            .unwrap();
        let data: Value = serde_json::from_slice(&raw).unwrap();
        for row in data["results"].as_array().unwrap() {
            let id = row["id"].as_str().unwrap();
            let value = &row["value"];
            let bytes = value["archive"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u8)
                .collect::<Vec<_>>();
            let path = temp.path().join("source.3dm");
            std::fs::write(&path, bytes).unwrap();
            let model = read_3dm_file(&path, Tolerance::DEFAULT).unwrap();
            let ThreeDmGeometry::SubD(source) = &model.objects[0].geometry else {
                panic!()
            };
            assert!(source.edge_sharpness().iter().flatten().any(|&v| v > 0.));
            let archive = source.archive().to_vec();
            for (actual, expected) in source
                .surface_points()
                .iter()
                .zip(value["before"]["surface_points"].as_array().unwrap())
            {
                assert!(actual.distance_to(native::point(expected)).unwrap() < 1e-12);
            }
            check(id, value, source);
            assert_eq!(source.archive(), archive);
        }
    }
}

#[test]
fn native_soft_surface_images() {
    let (mut accepted, mut rejected) = (0, 0);
    for_cases(|id, value, source| {
        if !supported(id) {
            assert!(source.to_nurbs_patches(mode(id)).is_err(), "{id}");
            rejected += 1;
            return;
        }
        accepted += 1;
        let patches = source.to_nurbs_patches(mode(id)).unwrap();
        let target = value["unpacked"]["geometry"]["faces"].as_array().unwrap();
        assert_eq!(patches.len(), target.len());
        let mut max = 0_f64;
        for (p, t) in patches.iter().zip(target) {
            let mut expected = surface::from_definition(&t["definition"]);
            if p.corner.is_some() {
                expected = expected.try_reversed_u().unwrap().try_reversed_v().unwrap();
            }
            let (a, b) = basis::common_basis(&p.surface, &expected);
            let error = bound::complete_bound(&a, &b);
            max = max.max(error);
        }
        println!("SOFT_IMAGE {id} {max:e}");
        assert!(max < 1e-11, "{id}: {max}");
    });
    assert_eq!((accepted, rejected), (79, 28));
}

#[test]
fn native_soft_crease_archives_preserve_complete_nurbs_images_and_topology() {
    for_cases(|id, value, source| {
        println!("SOFT_BREP {id}");
        for packed in [false, true] {
            let brep = if packed {
                source.to_packed_nurbs_brep(mode(id), Tolerance::DEFAULT)
            } else {
                source.to_nurbs_brep(mode(id), Tolerance::DEFAULT)
            };
            if !supported(id) {
                assert!(brep.is_err(), "{id} packed={packed}");
                continue;
            }
            let brep = brep.unwrap_or_else(|e| panic!("{id} packed={packed}: {e}"));
            complete::check(
                &brep,
                source,
                packed,
                mode(id),
                &value[if packed { "output" } else { "unpacked" }]["geometry"],
            );
        }
    });
}
