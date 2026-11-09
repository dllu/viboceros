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
fn covers(parent: Rect, parts: &[Rect]) -> bool {
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
fn native_rectangular(g: &Value, i: usize, s: &NurbsSurface) -> bool {
    let loops = g["faces"][i]["loops"].as_array().unwrap();
    if loops.len() != 1 {
        return false;
    }
    let r = rect(s);
    let mut sides: [Vec<[f64; 2]>; 4] = std::array::from_fn(|_| vec![]);
    for t in loops[0].as_array().unwrap() {
        let c = &t["definition"];
        let pts = c["control_points"].as_array().unwrap();
        if c["degree"] != 1 || pts.len() != 2 || pts.iter().any(|p| p["weight"] != 1.) {
            return false;
        }
        let p: [f64; 2] = serde_json::from_value(pts[0]["point"].clone()).unwrap();
        let q: [f64; 2] = serde_json::from_value(pts[1]["point"].clone()).unwrap();
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
    let mut a = vec![Vec::new(); parents.len()];
    let mut n = vec![Vec::new(); parents.len()];
    let mut regions = Vec::new();
    let mut actual_bound = 0_f64;
    let mut native_bound = 0_f64;
    for f in actual.faces() {
        let r = rect(f.surface());
        if f.rectangular_trim_bounds(captured::model_tolerance())
            .ok()??
            != r
        {
            return None;
        }
        let (i, b) = parents.iter().enumerate().find_map(|(i, p)| {
            if !contains(rect(p), r) {
                return None;
            }
            trim(p, r)?
                .parameter_surface_deviation_bound(f.surface(), EPS / 4.)
                .ok()?
                .map(|b| (i, b))
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
            .find_map(|(i, p)| {
                p.parameter_surface_deviation_bound(target, EPS / 4.)
                    .ok()?
                    .map(|b| (i, rect(p), b))
            })
            .or_else(|| {
                regions.iter().find_map(|&(i, r, s)| {
                    s.parameter_surface_deviation_bound(target, EPS / 4.)
                        .ok()?
                        .map(|b| (i, r, b))
                })
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
                    if let Some(b) = e.curve().parameter_curve_deviation_bound(&c, EPS).ok()? {
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

#[test]
fn incomplete_parameter_coverage_is_rejected() {
    let parent = [[0., 1.], [0., 1.]];
    assert!(covers(
        parent,
        &[[[0., 0.5], [0., 1.]], [[0.5, 1.], [0., 1.]]]
    ));
    assert!(!covers(
        parent,
        &[[[0., 0.5], [0., 1.]], [[0.5, 1.], [0., 0.75]]]
    ));
}

#[test]
fn a_missing_native_trim_or_wrong_edge_incidence_cannot_be_qualified() {
    let n = curves::read("ribbon_ruled");
    let value = &n["results"][0]["value"];
    let output = value["created"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["source"] == false)
        .unwrap();
    let original = &output["brep"]["geometry"];
    let surface = captured::surface(&output["surfaces"][0]);
    assert!(native_rectangular(original, 0, &surface));
    let mut changed = original.clone();
    changed["faces"][0]["loops"][0]
        .as_array_mut()
        .unwrap()
        .pop();
    assert!(!native_rectangular(&changed, 0, &surface));
    let source = curves::curve(&value["before"][0]["curve"]);
    let normal = Vector3::try_new(0., 0., 1.)
        .unwrap()
        .normalized_nonzero()
        .unwrap();
    let (actual, _) = build(&source, 0.5, false, normal, captured::model_tolerance()).unwrap();
    assert!(incidence(&actual, original, &[0]).is_some());
    let mut changed = original.clone();
    changed["topology"]["edges"][0] = json!([0, 0]);
    assert!(incidence(&actual, &changed, &[0]).is_none());
}
