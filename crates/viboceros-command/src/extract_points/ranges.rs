//! Temporary portions reuse the common range parser and exact curve trim path.
use super::*;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Target {
    pub object: ObjectId,
    pub parameters: [Real; 2],
}

pub(super) fn parse(value: &str) -> Result<Target, CommandError> {
    let input = crate::uv_inputs::SubcurveInput::parse(value, USAGE)?;
    Ok(Target {
        object: input.object,
        parameters: input.parameters,
    })
}

pub(super) fn visit(
    document: &Document,
    targets: &[Target],
    mut emit: impl FnMut(ObjectId, Vec<Point3>) -> Result<(), CommandError>,
) -> Result<(), CommandError> {
    if targets.is_empty() {
        return Ok(());
    }
    let wanted = targets.iter().map(|t| t.object).collect::<BTreeSet<_>>();
    // Resolve the table once, including for repeated portions of one curve.
    let sources = document
        .selectable_objects()
        .filter(|o| wanted.contains(&o.id()))
        .map(|o| (o.id(), o))
        .collect::<BTreeMap<_, _>>();
    for target in targets {
        let source = sources
            .get(&target.object)
            .and_then(|o| o.geometry().curve_ref())
            .ok_or(CommandError::Usage(USAGE))?;
        let piece = source
            .to_owned()
            .try_subcurve(target.parameters[0], target.parameters[1])?;
        emit(
            target.object,
            Geometry::from(piece).extract_point_locations()?,
        )?;
    }
    Ok(())
}
