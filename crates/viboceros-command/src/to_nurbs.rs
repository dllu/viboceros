//! Object representation conversion, including finite SubD approximation.
use super::*;

mod options;
#[cfg(test)]
mod subd_tests;
#[cfg(test)]
mod tests;
use options::{Kinds, Options, parse, prompt};

const MAX_OUTPUT_CONTROLS: usize = 1_048_576;

#[derive(Default)]
pub(super) struct ToNurbsCommand {
    options: remembered::Remembered<Options>,
}

impl Command for ToNurbsCommand {
    fn name(&self) -> &'static str {
        "ToNURBS"
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let (options, kinds) = parse(arguments, self.options.get())?;
        Ok(Some(prompt(options, kinds)))
    }

    fn object_selection_confirmation(
        &self,
        document: &Document,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let (options, required) = parse(arguments, self.options.get())?;
        let kinds = Kinds::selected(document);
        required.require(&kinds)?;
        let convertible = document.selected_objects().any(|o| {
            matches!(
                o.geometry(),
                Geometry::Line(_)
                    | Geometry::Circle(_)
                    | Geometry::Arc(_)
                    | Geometry::Polyline(_)
                    | Geometry::PolyCurve(_)
                    | Geometry::Mesh(_)
                    | Geometry::SubD(_)
            ) || matches!(o.geometry(), Geometry::Brep(b) if b.is_extrusion())
        });
        Ok(convertible.then(|| prompt(options, kinds)))
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        self.convert(document, arguments, false)
    }

    fn accept_object_selection_options(&self, arguments: &[&str]) -> Result<(), CommandError> {
        // ToNURBS commits choices only after a real conversion, not on prompt
        // edits or cancellation. Still validate the command-owned option syntax.
        parse(arguments, self.options.get()).map(|_| ())
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

impl ToNurbsCommand {
    fn convert(
        &self,
        document: &mut Document,
        arguments: &[&str],
        postselected: bool,
    ) -> Result<String, CommandError> {
        let (options, required) = parse(arguments, self.options.get())?;
        if document.selected_object_count() == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        required.require(&Kinds::selected(document))?;
        let mut eligible = 0;
        let mut controls = 0usize;
        let mut conversions = Vec::new();
        for object in document.objects().filter(|o| document.is_selected(o.id())) {
            let geometry = match object.geometry() {
                Geometry::Brep(brep) if brep.is_extrusion() => {
                    charge(
                        &mut controls,
                        brep.faces().iter().fold(0_usize, |n, f| {
                            n.saturating_add(f.surface().control_points().len())
                        }),
                    )?;
                    Geometry::Brep(brep.explicit_brep())
                }
                // Rhino stores an ellipse as a NURBS curve already. The compact
                // native analytic representation must not turn this into a copy.
                Geometry::Ellipse(_)
                | Geometry::NurbsCurve(_)
                | Geometry::NurbsSurface(_)
                | Geometry::Brep(_) => {
                    eligible += 1;
                    continue;
                }
                Geometry::Mesh(mesh) => {
                    charge(&mut controls, mesh.face_count().saturating_mul(4))?;
                    // ToNURBS retains disconnected pieces in one B-rep object;
                    // this intentionally differs from MeshToNURB's splitting.
                    Geometry::Brep(Brep::try_from_mesh(
                        mesh,
                        options.trim_triangles,
                        document.tolerance(),
                    )?)
                }
                Geometry::SubD(subd) => {
                    let brep = if options.packed {
                        subd.to_packed_nurbs_brep(options.continuity, document.tolerance())?
                    } else {
                        subd.to_nurbs_brep(options.continuity, document.tolerance())?
                    };
                    charge(
                        &mut controls,
                        brep.faces().iter().fold(0usize, |n, f| {
                            n.saturating_add(f.surface().control_points().len())
                        }),
                    )?;
                    Geometry::Brep(brep)
                }
                source => {
                    let Some(curve) = source.converted_to_nurbs_curve()? else {
                        continue;
                    };
                    charge(&mut controls, curve.control_points().len())?;
                    Geometry::NurbsCurve(curve)
                }
            };
            eligible += 1;
            conversions.push((object.id(), geometry));
        }
        if eligible == 0 {
            return Err(CommandError::UnsupportedToNurbsGeometry);
        }
        let converted = conversions.len();
        if postselected {
            let ranks = document
                .selected_object_ids()
                .enumerate()
                .map(|(i, id)| (id, i))
                .collect::<BTreeMap<_, _>>();
            conversions.sort_unstable_by_key(|(id, _)| ranks[id]);
            if converted > 0 {
                // Prompted picks are transient, including no-op peers. Clear
                // them before replacement captures selection in history.
                let picks = document.selected_object_ids().collect::<Vec<_>>();
                document.release_command_selection_on_history_replay(picks)?;
                document.clear_selection();
            }
        }
        if options.delete_input {
            let ids = conversions.iter().map(|(id, _)| *id).collect::<Vec<_>>();
            document.replace_object_geometries(conversions)?;
            if postselected {
                document.move_objects_to_end_in_order(ids)?;
            } else {
                document.move_objects_to_end(ids)?;
            }
        } else if postselected {
            document.copy_object_geometries_into_source_groups_in_order(conversions)?;
        } else {
            document.copy_object_geometries_into_source_groups(conversions)?;
        }
        // Rhino's ToNURBS no-op does not accept new choices, unlike
        // ConvertToSingleSpans. Only a real conversion updates option memory.
        if converted > 0 {
            self.options.set(options);
        }
        Ok(format!(
            "Converted {converted} object(s) to NURBS geometry; {} unchanged{}",
            eligible - converted,
            if options.delete_input {
                "; inputs replaced"
            } else {
                "; inputs retained"
            }
        ))
    }
}

fn charge(total: &mut usize, count: usize) -> Result<(), CommandError> {
    *total = total.saturating_add(count);
    if *total > MAX_OUTPUT_CONTROLS {
        return Err(CommandError::TooManyNurbsConversionControls {
            maximum: MAX_OUTPUT_CONTROLS,
        });
    }
    Ok(())
}
