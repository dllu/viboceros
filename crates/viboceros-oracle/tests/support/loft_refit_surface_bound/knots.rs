//! A shared knot correspondence bounds near-equal polynomial V bases cheaply.
use super::*;
use num_rational::BigRational as Rational;
use num_traits::{Signed, ToPrimitive, Zero};

fn r(x: f64) -> Rational {
    Rational::from_float(x).unwrap()
}

fn normalized(patch: &Value) -> Vec<Rational> {
    let start = r(patch["domain_v"][0].as_f64().unwrap());
    let width = r(patch["domain_v"][1].as_f64().unwrap()) - &start;
    patch["knots_v"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| (r(k.as_f64().unwrap()) - &start) / &width)
        .collect()
}

pub(super) fn bound(a: &Value, b: &Value, limit: f64) -> Result<f64, String> {
    if a["degree"][1] != b["degree"][1] || a["control_count"] != b["control_count"] {
        return Err("corresponding V layout required".into());
    }
    let ka = normalized(a);
    let kb = normalized(b);
    if ka.len() != kb.len()
        || ka
            .windows(2)
            .zip(kb.windows(2))
            .any(|(a, b)| (a[0] == a[1]) != (b[0] == b[1]))
    {
        return Err("corresponding V knot multiplicities required".into());
    }
    let delta = ka
        .iter()
        .zip(&kb)
        .map(|(a, b)| (a - b).abs())
        .max()
        .unwrap();
    let gap = ka
        .windows(2)
        .chain(kb.windows(2))
        .map(|p| &p[1] - &p[0])
        .filter(|x| x > &Rational::zero())
        .min()
        .unwrap();
    let target = b["control_points"].as_array().unwrap();
    let error = a["control_points"]
        .as_array()
        .unwrap()
        .iter()
        .zip(target)
        .map(|(a, b)| {
            (0..3)
                .map(|i| {
                    (r(a["point"][i].as_f64().unwrap()) - r(b["point"][i].as_f64().unwrap())).abs()
                })
                .sum::<Rational>()
        })
        .max()
        .unwrap();
    let diameter = (0..3)
        .map(|i| {
            let coordinates = target
                .iter()
                .map(|p| r(p["point"][i].as_f64().unwrap()))
                .collect::<Vec<_>>();
            coordinates.iter().max().unwrap() - coordinates.iter().min().unwrap()
        })
        .sum::<Rational>();
    // Match corresponding knot intervals by one monotone affine map per span.
    // In normalized charts |t-q| <= delta. At each de Boor interpolation,
    // numerator and denominator changes are <= 2*delta and the positive
    // denominator is >= gap. Since both blend weights lie in [0,1],
    // |alpha-beta| <= 4*delta/gap. Each of degree steps adds at most this
    // amount times the target control-hull diameter to the prior error.
    // Positive U basis functions extend the same V correspondence to the
    // complete patch. All quantities and the budget comparison are exact.
    let degree = a["degree"][1].as_u64().unwrap() as f64;
    let exact = error + r(4. * degree) * delta * diameter / gap;
    if exact > r(limit) {
        return Err("corresponding knot bound exceeds budget".into());
    }
    let mut upper = exact
        .to_f64()
        .ok_or("unrepresentable corresponding knot bound")?;
    if r(upper) < exact {
        upper = upper.next_up();
    }
    Ok(upper)
}
