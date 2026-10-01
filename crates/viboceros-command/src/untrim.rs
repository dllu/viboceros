//! Whole-surface trim removal, independently staged before document edits.
use super::*;
use viboceros_document::ReplacementHistory;
use viboceros_geometry::BrepLoopType;

#[cfg(test)]
mod tests;

const QUESTION: &str =
    "Keep trim objects? Yes or No finishes; Enter uses the shown choice, Esc cancels";

#[derive(Clone, Copy)]
enum Scope {
    All,
    Border,
}

pub(super) struct UntrimCommand {
    scope: Scope,
    keep_trim_objects: remembered::Remembered<bool>,
}

impl UntrimCommand {
    pub(super) fn all() -> Self {
        Self {
            scope: Scope::All,
            keep_trim_objects: Default::default(),
        }
    }

    pub(super) fn border() -> Self {
        Self {
            scope: Scope::Border,
            keep_trim_objects: Default::default(),
        }
    }

    fn usage(&self) -> &'static str {
        match self.scope {
            Scope::All => "UntrimAll [KeepTrimObjects=Yes|No]",
            Scope::Border => "UntrimBorder [KeepTrimObjects=Yes|No]",
        }
    }

    fn parse(&self, arguments: &[&str]) -> Result<bool, CommandError> {
        if arguments.is_empty() {
            return Ok(self.keep_trim_objects.get());
        }
        if let [value] = arguments
            && let Some(keep) = parse_yes_no(value)
        {
            return Ok(keep);
        }
        let (name, value, consumed) = orient_option(arguments, 0, self.usage())?;
        require_consumed(arguments, consumed, self.usage())?;
        if !option_name_eq(name, "KeepTrimObjects") {
            return Err(CommandError::Usage(self.usage()));
        }
        parse_yes_no(value).ok_or(CommandError::Usage(self.usage()))
    }

    fn untrim(
        &self,
        document: &mut Document,
        arguments: &[&str],
        postselected: bool,
    ) -> Result<String, CommandError> {
        let keep = self.parse(arguments)?;
        if document.selected_object_count() == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        let tolerance = document.tolerance();
        let mut sources = document.selected_objects().collect::<Vec<_>>();
        if postselected {
            let ranks = document
                .selected_object_ids()
                .enumerate()
                .map(|(rank, id)| (id, rank))
                .collect::<BTreeMap<_, _>>();
            sources.sort_unstable_by_key(|object| ranks[&object.id()]);
        }
        let mut staged = Vec::new();
        let mut count = 0usize;
        for object in sources {
            let converted;
            let brep = match object.geometry() {
                Geometry::NurbsSurface(surface) => {
                    converted = Brep::try_surface_face_with_native_edge_parameters(
                        surface.clone(),
                        tolerance,
                    )?;
                    &converted
                }
                Geometry::Brep(brep) if brep.faces().len() == 1 => brep,
                _ => continue,
            };
            let face = &brep.faces()[0];
            let untrimmed = match self.scope {
                Scope::All => brep.try_untrim_all(tolerance)?,
                Scope::Border => brep.try_untrim_outer_boundary(tolerance)?,
            };
            let mut curves = Vec::new();
            if keep {
                for (index, boundary) in face.loops().iter().enumerate() {
                    if matches!(self.scope, Scope::Border)
                        && boundary.loop_type() != BrepLoopType::Outer
                    {
                        continue;
                    }
                    for mut component in brep.loop_boundary_curve_components(0, index)? {
                        // Retained trims follow the surface's UV winding;
                        // face normal reversal does not reverse those curves.
                        if (boundary.loop_type() == BrepLoopType::Outer) ^ face.is_reversed() {
                            component = component
                                .into_iter()
                                .rev()
                                .map(|curve| curve.reversed())
                                .collect::<Result<_, _>>()?;
                        }
                        if component.len() > 1
                            && component.iter().all(|curve| {
                                curve.degree() == 1
                                    && curve
                                        .control_points()
                                        .iter()
                                        .all(|control| control.weight() > 0.)
                            })
                            && component
                                .iter()
                                .any(|curve| curve.control_points().len() > 2)
                        {
                            // Joining several degree-one boundaries returns a
                            // chord-parameterized polyline, including interior
                            // knots on a boundary. A single closed source edge
                            // instead retains its own knots and domain.
                            component = component
                                .iter()
                                .map(NurbsCurve::try_bezier_spans)
                                .collect::<Result<Vec<_>, _>>()?
                                .into_iter()
                                .flatten()
                                .collect();
                            let last = boundary.trims().iter().rev().find(|trim| {
                                matches!(
                                    trim.trim_type(),
                                    viboceros_geometry::BrepTrimType::Boundary
                                        | viboceros_geometry::BrepTrimType::Mated
                                )
                            });
                            if let Some(trim) = last {
                                let edge =
                                    &brep.edges()[trim.edge().expect("validated non-seam trim")];
                                let seam = edge.curve().evaluate(if trim.is_reversed_3d() {
                                    *edge.curve().domain().end()
                                } else {
                                    *edge.curve().domain().start()
                                })?;
                                if let Some(start) = component.iter().position(|curve| {
                                    curve
                                        .evaluate(*curve.domain().start())
                                        .is_ok_and(|point| point == seam)
                                }) {
                                    component.rotate_left(start);
                                }
                            }
                        }
                        curves.push(border::assemble(component, tolerance)?);
                    }
                }
            }
            count = count
                .checked_add(curves.len())
                .filter(|count| *count <= MAX_SPAN_OUTPUT_OBJECTS)
                .ok_or_else(|| too_many_span_outputs(self.name()))?;
            staged.push((
                object.id(),
                Geometry::Brep(untrimmed),
                curves,
                match self.scope {
                    Scope::All => face.loops().len(),
                    Scope::Border => 1,
                },
            ));
        }
        if staged.is_empty() {
            return Err(match self.scope {
                Scope::All => CommandError::UnsupportedUntrimAllGeometry,
                Scope::Border => CommandError::UnsupportedUntrimBorderGeometry,
            });
        }
        let objects = staged.len();
        let loops: usize = staged.iter().map(|(_, _, _, loops)| loops).sum();
        if postselected {
            document.clear_selection();
        }
        // Rhino appends retained curves before renewing each source object.
        // New curves have current-layer defaults and no source memberships.
        let mut replacements = Vec::with_capacity(objects);
        let mut order = Vec::with_capacity(objects + count);
        for (id, geometry, curves, _) in staged {
            for curve in curves {
                order.push(document.add_geometry(curve)?);
            }
            replacements.push((id, geometry));
            order.push(id);
        }
        document.replace_object_geometries_with_history(
            replacements,
            ReplacementHistory::EveryReplacement,
        )?;
        document.move_objects_to_end_in_order(order)?;
        self.keep_trim_objects.set(keep);
        Ok(format!(
            "Untrimmed {loops} loop(s) in {objects} surface(s); retained {count} trim curve(s)"
        ))
    }
}

impl Command for UntrimCommand {
    fn name(&self) -> &'static str {
        match self.scope {
            Scope::All => "UntrimAll",
            Scope::Border => "UntrimBorder",
        }
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::Surfaces,
            options: vec![BooleanSelectionOption {
                name: "KeepTrimObjects",
                value: self.parse(arguments)?,
                aliases: &[],
            }],
            menus: vec![],
            choices: vec![],
            workflow: ObjectSelectionWorkflow::QuestionOnPreselection { message: QUESTION },
        }))
    }

    fn object_selection_confirmation(
        &self,
        _document: &Document,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        self.object_selection_prompt(arguments)
    }

    fn accept_object_selection_options(&self, arguments: &[&str]) -> Result<(), CommandError> {
        self.keep_trim_objects.set(self.parse(arguments)?);
        Ok(())
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        self.untrim(document, arguments, false)
    }

    fn run_postselected(
        &self,
        document: &mut Document,
        arguments: &[&str],
        _context: CommandContext,
    ) -> Result<String, CommandError> {
        self.untrim(document, arguments, true)
    }

    fn cleanup_failed_selection(
        &self,
        document: &mut Document,
        error: &CommandError,
        _postselected: bool,
    ) {
        if matches!(
            error,
            CommandError::UnsupportedUntrimAllGeometry
                | CommandError::UnsupportedUntrimBorderGeometry
        ) {
            document.clear_selection();
        }
    }
}
