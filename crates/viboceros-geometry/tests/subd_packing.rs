//! Packed tiles retain complete native patch images and native pack partitions.
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    sync::OnceLock,
};
use viboceros_geometry::{NurbsSurface, SubDNurbsContinuity, Tolerance};
#[path = "support/nurbs_common_basis.rs"]
mod basis;
#[path = "support/positive_tensor.rs"]
mod bound;
#[path = "support/subd_packed_images.rs"]
mod images;
#[path = "support/subd_to_nurbs.rs"]
#[allow(dead_code)] // Baseline fixture lookups are used by the other SubD suites.
mod native;
#[path = "support/nurbs_surface.rs"]
mod surface;
#[path = "support/subd_packed_topology.rs"]
mod topology;

fn captures() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        let mut results = Vec::new();
        for bytes in [
            include_bytes!("../../../tools/rhino_oracle/observations/subd_packing_modes.json.gz")
                .as_slice(),
            include_bytes!("../../../tools/rhino_oracle/observations/subd_packing.json.gz")
                .as_slice(),
            include_bytes!(
                "../../../tools/rhino_oracle/observations/subd_packing_holdouts.json.gz"
            )
            .as_slice(),
        ] {
            let mut text = Vec::new();
            flate2::read::GzDecoder::new(bytes)
                .read_to_end(&mut text)
                .unwrap();
            let mut data: Value = serde_json::from_slice(&text).unwrap();
            results.append(data["results"].as_array_mut().unwrap());
        }
        serde_json::json!({"results":results})
    })
}

fn unrotate(surface: NurbsSurface, q: u8) -> NurbsSurface {
    let surface = match q {
        0 => surface,
        1 => surface.try_swapped_uv().unwrap().try_reversed_v().unwrap(),
        2 => surface.try_reversed_u().unwrap().try_reversed_v().unwrap(),
        3 => surface.try_swapped_uv().unwrap().try_reversed_u().unwrap(),
        _ => panic!("invalid chart rotation"),
    };
    surface.try_reparameterized(0. ..=1., 0. ..=1.).unwrap()
}

fn check(scene: &str) {
    for row in captures()["results"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["id"].as_str().unwrap().rsplit_once('_').unwrap().0 == scene)
    {
        let id = row["id"].as_str().unwrap();
        let value = &row["value"];
        let mode = match id.rsplit_once('_').unwrap().1 {
            "0" => SubDNurbsContinuity::G0,
            "1" => SubDNurbsContinuity::G1,
            "2" => SubDNurbsContinuity::G2,
            "3" => SubDNurbsContinuity::G1x,
            "4" => SubDNurbsContinuity::G1xx,
            _ => unreachable!(),
        };
        let source = native::source(&value["before"]);
        let archive = source.archive().to_vec();
        let packs = source
            .to_packed_nurbs_patches(mode)
            .unwrap_or_else(|e| panic!("{id}: {e}"));
        let expected = value["unpacked"]["geometry"]["faces"].as_array().unwrap();
        let mut offsets = Vec::new();
        let mut cursor = 0;
        let mut native_groups = BTreeMap::<u64, BTreeSet<usize>>::new();
        for (i, face) in source.faces().iter().enumerate() {
            offsets.push(cursor);
            let pid = value["unpacked"]["pack_ids"][cursor].as_u64().unwrap();
            native_groups.entry(pid).or_default().insert(i);
            cursor += if face.len() == 4 { 1 } else { face.len() };
        }
        let actual_groups = packs
            .iter()
            .map(|p| p.tiles.iter().map(|t| t.face).collect::<BTreeSet<_>>())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            actual_groups,
            native_groups.into_values().collect::<BTreeSet<_>>(),
            "{id}"
        );
        assert_eq!(
            packs.len(),
            value["output"]["geometry"]["faces"]
                .as_array()
                .unwrap()
                .len(),
            "{id}"
        );
        let mut maximum = 0_f64;
        let mut packed_maximum = 0_f64;
        let mut charts = Vec::new();
        let mut packed_images = value["output"]["geometry"]["faces"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| images::NativeImage::new(surface::from_definition(&f["definition"])))
            .collect::<Vec<_>>();
        let mut packed_indices = BTreeMap::<u64, Vec<usize>>::new();
        for (i, pid) in value["output"]["pack_ids"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            packed_indices
                .entry(pid.as_u64().unwrap())
                .or_default()
                .push(i);
        }
        for pack in &packs {
            let [w, h] = pack.grid_size;
            let mut placements = Vec::new();
            let mut native_face = None;
            for tile in &pack.tiles {
                let [x, y] = tile.cell;
                let part = pack
                    .surface
                    .try_trimmed(
                        x as f64 / w as f64..=(x + 1) as f64 / w as f64,
                        y as f64 / h as f64..=(y + 1) as f64 / h as f64,
                    )
                    .unwrap()
                    .try_reparameterized(0. ..=1., 0. ..=1.)
                    .unwrap();
                let part = unrotate(part, tile.quarter_turns);
                let pid = value["unpacked"]["pack_ids"][offsets[tile.face]]
                    .as_u64()
                    .unwrap();
                let target_index = packed_indices[&pid][tile.corner.unwrap_or(0)];
                let (error, placement) =
                    packed_images[target_index].check(&part, tile.corner.is_some());
                packed_maximum = packed_maximum.max(error);
                placements.push(placement);
                if let Some(old) = native_face {
                    assert_eq!(old, target_index);
                } else {
                    native_face = Some(target_index);
                }
                let mut target = surface::from_definition(
                    &expected[offsets[tile.face] + tile.corner.unwrap_or(0)]["definition"],
                );
                if tile.corner.is_some() {
                    target = target.try_reversed_u().unwrap().try_reversed_v().unwrap();
                }
                let (a, b) = basis::common_basis(&part, &target);
                let error = bound::complete_bound(&a, &b);
                assert!(error < 1e-11, "{id}, tile {tile:?}: {error}");
                maximum = maximum.max(error);
            }
            charts.push(topology::Chart {
                target: native_face.unwrap(),
                grid: pack.grid_size,
                tiles: pack.tiles.clone(),
                placements,
            });
        }
        for image in &packed_images {
            image.finish();
        }
        assert_eq!(
            charts.iter().map(|c| c.target).collect::<Vec<_>>(),
            (0..charts.len()).collect::<Vec<_>>(),
            "{id} packed face order"
        );
        println!(
            "SUBD_PACKING {}",
            serde_json::json!({"id":id,"surfaces":packs.len(),"maximum_full_tile_bound":maximum,"maximum_full_packed_bound":packed_maximum})
        );
        let brep = source
            .to_packed_nurbs_brep(mode, Tolerance::DEFAULT)
            .unwrap_or_else(|e| panic!("{id} B-rep: {e}"));
        let target = &value["output"]["geometry"];
        assert_eq!(
            brep.faces().len(),
            target["faces"].as_array().unwrap().len(),
            "{id} faces"
        );
        assert_eq!(
            brep.edges().len(),
            target["edges"].as_array().unwrap().len(),
            "{id} edges"
        );
        assert_eq!(
            brep.vertices().len(),
            target["vertices"].as_array().unwrap().len(),
            "{id} vertices"
        );
        assert_eq!(
            brep.is_solid(),
            value["output"]["solid"].as_bool().unwrap(),
            "{id} solidity"
        );
        assert!(brep.is_manifold(), "{id} manifold");
        let edge_bound = topology::check(&brep, value, &charts);
        println!(
            "SUBD_PACKING_TOPOLOGY {}",
            serde_json::json!({"id":id,"maximum_spatial_iso_bound":edge_bound})
        );
        assert_eq!(source.archive(), archive);
    }
}

macro_rules! cases {($($scene:ident),* $(,)?)=>{$(#[test]fn $scene(){check(stringify!($scene));})*};}
cases!(
    grid_2x1,
    grid_1x4,
    grid_2x2,
    grid,
    grid_smooth,
    grid_rotated,
    grid_reversed,
    grid_shuffled,
    grid_crease,
    grid_short_crease,
    grid_hole,
    cylinder,
    cylinder_rotated,
    torus,
    torus_rotated,
    torus_crease,
    two_grids,
    subdivided_box,
    pentagonal_prism,
    five_fan,
    seven_fan,
    fan6,
    fan8
);
cases!(
    subdivided_box2,
    torus_5x3,
    torus_shifted,
    torus_first_rotated,
    torus_crease_loop,
    cylinder_shifted,
    cylinder_crease,
    twisted_torus,
    coincident_grids
);
#[test]
fn grid_l_shuffled() {
    check("grid_L_shuffled");
}

#[test]
fn smooth_box() {
    check("box");
}

#[test]
fn grid_l() {
    check("grid_L");
}
#[test]
fn grid_t() {
    check("grid_T");
}
