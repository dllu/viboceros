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

/// One source lookup and one resolved curve per immutable edge/curve reference.
pub(super) struct Resolver<'a> {
    sources: BTreeMap<ObjectId, &'a viboceros_document::Object>,
    curves: BTreeMap<(ObjectId, Option<usize>), crate::curve_reference::ResolvedCurve<'a>>,
    tolerance: Tolerance,
}
impl<'a> Resolver<'a> {
    pub(super) fn new(document: &'a Document, ids: impl IntoIterator<Item = ObjectId>) -> Self {
        let wanted = ids.into_iter().collect::<BTreeSet<_>>();
        let sources = if wanted.is_empty() {
            BTreeMap::new()
        } else {
            document
                .selectable_objects()
                .filter(|o| wanted.contains(&o.id()))
                .map(|o| (o.id(), o))
                .collect()
        };
        Self {
            sources,
            curves: BTreeMap::new(),
            tolerance: document.tolerance(),
        }
    }
    pub(super) fn object(
        &self,
        id: ObjectId,
    ) -> Result<&'a viboceros_document::Object, CommandError> {
        self.sources
            .get(&id)
            .copied()
            .ok_or(CommandError::Usage(USAGE))
    }
    pub(super) fn points(&mut self, target: &Target) -> Result<Vec<Point3>, CommandError> {
        let key = (target.object, target.edge);
        let object = self.object(target.object)?;
        if let std::collections::btree_map::Entry::Vacant(entry) = self.curves.entry(key) {
            entry.insert(
                crate::curve_reference::resolve(object.geometry(), target.edge, self.tolerance)
                    .ok_or(CommandError::Usage(USAGE))?,
            );
        }
        let source = self.curves[&key].curve();
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
        Ok(Geometry::from(piece).extract_point_locations()?)
    }
}
