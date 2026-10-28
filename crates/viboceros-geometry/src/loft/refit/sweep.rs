//! Sweep-specific staged section fitting; Loft retains its own policy.
use super::*;

pub(crate) fn prepare_with_operations<E: From<GeometryError>>(
    curves: &[NurbsCurve],
    tolerance: Real,
    clamp: &mut impl FnMut(&NurbsCurve) -> Result<NurbsCurve, E>,
    elevate: &mut impl FnMut(&NurbsCurve, usize) -> Result<NurbsCurve, E>,
    refine: &mut impl FnMut(&NurbsCurve, Real, usize) -> Result<NurbsCurve, E>,
) -> Result<Vec<NurbsCurve>, E> {
    let single = curves.len() == 1;
    let (first, smooth) = prepare_sweep_pass(curves, tolerance, clamp, elevate, refine)?;
    if single && smooth && !fitting_disabled(tolerance) {
        prepare_sweep_pass(&first, tolerance, clamp, elevate, refine).map(|p| p.0)
    } else {
        Ok(first)
    }
}

fn prepare_sweep_pass<E: From<GeometryError>>(
    curves: &[NurbsCurve],
    tolerance: Real,
    clamp: &mut impl FnMut(&NurbsCurve) -> Result<NurbsCurve, E>,
    elevate: &mut impl FnMut(&NurbsCurve, usize) -> Result<NurbsCurve, E>,
    refine: &mut impl FnMut(&NurbsCurve, Real, usize) -> Result<NurbsCurve, E>,
) -> Result<(Vec<NurbsCurve>, bool), E> {
    let scope = FitScope {
        minimum_curves: 1,
        policy: adaptive::FitPolicy::sweep(curves.len()),
    };
    let prepared = preparation::propose_with_scope(curves, tolerance, None, scope)?;
    let smooth = prepared.smooth;
    let result = match prepared.mode {
        preparation::Mode::Original => {
            // Disabled fitting retains each profile's image and weights, but
            // the Refit overload still aligns its independent regions. Leave
            // basis edits and common weight scaling to Sweep1's preparation.
            Ok(
                super::super::parameterize::prepare_refit(&prepared.curves, None, clamp)?
                    .into_owned(),
            )
        }
        preparation::Mode::Adaptive => {
            prepared.finish(curves, tolerance, None, clamp, elevate, refine)
        }
        preparation::Mode::Initial => Ok(prepared.curves),
    };
    result.map(|curves| (curves, smooth))
}
