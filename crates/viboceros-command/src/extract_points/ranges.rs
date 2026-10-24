//! Temporary portions reuse the common range parser and exact curve trim path.
use super::*;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Target {
    pub object: ObjectId,
    pub edge: Option<usize>,
    pub edge_wrap: bool,
    pub parameters: [Real; 2],
}

pub(super) fn parse(value: &str) -> Result<Target, CommandError> {
    let fields = value.split(',').collect::<Vec<_>>();
    if let [object, kind, edge, start, end] = fields.as_slice() {
        if !kind.eq_ignore_ascii_case("Edge") && !kind.eq_ignore_ascii_case("EdgeWrap") {
            return Err(CommandError::Usage(USAGE));
        }
        return Ok(Target {
            object: object.parse().map_err(|_| CommandError::Usage(USAGE))?,
            edge: Some(edge.parse().map_err(|_| CommandError::Usage(USAGE))?),
            edge_wrap: kind.eq_ignore_ascii_case("EdgeWrap"),
            parameters: [parse_finite_real(start)?, parse_finite_real(end)?],
        });
    }
    let input = crate::uv_inputs::SubcurveInput::parse(value, USAGE)?;
    Ok(Target {
        object: input.object,
        edge: None,
        edge_wrap: false,
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
    let mut resolved = BTreeMap::new();
    for target in targets {
        let key = (target.object, target.edge);
        if let std::collections::btree_map::Entry::Vacant(entry) = resolved.entry(key) {
            let source = sources
                .get(&target.object)
                .ok_or(CommandError::Usage(USAGE))?;
            entry.insert(
                crate::curve_reference::resolve(
                    source.geometry(),
                    target.edge,
                    document.tolerance(),
                )
                .ok_or(CommandError::Usage(USAGE))?,
            );
        }
        let source = resolved[&key].curve();
        let piece = if target.edge.is_some() && !target.edge_wrap {
            crate::curve_reference::edge_subcurve(
                source,
                target.parameters[0],
                target.parameters[1],
            )?
        } else {
            source
                .to_owned()
                .try_subcurve(target.parameters[0], target.parameters[1])?
        };
        emit(
            target.object,
            Geometry::from(piece).extract_point_locations()?,
        )?;
    }
    Ok(())
}
