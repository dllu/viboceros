//! Finite tolerance checks; native output coefficients are expectations only.
use super::{brep_comparison, inputs, loose, native};
use serde_json::Value;
use viboceros_document::{Document, Geometry};
use viboceros_geometry::{Brep, BrepFace, Point3, Tolerance};

pub fn rows() -> Vec<(String, Value)> {
    [
        "offset_srf_tolerance_pilot",
        "offset_srf_tolerance_holdouts",
        "offset_srf_tolerance_final",
    ]
    .into_iter()
    .flat_map(|family| {
        native::read(family)["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| (r["id"].as_str().unwrap().to_owned(), r["value"].clone()))
            .collect::<Vec<_>>()
    })
    .filter(|(id, _)| id != "offset_tp_getter_cancel" && id != "offset_tp_getter_keep")
    .collect()
}
pub fn curved(v: &Value) -> bool {
    v["prescribed_case"]["curves"][0]["type"] != "normal_surface"
}
pub fn trimmed_rows() -> Vec<(String, Value)> {
    ["offset_srf_trims", "offset_srf_trims_final"]
        .into_iter()
        .flat_map(|family| {
            native::read(family)["results"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|r| r["id"] != "offset_te_delete_postselect")
                .map(|r| (r["id"].as_str().unwrap().to_owned(), r["value"].clone()))
                .collect::<Vec<_>>()
        })
        .collect()
}
pub fn setup(v: &Value) -> Document {
    if curved(v) {
        loose::setup(v)
    } else {
        inputs::setup(v)
    }
}
pub fn supported(id: &str, v: &Value) -> bool {
    if !inputs::success(v) {
        return true;
    }
    if cfg!(feature = "native-smlib") {
        id != "offset_th_rational"
    } else {
        !curved(v)
    }
}
pub fn options(v: &Value) -> viboceros_command::offset_surface::Options {
    let text = inputs::command(v);
    viboceros_command::offset_surface::parse(
        &text.split_whitespace().skip(1).collect::<Vec<_>>(),
        inputs::initial(v),
        &viboceros_geometry::LengthUnitSystem::Millimeters,
    )
    .unwrap()
    .options
}
pub fn epsilon(v: &Value) -> f64 {
    if !curved(v) {
        return 1e-6;
    }
    // Finite native refits can exceed their requested fit tolerance. Keep a
    // 1e-5 model-unit comparison floor; test our requested fit separately below.
    (2. * options(v).effective_tolerance(setup(v).tolerance())).max(1e-5)
}
pub fn reference_tolerance(v: &Value) -> f64 {
    let t = setup(v).tolerance();
    t.absolute().max(options(v).effective_tolerance(t))
}
pub fn reference(r: &Value, t: Tolerance, requested: f64) -> Brep {
    // Rhino's fitted edge/UV pairs can meet the command tolerance while
    // exceeding document tolerance. Only reference admission uses this budget.
    let native =
        Tolerance::try_new(t.absolute().max(requested), t.relative(), t.angular()).unwrap();
    brep_comparison::captured::expected(r, native)
}
pub fn check(d: &Document, rows: &Value, archive: bool, epsilon: f64, reference_fit: f64) -> f64 {
    loose::check_with(d, rows, archive, epsilon, |b, r, t| {
        if r["guide"] == true {
            let error = brep_comparison::geometry::error(b, r, t);
            assert!(error <= 1e-6, "source error {error:e}");
            return error;
        }
        let expected = reference(r, t, reference_fit);
        assert_eq!(b.is_solid(), expected.is_solid());
        assert_eq!(
            b.solid_orientation().unwrap(),
            expected.solid_orientation().unwrap()
        );
        directed(b, &expected, t, epsilon).max(directed(&expected, b, t, epsilon))
    })
}
fn stations(face: &BrepFace, t: Tolerance) -> Vec<(f64, f64)> {
    let s = face.surface();
    let mut result = Vec::new();
    for i in 0..=8 {
        for j in 0..=8 {
            let u = s.parameter_at_u(i as f64 / 8.).unwrap();
            let v = s.parameter_at_v(j as f64 / 8.).unwrap();
            if face.contains_parameters(u, v, t).unwrap() {
                result.push((u, v));
            }
        }
    }
    for trim in face.loops().iter().flat_map(|l| l.trims()) {
        let c = trim.curve();
        let d = c.domain();
        for i in 0..=16 {
            let uv = c
                .evaluate(*d.start() + (*d.end() - *d.start()) * i as f64 / 16.)
                .unwrap();
            result.push((
                uv.x().clamp(*s.domain_u().start(), *s.domain_u().end()),
                uv.y().clamp(*s.domain_v().start(), *s.domain_v().end()),
            ));
        }
    }
    assert!(!result.is_empty());
    result
}
fn distance(p: Point3, b: &Brep, _t: Tolerance, gate: f64) -> f64 {
    let t = Tolerance::try_new(1e-10, 1e-12, 1e-9).unwrap();
    let mut best = f64::INFINITY;
    for (i, f) in b.faces().iter().enumerate() {
        let (u, v) = b.closest_parameters_on_face(i, p, t).unwrap().unwrap();
        best = best.min(p.distance_to(f.surface().evaluate(u, v).unwrap()).unwrap());
        if best <= gate {
            break;
        }
    }
    best
}
fn chart_candidate(
    p: Point3,
    b: &Brep,
    fu: f64,
    fv: f64,
    original_uv: (f64, f64),
    t: Tolerance,
    gate: f64,
) -> f64 {
    let mut best = f64::INFINITY;
    // Chart swaps/reversals are only proposals. Any accepted point must lie
    // inside the actual face trims and supplies a valid distance upper bound.
    for f in b.faces() {
        // Trimmed supports can retain their original parameter clock while
        // changing the active domain. Try that clock before closest-point work.
        let s = f.surface();
        for (u, v) in [original_uv, (original_uv.1, original_uv.0)] {
            if s.domain_u().contains(&u)
                && s.domain_v().contains(&v)
                && f.contains_parameters(u, v, t).unwrap()
            {
                best = best.min(p.distance_to(s.evaluate(u, v).unwrap()).unwrap());
                if best <= gate {
                    return best;
                }
            }
        }
        for (u, v) in [
            (fu, fv),
            (fv, fu),
            (1. - fu, fv),
            (fu, 1. - fv),
            (1. - fu, 1. - fv),
            (1. - fv, fu),
            (fv, 1. - fu),
            (1. - fv, 1. - fu),
        ] {
            let s = f.surface();
            let u = s.parameter_at_u(u).unwrap();
            let v = s.parameter_at_v(v).unwrap();
            if f.contains_parameters(u, v, t).unwrap() {
                best = best.min(p.distance_to(s.evaluate(u, v).unwrap()).unwrap());
                if best <= gate {
                    return best;
                }
            }
        }
    }
    best
}
fn directed(a: &Brep, b: &Brep, t: Tolerance, gate: f64) -> f64 {
    let mut error = 0_f64;
    for face in a.faces() {
        let s = face.surface();
        for (u, v) in stations(face, t) {
            let p = s.evaluate(u, v).unwrap();
            // A valid chart candidate supplies an upper bound. Fall back to a
            // trim-aware closest point when charts differ or it exceeds gate.
            let fu = (u - *s.domain_u().start()) / (*s.domain_u().end() - *s.domain_u().start());
            let fv = (v - *s.domain_v().start()) / (*s.domain_v().end() - *s.domain_v().start());
            let candidate = chart_candidate(p, b, fu, fv, (u, v), t, gate);
            let cost = if candidate <= gate {
                candidate
            } else {
                candidate.min(distance(p, b, t, gate))
            };
            assert!(cost.is_finite());
            error = error.max(cost);
        }
    }
    error
}
pub fn field_error(d: &Document, v: &Value) -> (f64, f64) {
    if !curved(v) {
        return (0., 0.);
    }
    let source = setup(v);
    let o = options(v);
    let t = d.tolerance();
    let brep = |g: &Geometry| match g {
        Geometry::Brep(b) => b.clone(),
        Geometry::NurbsSurface(s) => Brep::try_surface_face(s.clone(), t).unwrap(),
        _ => panic!("surface"),
    };
    let outputs = d
        .objects()
        .filter(|r| r.attributes().name().is_none())
        .map(|r| brep(r.geometry()))
        .collect::<Vec<_>>();
    assert!(!outputs.is_empty());
    let expected = v["after"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["guide"] != true && r["source"] != true)
        .map(|r| reference(r, t, o.effective_tolerance(t)))
        .collect::<Vec<_>>();
    assert!(!expected.is_empty());
    let mut native_error = 0_f64;
    let mut error = 0_f64;
    for obj in source.objects().filter(|r| {
        r.attributes()
            .name()
            .is_some_and(|n| n.starts_with("guide"))
    }) {
        let b = brep(obj.geometry());
        for f in b.faces() {
            for (u, v) in stations(f, t) {
                let s = f.surface();
                let p = s.evaluate(u, v).unwrap();
                let n = s.normal_at(u, v).unwrap().as_vector();
                let fu =
                    (u - *s.domain_u().start()) / (*s.domain_u().end() - *s.domain_u().start());
                let fv =
                    (v - *s.domain_v().start()) / (*s.domain_v().end() - *s.domain_v().start());
                let signs = if o.both_sides {
                    vec![1., -1.]
                } else {
                    vec![1.]
                };
                for sign in signs {
                    let q = p
                        .translated(
                            n.scaled(o.distance * sign * if f.is_reversed() { -1. } else { 1. })
                                .unwrap(),
                        )
                        .unwrap();
                    let distance = |b: &Brep| {
                        let candidate =
                            chart_candidate(q, b, fu, fv, (u, v), t, o.effective_tolerance(t));
                        if candidate <= o.effective_tolerance(t) {
                            candidate
                        } else {
                            candidate.min(distance(q, b, t, o.effective_tolerance(t)))
                        }
                    };
                    let cost = outputs.iter().map(&distance).fold(f64::INFINITY, f64::min);
                    error = error.max(cost);
                    let cost = expected.iter().map(distance).fold(f64::INFINITY, f64::min);
                    assert!(cost.is_finite());
                    native_error = native_error.max(cost);
                }
            }
        }
    }
    let bound = o.effective_tolerance(t) + 1e-10;
    assert!(
        error <= bound,
        "sampled true offset error {error:e}, requested {bound:e}"
    );
    (error, native_error)
}
