use super::*;
use viboceros_geometry::{Sweep1, SweepBlend, SweepFrameStyle, SweepSection};

const USAGE: &str = "Sweep1 Parameters=native-rail-parameters [RailName=name] [FrameStyle=Freeform|Roadlike] [Axis=x,y,z] [GlobalShapeBlending=Yes|No] [RefitRail=Yes|No]";

pub(super) fn model(
    rail: CurveRef<'_>,
    sections: &[SweepSection],
    style: SweepFrameStyle,
    blend: SweepBlend,
    tolerance: Tolerance,
) -> Result<Sweep1, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
        viboceros_smlib::sweep_model(rail, sections, style, blend, tolerance)
            .map_err(knot_refinement::native_geometry_error)
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        Ok(Sweep1::try_new(rail, sections, style, blend, tolerance)?)
    }
}

pub(super) fn surface(sweep: &Sweep1, refit: bool) -> Result<NurbsSurface, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
        viboceros_smlib::sweep_surface(sweep, refit).map_err(knot_refinement::native_geometry_error)
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        Ok(if refit {
            sweep.to_surface()?
        } else {
            sweep.to_rail_basis_surface()?
        })
    }
}

pub(super) fn fitted_model_surface(sweep: &Sweep1) -> Result<NurbsSurface, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
        viboceros_smlib::fit_sweep_model_surface(sweep)
            .map_err(knot_refinement::native_geometry_error)
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        Ok(sweep.fit_model_surface()?)
    }
}

mod options;
use options::Options;

pub(super) struct SweepCommand;

impl Command for SweepCommand {
    fn name(&self) -> &'static str {
        "Sweep1"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let Options {
            rail_name,
            parameters,
            roadlike,
            axis,
            global,
            refit,
        } = Options::parse(arguments)?;
        let rail_name = rail_name.as_deref();
        let selected = selected_ids(document)?;
        let rail_id =
            extrude_along_curve::resolve_curve_along_curve_path(document, &selected, rail_name)?;
        let rail = document
            .object(rail_id)
            .unwrap()
            .geometry()
            .curve_ref()
            .ok_or(CommandError::Usage(USAGE))?;
        let profiles = selected
            .into_iter()
            .filter(|id| *id != rail_id)
            .collect::<Vec<_>>();
        if profiles.len() != parameters.len() {
            return Err(CommandError::Usage(USAGE));
        }
        let sections = profiles
            .iter()
            .zip(parameters)
            .map(|(id, parameter)| {
                let curve = document
                    .object(*id)
                    .unwrap()
                    .geometry()
                    .nurbs_curve_representation()?
                    .ok_or(CommandError::Usage(USAGE))?;
                Ok(SweepSection { parameter, curve })
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        let style = if roadlike.unwrap_or(false) {
            SweepFrameStyle::Roadlike(axis.unwrap_or(UnitVector3::try_new(
                0.,
                0.,
                1.,
                document.tolerance(),
            )?))
        } else {
            SweepFrameStyle::Freeform
        };
        let blend = if global.unwrap_or(false) {
            SweepBlend::Global
        } else {
            SweepBlend::Local
        };
        let sweep = model(rail, &sections, style, blend, document.tolerance())?;
        let surface = surface(&sweep, refit.unwrap_or(false))?;
        let [u, v] = surface.sampled_kink_parameters(document.tolerance().angular())?;
        let brep = Brep::try_surface_grid(&surface, &u, &v, document.tolerance())?;
        let count = brep.faces().len();
        document.add_geometry(Geometry::Brep(brep))?;
        Ok(format!(
            "Swept {} sections into one BRep with {count} faces",
            sections.len()
        ))
    }
}

#[cfg(test)]
mod tests;
