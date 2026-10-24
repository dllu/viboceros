//! Whole sources precede visible grips; selected grips override their owner.
use super::*;

pub(super) fn visit(
    document: &Document,
    options: &ExtractPointOptions,
    postselected: bool,
    mut emit: impl FnMut(ObjectId, Vec<Point3>) -> Result<(), CommandError>,
) -> Result<BTreeSet<ObjectId>, CommandError> {
    let mut owners = document
        .selected_control_points()
        .map(|(g, _)| g.object)
        .collect::<BTreeSet<_>>();
    let mut component_points = components::resolve(document, options, &owners)?;
    owners.extend(component_points.keys().copied());
    for object in document
        .selected_objects()
        .filter(|o| !owners.contains(&o.id()))
    {
        emit(object.id(), object.geometry().extract_point_locations()?)?;
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
        emit(grip.object, points)?;
    }
    if postselected {
        for target in &options.components {
            if let Some(points) = component_points.remove(&target.object) {
                emit(target.object, points)?;
            }
        }
    } else {
        for object in document.objects() {
            if let Some(points) = component_points.remove(&object.id()) {
                emit(object.id(), points)?;
            }
        }
    }
    Ok(owners)
}
