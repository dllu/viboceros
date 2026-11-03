//! Complete native cell images and oriented topology of command output.
use serde_json::{Value, json};
use viboceros_geometry::{Brep, SubD, SubDNurbsContinuity, SubDNurbsTile};
pub fn check(
    actual: &Brep,
    source: &SubD,
    packed: bool,
    mode: SubDNurbsContinuity,
    target: &Value,
) {
    let layouts = if packed {
        source
            .to_packed_nurbs_patches(mode)
            .unwrap()
            .into_iter()
            .map(|p| (p.grid_size, p.tiles))
            .collect::<Vec<_>>()
    } else {
        source
            .faces()
            .iter()
            .enumerate()
            .flat_map(|(face, f)| {
                let corners = if f.len() == 4 {
                    vec![None]
                } else {
                    (0..f.len()).map(Some).collect()
                };
                corners.into_iter().map(move |corner| {
                    (
                        [1, 1],
                        vec![SubDNurbsTile {
                            face,
                            corner,
                            cell: [0, 0],
                            quarter_turns: 0,
                        }],
                    )
                })
            })
            .collect()
    };
    assert_eq!(
        actual.faces().len(),
        target["faces"].as_array().unwrap().len()
    );
    assert_eq!(layouts.len(), actual.faces().len());
    let mut charts = Vec::new();
    let mut maximum = 0_f64;
    for (index, ((grid, tiles), face)) in layouts.into_iter().zip(actual.faces()).enumerate() {
        let mut native = crate::images::NativeImage::new(crate::surface::from_definition(
            &target["faces"][index]["definition"],
        ));
        let [w, h] = grid;
        let mut placements = Vec::new();
        for tile in &tiles {
            let [x, y] = tile.cell;
            let cell = face
                .surface()
                .try_trimmed(
                    x as f64 / w as f64..=(x + 1) as f64 / w as f64,
                    y as f64 / h as f64..=(y + 1) as f64 / h as f64,
                )
                .unwrap();
            let cell = crate::images::unrotate(cell, tile.quarter_turns);
            let (error, placement) = native.check(&cell, tile.corner.is_some());
            maximum = maximum.max(error);
            placements.push(placement);
        }
        native.finish();
        charts.push(crate::topology::Chart {
            target: index,
            grid,
            tiles,
            placements,
        });
    }
    let value = json!({"output":{"geometry":target,"solid":target["topology"]["solid"]}});
    let edges = crate::topology::check(actual, &value, &charts);
    println!("complete command surface bound {maximum:e}, edge bound {edges:e}");
}
