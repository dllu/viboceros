//! Trim the open ruled stock of an oblique taper and close its section.
use super::*;
use crate::surface_boundary_cut::{boundary_tool, mount_planar_cap};

pub(super) fn crosses_circular_focal_radius(
    source: &Brep,
    direction: Vector3,
    normal: Vector3,
    inset: f64,
    tolerance: Tolerance,
) -> Result<bool, CommandError> {
    let [edge] = source.edges() else {
        return Ok(false);
    };
    let Some(radius) = edge.curve().circular_radius(tolerance)? else {
        return Ok(false);
    };
    // Pulling along an oblique axis offsets the profile's projection onto
    // the normal plane of that axis. A projected circle is an ellipse with
    // minor radius r*cosine, whose first inward cusp is at r*cosine^2.
    let cosine = direction.dot(normal)?;
    Ok(inset >= radius * cosine * cosine - tolerance.absolute())
}

pub(super) fn has_wall_gap(
    stock: &Brep,
    base: Plane,
    tolerance: Tolerance,
) -> Result<bool, CommandError> {
    for edge in naked_edges(stock) {
        for vertex in edge.vertices() {
            if base
                .signed_distance_to(stock.vertices()[vertex].point())?
                .abs()
                <= tolerance.absolute()
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

pub(super) fn trim(
    sweep: &Shape,
    stock: &Brep,
    source: &Brep,
    cutter: &Brep,
    displacement: Vector3,
    cap: bool,
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    let face = &cutter.faces()[0];
    if face.loops().len() != 1 {
        return Err(REJECTED);
    }
    let plane = face.surface().plane(tolerance)?.ok_or(REJECTED)?;
    let projection = displacement.dot(plane.normal().as_vector())?;
    if projection.abs() <= tolerance.absolute() {
        return Err(REJECTED);
    }
    let height =
        |point| Ok::<_, GeometryError>(plane.signed_distance_to(point)? * projection.signum());
    let epsilon = tolerance.absolute();
    for control in source
        .edges()
        .iter()
        .flat_map(|edge| edge.curve().control_points())
    {
        if control.weight() <= 0. || height(control.point())? >= -epsilon {
            return Err(REJECTED);
        }
    }
    let mut far_rim_found = false;
    for edge in naked_edges(stock) {
        far_rim_found = true;
        for control in edge.curve().control_points() {
            if control.weight() <= 0. || height(control.point())? <= epsilon {
                return Err(REJECTED);
            }
        }
    }
    if !far_rim_found {
        return Err(REJECTED);
    }

    // The finite cutter's exterior extrudes back toward the source. Any
    // surviving naked edge off its plane signals an uncovered section.
    let tool = boundary_tool(cutter, displacement.scaled(-2.)?, tolerance)?;
    let clipped = match sweep.trimmed_inside(&tool) {
        Ok(Some(clipped)) => clipped.to_brep(tolerance)?,
        Ok(None) | Err(viboceros_smlib::Error::Kernel(_)) => return Err(REJECTED),
        Err(error) => return Err(error.into()),
    };
    if clipped.faces().len() != stock.faces().len()
        || clipped.edge_connected_face_components().len() != 1
    {
        return Err(REJECTED);
    }
    let mut section_found = false;
    for edge in naked_edges(&clipped) {
        section_found = true;
        for control in edge.curve().control_points() {
            if height(control.point())?.abs() > epsilon {
                return Err(REJECTED);
            }
        }
    }
    if !section_found {
        return Err(REJECTED);
    }

    let base = source.faces()[0]
        .surface()
        .plane(tolerance)?
        .ok_or(REJECTED)?;
    let (base_faces, walls): (Vec<_>, Vec<_>) = (0..clipped.faces().len())
        .partition(|&i| on_plane(clipped.faces()[i].surface(), base, tolerance));
    let [base_index] = base_faces.as_slice() else {
        return Err(REJECTED);
    };
    let retained_area = clipped.sub_brep(&base_faces, tolerance)?.area(tolerance)?;
    let source_area = source.area(tolerance)?;
    let bounds = source.bounds();
    let span = bounds.min().distance_to(bounds.max())?;
    if (retained_area - source_area).abs()
        > 4. * span * epsilon + source_area * tolerance.relative()
    {
        return Err(REJECTED);
    }
    if !cap {
        return Ok(clipped.sub_brep(&walls, tolerance)?);
    }

    let cap_index = clipped.faces().len();
    let mut closed = clipped.try_cap_planar_holes(tolerance)?.ok_or(REJECTED)?;
    if closed.faces().len() != cap_index + 1 || !closed.is_solid() {
        return Err(REJECTED);
    }
    // The outward base normal points away from the cut.
    let base_face = &closed.faces()[*base_index];
    let surface = base_face.surface();
    let base_normal =
        surface.normal_at(surface.parameter_at_u(0.5)?, surface.parameter_at_v(0.5)?)?;
    if (base_normal.as_vector().dot(displacement)? > 0.) != base_face.is_reversed() {
        closed = closed.reversed();
    }
    mount_planar_cap(closed, cap_index, face.surface(), tolerance)
}

fn naked_edges(brep: &Brep) -> impl Iterator<Item = &viboceros_geometry::BrepEdge> {
    brep.edges()
        .iter()
        .zip(brep.edge_use_counts())
        .filter_map(|(edge, count)| (count == 1).then_some(edge))
}

#[cfg(test)]
mod tests;
