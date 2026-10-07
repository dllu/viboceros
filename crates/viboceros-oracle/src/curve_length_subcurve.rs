//! Geometry-only signed-length queries from untouched full source definitions.
use super::*;
use viboceros_geometry::{Curve3, Real};
#[cfg(test)]
mod tests;

pub(super) fn run(
    definition: &NurbsCurveDefinition,
    start: Real,
    length: Real,
    tolerance: Tolerance,
    iterations: u32,
) -> Result<(Value, u64), ProbeError> {
    let original = nurbs_curve_from_definition(definition)?;
    let before = nurbs_curve_definition_value(&original);
    let source = Curve3::NurbsCurve(original);
    measure(iterations, || {
        let result = if length == 0. && source.as_ref().domain().contains(&start) {
            None
        } else {
            source.try_subcurve_at_arc_length(start, length, tolerance)?
        };
        let curve = if let Some(result) = result {
            let curve = result.as_ref().to_nurbs()?;
            let sampler = curve.parameter_sampler()?;
            Some(json!({"definition":nurbs_curve_definition_value(&curve),
                "samples":(0..=32).map(|i|sampler.evaluate(i as Real/32.).map(Point3::to_array)).collect::<Result<Vec<_>,_>>()?}))
        } else {
            None
        };
        let Curve3::NurbsCurve(original) = &source else {
            unreachable!()
        };
        Ok(
            json!({"available":curve.is_some(),"curve":curve,"source_unchanged":nurbs_curve_definition_value(original)==before}),
        )
    })
}
