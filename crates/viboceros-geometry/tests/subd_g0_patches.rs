use serde_json::Value;
use std::{io::Read, sync::OnceLock};
#[path = "support/nurbs_common_basis.rs"]
mod basis;
#[path = "support/positive_tensor.rs"]
mod bound;
#[path = "subd_g0_patches/brep.rs"]
mod brep;
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
            include_bytes!("../../../tools/rhino_oracle/observations/subd_to_nurbs_g0.json.gz");
        let mut text = Vec::new();
        flate2::read::GzDecoder::new(&bytes[..])
            .read_to_end(&mut text)
            .unwrap();
        let mut data: Value = serde_json::from_slice(&text).unwrap();
        let bytes = include_bytes!(
            "../../../tools/rhino_oracle/observations/subd_to_nurbs_g0_holdouts.json.gz"
        );
        let mut text = Vec::new();
        flate2::read::GzDecoder::new(&bytes[..])
            .read_to_end(&mut text)
            .unwrap();
        let extra: Value = serde_json::from_slice(&text).unwrap();
        data["results"]
            .as_array_mut()
            .unwrap()
            .extend(extra["results"].as_array().unwrap().iter().cloned());
        data
    })
}

fn check(scene: &str) {
    let value = &captures()["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == scene)
        .unwrap()["value"];
    let source = native::source(&value["before"]);
    let archive = source.archive().to_vec();
    let patches = source.to_nurbs_patches_g0().unwrap();
    let expected = value["output"]["geometry"]["faces"].as_array().unwrap();
    assert_eq!(patches.len(), expected.len());
    let mut maximum = 0_f64;
    for (patch, native) in patches.iter().zip(expected) {
        let mut expected = surface::from_definition(&native["definition"]);
        if patch.corner.is_some() {
            expected = expected.try_reversed_u().unwrap().try_reversed_v().unwrap();
        }
        let (a, b) = basis::common_basis(&patch.surface, &expected);
        let error = bound::complete_bound(&a, &b);
        maximum = maximum.max(error);
    }
    println!(
        "SUBD_G0_PATCH {}",
        serde_json::json!({"scene":scene,"patches":patches.len(),"maximum_full_image_bound":maximum})
    );
    assert!(maximum < 1e-11, "{scene}: {maximum}");
    brep::check(&source, value);
    assert_eq!(source.archive(), archive);
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
    reversed_fan
);
#[test]
fn smooth_box() {
    check("box");
}

#[test]
fn variable_soft_sharpness_rejects_without_changing_the_source() {
    let source = native::source(&native::case("box")["before"]);
    let (soft, _) = source
        .try_with_soft_crease_weights(&[(0, [20., 80.])])
        .unwrap();
    let archive = soft.archive().to_vec();
    assert!(soft.to_nurbs_patches_g0().is_err());
    assert_eq!(soft.archive(), archive);
}

#[test]
fn partial_stencils_identify_unresolved_soft_corner_neighborhoods() {
    let source = native::source(&native::case("box")["before"]);
    let (soft, _) = source
        .try_with_soft_crease_weights(&[(0, [100.; 2])])
        .unwrap();
    let archive = soft.archive().to_vec();
    assert!(viboceros_opennurbs::subd_patch_stencils(&archive, 3).is_err());
    let partial = viboceros_opennurbs::subd_partial_patch_stencils(&archive, 3).unwrap();
    assert_eq!(partial.len(), source.faces().len());
    let ends = source.edges()[0];
    for net in partial {
        assert!(net.stencil.corner.is_none());
        for (i, &v) in source.faces()[net.stencil.face].iter().enumerate() {
            let face = &source.faces()[net.stencil.face];
            let adjacent = [face[(i + 1) % 4], face[(i + 3) % 4]];
            let expected = ends.contains(&v) && adjacent.iter().any(|n| ends.contains(n));
            assert_eq!(net.unresolved_corners[i], expected);
        }
    }
    let resolved = viboceros_opennurbs::subd_partial_patch_stencils(&archive, 5).unwrap();
    assert!(resolved.iter().all(|n| n.unresolved_corners == [false; 4]));
    assert_eq!(soft.archive(), archive);
}

#[test]
fn regular_only_conversion_avoids_unnecessary_global_refinement() {
    let source = native::source(&native::case("torus")["before"])
        .subdivided(5)
        .unwrap();
    assert_eq!(source.faces().len(), 32768);
    assert!(
        viboceros_opennurbs::subd_patch_stencils(source.archive(), 3)
            .unwrap_err()
            .to_string()
            .contains("budget")
    );
    let patches = source.to_nurbs_patches_g0().unwrap();
    assert_eq!(patches.len(), source.faces().len());
    assert!(
        patches
            .iter()
            .all(|p| p.surface.control_points().len() == 16)
    );
}
