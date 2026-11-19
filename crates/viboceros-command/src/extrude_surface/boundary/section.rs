//! Classify the near cap and sweep walls after native subtraction.
use super::*;
use std::collections::HashMap;
use viboceros_smlib::{BooleanOperation, Solid};

pub(super) fn cut(
    source: &Brep,
    native_source: &Shape,
    source_bounds: BoundingBox3,
    cutter_bounds: BoundingBox3,
    boundary: &Brep,
    direction: Vector3,
    length: f64,
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    let offset = direction.scaled(length)?;
    let prism = native_source
        .extruded_faces(offset, true, tolerance)?
        .to_brep(tolerance)?;
    let tool = Shape::from_brep(boundary, tolerance)?
        .extruded_faces(offset.scaled(2.)?, true, tolerance)?
        .to_brep(tolerance)?;
    let prism = Solid::from_brep(&prism, tolerance)?;
    let tool = Solid::from_brep(&tool, tolerance)?;
    let result = prism.boolean_with_postprocess(&tool, BooleanOperation::Difference, false)?;
    if result.is_empty()? {
        return Err(CommandError::SurfaceExtrusionBoundaryRejected);
    }
    let result = result.to_brep(tolerance)?;
    let source_extent = axial_extent(source_bounds, direction)?;
    let target_extent = axial_extent(cutter_bounds, direction)?;
    let source_sweep = prism.to_brep(tolerance)?;
    let cutter_sweep = tool.to_brep(tolerance)?;
    let project_vertices = |brep: &Brep| {
        brep.vertices()
            .iter()
            .map(|vertex| axial_point(vertex.point(), direction))
            .collect::<Result<Vec<_>, _>>()
    };
    let source_parameters = project_vertices(&source_sweep)?;
    let cutter_parameters = project_vertices(&cutter_sweep)?;
    let result_parameters = project_vertices(&result)?;
    let epsilon = tolerance.absolute();
    let far = source_extent[0] + length - epsilon;
    let source_cap = source_sweep
        .faces()
        .iter()
        .position(|face| {
            face_parameters(&source_parameters, face).all(|p| p <= source_extent[1] + epsilon)
        })
        .ok_or(CommandError::SurfaceExtrusionBoundaryRejected)?;
    let boundary_cap = cutter_sweep
        .faces()
        .iter()
        .find(|face| {
            face_parameters(&cutter_parameters, face).all(|p| p <= target_extent[1] + epsilon)
        })
        .ok_or(CommandError::SurfaceExtrusionBoundaryRejected)?
        .surface();
    let mut by_support = HashMap::<u64, Vec<usize>>::new();
    for (index, face) in source_sweep.faces().iter().enumerate() {
        by_support
            .entry(sweep_surface_key(face.surface()))
            .or_default()
            .push(index);
    }

    // Volume over a completely enclosed cutter hole survives subtraction as
    // an extension to the temporary end cap. Keep the hole in the boundary
    // cap and discard the extension. Walls reaching the temporary end mean
    // the cutter failed to cover the outer section.
    let has_holes = boundary.faces()[0].loops().len() > 1;
    let mut keep = Vec::new();
    let mut retained_source = Vec::new();
    let mut cut_found = false;
    for (index, face) in result.faces().iter().enumerate() {
        if face.surface() == boundary_cap {
            cut_found = true;
        } else {
            let reaches_far = face_parameters(&result_parameters, face).any(|p| p >= far);
            let from_sweep =
                by_support
                    .get(&sweep_surface_key(face.surface()))
                    .and_then(|matches| {
                        matches
                            .iter()
                            .copied()
                            .find(|&i| source_sweep.faces()[i].surface() == face.surface())
                    });
            if let Some(from_sweep) = from_sweep {
                if reaches_far {
                    if has_holes
                        && face_parameters(&source_parameters, &source_sweep.faces()[from_sweep])
                            .all(|p| p >= far)
                    {
                        continue;
                    }
                    return Err(CommandError::SurfaceExtrusionBoundaryRejected);
                }
                if from_sweep == source_cap {
                    retained_source.push(index);
                }
            } else if has_holes && reaches_far {
                continue;
            } else {
                return Err(CommandError::SurfaceExtrusionBoundaryRejected);
            }
        }
        keep.push(index);
    }
    if !cut_found || retained_source.is_empty() {
        return Err(CommandError::SurfaceExtrusionBoundaryRejected);
    }
    let area = source.area(tolerance)?;
    let retained_area = result
        .sub_brep(&retained_source, tolerance)?
        .area(tolerance)?;
    let box_diagonal = source_bounds.min().distance_to(source_bounds.max())?;
    if (area - retained_area).abs() > 4. * box_diagonal * epsilon + area * tolerance.relative() {
        return Err(CommandError::SurfaceExtrusionBoundaryRejected);
    }
    if keep.len() == result.faces().len() {
        Ok(result)
    } else {
        Ok(result.sub_brep(&keep, tolerance)?)
    }
}

fn face_parameters<'a>(
    parameters: &'a [f64],
    face: &'a BrepFace,
) -> impl Iterator<Item = f64> + 'a {
    face.loops()
        .iter()
        .flat_map(|boundary| boundary.trims())
        .flat_map(|trim| trim.vertices())
        .map(|i| parameters[i])
}
