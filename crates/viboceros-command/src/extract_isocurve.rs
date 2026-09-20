//! Exact isocurve extraction from surfaces and trimmed B-rep faces.
use super::*;

#[cfg(test)]
mod tests;

const EXTRACT_ISOCURVE_USAGE: &str =
    "ExtractIsocurve (point|ExtractAll) [Direction=U|V|Both] [IgnoreTrims=Yes|No]";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExtractIsocurveDirection {
    U,
    V,
    Both,
}

impl ExtractIsocurveDirection {
    const fn label(self) -> &'static str {
        match self {
            Self::U => "U",
            Self::V => "V",
            Self::Both => "U/V",
        }
    }
}

struct ExtractIsocurveOptions {
    point: Option<Point3>,
    direction: ExtractIsocurveDirection,
    ignore_trims: bool,
}

pub(super) struct ExtractIsocurveCommand;

impl Command for ExtractIsocurveCommand {
    fn name(&self) -> &'static str {
        "ExtractIsocurve"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["IsoCurve"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_extract_isocurve_arguments(arguments)?;
        let mut curves = Vec::new();
        let mut source_count = 0;
        for object in document.selected_objects() {
            source_count += 1;
            match object.geometry() {
                Geometry::NurbsSurface(surface) => {
                    if let Some(point) = options.point {
                        let (u, v) = surface.closest_parameters(point, document.tolerance())?;
                        append_surface_isocurves_at(&mut curves, surface, u, v, options.direction)?;
                    } else {
                        append_all_surface_isocurves(
                            &mut curves,
                            surface,
                            options.direction,
                            object.attributes().wire_density(),
                        )?;
                    }
                }
                Geometry::Brep(brep) => {
                    if let Some(point) = options.point {
                        let closest =
                            if options.ignore_trims {
                                Some(brep.closest_underlying_face_parameters(
                                    point,
                                    document.tolerance(),
                                )?)
                            } else {
                                brep.closest_face_parameters(point, document.tolerance())?
                            };
                        let Some((face_index, u, v)) = closest else {
                            continue;
                        };
                        append_face_isocurves_at(
                            &mut curves,
                            &brep.faces()[face_index],
                            u,
                            v,
                            options.direction,
                            options.ignore_trims,
                            document.tolerance(),
                        )?;
                    } else {
                        for face in brep.faces() {
                            append_all_face_isocurves(
                                &mut curves,
                                face,
                                options.direction,
                                object.attributes().wire_density(),
                                options.ignore_trims,
                                document.tolerance(),
                            )?;
                        }
                    }
                }
                _ => return Err(CommandError::UnsupportedExtractIsocurveGeometry),
            }
        }
        if source_count == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        if curves.is_empty() {
            return Err(CommandError::NoExtractableIsocurves);
        }

        let curve_count = curves.len();
        let mut ids = Vec::with_capacity(curve_count);
        for curve in curves {
            ids.push(document.add_geometry(Geometry::NurbsCurve(curve))?);
        }
        replace_selection(document, ids)?;
        Ok(format!(
            "Extracted {curve_count} exact {} isocurve(s) from {} surface(s)",
            options.direction.label(),
            source_count
        ))
    }
}

fn append_surface_isocurves_at(
    curves: &mut Vec<NurbsCurve>,
    surface: &NurbsSurface,
    u: Real,
    v: Real,
    direction: ExtractIsocurveDirection,
) -> Result<(), CommandError> {
    if matches!(
        direction,
        ExtractIsocurveDirection::U | ExtractIsocurveDirection::Both
    ) {
        append_extractable_isocurves(curves, [surface.isocurve_u(v)?])?;
    }
    if matches!(
        direction,
        ExtractIsocurveDirection::V | ExtractIsocurveDirection::Both
    ) {
        append_extractable_isocurves(curves, [surface.isocurve_v(u)?])?;
    }
    Ok(())
}

fn append_face_isocurves_at(
    curves: &mut Vec<NurbsCurve>,
    face: &BrepFace,
    u: Real,
    v: Real,
    direction: ExtractIsocurveDirection,
    ignore_trims: bool,
    tolerance: Tolerance,
) -> Result<(), CommandError> {
    if matches!(
        direction,
        ExtractIsocurveDirection::U | ExtractIsocurveDirection::Both
    ) {
        if ignore_trims {
            append_extractable_isocurves(curves, [face.surface().isocurve_u(v)?])?;
        } else {
            append_extractable_isocurves(curves, face.isocurve_u_segments(v, tolerance)?)?;
        }
    }
    if matches!(
        direction,
        ExtractIsocurveDirection::V | ExtractIsocurveDirection::Both
    ) {
        if ignore_trims {
            append_extractable_isocurves(curves, [face.surface().isocurve_v(u)?])?;
        } else {
            append_extractable_isocurves(curves, face.isocurve_v_segments(u, tolerance)?)?;
        }
    }
    Ok(())
}

fn append_all_surface_isocurves(
    curves: &mut Vec<NurbsCurve>,
    surface: &NurbsSurface,
    direction: ExtractIsocurveDirection,
    wire_density: i32,
) -> Result<(), CommandError> {
    if matches!(
        direction,
        ExtractIsocurveDirection::U | ExtractIsocurveDirection::Both
    ) {
        for v in surface.wire_parameters_v(wire_density)? {
            append_extractable_isocurves(curves, [surface.isocurve_u(v)?])?;
        }
    }
    if matches!(
        direction,
        ExtractIsocurveDirection::V | ExtractIsocurveDirection::Both
    ) {
        for u in surface.wire_parameters_u(wire_density)? {
            append_extractable_isocurves(curves, [surface.isocurve_v(u)?])?;
        }
    }
    Ok(())
}

fn append_all_face_isocurves(
    curves: &mut Vec<NurbsCurve>,
    face: &BrepFace,
    direction: ExtractIsocurveDirection,
    wire_density: i32,
    ignore_trims: bool,
    tolerance: Tolerance,
) -> Result<(), CommandError> {
    if ignore_trims {
        return append_all_surface_isocurves(curves, face.surface(), direction, wire_density);
    }
    if matches!(
        direction,
        ExtractIsocurveDirection::U | ExtractIsocurveDirection::Both
    ) {
        append_extractable_isocurves(
            curves,
            face.isocurve_u_segments_at_density(wire_density, tolerance)?,
        )?;
    }
    if matches!(
        direction,
        ExtractIsocurveDirection::V | ExtractIsocurveDirection::Both
    ) {
        append_extractable_isocurves(
            curves,
            face.isocurve_v_segments_at_density(wire_density, tolerance)?,
        )?;
    }
    Ok(())
}

fn append_extractable_isocurves(
    curves: &mut Vec<NurbsCurve>,
    candidates: impl IntoIterator<Item = NurbsCurve>,
) -> Result<(), CommandError> {
    for curve in candidates {
        if !nurbs_curve_has_extent(&curve) {
            continue;
        }
        if curves.len() == MAX_SPAN_OUTPUT_OBJECTS {
            return Err(too_many_span_outputs("ExtractIsocurve"));
        }
        curves.push(curve);
    }
    Ok(())
}

fn parse_extract_isocurve_arguments(
    arguments: &[&str],
) -> Result<ExtractIsocurveOptions, CommandError> {
    let mut direction = ExtractIsocurveDirection::U;
    let mut ignore_trims = false;
    let mut extract_all = false;
    let mut direction_seen = false;
    let mut ignore_trims_seen = false;
    let mut positional = Vec::new();
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        if option_name_eq(argument, "ExtractAll") {
            if extract_all {
                return Err(CommandError::Usage(EXTRACT_ISOCURVE_USAGE));
            }
            extract_all = true;
            index += 1;
            continue;
        }
        let option = if let Some((name, value)) = argument.split_once('=') {
            Some((name, value, 1))
        } else if option_name_eq(argument, "Direction") || option_name_eq(argument, "IgnoreTrims") {
            let value = arguments
                .get(index + 1)
                .ok_or(CommandError::Usage(EXTRACT_ISOCURVE_USAGE))?;
            Some((argument, *value, 2))
        } else {
            None
        };
        if let Some((name, value, consumed)) = option {
            if option_name_eq(name, "Direction") && !direction_seen {
                let value = value.trim_start_matches('_');
                direction = if value.eq_ignore_ascii_case("U") {
                    ExtractIsocurveDirection::U
                } else if value.eq_ignore_ascii_case("V") {
                    ExtractIsocurveDirection::V
                } else if value.eq_ignore_ascii_case("Both") {
                    ExtractIsocurveDirection::Both
                } else {
                    return Err(CommandError::Usage(EXTRACT_ISOCURVE_USAGE));
                };
                direction_seen = true;
            } else if option_name_eq(name, "IgnoreTrims") && !ignore_trims_seen {
                ignore_trims =
                    parse_yes_no(value).ok_or(CommandError::Usage(EXTRACT_ISOCURVE_USAGE))?;
                ignore_trims_seen = true;
            } else {
                return Err(CommandError::Usage(EXTRACT_ISOCURVE_USAGE));
            }
            index += consumed;
        } else {
            positional.push(argument);
            index += 1;
        }
    }
    let point = if extract_all {
        require_consumed(&positional, 0, EXTRACT_ISOCURVE_USAGE)?;
        None
    } else {
        let (point, consumed) = parse_point(&positional)?;
        require_consumed(&positional, consumed, EXTRACT_ISOCURVE_USAGE)?;
        Some(point)
    };
    Ok(ExtractIsocurveOptions {
        point,
        direction,
        ignore_trims,
    })
}
