//! Prescribed common-parameter families for the network fitting stage.
use super::*;

/// Construct from two already oriented polynomial families in a shared unit
/// parameter chart. `u_stations` locate the V curves on each U curve and vice
/// versa; both station lists cover 0..=1 in strictly increasing order. Every
/// prescribed contact must agree within the smaller model tolerance. Complete
/// original curves must certify at their prescribed isoparameters; no general
/// pullback can substitute another correspondence. Source bases are made
/// compatible without inserting alignment knots for an identity mapping.
///
/// This is a construction primitive, not NetworkSrf fitting. Inputs must have
/// unit domains and constant common-sign weights. Family topology, smoothness,
/// injectivity and interior fitting parity are not certified by this API.
pub fn prepared_gordon_surface(
    u_curves: &[NurbsCurve],
    v_curves: &[NurbsCurve],
    u_stations: &[f64],
    v_stations: &[f64],
    edge: Tolerance,
    interior: Tolerance,
) -> Result<NurbsSurface, Error> {
    let stations_valid = |stations: &[f64], count: usize| {
        stations.len() == count
            && stations.first() == Some(&0.)
            && stations.last() == Some(&1.)
            && stations.iter().all(|t| t.is_finite())
            && stations.windows(2).all(|p| p[0] < p[1])
    };
    if !(2..=32).contains(&u_curves.len())
        || !(2..=32).contains(&v_curves.len())
        || !stations_valid(u_stations, v_curves.len())
        || !stations_valid(v_stations, u_curves.len())
        || u_curves
            .iter()
            .chain(v_curves)
            .map(|c| c.control_points().len())
            .sum::<usize>()
            > 2048
    {
        return Err(failure(
            "invalid prepared families, stations or control budget",
        ));
    }
    for curve in u_curves.iter().chain(v_curves) {
        if curve.domain() != (0. ..=1.)
            || curve
                .control_points()
                .iter()
                .any(|p| p.weight() != curve.control_points()[0].weight())
        {
            return Err(failure(
                "prepared families require polynomial curves with unit domains",
            ));
        }
    }
    let curves = u_curves.iter().chain(v_curves).cloned().collect::<Vec<_>>();
    let working = prepare(&curves, edge, interior)?;
    for (j, u_curve) in working.local[..u_curves.len()].iter().enumerate() {
        for (i, v_curve) in working.local[u_curves.len()..].iter().enumerate() {
            if u_curve
                .evaluate(u_stations[i])?
                .distance_to(v_curve.evaluate(v_stations[j])?)?
                > edge.absolute().min(interior.absolute()) / working.scale
            {
                return Err(failure("prepared curves miss a prescribed contact"));
            }
        }
    }
    let parameters = (0..curves.len())
        .map(|i| {
            if i < u_curves.len() {
                u_stations.to_vec()
            } else {
                v_stations.to_vec()
            }
        })
        .collect();
    construct(
        &curves,
        working,
        Layout {
            families: [
                (0..u_curves.len()).collect(),
                (u_curves.len()..curves.len()).collect(),
            ],
            parameters,
            reversed: vec![false; curves.len()],
            strict: true,
        },
        edge,
        interior,
    )
}
