//! Trim a folded cutter to the sweep before resolving its separate sections.
use super::*;

pub(super) fn truncate(
    cone: &Brep,
    boundary: &Brep,
    apex: Point3,
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    let solid = Solid::from_brep(cone, tolerance)?;
    let boundary = Shape::from_brep(boundary, tolerance)?;
    let inside = boundary
        .trimmed_inside(&solid)?
        .ok_or(CommandError::Usage(NO_SECTION))?;
    let sections = inside
        .to_brep(tolerance)?
        .into_edge_connected_components(tolerance)?;
    if let [section] = sections.as_slice() {
        return truncate_radially(cone, section, apex, tolerance);
    }

    // A section that spans the sweep and is first on every ruling keeps
    // the least volume. Check the other sections against that one result
    // so a nearer partial section cannot be bypassed by a complete far cut.
    let mut nearest = None::<(usize, f64, Brep)>;
    for (index, section) in sections.iter().enumerate() {
        let Ok(cut) = truncate_radially(cone, section, apex, tolerance) else {
            continue;
        };
        let Ok(volume) = cut.signed_volume(tolerance) else {
            continue;
        };
        let volume = volume.abs();
        if nearest.as_ref().is_none_or(|(_, best, _)| volume < *best) {
            nearest = Some((index, volume, cut));
        }
    }
    let (selected, _, cut) = nearest.ok_or(CommandError::Usage(NO_SECTION))?;
    let retained = Solid::from_brep(&cut, tolerance)?;
    for (index, section) in sections.iter().enumerate() {
        if index != selected
            && Shape::from_brep(section, tolerance)?
                .trimmed_inside(&retained)?
                .is_some()
        {
            return Err(CommandError::Usage(NO_SECTION));
        }
    }
    Ok(cut)
}
