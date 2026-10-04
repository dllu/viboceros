//! Independent decomposition diagnostics against public fitted-plane captures.
use faer::Mat;
use nalgebra::DMatrix;
use serde_json::{Value, json};
use viboceros_geometry::{Frame3, Point3, Tolerance, Vector3};
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn record(normal: [f64; 3], native: &Value) -> Value {
    match Frame3::try_from_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_from(normal).unwrap(),
        Tolerance::NUMERICAL_VALIDATION,
    ) {
        Ok(f) => {
            let n = f.z_axis().as_vector().to_array();
            let x = f.x_axis().as_vector().to_array();
            let y = f.y_axis().as_vector().to_array();
            let diff = |a: [f64; 3], b: &Value| {
                a.into_iter()
                    .enumerate()
                    .map(|(i, v)| (v - b[i].as_f64().unwrap()).powi(2))
                    .sum::<f64>()
                    .sqrt()
            };
            json!({"normal":n,"normal_error":diff(n,&native["normal"]),"basis_error":diff(x,&native["x"]).max(diff(y,&native["y"]))})
        }
        Err(_) => json!(null),
    }
}
fn main() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut rows = Vec::new();
    for name in [
        "circle_fit_diagnostics",
        "circle_fit_distant_arcs",
        "circle_fit_distant_noisy",
    ] {
        let q: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join("fixtures").join(format!("{name}.json"))).unwrap(),
        )
        .unwrap();
        let r: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join("observations").join(format!("{name}.json")))
                .unwrap(),
        )
        .unwrap();
        for (op, row) in q["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(r["results"].as_array().unwrap())
        {
            let p = op["points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| std::array::from_fn::<_, 3, _>(|i| p[i].as_f64().unwrap()))
                .collect::<Vec<_>>();
            let mean: [f64; 3] =
                std::array::from_fn(|i| p.iter().map(|p| p[i]).sum::<f64>() / p.len() as f64);
            let mut variants = serde_json::Map::new();
            let native = &row["value"]["plane"];
            for centered in [false, true] {
                for width in [3, 4] {
                    let entries = (0..p.len() * width)
                        .map(|i| {
                            let r = i / width;
                            let c = i % width;
                            if c == 3 {
                                1.
                            } else {
                                p[r][c] - if centered { mean[c] } else { 0. }
                            }
                        })
                        .collect::<Vec<_>>();
                    let a = Mat::from_fn(p.len(), width, |r, c| entries[r * width + c]);
                    let s = a.svd().unwrap();
                    let label = format!("faer_{centered}_{width}");
                    variants.insert(
                        label.clone(),
                        record(std::array::from_fn(|i| s.V()[(i, width - 1)]), native),
                    );
                    if width == 3 {
                        variants.insert(
                            label + "_cross",
                            record(
                                cross(
                                    std::array::from_fn(|i| s.V()[(i, 0)]),
                                    std::array::from_fn(|i| s.V()[(i, 1)]),
                                ),
                                native,
                            ),
                        );
                    }
                    if p.len() >= width {
                        let s = DMatrix::from_row_slice(p.len(), width, &entries).svd(false, true);
                        let v = s.v_t.unwrap();
                        let label = format!("nalgebra_{centered}_{width}");
                        variants.insert(
                            label.clone(),
                            record(std::array::from_fn(|i| v[(width - 1, i)]), native),
                        );
                        if width == 3 {
                            variants.insert(
                                label + "_cross",
                                record(
                                    cross(
                                        std::array::from_fn(|i| v[(0, i)]),
                                        std::array::from_fn(|i| v[(1, i)]),
                                    ),
                                    native,
                                ),
                            );
                        }
                    }
                }
            }
            rows.push(json!({"id":op["id"],"variants":variants}));
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "schema_version":1,"native_records":44,"normal_epsilon":1e-12,
            "basis_epsilon":1e-12,"centroid":"sequential arithmetic mean",
            "input_scaling":"none","measurements":rows
        }))
        .unwrap()
    );
}
