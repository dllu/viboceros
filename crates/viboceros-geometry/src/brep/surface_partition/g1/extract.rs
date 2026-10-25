//! Local homogeneous extraction shares the existing Bezier span evaluator.
use super::*;
use crate::bezier::{extract_homogeneous_span, polynomial::product};
type H = [Real; 4];

pub(super) fn seam(
    surface: &NurbsSurface,
    axis: usize,
    spans: [usize; 2],
    transverse: usize,
    budget: &mut usize,
) -> Result<Seam, GeometryError> {
    let degrees = [surface.degree_u(), surface.degree_v()];
    let knots = [surface.knots_u(), surface.knots_v()];
    let (p, q) = (degrees[axis], degrees[1 - axis]);
    charge(
        budget,
        2usize
            .saturating_mul(p + 1)
            .saturating_mul(q + 1)
            .saturating_mul((p + 1).saturating_pow(2) + (q + 1).saturating_pow(2)),
    )?;
    let patches = spans.map(|span| {
        (transverse - q..=transverse)
            .flat_map(|j| {
                (span - p..=span).map(move |i| {
                    if axis == 0 {
                        surface.control_point(i, j).unwrap()
                    } else {
                        surface.control_point(j, i).unwrap()
                    }
                })
            })
            .collect::<Vec<_>>()
    });
    let candidate = patches[0][0].point().to_array();
    let origin = if patches.iter().flatten().all(|c| {
        c.point()
            .to_array()
            .into_iter()
            .zip(candidate)
            .all(|(a, b)| (a - b).is_finite())
    }) {
        candidate
    } else {
        [0.; 3]
    };
    let scale = patches
        .iter()
        .flatten()
        .flat_map(|c| {
            c.point()
                .to_array()
                .into_iter()
                .zip(origin)
                .map(|(a, b)| (a - b).abs())
        })
        .fold(0., Real::max);
    let scale = if scale == 0. { 1. } else { scale };
    let gauge = patches
        .iter()
        .flatten()
        .map(|c| c.weight().abs())
        .fold(0., Real::max);
    let mut rows = Vec::new();
    for (side, (span, patch)) in spans.into_iter().zip(patches).enumerate() {
        let mut controls = patch
            .into_iter()
            .map(|c| {
                let w = c.weight() / gauge;
                let point = c.point().to_array();
                [
                    (point[0] - origin[0]) / scale * w,
                    (point[1] - origin[1]) / scale * w,
                    (point[2] - origin[2]) / scale * w,
                    w,
                ]
            })
            .collect::<Vec<H>>();
        for row in controls.chunks_exact_mut(p + 1) {
            row.copy_from_slice(&extract_homogeneous_span(p, knots[axis], span, row)?);
        }
        let columns = if side == 0 { [p - 1, p] } else { [0, 1] };
        for column in columns {
            let source = (0..=q)
                .map(|j| controls[j * (p + 1) + column])
                .collect::<Vec<_>>();
            rows.push(extract_homogeneous_span(
                q,
                knots[1 - axis],
                transverse,
                &source,
            )?);
        }
    }
    let directions = [
        difference(&rows[1], &rows[0], budget)?,
        difference(&rows[3], &rows[2], budget)?,
    ];
    if directions
        .iter()
        .flatten()
        .flatten()
        .any(|x| !x.is_finite())
    {
        return unresolved();
    }
    Ok(Seam {
        directions,
        weights: rows[1].iter().map(|h| [h[3]]).collect(),
        depth: 0,
    })
}

// At a clamped side, the positive scalar p/span cancels from direction.
// (Q-P)'s rational numerator is Q.xyz*P.w-P.xyz*Q.w, all in the transverse basis.
fn difference(q: &[H], p: &[H], budget: &mut usize) -> Result<Vec<[Real; 3]>, GeometryError> {
    let pw = p.iter().map(|h| h[3]).collect::<Vec<_>>();
    let qw = q.iter().map(|h| h[3]).collect::<Vec<_>>();
    let mut output = vec![[0.; 3]; q.len() + p.len() - 1];
    for axis in 0..3 {
        let a = product(
            &q.iter().map(|h| h[axis]).collect::<Vec<_>>(),
            &pw,
            &mut |n| charge(budget, n),
        )?;
        let b = product(
            &p.iter().map(|h| h[axis]).collect::<Vec<_>>(),
            &qw,
            &mut |n| charge(budget, n),
        )?;
        for (control, (a, b)) in output.iter_mut().zip(a.into_iter().zip(b)) {
            control[axis] = a - b;
        }
    }
    Ok(output)
}
