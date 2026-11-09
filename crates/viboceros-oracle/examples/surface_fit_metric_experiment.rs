//! Offline sampled diagnostics, never a complete surface accuracy certificate.
use serde_json::{Value, json};
use viboceros_geometry::*;
#[path = "../../viboceros-command/tests/support/merge_surfaces.rs"]
#[allow(dead_code)]
mod captured;

fn candidate(source: &NurbsSurface, p: [usize; 2], cuts: [Vec<f64>; 2]) -> NurbsSurface {
    let knots: [Vec<f64>; 2] =
        std::array::from_fn(|a| [vec![0.; p[a]], cuts[a].clone(), vec![1.; p[a]]].concat());
    try_interpolate_surface_in_basis(source, p, [&knots[0], &knots[1]]).unwrap()
}
fn metrics(a: &NurbsSurface, b: &NurbsSurface, n: usize) -> [f64; 3] {
    let tol = Tolerance::try_new(1e-12, 1e-12, 1e-9).unwrap();
    let samples = a.deviation_samples(b, [n, n], tol).unwrap();
    assert_eq!(samples.first_to_second.refinement_failures, 0);
    assert_eq!(samples.second_to_first.refinement_failures, 0);
    [
        samples.same_parameters.maximum.distance,
        samples.first_to_second.maximum.distance,
        samples.second_to_first.maximum.distance,
    ]
}
fn main() {
    let ids = [
        "fit-strip-rational_bezier-0-3-0.01",
        "fit-strip-rational_bezier-0-3-0.001",
        "fit-adaptive-polynomial-1-3-0.05",
        "fit-adaptive-polynomial-1-3-0.01",
        "fit-adaptive-rational_bezier-1-3-0.05",
        "fit-adaptive-rational_bezier-1-3-0.01",
        "fit-adaptive-rational_bezier-1-3-0.001",
        "fit-capacity-holdout-2-1-distinct",
        "fit-capacity-holdout-4-3-distinct",
    ];
    for family in [
        "surface_fit_strips",
        "surface_fit_adaptive",
        "surface_fit_capacity_holdouts",
    ] {
        let q: Value = serde_json::from_slice(
            &std::fs::read(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join(format!("../../tools/rhino_oracle/fixtures/{family}.json")),
            )
            .unwrap(),
        )
        .unwrap();
        let native = captured::read(family);
        for (op, row) in q["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            if !ids.contains(&op["id"].as_str().unwrap()) {
                continue;
            }
            let source = captured::surface(&row["value"]["before"])
                .try_reparameterized(0. ..=1., 0. ..=1.)
                .unwrap();
            let target = captured::surface(&row["value"]["surface"])
                .try_reparameterized(0. ..=1., 0. ..=1.)
                .unwrap();
            let p = [
                op["degree_u"].as_u64().unwrap() as usize,
                op["degree_v"].as_u64().unwrap() as usize,
            ];
            let base = vec![0., 1.];
            let mid = vec![0., 0.5, 1.];
            let quarter = vec![0., 0.25, 0.5, 0.75, 1.];
            for (label, candidate) in [
                ("native", target),
                (
                    "initial",
                    candidate(&source, p, [base.clone(), base.clone()]),
                ),
                ("u", candidate(&source, p, [mid.clone(), base.clone()])),
                ("v", candidate(&source, p, [base, mid.clone()])),
                ("uv", candidate(&source, p, [mid.clone(), mid])),
                ("quarter", candidate(&source, p, [quarter.clone(), quarter])),
            ] {
                let error = metrics(&source, &candidate, 16);
                println!(
                    "FIT_METRICS {}",
                    json!({"id":op["id"],"limit":op["fit_tolerance"],"proposal":label,"counts":[candidate.control_point_count_u(),candidate.control_point_count_v()],
                    "sampled_parameter":error[0],"source_to_candidate_witness":error[1],"candidate_to_source_witness":error[2]})
                );
            }
        }
    }
}
