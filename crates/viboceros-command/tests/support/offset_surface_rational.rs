//! Finite spatial-edge and tensor-chart topology checks for rational offsets.
use viboceros_geometry::{Brep, BrepFace, NurbsCurve, Tolerance};
#[path = "offset_surface_rational_partition.rs"]
mod partition;

pub fn partition_difference(a: &Brep, b: &Brep, t: Tolerance, gate: f64) -> f64 {
    partition::error(a, b, t, gate)
}

fn curve_error(a: &NurbsCurve, b: &NurbsCurve, t: Tolerance, gate: f64) -> f64 {
    let direct = [false, true]
        .into_iter()
        .map(|reversed| {
            (0..=16)
                .map(|i| {
                    let fraction = i as f64 / 16.;
                    a.evaluate(a.parameter_at(fraction).unwrap())
                        .unwrap()
                        .distance_to(
                            b.evaluate(
                                b.parameter_at(if reversed { 1. - fraction } else { fraction })
                                    .unwrap(),
                            )
                            .unwrap(),
                        )
                        .unwrap()
                })
                .fold(0_f64, f64::max)
        })
        .fold(f64::INFINITY, f64::min);
    if direct <= gate {
        return direct;
    }
    (0..=16)
        .map(|i| {
            let p = a.evaluate(a.parameter_at(i as f64 / 16.).unwrap()).unwrap();
            let parameter = b.closest_parameter(p, t).unwrap();
            p.distance_to(b.evaluate(parameter).unwrap()).unwrap()
        })
        .fold(0_f64, f64::max)
}

pub fn topology_and_edges(a: &Brep, b: &Brep, t: Tolerance, gate: f64) -> f64 {
    assert_eq!(a.is_solid(), b.is_solid());
    assert_eq!(a.faces().len(), b.faces().len());
    assert_eq!(a.edges().len(), b.edges().len());
    assert_eq!(a.vertices().len(), b.vertices().len());
    let counts = |b: &Brep| {
        let mut uses = b.edge_use_counts();
        uses.sort_unstable();
        uses
    };
    assert_eq!(counts(a), counts(b));
    let mut error = 0_f64;
    for face in a.faces() {
        let s = face.surface();
        let matching = b
            .faces()
            .iter()
            .filter(|f| {
                f.surface().domain_u() == s.domain_u() && f.surface().domain_v() == s.domain_v()
            })
            .collect::<Vec<_>>();
        assert_eq!(matching.len(), 1, "one matching inherited native chart");
        let other = matching[0];
        let kinds = |f: &BrepFace| {
            f.loops()
                .iter()
                .map(|boundary| {
                    let mut trims = boundary
                        .trims()
                        .iter()
                        .map(|tr| format!("{:?}", tr.trim_type()))
                        .collect::<Vec<_>>();
                    trims.sort();
                    (format!("{:?}", boundary.loop_type()), trims)
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(kinds(face), kinds(other));
        let u = s.parameter_at_u(0.5).unwrap();
        let v = s.parameter_at_v(0.5).unwrap();
        let n = s
            .normal_at(u, v)
            .unwrap()
            .as_vector()
            .scaled(if face.is_reversed() { -1. } else { 1. })
            .unwrap();
        let m = other
            .surface()
            .normal_at(u, v)
            .unwrap()
            .as_vector()
            .scaled(if other.is_reversed() { -1. } else { 1. })
            .unwrap();
        // Native refits need not have identical derivatives. Check the sense
        // separately from the model-space positional tolerance.
        assert!(n.dot(m).unwrap() > 0., "native face orientation");
    }
    let mut available = (0..b.edges().len()).collect::<Vec<_>>();
    for edge in a.edges() {
        let curve = edge.curve();
        let ends = [
            curve.evaluate(*curve.domain().start()).unwrap(),
            curve.evaluate(*curve.domain().end()).unwrap(),
        ];
        let matching = available
            .iter()
            .enumerate()
            .filter(|&(_, &i)| {
                let c = b.edges()[i].curve();
                let q = [
                    c.evaluate(*c.domain().start()).unwrap(),
                    c.evaluate(*c.domain().end()).unwrap(),
                ];
                [false, true].into_iter().any(|reversed| {
                    ends[0].distance_to(q[usize::from(reversed)]).unwrap() <= gate
                        && ends[1].distance_to(q[usize::from(!reversed)]).unwrap() <= gate
                })
            })
            .map(|(slot, &i)| (slot, i))
            .collect::<Vec<_>>();
        assert_eq!(
            matching.len(),
            1,
            "one native edge with corresponding endpoints"
        );
        let (slot, i) = matching[0];
        available.remove(slot);
        let native = b.edges()[i].curve();
        error = error
            .max(curve_error(curve, native, t, gate))
            .max(curve_error(native, curve, t, gate));
    }
    for (a, b) in [(a, b), (b, a)] {
        for vertex in a.vertices() {
            error = error.max(
                b.vertices()
                    .iter()
                    .map(|other| vertex.point().distance_to(other.point()).unwrap())
                    .fold(f64::INFINITY, f64::min),
            );
        }
    }
    error
}
