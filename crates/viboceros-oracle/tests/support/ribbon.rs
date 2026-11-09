//! Complete tensor/curve bounds plus rectangular coverage and incidence checks.
use super::*;
const EPS: f64 = 1e-9;
type Rect = [[f64; 2]; 2];
fn rect(s: &NurbsSurface) -> Rect {
    [
        [*s.domain_u().start(), *s.domain_u().end()],
        [*s.domain_v().start(), *s.domain_v().end()],
    ]
}
fn trim(s: &NurbsSurface, r: Rect) -> Option<NurbsSurface> {
    s.try_trimmed(r[0][0]..=r[0][1], r[1][0]..=r[1][1]).ok()
}
fn contains(a: Rect, b: Rect) -> bool {
    (0..2).all(|i| a[i][0] <= b[i][0] && b[i][1] <= a[i][1])
}
pub(super) fn covers(parent: Rect, parts: &[Rect]) -> bool {
    if parts.is_empty() || parts.iter().any(|&r| !contains(parent, r)) {
        return false;
    }
    let cuts: [Vec<f64>; 2] = std::array::from_fn(|a| {
        let mut v = vec![parent[a][0], parent[a][1]];
        v.extend(parts.iter().flat_map(|r| r[a]));
        v.sort_by(f64::total_cmp);
        v.dedup();
        v
    });
    cuts[0].windows(2).all(|u| {
        cuts[1].windows(2).all(|v| {
            parts
                .iter()
                .any(|r| contains(*r, [[u[0], u[1]], [v[0], v[1]]]))
        })
    })
}
pub(super) fn native_rectangular(g: &Value, i: usize, s: &NurbsSurface) -> bool {
    let loops = g["faces"][i]["loops"].as_array().unwrap();
    if loops.len() != 1 {
        return false;
    }
    let r = rect(s);
    let mut segments = Vec::new();
    for t in loops[0].as_array().unwrap() {
        let c = &t["definition"];
        let pts = c["control_points"].as_array().unwrap();
        if c["degree"] != 1 || pts.len() != 2 || pts.iter().any(|p| p["weight"] != 1.) {
            return false;
        }
        let p: [f64; 2] = serde_json::from_value(pts[0]["point"].clone()).unwrap();
        let q: [f64; 2] = serde_json::from_value(pts[1]["point"].clone()).unwrap();
        segments.push([p, q]);
    }
    rectangular_loop(r, segments)
}

fn actual_rectangular(f: &BrepFace) -> bool {
    if f.loops().len() != 1 {
        return false;
    }
    let mut segments = Vec::new();
    for t in f.loops()[0].trims() {
        let c = t.curve();
        if c.degree() != 1
            || c.control_points().len() != 2
            || c.control_points().iter().any(|p| p.weight() != 1.)
        {
            return false;
        }
        segments.push(std::array::from_fn(|i| {
            c.control_points()[i].point().to_array()
        }));
    }
    rectangular_loop(rect(f.surface()), segments)
}

fn rectangular_loop(r: Rect, segments: Vec<[[f64; 2]; 2]>) -> bool {
    let mut sides: [Vec<[f64; 2]>; 4] = std::array::from_fn(|_| vec![]);
    for [p, q] in segments {
        let side = (0..4).find(|&side| {
            let axis = side % 2;
            let value = r[axis][side / 2];
            p[axis] == value && q[axis] == value
        });
        let Some(side) = side else {
            return false;
        };
        let a = 1 - side % 2;
        sides[side].push([p[a].min(q[a]), p[a].max(q[a])]);
    }
    (0..4).all(|side| {
        let a = 1 - side % 2;
        let mut p = sides[side].clone();
        p.sort_by(|x, y| x[0].total_cmp(&y[0]));
        !p.is_empty()
            && p[0][0] == r[a][0]
            && p.last().unwrap()[1] == r[a][1]
            && p.windows(2).all(|w| w[0][1] == w[1][0])
    })
}

pub fn coverage(
    actual: &Brep,
    parents: &[NurbsSurface],
    native: &Value,
    targets: &[NurbsSurface],
) -> Option<f64> {
    coverage_with(actual, parents, native, targets, &|a, b, limit| {
        a.parameter_surface_deviation_bound(b, limit).ok().flatten()
    })
}

pub fn coverage_with(
    actual: &Brep,
    parents: &[NurbsSurface],
    native: &Value,
    targets: &[NurbsSurface],
    compare: &impl Fn(&NurbsSurface, &NurbsSurface, f64) -> Option<f64>,
) -> Option<f64> {
    let mut a = vec![Vec::new(); parents.len()];
    let mut n = vec![Vec::new(); parents.len()];
    let mut regions = Vec::new();
    let mut actual_bound = 0_f64;
    let mut native_bound = 0_f64;
    for f in actual.faces() {
        let r = rect(f.surface());
        if !actual_rectangular(f) {
            return None;
        }
        let (i, b) = parents.iter().enumerate().find_map(|(i, p)| {
            if !contains(rect(p), r) {
                return None;
            }
            compare(&trim(p, r)?, f.surface(), EPS / 4.).map(|b| (i, b))
        })?;
        a[i].push(r);
        regions.push((i, r, f.surface()));
        actual_bound = actual_bound.max(b);
    }
    for (j, target) in targets.iter().enumerate() {
        if !native_rectangular(native, j, target) {
            return None;
        }
        let matched = parents
            .iter()
            .enumerate()
            .find_map(|(i, p)| compare(p, target, EPS / 4.).map(|b| (i, rect(p), b)))
            .or_else(|| {
                regions
                    .iter()
                    .find_map(|&(i, r, s)| compare(s, target, EPS / 4.).map(|b| (i, r, b)))
            })?;
        n[matched.0].push(matched.1);
        native_bound = native_bound.max(matched.2);
    }
    if !(0..parents.len())
        .all(|i| covers(rect(&parents[i]), &a[i]) && covers(rect(&parents[i]), &n[i]))
    {
        return None;
    }
    // A native patch can match an actual child rather than the raw parent.
    // Its parent discrepancy adds one more child-to-parent term.
    let bound = 2. * actual_bound + native_bound;
    Some(if bound == 0. { 0. } else { bound.next_up() })
}

pub fn incidence(actual: &Brep, g: &Value, face_map: &[usize]) -> Option<f64> {
    incidence_with(actual, g, face_map, &|a, b, limit| {
        a.parameter_curve_deviation_bound(b, limit).ok().flatten()
    })
}

pub fn incidence_with(
    actual: &Brep,
    g: &Value,
    face_map: &[usize],
    compare: &impl Fn(&NurbsCurve, &NurbsCurve, f64) -> Option<f64>,
) -> Option<f64> {
    let vertices = g["vertices"].as_array().unwrap();
    let edges = g["edges"].as_array().unwrap();
    let topology = &g["topology"];
    if actual.vertices().len() != vertices.len()
        || actual.edges().len() != edges.len()
        || actual.faces().len() != face_map.len()
    {
        return None;
    }
    let mut used = vec![false; vertices.len()];
    let mut vm = Vec::new();
    let mut maximum = 0_f64;
    for v in actual.vertices() {
        let (i, d) = vertices
            .iter()
            .enumerate()
            .filter(|(i, _)| !used[*i])
            .find_map(|(i, p)| {
                let d = v.point().distance_to(captured::point(&p["point"])).ok()?;
                (d <= EPS).then_some((i, d))
            })?;
        used[i] = true;
        vm.push(i);
        maximum = maximum.max(d);
    }
    let mut used = vec![false; edges.len()];
    let mut em = Vec::new();
    for e in actual.edges() {
        let endpoints = e.vertices().map(|v| vm[v]);
        let (i, b) = edges
            .iter()
            .enumerate()
            .filter(|(i, _)| !used[*i])
            .find_map(|(i, t)| {
                let ev: Vec<usize> = serde_json::from_value(topology["edges"][i].clone()).unwrap();
                let c = curves::curve(&t["curve"]["definition"]);
                for reverse in [false, true] {
                    let mut ev = ev.clone();
                    if reverse {
                        ev.reverse();
                    }
                    if ev != endpoints {
                        continue;
                    }
                    let c = if reverse {
                        c.reversed().ok()?
                    } else {
                        c.clone()
                    };
                    if let Some(b) = compare(e.curve(), &c, EPS) {
                        return Some((i, b));
                    }
                }
                None
            })?;
        used[i] = true;
        em.push(i);
        maximum = maximum.max(b);
    }
    for (i, f) in actual.faces().iter().enumerate() {
        let nl = topology["faces"][face_map[i]]["loops"].as_array().unwrap();
        if f.loops().len() != nl.len() {
            return None;
        }
        for (l, nl) in f.loops().iter().zip(nl) {
            if (l.loop_type() == BrepLoopType::Outer) != nl["outer"].as_bool().unwrap() {
                return None;
            }
            let mut ours = l
                .trims()
                .iter()
                .map(|t| (t.edge().map(|e| em[e]), format!("{:?}", t.trim_type())))
                .collect::<Vec<_>>();
            let mut theirs = nl["trims"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| {
                    (
                        t["edge"].as_u64().map(|i| i as usize),
                        t["type"].as_str().unwrap().to_owned(),
                    )
                })
                .collect::<Vec<_>>();
            ours.sort();
            theirs.sort();
            if ours != theirs {
                return None;
            }
        }
    }
    Some(maximum)
}
