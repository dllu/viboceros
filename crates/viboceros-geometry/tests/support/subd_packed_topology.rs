//! Oriented topology and complete spatial-isocurve bounds through recovered charts.
use super::images::Placement;
use serde_json::Value;
use std::collections::BTreeSet;
use viboceros_geometry::{Brep, NurbsCurve, NurbsSurface, Point2, SubDNurbsTile};
#[path = "subd_edge_correspondence.rs"]
mod correspondence;
#[path = "curve_full_image.rs"]
mod curves;

pub struct Chart {
    pub target: usize,
    pub grid: [usize; 2],
    pub tiles: Vec<SubDNurbsTile>,
    pub placements: Vec<Placement>,
}
fn rotate([u, v]: [f64; 2], q: u8) -> [f64; 2] {
    match q {
        0 => [u, v],
        1 => [1. - v, u],
        2 => [1. - u, 1. - v],
        3 => [v, 1. - u],
        _ => unreachable!(),
    }
}
impl Chart {
    fn point(&self, p: Point2) -> [f64; 2] {
        let [w, h] = self.grid;
        let [u, v] = p.to_array();
        let x = ((u * w as f64).floor() as usize).min(w - 1);
        let y = ((v * h as f64).floor() as usize).min(h - 1);
        let i = y * w + x;
        let tile = self.tiles[i];
        let placement = self.placements[i];
        let local = rotate(
            [u * w as f64 - x as f64, v * h as f64 - y as f64],
            (4 - tile.quarter_turns) % 4,
        );
        let q = rotate(local, placement.rotation);
        let [u0, u1, v0, v1] = placement.range;
        [u0 + (u1 - u0) * q[0], v0 + (v1 - v0) * q[1]]
    }
}
fn line(value: &Value) -> [Point2; 2] {
    assert_eq!(value["degree"], 1);
    let points = value["control_points"].as_array().unwrap();
    assert_eq!(points.len(), 2);
    std::array::from_fn(|i| {
        Point2::try_new(
            points[i]["point"][0].as_f64().unwrap(),
            points[i]["point"][1].as_f64().unwrap(),
        )
        .unwrap()
    })
}
fn contains([a, b]: [Point2; 2], p: [f64; 2]) -> bool {
    let a = a.to_array();
    let b = b.to_array();
    let d = [b[0] - a[0], b[1] - a[1]];
    let n = d[0] * d[0] + d[1] * d[1];
    let t = ((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / n;
    (-1e-10..=1. + 1e-10).contains(&t)
        && (p[0] - a[0] - t * d[0]).hypot(p[1] - a[1] - t * d[1]) < 1e-10
}
fn iso(surface: &NurbsSurface, [a, b]: [Point2; 2]) -> NurbsCurve {
    let (curve, start, end) = if a.y() == b.y() {
        (surface.isocurve_u(a.y()).unwrap(), a.x(), b.x())
    } else {
        assert_eq!(a.x(), b.x());
        (surface.isocurve_v(a.x()).unwrap(), a.y(), b.y())
    };
    let curve = curve.try_trimmed(start.min(end)..=start.max(end)).unwrap();
    if end > start {
        curve
    } else {
        curve.reversed().unwrap()
    }
}
fn cycle(values: &[usize]) -> Vec<usize> {
    (0..values.len())
        .map(|i| {
            values[i..]
                .iter()
                .chain(&values[..i])
                .copied()
                .collect::<Vec<_>>()
        })
        .min()
        .unwrap()
}
pub fn check(actual: &Brep, value: &Value, charts: &[Chart]) -> f64 {
    check_impl(actual, value, charts, false)
}
#[allow(dead_code)]
pub fn check_measured_gaps(actual: &Brep, value: &Value, charts: &[Chart]) -> f64 {
    check_impl(actual, value, charts, true)
}
fn check_impl(actual: &Brep, value: &Value, charts: &[Chart], measured_gaps: bool) -> f64 {
    let target = &value["output"]["geometry"];
    let topology = &target["topology"];
    let mut incidence = vec![BTreeSet::new(); actual.vertices().len()];
    for (f, face) in actual.faces().iter().enumerate() {
        for l in face.loops() {
            for t in l.trims() {
                for v in t.vertices() {
                    incidence[v].insert(charts[f].target);
                }
            }
        }
    }
    let mut native_incidence = vec![BTreeSet::new(); target["vertices"].as_array().unwrap().len()];
    for (f, face) in topology["faces"].as_array().unwrap().iter().enumerate() {
        for l in face["loops"].as_array().unwrap() {
            for t in l["trims"].as_array().unwrap() {
                for v in t["vertices"].as_array().unwrap() {
                    native_incidence[v.as_u64().unwrap() as usize].insert(f);
                }
            }
        }
    }
    let mut used = vec![false; native_incidence.len()];
    let mut vertices = Vec::new();
    for (i, vertex) in actual.vertices().iter().enumerate() {
        let (j, d) = target["vertices"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .filter(|(j, _)| !used[*j] && incidence[i] == native_incidence[*j])
            .map(|(j, v)| {
                (
                    j,
                    vertex
                        .point()
                        .distance_to(crate::native::point(&v["point"]))
                        .unwrap(),
                )
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap();
        assert!(d < 1e-11, "packed vertex {i} to native {j}: {d}");
        used[j] = true;
        vertices.push(j);
    }
    let mut edge_map = vec![None; actual.edges().len()];
    let mut maximum = 0_f64;
    let mut native_bounds = vec![f64::INFINITY; target["edges"].as_array().unwrap().len()];
    let mut edge_bounds = vec![f64::INFINITY; actual.edges().len()];
    let mut maximum_gap = 0_f64;
    for (f, face) in actual.faces().iter().enumerate() {
        let chart = &charts[f];
        let native = &target["faces"][chart.target];
        let surface = crate::surface::from_definition(&native["definition"]);
        assert_eq!(face.loops().len(), 1);
        let trims = &face.loops()[0].trims();
        let nt = topology["faces"][chart.target]["loops"][0]["trims"]
            .as_array()
            .unwrap();
        let nc = native["loops"][0].as_array().unwrap();
        assert_eq!(trims.len(), nt.len());
        let mut mapped = Vec::new();
        for trim in *trims {
            let range = trim.curve().domain();
            let t = *range.start() + 0.371 * (*range.end() - *range.start());
            let p = chart.point(trim.curve().evaluate(t).unwrap());
            let candidates = nc
                .iter()
                .enumerate()
                .filter(|(_, c)| contains(line(&c["definition"]), p))
                .map(|(i, _)| i)
                .collect::<Vec<_>>();
            assert_eq!(
                candidates.len(),
                1,
                "packed trim has no unique native boundary mapping at {p:?}"
            );
            let k = candidates[0];
            assert_eq!(
                trim.vertices().map(|v| vertices[v]),
                [
                    nt[k]["vertices"][0].as_u64().unwrap() as usize,
                    nt[k]["vertices"][1].as_u64().unwrap() as usize
                ]
            );
            assert_eq!(
                format!("{:?}", trim.trim_type()),
                nt[k]["type"].as_str().unwrap()
            );
            let ni = nt[k]["edge"].as_u64().unwrap() as usize;
            let flip = trim.is_reversed_3d() ^ nt[k]["reversed"].as_bool().unwrap();
            let edge = trim.edge().unwrap();
            let pair = (ni, flip);
            if let Some(old) = edge_map[edge] {
                assert_eq!(old, pair);
            } else {
                edge_map[edge] = Some(pair);
            }
            mapped.push(ni);
            let mut own = iso(
                face.surface(),
                [
                    trim.curve().evaluate(*range.start()).unwrap(),
                    trim.curve().evaluate(*range.end()).unwrap(),
                ],
            );
            if trim.is_reversed_3d() {
                own = own.reversed().unwrap();
            }
            let own_bound = curves::curve_bound(actual.edges()[edge].curve(), &own);
            if measured_gaps {
                assert!(
                    own_bound <= actual.edges()[edge].tolerance()
                        || actual.edges()[edge]
                            .curve()
                            .parameter_curve_deviation_bound(&own, actual.edges()[edge].tolerance())
                            .unwrap()
                            .is_some(),
                    "uncertified edge {edge}, face {f}"
                );
            } else {
                assert!(
                    own_bound < 1e-11,
                    "actual packed edge {edge}, face {f}: {own_bound}"
                );
            }
            if !measured_gaps {
                maximum = maximum.max(own_bound);
            }
            maximum_gap = maximum_gap.max(own_bound);
            let mut expected = iso(&surface, line(&nc[k]["definition"]));
            if nt[k]["reversed"].as_bool().unwrap() {
                expected = expected.reversed().unwrap();
            }
            let native_bound = curves::curve_bound(
                &curves::curve(&target["edges"][ni]["curve"]["definition"]),
                &expected,
            );
            // Shared native edges inherit one incident chart's parameter
            // speed; the other chart may use different source-cell lengths.
            // Full tile bounds certify both images. At least one incident
            // isocurve must certify the complete stored spatial curve.
            native_bounds[ni] = native_bounds[ni].min(native_bound);
            if measured_gaps && own_bound < 1e-11 && native_bound < 1e-11 {
                let mut expected = curves::curve(&target["edges"][ni]["curve"]["definition"]);
                if flip {
                    expected = expected.reversed().unwrap();
                }
                let affine = curves::curve_bound(actual.edges()[edge].curve(), &expected);
                if affine < 1e-11 {
                    edge_bounds[edge] = edge_bounds[edge].min(affine);
                } else {
                    let pairs = correspondence::pairs(
                        chart,
                        [
                            trim.curve().evaluate(*range.start()).unwrap(),
                            trim.curve().evaluate(*range.end()).unwrap(),
                        ],
                        line(&nc[k]["definition"]),
                        trim.is_reversed_3d(),
                    );
                    if let Some(bound) = actual.edges()[edge]
                        .curve()
                        .mapped_curve_deviation_bound(&expected, &pairs, 1e-11)
                        .unwrap()
                    {
                        edge_bounds[edge] = edge_bounds[edge].min(bound);
                    }
                }
            }
        }
        assert_eq!(
            cycle(&mapped),
            cycle(
                &nt.iter()
                    .map(|t| t["edge"].as_u64().unwrap() as usize)
                    .collect::<Vec<_>>()
            )
        );
    }
    maximum = maximum.max(native_bounds.into_iter().fold(0_f64, f64::max));
    if measured_gaps {
        maximum = maximum.max(edge_bounds.into_iter().fold(0_f64, f64::max));
        println!(
            "certified native spatial curve bound {maximum:e}; finite patch gap bound {maximum_gap:e}"
        );
    }
    let mut used_edges = BTreeSet::new();
    for (i, mapped) in edge_map.into_iter().enumerate() {
        let (j, flip) = mapped.unwrap();
        assert!(used_edges.insert(j));
        let actual = actual.edges()[i].vertices().map(|v| vertices[v]);
        let native = [
            topology["edges"][j][0].as_u64().unwrap() as usize,
            topology["edges"][j][1].as_u64().unwrap() as usize,
        ];
        assert_eq!(actual, if flip { [native[1], native[0]] } else { native });
    }
    assert!(maximum < 1e-11, "packed spatial-isocurve bound {maximum}");
    maximum
}
