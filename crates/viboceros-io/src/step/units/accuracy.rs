//! Length uncertainty declared by a STEP geometric representation.
use super::*;

/// Keep the receiver's modeling tolerance separate from the uncertainty
/// already present in imported topology. Until shell placements carry their
/// own context, use a declared bound only when every 3D context supplies one.
pub(in crate::step) fn native_validation_tolerance(
    data: &DataSection,
    tolerance: Tolerance,
) -> Tolerance {
    let Some(accuracy) = declared_accuracy(data) else {
        return tolerance;
    };
    Tolerance::try_new(
        tolerance.absolute().max(accuracy),
        tolerance.relative(),
        tolerance.angular(),
    )
    .unwrap_or(tolerance)
}

fn declared_accuracy(data: &DataSection) -> Option<f64> {
    let mut resolver = resolver(data).ok()?;
    let contexts = resolver
        .entities
        .values()
        .copied()
        .filter(|records| {
            component(records, "GEOMETRIC_REPRESENTATION_CONTEXT").is_some_and(|context| {
                matches!(list(&context.parameter), Ok([Parameter::Integer(3)]))
            })
        })
        .collect::<Vec<_>>();
    let mut common = None::<f64>;
    for records in contexts {
        let scale = context_length_scale(&mut resolver, records)?;
        let context = component(records, "GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT")?;
        let [measures] = list(&context.parameter).ok()? else {
            return None;
        };
        let accuracy = list(measures)
            .ok()?
            .iter()
            .filter_map(|parameter| {
                let id = reference(parameter).ok()?;
                uncertainty_in_context_units(&mut resolver, id, scale)
            })
            .reduce(f64::min)?;
        common = Some(common.map_or(accuracy, |value| value.min(accuracy)));
    }
    common
}

fn context_length_scale(resolver: &mut Resolver<'_>, records: &[Record]) -> Option<f64> {
    let context = component(records, "GLOBAL_UNIT_ASSIGNED_CONTEXT")?;
    let [units] = list(&context.parameter).ok()? else {
        return None;
    };
    let mut length = None;
    for parameter in list(units).ok()? {
        let id = reference(parameter).ok()?;
        if component(resolver.entities.get(&id)?, "LENGTH_UNIT").is_some()
            && length.replace(resolver.length_scale(id).ok()?).is_some()
        {
            return None;
        }
    }
    length
}

fn uncertainty_in_context_units(
    resolver: &mut Resolver<'_>,
    id: u64,
    context_scale: f64,
) -> Option<f64> {
    let records = *resolver.entities.get(&id)?;
    let uncertainty = component(records, "UNCERTAINTY_MEASURE_WITH_UNIT")?;
    let measure = component(records, "MEASURE_WITH_UNIT")
        .or_else(|| component(records, "LENGTH_MEASURE_WITH_UNIT"))
        .unwrap_or(uncertainty);
    let [Parameter::Typed { keyword, parameter }, unit, ..] = list(&measure.parameter).ok()? else {
        return None;
    };
    if keyword != "LENGTH_MEASURE" && keyword != "POSITIVE_LENGTH_MEASURE" {
        return None;
    }
    let value = match parameter.as_ref() {
        Parameter::Real(value) => *value,
        Parameter::Integer(value) => *value as f64,
        _ => return None,
    };
    let unit_scale = resolver.length_scale(reference(unit).ok()?).ok()?;
    let accuracy = value * (unit_scale / context_scale);
    (accuracy.is_finite() && accuracy > 0.).then_some(accuracy)
}
