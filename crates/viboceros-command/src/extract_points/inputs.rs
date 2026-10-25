//! Whole sources precede visible grips; selected grips override their owner.
use super::*;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Target {
    Object(ObjectId),
    Range(ranges::Target),
}
impl Target {
    fn object(&self) -> ObjectId {
        match self {
            Self::Object(id) => *id,
            Self::Range(range) => range.object,
        }
    }
}

fn whole_points(geometry: &Geometry, control_net: bool) -> Result<Vec<Point3>, CommandError> {
    Ok(match geometry {
        Geometry::Brep(brep) => components::whole_brep_points(brep)?,
        Geometry::SubD(subd) if !control_net => subd.surface_points().to_vec(),
        geometry => geometry.extract_point_locations()?,
    })
}

pub(super) fn visit(
    document: &Document,
    options: &ExtractPointOptions,
    postselected: bool,
    mut emit: impl FnMut(ObjectId, Vec<Point3>, bool) -> Result<(), CommandError>,
) -> Result<BTreeSet<ObjectId>, CommandError> {
    let mut owners = document
        .selected_control_points()
        .map(|(g, _)| g.object)
        .collect::<BTreeSet<_>>();
    let mut component_points = components::resolve(document, options, &owners)?;
    owners.extend(component_points.keys().copied());
    let explicit = options
        .inputs
        .iter()
        .filter_map(|input| match input {
            Target::Object(id) => Some(*id),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    for object in document
        .selected_objects()
        .filter(|o| !owners.contains(&o.id()) && !explicit.contains(&o.id()))
    {
        emit(
            object.id(),
            whole_points(object.geometry(), options.control_net)?,
            false,
        )?;
    }
    // Temporary curve references precede selected grips, just like whole
    // curve inputs, while keeping their own fresh output attributes.
    let mut resolver = ranges::Resolver::new(document, options.inputs.iter().map(Target::object));
    let mut seen = BTreeSet::new();
    for input in &options.inputs {
        match input {
            Target::Object(id) => {
                let object = resolver.object(*id)?;
                if !owners.contains(id) && seen.insert(*id) {
                    emit(
                        *id,
                        whole_points(object.geometry(), options.control_net)?,
                        false,
                    )?;
                }
            }
            Target::Range(range) => emit(range.object, resolver.points(range)?, true)?,
        }
    }
    // Document grip iteration follows object table order and then grip index,
    // independently of the order in which the user selected those grips.
    let mut selected = document.selected_control_points().peekable();
    while let Some((grip, point)) = selected.next() {
        let mut points = vec![point];
        while selected
            .peek()
            .is_some_and(|(g, _)| g.object == grip.object)
        {
            if points.len() >= MAX_EXTRACTED_POINTS {
                return Err(CommandError::TooManyExtractedPoints {
                    maximum: MAX_EXTRACTED_POINTS,
                });
            }
            points.push(selected.next().unwrap().1);
        }
        emit(grip.object, points, false)?;
    }
    if postselected {
        for target in &options.components {
            if let Some(points) = component_points.remove(&target.object) {
                emit(target.object, points, false)?;
            }
        }
    } else {
        for object in document.objects() {
            if let Some(points) = component_points.remove(&object.id()) {
                emit(object.id(), points, false)?;
            }
        }
    }
    Ok(owners)
}
