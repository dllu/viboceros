//! Explicit final-capture partition witness, without claiming equal topology.
use viboceros_geometry::{Brep, Tolerance};

pub(super) fn error(a: &Brep, b: &Brep, t: Tolerance, gate: f64) -> f64 {
    assert_eq!(
        (a.faces().len(), a.edges().len(), a.vertices().len()),
        (4, 12, 9)
    );
    assert_eq!(
        (b.faces().len(), b.edges().len(), b.vertices().len()),
        (3, 10, 8)
    );
    assert!(!a.is_solid() && !b.is_solid());
    for face in a.faces() {
        let s = face.surface();
        let (u, v) = (
            s.parameter_at_u(0.5).unwrap(),
            s.parameter_at_v(0.5).unwrap(),
        );
        let matches = b
            .faces()
            .iter()
            .filter(|f| f.contains_parameters(u, v, t).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(matches.len(), 1);
        let other = matches[0];
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
        assert!(n.dot(m).unwrap() > 0.);
    }
    let mut error = 0_f64;
    for (a, b) in [(a, b), (b, a)] {
        let boundary = b.edge_use_counts();
        let own_boundary = a.edge_use_counts();
        for (index, edge) in a.edges().iter().enumerate() {
            let (face, trim) = a
                .faces()
                .iter()
                .find_map(|f| {
                    f.loops()
                        .iter()
                        .flat_map(|l| l.trims())
                        .find(|tr| tr.edge() == Some(index))
                        .map(|tr| (f, tr))
                })
                .unwrap();
            for i in 0..=16 {
                let fraction = i as f64 / 16.;
                let parameter = edge.curve().parameter_at(fraction).unwrap();
                let point = edge.curve().evaluate(parameter).unwrap();
                let uv = trim
                    .curve()
                    .evaluate(
                        trim.curve()
                            .parameter_at(if trim.is_reversed_3d() {
                                1. - fraction
                            } else {
                                fraction
                            })
                            .unwrap(),
                    )
                    .unwrap();
                let s = face.surface();
                let fu =
                    (uv.x() - s.domain_u().start()) / (s.domain_u().end() - s.domain_u().start());
                let fv =
                    (uv.y() - s.domain_v().start()) / (s.domain_v().end() - s.domain_v().start());
                let mut bound =
                    super::super::chart_candidate(point, b, fu, fv, (uv.x(), uv.y()), t, gate);
                if bound > gate {
                    bound = bound.min(super::super::distance(point, b, t, gate));
                }
                error = error.max(bound);
                if own_boundary[index] != 1 {
                    continue;
                }
                let mut boundary_bound = f64::INFINITY;
                for (_, other) in b
                    .edges()
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| boundary[*j] == 1)
                {
                    let c = other.curve();
                    if c.domain().contains(&parameter) {
                        boundary_bound = boundary_bound
                            .min(point.distance_to(c.evaluate(parameter).unwrap()).unwrap());
                    }
                }
                if boundary_bound > gate {
                    for (j, other) in b.edges().iter().enumerate() {
                        if boundary[j] != 1 {
                            continue;
                        }
                        let c = other.curve();
                        boundary_bound = boundary_bound.min(
                            point
                                .distance_to(
                                    c.evaluate(c.closest_parameter(point, t).unwrap()).unwrap(),
                                )
                                .unwrap(),
                        );
                    }
                }
                error = error.max(boundary_bound);
            }
        }
    }
    error
}
