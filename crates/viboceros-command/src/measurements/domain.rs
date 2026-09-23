//! Native parameter domains; no evaluation-driven reparameterization.
use super::*;
use crate::{ObjectSelectionFilter, ObjectSelectionPrompt, ObjectSelectionWorkflow, parse_point};

pub(crate) struct DomainCommand;
const USAGE: &str = "Domain [Face=index|point-on-selected-polysurface|SubCrv Parameter=start,end|SubCrv start_point end_point]";

#[cfg(test)]
mod tests;

impl Command for DomainCommand {
    fn name(&self) -> &'static str {
        "Domain"
    }
    fn records_history(&self) -> bool {
        false
    }
    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let subcurve = matches!(arguments, [option] if crate::option_name_eq(option, "SubCrv"));
        Ok(
            (arguments.is_empty() || subcurve).then_some(ObjectSelectionPrompt {
                command: "Domain",
                filter: if subcurve {
                    ObjectSelectionFilter::Curves
                } else {
                    ObjectSelectionFilter::Parametric
                },
                options: vec![],
                menus: vec![],
                choices: vec![],
                workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
            }),
        )
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let mut selected = document.selected_objects();
        let object = selected.next().ok_or(CommandError::NoObjectsSelected)?;
        if selected.next().is_some() {
            return Err(CommandError::Usage("Domain requires one selected object"));
        }
        let interval = |domain: std::ops::RangeInclusive<Real>| {
            format!(
                "[{},{}]",
                format_measurement(*domain.start()),
                format_measurement(*domain.end())
            )
        };
        if let Some(curve) = geometry_curve_ref(object.geometry()) {
            let domain = if arguments
                .first()
                .is_some_and(|arg| crate::option_name_eq(arg, "SubCrv"))
            {
                super::subcurve::parse_subcurve(
                    curve,
                    document.tolerance(),
                    &arguments[1..],
                    USAGE,
                )?
                .as_ref()
                .domain()
            } else {
                require_consumed(arguments, 0, "Domain")?;
                curve.domain()
            };
            return Ok(format!("Curve domain = {}", interval(domain)));
        }
        let (surface, face) = match object.geometry() {
            Geometry::NurbsSurface(surface) => {
                require_consumed(arguments, 0, "Domain")?;
                (surface, None)
            }
            Geometry::Brep(brep) => {
                let index = match arguments {
                    [] if brep.faces().len() == 1 => 0,
                    [option]
                        if option
                            .split_once('=')
                            .is_some_and(|(key, _)| crate::option_name_eq(key, "Face")) =>
                    {
                        option
                            .split_once('=')
                            .unwrap()
                            .1
                            .parse::<usize>()
                            .map_err(|_| CommandError::Usage(USAGE))?
                    }
                    [] => return Err(CommandError::Usage(USAGE)),
                    _ => {
                        let (point, consumed) = parse_point(arguments)?;
                        require_consumed(arguments, consumed, USAGE)?;
                        brep.closest_face_parameters(point, document.tolerance())?
                            .ok_or(CommandError::Usage(
                                "Domain could not locate a component surface",
                            ))?
                            .0
                    }
                };
                (
                    brep.faces()
                        .get(index)
                        .ok_or(CommandError::Usage("Domain face index is out of range"))?
                        .surface(),
                    Some(index),
                )
            }
            _ => return Err(CommandError::Usage("Domain requires a curve or surface")),
        };
        Ok(format!(
            "{}U domain = {}; V domain = {}",
            face.map_or_else(|| "Surface: ".into(), |i| format!("Face {i}: ")),
            interval(surface.domain_u()),
            interval(surface.domain_v())
        ))
    }
}
