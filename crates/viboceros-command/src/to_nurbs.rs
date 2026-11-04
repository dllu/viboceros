//! Object representation conversion, including finite SubD approximation.
use super::*;

mod edges;
mod options;
mod packs;
mod targets;
pub use packs::{FacePackPreview, FacePackView};
pub use targets::{ToNurbsContext, ToNurbsTarget};
#[cfg(test)]
mod subd_tests;
#[cfg(test)]
mod target_tests;
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
        let (arguments, selectors) = targets::split(arguments)?;
        let (options, kinds) = parse(&arguments, self.options.get())?;
        if selectors.is_some() {
            return Ok(None);
        }
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
    fn to_nurbs_target_confirmation(
        &self,
        document: &Document,
        arguments: &[&str],
        targets: &[ToNurbsTarget],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let (options, required) = parse(arguments, self.options.get())?;
        let input = targets::Input::prepare(document, Some(targets), false)?;
        required.require(&input.kinds)?;
        let real = input.order.iter().any(|t| match t {
            ToNurbsTarget::Edge(..) => true,
            ToNurbsTarget::Whole(id) => {
                let geometry = document.object(*id).unwrap().geometry();
                matches!(
                    geometry,
                    Geometry::Line(_)
                        | Geometry::Circle(_)
                        | Geometry::Arc(_)
                        | Geometry::Polyline(_)
                        | Geometry::PolyCurve(_)
                        | Geometry::Mesh(_)
                        | Geometry::SubD(_)
                ) || matches!(geometry,Geometry::Brep(b) if b.is_extrusion())
            }
        });
        Ok(real.then(|| prompt(options, input.kinds)))
    }
    fn run_to_nurbs_targets(
        &self,
        document: &mut Document,
        arguments: &[&str],
        targets: &[ToNurbsTarget],
        context: ToNurbsContext,
    ) -> Result<String, CommandError> {
        let message = self.convert_targets(
            document,
            arguments,
            context.postselected,
            Some(targets),
            context.control_net,
        )?;
        if context.postselected {
            document.clear_selection();
        }
        Ok(message)
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
        let (arguments, selectors) = targets::split(arguments)?;
        let targets = selectors.map(|s| s.resolve(document)).transpose()?;
        let control_net = document.subd_control_net();
        self.convert_targets(
            document,
            &arguments,
            postselected,
            targets.as_deref(),
            control_net,
        )
    }
    fn convert_targets(
        &self,
        document: &mut Document,
        arguments: &[&str],
        postselected: bool,
        targets: Option<&[ToNurbsTarget]>,
        control_net: bool,
    ) -> Result<String, CommandError> {
        let (options, required) = parse(arguments, self.options.get())?;
        let input = targets::Input::prepare(document, targets, postselected)?;
        required.require(&input.kinds)?;
        let mut conversions = edges::prepare(document, &input, control_net)?;
        let mut eligible = conversions.len();
        let mut controls = 0usize;
        for (_, geometry) in &conversions {
            if let Geometry::NurbsCurve(c) = geometry {
                charge(&mut controls, c.control_points().len())?;
            }
        }
        let mut repacked_sources = Vec::new();
        let mut pack_bytes = 0usize;
        for object in document.objects().filter(|o| input.whole.contains(&o.id())) {
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
                    let repacked = options
                        .force_packs
                        .then(|| subd.try_repacked_faces())
                        .transpose()?;
                    if let Some(source) = &repacked {
                        pack_bytes = pack_bytes.saturating_add(source.archive().len());
                        if pack_bytes > 64 * 1024 * 1024 {
                            return Err(CommandError::FacePackingResourceLimit);
                        }
                    }
                    let subd = repacked.as_ref().unwrap_or(subd);
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
                    if let Some(repacked) = repacked
                        && !options.delete_input
                    {
                        repacked_sources.push((object.id(), Geometry::SubD(repacked)));
                    }
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
            conversions.push((ToNurbsTarget::Whole(object.id()), geometry));
        }
        if eligible == 0 {
            return Err(CommandError::UnsupportedToNurbsGeometry);
        }
        let converted = conversions.len();
        let ranks = input
            .order
            .iter()
            .enumerate()
            .map(|(i, &t)| (t, i))
            .collect::<BTreeMap<_, _>>();
        conversions.sort_unstable_by_key(|(t, _)| ranks[t]);
        if postselected {
            repacked_sources.sort_unstable_by_key(|(id, _)| ranks[&ToNurbsTarget::Whole(*id)]);
            if converted > 0 {
                // Prompted picks are transient, including no-op peers. Clear
                // them before replacement captures selection in history.
                let picks = document.selected_object_ids().collect::<Vec<_>>();
                document.release_command_selection_on_history_replay(picks)?;
                document.clear_selection();
            }
        }
        let repacked_ids = repacked_sources
            .iter()
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();
        document.replace_object_geometries_with_history(
            repacked_sources,
            viboceros_document::ReplacementHistory::EveryReplacement,
        )?;
        document.move_objects_to_end_in_order(repacked_ids)?;
        let order = conversions.iter().map(|(t, _)| *t).collect::<Vec<_>>();
        let has_edges = order.iter().any(|t| matches!(t, ToNurbsTarget::Edge(..)));
        let (mut replacements, mut copies) = (Vec::new(), Vec::new());
        for (target, geometry) in conversions {
            if matches!(target, ToNurbsTarget::Whole(_)) && options.delete_input {
                replacements.push((target.object(), geometry));
            } else {
                copies.push((target.object(), geometry));
            }
        }
        document.replace_object_geometries(replacements)?;
        let mut copied = document
            .copy_object_pieces_into_source_groups(copies)?
            .into_iter();
        let outputs = order
            .into_iter()
            .map(|t| {
                if matches!(t, ToNurbsTarget::Whole(_)) && options.delete_input {
                    t.object()
                } else {
                    copied.next().unwrap()
                }
            })
            .collect::<Vec<_>>();
        document.move_objects_to_end_in_order(outputs)?;
        // Rhino's ToNURBS no-op does not accept new choices, unlike
        // ConvertToSingleSpans. Only a real conversion updates option memory.
        if converted > 0 {
            self.options.set(options.remembered());
        }
        Ok(format!(
            "Converted {converted} object(s) to NURBS geometry; {} unchanged{}",
            eligible - converted,
            if options.delete_input && has_edges && !input.whole.is_empty() {
                "; whole inputs replaced; edge owners retained"
            } else if options.delete_input && !has_edges {
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
