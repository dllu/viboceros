use serde_json::Value;
use std::{io::Read, sync::OnceLock};
use viboceros_geometry::{SubDNurbsContinuity, Tolerance};
#[path = "support/nurbs_common_basis.rs"]
mod basis;
#[path = "support/positive_tensor.rs"]
mod bound;
#[path = "support/subd_nurbs_brep.rs"]
mod compare;
#[path = "support/subd_to_nurbs.rs"]
mod native;
#[path = "support/nurbs_surface.rs"]
mod surface;

fn captures() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        let bytes =
            include_bytes!("../../../tools/rhino_oracle/observations/subd_continuity.json.gz");
        let mut text = Vec::new();
        flate2::read::GzDecoder::new(&bytes[..])
            .read_to_end(&mut text)
            .unwrap();
        serde_json::from_slice(&text).unwrap()
    })
}

fn check(scene: &str) {
    for (mode, process) in [
        (0, SubDNurbsContinuity::G0),
        (1, SubDNurbsContinuity::G1),
        (2, SubDNurbsContinuity::G2),
    ] {
        let id = format!("{scene}_{mode}");
        let value = &captures()["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id)
            .unwrap()["value"];
        let source = native::source(&value["before"]);
        let archive = source.archive().to_vec();
        let patches = source.to_nurbs_patches(process).unwrap();
        let expected = value["output"]["geometry"]["faces"].as_array().unwrap();
        assert_eq!(patches.len(), expected.len());
        let mut maximum = 0_f64;
        for (patch, face) in patches.iter().zip(expected) {
            let mut target = surface::from_definition(&face["definition"]);
            if patch.corner.is_some() {
                target = target.try_reversed_u().unwrap().try_reversed_v().unwrap();
            }
            let (a, b) = basis::common_basis(&patch.surface, &target);
            maximum = maximum.max(bound::complete_bound(&a, &b));
        }
        println!(
            "SUBD_CONTINUITY {}",
            serde_json::json!({"id":id,"patches":patches.len(),"maximum_full_image_bound":maximum})
        );
        assert!(maximum < 1e-11, "{id}: {maximum}");
        compare::check(
            &source.to_nurbs_brep(process, Tolerance::DEFAULT).unwrap(),
            value,
        );
        assert_eq!(source.archive(), archive);
    }
}
macro_rules! cases {($($scene:ident),* $(,)?) => {$ (
    #[test]fn $scene(){check(stringify!($scene));}
)*};}
cases!(
    quad_corners,
    quad_smooth,
    grid,
    pentagon,
    creased_box,
    torus,
    smooth_pentagon,
    five_fan,
    seven_fan,
    pentagonal_prism,
    tilted_grid,
    reversed_fan,
    skew_box,
    one_crease_box,
    crease_chain_box,
    fan_asymmetric
);
#[test]
fn smooth_box() {
    check("box");
}

#[test]
fn smooth_limit_normals_preserve_order_units_and_immutable_queries() {
    let source = native::source(&native::case("box")["before"]);
    let archive = source.archive().to_vec();
    let normals =
        viboceros_opennurbs::subd_smooth_vertex_normals(source.archive(), &[7, 0, 7]).unwrap();
    assert_eq!(normals[0], normals[2]);
    let normal = [-0.25, -1. / 3., -0.5_f64];
    let length = normal[0].hypot(normal[1]).hypot(normal[2]);
    for (a, b) in normals[1].iter().zip(normal) {
        assert!((a - b / length).abs() < 1e-12);
    }
    assert!(
        viboceros_opennurbs::subd_smooth_vertex_normals(source.archive(), &[])
            .unwrap()
            .is_empty()
    );
    assert!(
        viboceros_opennurbs::subd_smooth_vertex_normals(source.archive(), &[usize::MAX]).is_err()
    );
    let boundary = native::source(&native::case("quad_smooth")["before"]);
    assert!(viboceros_opennurbs::subd_smooth_vertex_normals(b"invalid archive data", &[]).is_err());
    assert!(viboceros_opennurbs::subd_smooth_vertex_normals(boundary.archive(), &[0]).is_err());
    assert_eq!(source.archive(), archive);
}

#[test]
fn unfinished_modes_reject_without_editing_the_source() {
    let source = native::source(&native::case("box")["before"]);
    let before = source.archive().to_vec();
    for mode in [SubDNurbsContinuity::G1x, SubDNurbsContinuity::G1xx] {
        assert!(source.to_nurbs_patches(mode).is_err());
    }
    assert_eq!(source.archive(), before);
}
