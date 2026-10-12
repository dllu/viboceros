//! Curve extrusion parsing, geometry staging, and document policy.
use super::*;

#[cfg(feature = "native-smlib")]
mod native;
#[cfg(test)]
mod tests;

const EXTRUDE_CURVE_USAGE: &str = "ExtrudeCrv distance | base target [BothSides=Yes|No] [DeleteInput=Yes|No] [Output=Surface] [Solid=Yes|No]";

pub(super) struct ExtrudeCurveCommand;

impl Command for ExtrudeCurveCommand {
    fn name(&self) -> &'static str {
        "ExtrudeCrv"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(document, arguments, CommandContext::default())
    }

    fn run_in_context(
        &self,
        document: &mut Document,
        arguments: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let selected = selected_ids(document)?;
        let options = parse_curve_extrusion(arguments, document.tolerance())?;
        let curves = selected
            .into_iter()
            .filter_map(|id| {
                document
                    .object(id)
                    .unwrap()
                    .geometry()
                    .nurbs_curve_representation()
                    .map(|c| c.map(|c| (id, c)))
                    .transpose()
            })
            .collect::<Result<Vec<_>, _>>()?;
        if curves.is_empty() {
            return Err(CommandError::NoExtrudableCurves);
        }
        #[cfg(feature = "native-smlib")]
        let profiles = curves
            .iter()
            .map(|(_, curve)| viboceros_smlib::KernelCurve::from_nurbs_for_modeling(curve))
            .collect::<Result<Vec<_>, _>>()?;
        let direction = match options.direction {
            Direction::Explicit(direction) => direction,
            Direction::Numeric(distance) => {
                #[cfg(feature = "native-smlib")]
                let normal = viboceros_smlib::KernelCurve::parallel_plane_normal(
                    &profiles.iter().collect::<Vec<_>>(),
                    document.tolerance(),
                )?;
                #[cfg(not(feature = "native-smlib"))]
                let normal = portable_parallel_normal(&curves, document.tolerance())?;
                let normal = if let Some(normal) = normal {
                    positive_normal(normal)?
                } else {
                    context.construction_plane.z_axis().as_vector()
                };
                normal.scaled(distance)?
            }
        };
        let start_offset = if options.both_sides {
            direction.scaled(-1.)?
        } else {
            Vector3::try_new(0., 0., 0.)?
        };
        let end_offset = direction;
        let delete_input = options.delete_input;
        let both_sides = options.both_sides;
        let solid = options.solid;
        let mut extrusions = Vec::new();
        for (index, (id, curve)) in curves.into_iter().enumerate() {
            #[cfg(not(feature = "native-smlib"))]
            let _ = index;
            if curve.is_linear(document.tolerance())? {
                // The shared linearity predicate already requires a nonzero
                // endpoint chord. Interior controls can be within tolerance of
                // the line, so they must not choose the extrusion's tangent.
                let tangent = curve.control_points()[0]
                    .point()
                    .vector_to(curve.control_points().last().unwrap().point())?;
                if tangent
                    .normalized_nonzero()?
                    .as_vector()
                    .cross(direction.normalized_nonzero()?.as_vector())?
                    .length()?
                    <= document.tolerance().angular()
                {
                    continue;
                }
            }
            #[cfg(not(feature = "native-smlib"))]
            let geometry =
                if solid && curve.is_closed()? && curve.is_planar(document.tolerance())? {
                    Geometry::Brep(Brep::try_extruded_curve(
                        &curve,
                        start_offset,
                        end_offset,
                        document.tolerance(),
                    )?)
                } else {
                    Geometry::NurbsSurface(NurbsSurface::try_extruded_curve(
                        &curve,
                        start_offset,
                        end_offset,
                    )?)
                };
            #[cfg(feature = "native-smlib")]
            let geometry = native::extrude(
                &curve,
                &profiles[index],
                start_offset,
                end_offset,
                solid,
                document.tolerance(),
            )?;
            extrusions.push((id, geometry));
        }
        if extrusions.is_empty() {
            return Err(CommandError::NoExtrudableCurves);
        }

        let output_count = extrusions.len();
        let solid_count = extrusions
            .iter()
            .filter(|(_, geometry)| matches!(geometry, Geometry::Brep(_)))
            .count();
        for (_, geometry) in &extrusions {
            document.add_geometry(geometry.clone())?;
        }
        if delete_input {
            for (id, _) in &extrusions {
                document.delete_object(*id)?;
            }
        }
        let surface_count = output_count - solid_count;
        Ok(format!(
            "Extruded {output_count} curve object(s) into {solid_count} exact capped B-rep solid(s) and {surface_count} exact NURBS surface(s){}{}",
            if both_sides { " on both sides" } else { "" },
            if delete_input {
                ", deleting the input curves"
            } else {
                ""
            }
        ))
    }
}

enum Direction {
    Numeric(Real),
    Explicit(Vector3),
}
struct Options {
    direction: Direction,
    delete_input: bool,
    both_sides: bool,
    solid: bool,
}

fn positive_normal(normal: Vector3) -> Result<Vector3, CommandError> {
    let components = normal.to_array();
    let axis = (0..3)
        .max_by(|&a, &b| components[a].abs().total_cmp(&components[b].abs()))
        .unwrap();
    Ok(if components[axis] < 0. {
        normal.scaled(-1.)?
    } else {
        normal
    })
}

#[cfg(not(feature = "native-smlib"))]
fn portable_parallel_normal(
    curves: &[(ObjectId, NurbsCurve)],
    tolerance: Tolerance,
) -> Result<Option<Vector3>, CommandError> {
    let zero = Point3::try_new(0., 0., 0.)?;
    let mut points = Vec::new();
    for (_, curve) in curves {
        let origin = curve.control_points()[0].point();
        for p in curve.control_points() {
            points.push(zero.translated(origin.vector_to(p.point())?)?);
        }
    }
    if let Some(plane) = viboceros_geometry::try_control_polygon_plane(&points, tolerance)? {
        return Ok(Some(plane.normal().as_vector()));
    }
    for (_, curve) in curves {
        if !curve.is_linear(tolerance)? {
            return Ok(None);
        }
    }
    let positions = curves
        .iter()
        .flat_map(|(_, c)| c.control_points().iter().map(|p| p.point()))
        .collect::<Vec<_>>();
    Ok(
        viboceros_geometry::try_control_polygon_plane(&positions, tolerance)?
            .map(|p| p.normal().as_vector()),
    )
}

fn parse_curve_extrusion(
    arguments: &[&str],
    tolerance: Tolerance,
) -> Result<Options, CommandError> {
    let mut positional = Vec::new();
    let mut both_sides = None;
    let mut delete_input = None;
    let mut output_seen = false;
    let mut solid = None;
    for argument in arguments {
        let Some((name, value)) = argument.split_once('=') else {
            positional.push(*argument);
            continue;
        };
        if option_name_eq(name, "BothSides") && both_sides.is_none() {
            both_sides = parse_yes_no(value);
            if both_sides.is_none() {
                return Err(CommandError::Usage(EXTRUDE_CURVE_USAGE));
            }
        } else if option_name_eq(name, "DeleteInput") && delete_input.is_none() {
            delete_input = parse_yes_no(value);
            if delete_input.is_none() {
                return Err(CommandError::Usage(EXTRUDE_CURVE_USAGE));
            }
        } else if option_name_eq(name, "Output") && !output_seen {
            if !value
                .trim_start_matches('_')
                .eq_ignore_ascii_case("Surface")
            {
                return Err(CommandError::UnsupportedCurveExtrusionOutput);
            }
            output_seen = true;
        } else if option_name_eq(name, "Solid") && solid.is_none() {
            solid = Some(parse_yes_no(value).ok_or(CommandError::Usage(EXTRUDE_CURVE_USAGE))?);
        } else {
            return Err(CommandError::Usage(EXTRUDE_CURVE_USAGE));
        }
    }

    let direction = if positional.len() == 1 && !positional[0].contains(',') {
        let distance = parse_finite_real(positional[0])?;
        Vector3::try_new(0., 0., distance)?.normalized(tolerance)?;
        Direction::Numeric(distance)
    } else {
        let (base, base_consumed) = parse_point(&positional)?;
        let (target, target_consumed) = parse_point(&positional[base_consumed..])?;
        require_consumed(
            &positional,
            base_consumed + target_consumed,
            EXTRUDE_CURVE_USAGE,
        )?;
        let direction = base.vector_to(target)?;
        direction.normalized(tolerance)?;
        Direction::Explicit(direction)
    };
    Ok(Options {
        direction,
        delete_input: delete_input.unwrap_or(false),
        both_sides: both_sides.unwrap_or(false),
        solid: solid.unwrap_or(false),
    })
}
