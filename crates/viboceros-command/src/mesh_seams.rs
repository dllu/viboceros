//! Whole-mesh seam commands, angular options and atomic document replacement.
use super::*;

const WELD_USAGE: &str = "Weld [angle-degrees|Angle=degrees]";

pub(super) struct WeldCommand;

impl Command for WeldCommand {
    fn name(&self) -> &'static str {
        "Weld"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let angle_degrees = parse_weld_angle(arguments)?;
        let selected = document
            .selected_objects()
            .map(|object| (object.id(), object.geometry()))
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }

        let mesh_count = selected.len();
        let mut removed_vertex_count = 0;
        let mut replacements = Vec::new();
        for (id, geometry) in selected {
            let Geometry::Mesh(mesh) = geometry else {
                return Err(CommandError::UnsupportedWeldGeometry);
            };
            let (welded, removed) = mesh.welded_vertices(angle_degrees.to_radians())?;
            removed_vertex_count += removed;
            if removed > 0 {
                replacements.push((id, Geometry::Mesh(welded)));
            }
        }
        if replacements.is_empty() {
            return Err(CommandError::NoMeshVerticesToWeld);
        }
        let changed_mesh_count = document.replace_object_geometries(replacements)?;
        Ok(format!(
            "Welded {removed_vertex_count} vertex occurrence(s) in {changed_mesh_count} mesh(es); {} mesh(es) unchanged",
            mesh_count - changed_mesh_count
        ))
    }
}

fn parse_weld_angle(arguments: &[&str]) -> Result<Real, CommandError> {
    let value = match arguments {
        [] => return Ok(180.0),
        [argument] => {
            if let Some((name, value)) = argument.split_once('=') {
                if !(option_name_eq(name, "Angle") || option_name_eq(name, "AngleTolerance")) {
                    return Err(CommandError::Usage(WELD_USAGE));
                }
                value
            } else {
                argument
            }
        }
        [name, value]
            if option_name_eq(name, "Angle") || option_name_eq(name, "AngleTolerance") =>
        {
            value
        }
        _ => return Err(CommandError::Usage(WELD_USAGE)),
    };
    let angle_degrees = parse_finite_real(value)?;
    if !(0.0..=180.0).contains(&angle_degrees) {
        return Err(GeometryError::InvalidMeshWeldAngle.into());
    }
    Ok(angle_degrees)
}

const UNWELD_USAGE: &str = "Unweld [angle-degrees|Angle=degrees] [ModifyNormals=Yes|No]";

pub(super) struct UnweldCommand;

impl Command for UnweldCommand {
    fn name(&self) -> &'static str {
        "Unweld"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_unweld_arguments(arguments)?;
        let selected = document
            .selected_objects()
            .map(|object| (object.id(), object.geometry()))
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }

        let mesh_count = selected.len();
        let mut qualifying_edge_count = 0;
        let mut replacements = Vec::new();
        for (id, geometry) in selected {
            let Geometry::Mesh(mesh) = geometry else {
                return Err(CommandError::UnsupportedUnweldGeometry);
            };
            let (unwelded, edges) = mesh.unwelded_vertices_with_normal_update(
                options.angle_degrees.to_radians(),
                options.modify_normals,
            )?;
            qualifying_edge_count += edges;
            if &unwelded != mesh {
                replacements.push((id, Geometry::Mesh(unwelded)));
            }
        }
        if replacements.is_empty() {
            return Err(CommandError::NoMeshEdgesToUnweld);
        }
        let changed_mesh_count = document.replace_object_geometries(replacements)?;
        Ok(format!(
            "Processed {qualifying_edge_count} unweld edge(s) in {changed_mesh_count} changed mesh(es); {} mesh(es) unchanged",
            mesh_count - changed_mesh_count
        ))
    }
}

#[derive(Clone, Copy)]
struct UnweldOptions {
    angle_degrees: Real,
    modify_normals: bool,
}

fn parse_unweld_arguments(arguments: &[&str]) -> Result<UnweldOptions, CommandError> {
    let mut angle_degrees = None;
    let mut modify_normals = None;
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        let option = if let Some((name, value)) = argument.split_once('=') {
            Some((name, value, 1))
        } else if option_name_eq(argument, "Angle")
            || option_name_eq(argument, "AngleTolerance")
            || option_name_eq(argument, "ModifyNormals")
        {
            let value = arguments
                .get(index + 1)
                .ok_or(CommandError::Usage(UNWELD_USAGE))?;
            Some((argument, *value, 2))
        } else {
            None
        };
        if let Some((name, value, consumed)) = option {
            if (option_name_eq(name, "Angle") || option_name_eq(name, "AngleTolerance"))
                && angle_degrees.is_none()
            {
                angle_degrees = Some(parse_finite_real(value)?);
            } else if option_name_eq(name, "ModifyNormals") && modify_normals.is_none() {
                modify_normals =
                    Some(parse_yes_no(value).ok_or(CommandError::Usage(UNWELD_USAGE))?);
            } else {
                return Err(CommandError::Usage(UNWELD_USAGE));
            }
            index += consumed;
        } else if angle_degrees.is_none() {
            angle_degrees = Some(parse_finite_real(argument)?);
            index += 1;
        } else {
            return Err(CommandError::Usage(UNWELD_USAGE));
        }
    }
    let angle_degrees = angle_degrees.unwrap_or(0.0);
    if !(0.0..=180.0).contains(&angle_degrees) {
        return Err(GeometryError::InvalidMeshUnweldAngle.into());
    }
    Ok(UnweldOptions {
        angle_degrees,
        modify_normals: modify_normals.unwrap_or(true),
    })
}
