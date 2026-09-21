//! Line proposals share the edge merge's displacement and work accounting.
use super::*;

#[cfg(test)]
mod tests;

impl State {
    pub(super) fn simplify_linear_edges(
        &mut self,
        tolerance: Tolerance,
        budget: &mut Budget,
    ) -> Result<bool, GeometryError> {
        let mut changed = false;
        for index in 0..self.edges.len() {
            let Some(edge) = &self.edges[index] else {
                continue;
            };
            let allowance = if edge.displacement == 0. {
                tolerance.absolute()
            } else {
                (tolerance.absolute() - edge.displacement)
                    .next_down()
                    .max(0.)
            };
            let Some((curve, displacement)) =
                line(&edge.geometry.curve, allowance, tolerance, budget)?
            else {
                continue;
            };
            let total = certificate::add_bound(edge.displacement, displacement)?;
            if total > tolerance.absolute() {
                continue;
            }
            let length = *curve.domain().end();
            let mut trims = Vec::with_capacity(edge.uses.len());
            for &index in &edge.uses {
                let old = &self.trim(index).geometry;
                budget.charge(old.curve.control_points().len())?;
                // Exact positive-basis collinearity proves identical UV loci
                // without assuming that UV distance is a model-space distance.
                let lifted = NurbsCurve::try_new_rational(
                    old.curve.degree(),
                    old.curve
                        .control_points()
                        .iter()
                        .map(|p| {
                            WeightedPoint3::try_new(
                                Point3::try_new(p.point().x(), p.point().y(), 0.)?,
                                p.weight(),
                            )
                        })
                        .collect::<Result<_, GeometryError>>()?,
                    old.curve.knots().to_vec(),
                )?;
                if let Some([a, b]) = certificate::linear_endpoints(&lifted) {
                    let replacement = NurbsCurve2::try_new(
                        1,
                        vec![
                            Point2::try_new(a.x(), a.y())?,
                            Point2::try_new(b.x(), b.y())?,
                        ],
                        vec![0., 0., length, length],
                    )?;
                    if replacement != old.curve {
                        trims.push((index, replacement));
                    }
                }
            }
            let geometry_changed = curve != edge.geometry.curve;
            if !geometry_changed && trims.is_empty() {
                continue;
            }
            let edge = self.edges[index].as_mut().unwrap();
            if geometry_changed {
                edge.geometry.curve = curve;
                edge.displacement = total;
                if displacement > 0. {
                    edge.geometry.tolerance =
                        certificate::add_bound(edge.geometry.tolerance, displacement)?
                            .max(tolerance.absolute());
                }
            }
            for (index, curve) in trims {
                self.trims[index].as_mut().unwrap().geometry.curve = curve;
            }
            changed = true;
        }
        Ok(changed)
    }
}

fn line(
    source: &NurbsCurve,
    allowance: Real,
    tolerance: Tolerance,
    budget: &mut Budget,
) -> Result<Option<(NurbsCurve, Real)>, GeometryError> {
    budget.charge(source.control_points().len())?;
    let exact = certificate::linear_endpoints(source);
    if exact.is_none() && (source.degree() > 16 || !source.is_linear(tolerance)?) {
        return Ok(None);
    }
    // is_linear requires clamped ends. Its tolerance-based control test is
    // merely a proposal; mixed weights, reversals and parameterization changes
    // still have to pass the exact whole-curve certificate below.
    let controls = source.control_points();
    let a = controls[0].point();
    let b = controls[controls.len() - 1].point();
    let Ok(length) = a.distance_to(b) else {
        return Ok(None);
    };
    if length == 0. {
        return Ok(None);
    }
    let candidate = NurbsCurve::try_new(1, vec![a, b], vec![0., 0., length, length])?;
    let Some(displacement) =
        certificate::whole_curve_bound(source, &candidate, false, allowance, |n| budget.charge(n))?
    else {
        return Ok(None);
    };
    Ok(Some((candidate, displacement)))
}
