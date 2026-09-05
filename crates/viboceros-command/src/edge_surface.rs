use super::*;

const USAGE: &str = "EdgeSrf (select two, three, or four open curves)";
pub(super) struct EdgeSurfaceCommand;

impl Command for EdgeSurfaceCommand {
    fn name(&self) -> &'static str {
        "EdgeSrf"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if !arguments.is_empty() {
            return Err(CommandError::Usage(USAGE));
        }
        let ids = selected_ids(document)?;
        if !(2..=4).contains(&ids.len()) {
            return Err(CommandError::Usage(USAGE));
        }
        let curves = ids
            .iter()
            .map(|id| {
                document
                    .object(*id)
                    .unwrap()
                    .geometry()
                    .nurbs_curve_representation()?
                    .ok_or(CommandError::Usage(USAGE))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let brep = Brep::try_edge_surface(&curves, document.tolerance())?;
        document.add_geometry(Geometry::Brep(brep))?;
        Ok(format!(
            "Created edge surface from {} boundary curves",
            curves.len()
        ))
    }
}

#[cfg(test)]
mod tests;
