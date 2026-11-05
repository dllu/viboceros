//! Apex extrusion parsing and transactional document policy.
use super::*;
mod options;
mod subd;
pub use options::Options;
#[cfg(feature = "native-smlib")]
mod native;

const EXTRUDE_CURVE_TO_POINT_USAGE: &str =
    "ExtrudeCrvToPoint apex [DeleteInput=Yes|No] [Output=Surface|SubD] [Solid=Yes|No]";

pub(super) struct ExtrudeCurveToPointCommand(pub crease_splitting::Preference);

impl Command for ExtrudeCurveToPointCommand {
    fn name(&self) -> &'static str {
        "ExtrudeCrvToPoint"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let selected = selected_ids(document)?;
        let (apex, options) = options::parse(arguments)?;
        let Options {
            delete_input,
            solid,
            ..
        } = options;
        let mut extrusions = Vec::new();
        for id in selected {
            let Some(curve) = document
                .object(id)
                .expect("selected objects exist")
                .geometry()
                .nurbs_curve_representation()?
            else {
                continue;
            };
            if options.output == CurveExtrusionOutput::SubD {
                extrusions.push((
                    id,
                    subd::extrude(&curve, apex, self.0.get().0, document.tolerance())?,
                ));
                continue;
            }
            #[cfg(not(feature = "native-smlib"))]
            let geometry =
                if solid && curve.is_closed()? && curve.is_planar(document.tolerance())? {
                    Geometry::Brep(Brep::try_extruded_curve_to_point(
                        &curve,
                        apex,
                        document.tolerance(),
                    )?)
                } else {
                    Geometry::NurbsSurface(NurbsSurface::try_extruded_curve_to_point(&curve, apex)?)
                };
            #[cfg(feature = "native-smlib")]
            let geometry = native::extrude(&curve, apex, solid, document.tolerance())?;
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
        if options.output == CurveExtrusionOutput::SubD {
            return Ok(format!(
                "Extruded {output_count} curve object(s) to {} SubD(s) and {solid_count} fallback B-rep(s){}",
                output_count - solid_count,
                if delete_input {
                    ", deleting the input curves"
                } else {
                    ""
                }
            ));
        }
        let surface_count = output_count - solid_count;
        Ok(format!(
            "Extruded {output_count} curve object(s) to {solid_count} exact capped B-rep solid(s) and {surface_count} exact NURBS apex surface(s){}",
            if delete_input {
                ", deleting the input curves"
            } else {
                ""
            }
        ))
    }
}
