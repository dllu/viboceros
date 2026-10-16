//! Two ordered rail picks followed by cross sections, or explicit named rails.
use super::*;

const NAME: &str = "Sweep2";
const USAGE: &str = "Sweep2 [Rail1Name=name Rail2Name=name] (select two rails first, then at least two cross sections)";
pub(super) struct SweepTwoRailsCommand;

fn parse<'a>(args: &[&'a str]) -> Result<Option<[&'a str; 2]>, CommandError> {
    let mut names = [None, None];
    for arg in args {
        let (key, value) = arg.split_once('=').ok_or(CommandError::Usage(USAGE))?;
        let index = if option_name_eq(key, "Rail1Name") {
            0
        } else if option_name_eq(key, "Rail2Name") {
            1
        } else {
            return Err(CommandError::Usage(USAGE));
        };
        if value.is_empty() || names[index].replace(value).is_some() {
            return Err(CommandError::Usage(USAGE));
        }
    }
    match names {
        [None, None] => Ok(None),
        [Some(a), Some(b)] => Ok(Some([a, b])),
        _ => Err(CommandError::Usage(USAGE)),
    }
}

impl Command for SweepTwoRailsCommand {
    fn name(&self) -> &'static str {
        NAME
    }

    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        parse(args)?;
        Ok(Some(ObjectSelectionPrompt {
            command: NAME,
            filter: ObjectSelectionFilter::Curves,
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
            options: vec![],
            menus: vec![],
            choices: vec![],
        }))
    }

    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let names = parse(args)?;
        let selected = selected_ids(doc)?;
        let rails = if let Some(names) = names {
            [
                extrude_along_curve::resolve_curve_along_curve_path(
                    doc,
                    &selected,
                    Some(names[0]),
                )?,
                extrude_along_curve::resolve_curve_along_curve_path(
                    doc,
                    &selected,
                    Some(names[1]),
                )?,
            ]
        } else {
            if selected.len() < 4 {
                return Err(CommandError::Usage(USAGE));
            }
            [selected[0], selected[1]]
        };
        if rails[0] == rails[1] {
            return Err(CommandError::Usage(USAGE));
        }
        let sections = selected
            .iter()
            .copied()
            .filter(|id| !rails.contains(id))
            .collect::<Vec<_>>();
        if !(2..=32).contains(&sections.len()) {
            return Err(CommandError::Usage(USAGE));
        }
        let curves = rails
            .into_iter()
            .chain(sections.iter().copied())
            .map(|id| {
                if !doc.is_object_selectable(id) {
                    return Err(CommandError::Usage(USAGE));
                }
                doc.object(id)
                    .ok_or(CommandError::Usage(USAGE))?
                    .geometry()
                    .nurbs_curve_representation()?
                    .ok_or(CommandError::Usage(USAGE))
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        let surface = construct(&curves[0], &curves[1], &curves[2..], doc.tolerance())?;
        let [u, v] = surface.sampled_kink_parameters(doc.tolerance().angular())?;
        if (u.len() + 1).saturating_mul(v.len() + 1) > 4096 {
            return Err(CommandError::Usage("Sweep2 output exceeds 4096 faces"));
        }
        let output = Brep::try_surface_grid(&surface, &u, &v, doc.tolerance())?;
        let count = output.faces().len();
        doc.add_geometry(Geometry::Brep(output))?;
        Ok(format!(
            "Swept {} sections along two rails into one BRep with {count} faces",
            sections.len()
        ))
    }
}

fn construct(
    first: &NurbsCurve,
    second: &NurbsCurve,
    sections: &[NurbsCurve],
    tolerance: Tolerance,
) -> Result<NurbsSurface, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
        Ok(viboceros_smlib::two_rail_sweep_surface(
            first, second, sections, tolerance,
        )?)
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        let _ = (first, second, sections, tolerance);
        Err(CommandError::Usage(
            "Sweep2 requires the native-smlib feature",
        ))
    }
}

#[cfg(test)]
mod tests;
