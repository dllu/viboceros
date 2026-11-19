//! Select source parameters before offset fitting changes the curve's basis.
use super::*;
use viboceros_geometry::{CurveContinuity, CurveRef, ParameterSide};
use viboceros_smlib::{Error, trim_curve};

const ANGLE: f64 = 0.1_f64.to_radians();

pub(super) fn for_sweep(
    source: &NurbsCurve,
    displacement: Vector3,
    draft: f64,
    split_at_tangents: bool,
    tolerance: Tolerance,
) -> Result<Vec<f64>, Error> {
    let curvature_breaks = source.discontinuity_parameters(CurveContinuity::G2, ANGLE)?;
    if split_at_tangents || curvature_breaks.is_empty() {
        return Ok(curvature_breaks);
    }
    let corners = source.discontinuity_parameters(CurveContinuity::G1, ANGLE)?;
    if corners == curvature_breaks {
        return Ok(corners);
    }
    let domain = source.domain();
    let bounds = std::iter::once(*domain.start())
        .chain(curvature_breaks.iter().copied())
        .chain([*domain.end()])
        .collect::<Vec<_>>();
    let analytic = bounds
        .windows(2)
        .map(|span| {
            let piece = trim_curve(source, span[0]..=span[1])?;
            Ok::<_, Error>(analytic_offset(&piece, tolerance)?)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let weights = weight_polynomial(source)?;
    let axis = displacement.normalized_nonzero()?.as_vector();
    let normal = if source.is_closed()?
        && CurveRef::NurbsCurve(source)
            .supporting_plane_normal(tolerance)?
            .map(|normal| normal.as_vector().dot(axis))
            .transpose()?
            .is_some_and(|dot| dot < 0.)
    {
        axis.scaled(-1.)?
    } else {
        axis
    };
    let sweep = Sweep {
        displacement,
        normal,
        offset: displacement.length()? * draft.to_radians().tan(),
    };
    let mut cuts = Vec::with_capacity(curvature_breaks.len());
    for (&parameter, kinds) in curvature_breaks.iter().zip(analytic.windows(2)) {
        if corners
            .binary_search_by(|corner| corner.total_cmp(&parameter))
            .is_ok()
            || kinds[0] != kinds[1]
            || !kinds[0]
                && weights
                    .as_ref()
                    .map(|weights| sweep.has_ruling_seam(source, weights, parameter))
                    .transpose()?
                    .unwrap_or(false)
        {
            cuts.push(parameter);
        }
    }
    Ok(cuts)
}

fn analytic_offset(curve: &NurbsCurve, tolerance: Tolerance) -> Result<bool, GeometryError> {
    if curve.is_linear(tolerance)? {
        return Ok(true);
    }
    Ok(curve.circular_radius(tolerance)?.is_some())
}

fn weight_polynomial(source: &NurbsCurve) -> Result<Option<NurbsCurve>, GeometryError> {
    let controls = source.control_points();
    if controls
        .iter()
        .all(|control| control.weight() == controls[0].weight())
    {
        return Ok(None);
    }
    let weights = controls
        .iter()
        .map(|control| Point3::try_new(control.weight(), 0., 0.))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(NurbsCurve::try_new(
        source.degree(),
        weights,
        source.knots().to_vec(),
    )?))
}

#[derive(Clone, Copy)]
struct Sweep {
    displacement: Vector3,
    normal: Vector3,
    offset: f64,
}

impl Sweep {
    fn has_ruling_seam(
        self,
        source: &NurbsCurve,
        weights: &NurbsCurve,
        parameter: f64,
    ) -> Result<bool, GeometryError> {
        let before = self.rail_jet(source, weights, parameter, ParameterSide::Left)?;
        let after = self.rail_jet(source, weights, parameter, ParameterSide::Right)?;
        // Compare profile-direction partials along the ruling. The fitted
        // offset row has unit weights. Blending it with the rational source
        // row can turn the partials even when the rail tangents agree.
        for fraction in [0.25, 0.5, 0.75] {
            let left = before.tangent(fraction)?.normalized_nonzero();
            let right = after.tangent(fraction)?.normalized_nonzero();
            let (Ok(left), Ok(right)) = (left, right) else {
                return Ok(true);
            };
            if left.as_vector().dot(right.as_vector())? < ANGLE.cos() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn rail_jet(
        self,
        source: &NurbsCurve,
        weights: &NurbsCurve,
        parameter: f64,
        side: ParameterSide,
    ) -> Result<RailJet, GeometryError> {
        let (_, first, second) = source.evaluate_with_second_derivative_on_side(parameter, side)?;
        let (weight, derivative) = weights.evaluate_with_derivative_on_side(parameter, side)?;
        let speed = first.length()?;
        let unit = first.normalized_nonzero()?.as_vector();
        let curvature = unit
            .cross(second.scaled(speed.recip())?.scaled(speed.recip())?)?
            .dot(self.normal)?;
        Ok(RailJet {
            first,
            ruling: add_vectors(
                self.displacement,
                self.normal.cross(unit)?.scaled(self.offset)?,
            )?,
            curvature_offset: curvature * self.offset,
            weight: weight.x(),
            log_weight_derivative: derivative.x() / weight.x(),
        })
    }
}

struct RailJet {
    first: Vector3,
    ruling: Vector3,
    curvature_offset: f64,
    weight: f64,
    log_weight_derivative: f64,
}

impl RailJet {
    fn tangent(&self, fraction: f64) -> Result<Vector3, GeometryError> {
        let alpha = fraction / ((1. - fraction) * self.weight + fraction);
        add_vectors(
            self.first.scaled(1. - alpha * self.curvature_offset)?,
            self.ruling
                .scaled(-alpha * (1. - alpha) * self.log_weight_derivative)?,
        )
    }
}

fn add_vectors(a: Vector3, b: Vector3) -> Result<Vector3, GeometryError> {
    Vector3::try_new(a.x() + b.x(), a.y() + b.y(), a.z() + b.z())
}
