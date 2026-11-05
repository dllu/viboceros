//! Curve extrusion parsing, geometry staging, and document policy.
use super::*;
mod options;
mod subd;
pub use options::Output;
use options::{Direction, parse_curve_extrusion};

#[cfg(feature = "native-smlib")]
mod native;
#[cfg(test)]
mod tests;

const EXTRUDE_CURVE_USAGE: &str = "ExtrudeCrv distance | base target [BothSides=Yes|No] [DeleteInput=Yes|No] [Output=Surface|SubD] [Solid=Yes|No]";

pub(super) struct ExtrudeCurveCommand(pub use_extrusions::Preference);

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
        let profiles = if options.output == Output::Surface
            || matches!(&options.direction, Direction::Numeric(_))
        {
            curves
                .iter()
                .map(|(_, curve)| viboceros_smlib::KernelCurve::from_nurbs_for_modeling(curve))
                .collect::<Result<Vec<_>, _>>()?
        } else {
            Vec::new()
        };
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
            if options.output == Output::SubD {
                extrusions.push((
                    id,
                    Geometry::SubD(subd::extrude(
                        &curve,
                        start_offset,
                        end_offset,
                        document.tolerance(),
                    )?),
                ));
                continue;
            }
            let typed = Brep::try_command_curve_extrusion(
                &curve,
                start_offset,
                end_offset,
                solid,
                document.tolerance(),
            )?;
            let typed = typed
                .map(|brep| use_extrusions::output(brep, &self.0, document.tolerance()))
                .transpose()?;
            #[cfg(not(feature = "native-smlib"))]
            let geometry = if let Some(extrusion) = typed {
                Geometry::Brep(extrusion)
            } else {
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
                }
            };
            #[cfg(feature = "native-smlib")]
            let geometry = if let Some(extrusion) = typed {
                Geometry::Brep(extrusion)
            } else {
                native::extrude(
                    &curve,
                    &profiles[index],
                    start_offset,
                    end_offset,
                    solid,
                    document.tolerance(),
                )?
            };
            let geometry = match geometry {
                Geometry::NurbsSurface(surface) => {
                    Geometry::Brep(Brep::try_surface_face(surface, document.tolerance())?)
                }
                other => other,
            };
            #[cfg(not(feature = "native-smlib"))]
            let geometry = match geometry {
                Geometry::Brep(brep) if !brep.is_extrusion() => {
                    let split = brep
                        .try_split_kinky_faces(std::f64::consts::PI / 180., document.tolerance())?;
                    Geometry::Brep(split.unwrap_or(brep))
                }
                other => other,
            };
            extrusions.push((id, geometry));
        }
        if extrusions.is_empty() {
            return Err(CommandError::NoExtrudableCurves);
        }

        let output_count = extrusions.len();
        let solid_count = extrusions
            .iter()
            .filter(|(_, geometry)| matches!(geometry, Geometry::Brep(b) if b.is_solid()))
            .count();
        for (_, geometry) in &extrusions {
            document.add_geometry(geometry.clone())?;
        }
        if delete_input {
            for (id, _) in &extrusions {
                document.delete_object(*id)?;
            }
        }
        if options.output == Output::SubD {
            return Ok(format!(
                "Extruded {output_count} curve object(s) into SubD object(s){}{}",
                if both_sides { " on both sides" } else { "" },
                if delete_input {
                    ", deleting the input curves"
                } else {
                    ""
                }
            ));
        }
        let surface_count = output_count - solid_count;
        Ok(format!(
            "Extruded {output_count} curve object(s) into {solid_count} exact capped solid(s) and {surface_count} exact surface(s){}{}",
            if both_sides { " on both sides" } else { "" },
            if delete_input {
                ", deleting the input curves"
            } else {
                ""
            }
        ))
    }
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
