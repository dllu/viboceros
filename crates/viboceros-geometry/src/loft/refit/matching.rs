//! Match certified polygon regions before constructing the shared cubic basis.
use super::*;

pub(super) fn prepare(
    sections: Vec<Section>,
    tolerance: Real,
    angle: Option<Real>,
) -> Result<Vec<NurbsCurve>, GeometryError> {
    if sections.iter().all(|s| s.vertices.len() == 2) {
        return Ok(sections.into_iter().map(|s| s.curve).collect());
    }
    // Only these degree-one proxies propose the region chart. They do not
    // replace the exact source vertices in the final construction/proof.
    let proxies = sections
        .iter()
        .map(|section| {
            let points = section
                .curve
                .control_points()
                .iter()
                .step_by(3)
                .map(|c| c.point())
                .collect::<Vec<_>>();
            let mut knots = vec![0., 0.];
            let mut total = 0.;
            for pair in points.windows(2) {
                let next = total + pair[0].distance_to(pair[1])?;
                if !next.is_finite() || next <= total {
                    return Err(GeometryError::UnsupportedCurveApproximation {
                        context: "Loft Refit region lengths are not distinguishable",
                    });
                }
                total = next;
                knots.push(total);
            }
            knots.push(total);
            NurbsCurve::try_new(1, points, knots)
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    let matched = super::super::parameterize::prepare_refit::<GeometryError>(
        &proxies,
        Some(corner_angle(angle)),
        &mut |c| Ok(c.clone()),
    )?;
    let mut cuts = matched
        .iter()
        .flat_map(|c| c.knots().iter().copied())
        .collect::<Vec<_>>();
    cuts.sort_by(Real::total_cmp);
    cuts.dedup();
    if 3 * (cuts.len() - 1) + 1 > MAX_LOFT_SECTION_CONTROLS {
        return Err(GeometryError::LoftResourceLimit {
            context: "Refit compatible section controls",
            maximum: MAX_LOFT_SECTION_CONTROLS,
        });
    }
    let mut knots = vec![cuts[0]; 4];
    for &cut in &cuts[1..cuts.len() - 1] {
        knots.extend([cut; 3]);
    }
    knots.extend([*cuts.last().unwrap(); 4]);
    sections
        .iter()
        .zip(matched.iter())
        .map(|(source, proxy)| {
            let mut local = proxy.knots().to_vec();
            local.dedup();
            if local.len() != source.vertices.len() {
                return Err(GeometryError::InvalidControlNet {
                    context: "Refit region chart changed vertex count",
                });
            }
            let mut controls = Vec::with_capacity(3 * (cuts.len() - 1) + 1);
            let mut region = 0;
            for pair in cuts.windows(2) {
                while region + 2 < local.len() && local[region + 1] <= pair[0] {
                    region += 1;
                }
                let a = rational(local[region]);
                let width = rational(local[region + 1]) - &a;
                let delta = std::array::from_fn::<_, 3, _>(|i| {
                    &source.vertices[region + 1][i] - &source.vertices[region][i]
                });
                let endpoints = [pair[0], pair[1]].map(|t| {
                    let fraction = (rational(t) - &a) / &width;
                    std::array::from_fn(|i| &source.vertices[region][i] + &fraction * &delta[i])
                });
                // Direct construction avoids rounded knot-insertion drift. The
                // two exact endpoints cover a subsegment of the original image;
                // the shared rounding check certifies every generated cubic.
                let segment = cubic_controls(&endpoints[0], &endpoints[1], tolerance)?;
                controls.extend(segment.into_iter().skip(usize::from(!controls.is_empty())));
            }
            NurbsCurve::try_new_rational(3, controls, knots.clone())
        })
        .collect()
}
