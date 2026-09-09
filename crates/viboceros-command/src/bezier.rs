//! Exact Bezier conversion with fresh document attributes and unit domains.
use super::*;
use viboceros_geometry::MAX_BEZIER_CONTROL_POINTS;

#[cfg(test)]
mod tests;

const USAGE: &str = "ConvertToBeziers [DeleteInput=Yes|No]";
#[derive(Default)]
pub(super) struct ConvertToBeziersCommand {
    delete_input: remembered::Remembered<bool>,
}

impl Command for ConvertToBeziersCommand {
    fn name(&self) -> &'static str {
        "ConvertToBeziers"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        self.convert(document, arguments, false)
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::Beziers,
            options: vec![BooleanSelectionOption {
                name: "DeleteInput",
                value: self.parse(arguments)?,
                aliases: &[],
            }],
            menus: vec![],
            choices: vec![],
            workflow: ObjectSelectionWorkflow::ChooseBooleanAfterSelection,
        }))
    }

    fn object_selection_confirmation(
        &self,
        document: &Document,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        if !document
            .selected_objects()
            .any(|o| ObjectSelectionFilter::Beziers.accepts_object(o))
        {
            return Err(CommandError::UnsupportedConvertToBeziersGeometry);
        }
        self.object_selection_prompt(arguments)
    }

    fn accept_object_selection_options(&self, arguments: &[&str]) -> Result<(), CommandError> {
        // Cancellation has not answered Rhino's deletion question.
        self.parse(arguments).map(|_| ())
    }

    fn run_postselected(
        &self,
        document: &mut Document,
        arguments: &[&str],
        _context: CommandContext,
    ) -> Result<String, CommandError> {
        let message = self.convert(document, arguments, true)?;
        document.clear_selection();
        Ok(message)
    }
}

impl ConvertToBeziersCommand {
    fn parse(&self, arguments: &[&str]) -> Result<bool, CommandError> {
        if arguments.is_empty() {
            Ok(self.delete_input.get())
        } else {
            parse_delete_input(arguments, USAGE, &["DeleteInput"])
        }
    }

    fn convert(
        &self,
        document: &mut Document,
        arguments: &[&str],
        postselected: bool,
    ) -> Result<String, CommandError> {
        let delete_input = self.parse(arguments)?;
        if document.selected_object_ids().len() == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        let mut sources = Vec::new();
        let mut outputs = Vec::new();
        let mut controls = 0usize;
        let mut selected = document
            .objects()
            .filter(|o| document.is_selected(o.id()))
            .collect::<Vec<_>>();
        if postselected {
            let ranks = document
                .selected_object_ids()
                .enumerate()
                .map(|(rank, id)| (id, rank))
                .collect::<BTreeMap<_, _>>();
            selected.sort_unstable_by_key(|o| ranks[&o.id()]);
        }
        // Preselection follows document order; command-first picks keep their order.
        // Stage every result before the registry's transaction mutates anything.
        for object in selected {
            let pieces = match object.geometry() {
                Geometry::NurbsSurface(surface) => surface_pieces(surface)?,
                Geometry::Brep(brep) if brep.faces().len() == 1 => {
                    surface_pieces(brep.faces()[0].surface())?
                }
                Geometry::Brep(_) => continue,
                geometry => {
                    let Some(curve) = geometry.curve_ref() else {
                        continue;
                    };
                    curve
                        .to_nurbs()?
                        .try_bezier_spans()?
                        .into_iter()
                        .map(|piece| {
                            Ok(Geometry::NurbsCurve(piece.try_reparameterized(0.0..=1.0)?))
                        })
                        .collect::<Result<Vec<_>, GeometryError>>()?
                }
            };
            append_bounded(&mut outputs, pieces, &mut controls)?;
            sources.push(object.id());
        }
        if sources.is_empty() {
            return Err(CommandError::UnsupportedConvertToBeziersGeometry);
        }
        let count = outputs.len();
        for piece in outputs {
            document.add_geometry(piece)?;
        }
        if delete_input {
            for id in &sources {
                document.delete_object(*id)?;
            }
        }
        self.delete_input.set(delete_input);
        Ok(format!(
            "Converted {} object(s) into {count} exact Bezier object(s){}",
            sources.len(),
            if delete_input {
                ""
            } else {
                "; inputs retained"
            }
        ))
    }
}

fn surface_pieces(surface: &NurbsSurface) -> Result<Vec<Geometry>, GeometryError> {
    surface
        .try_bezier_patches()?
        .into_iter()
        .map(|piece| {
            Ok(Geometry::NurbsSurface(
                piece.try_reparameterized(0.0..=1.0, 0.0..=1.0)?,
            ))
        })
        .collect()
}

fn append_bounded(
    outputs: &mut Vec<Geometry>,
    pieces: Vec<Geometry>,
    controls: &mut usize,
) -> Result<(), CommandError> {
    for piece in &pieces {
        let count = match piece {
            Geometry::NurbsCurve(c) => c.control_points().len(),
            Geometry::NurbsSurface(s) => s.control_points().len(),
            _ => unreachable!("Bezier geometry"),
        };
        *controls = controls.saturating_add(count);
        if *controls > MAX_BEZIER_CONTROL_POINTS {
            return Err(GeometryError::BezierDecompositionLimit.into());
        }
    }
    outputs.extend(pieces);
    Ok(())
}
