//! Pickable planar regions, staged output and partial input-curve deletion.
use super::*;
use viboceros_geometry::{PlanarCurveArrangement, Plane, curve_overlap_source_intervals};
mod options;
mod sources;
mod used;
pub use options::{DeleteInput, Input, Options, Output, parse};
pub use sources::Source;
#[cfg(test)]
mod tests;
const USAGE: &str = "CurveBoolean [Sources=id,...] [SubCrv=id,start,end | SubCrv=id,Edge,index,start,end | SubCrvLength=id,start,length] [AllRegions | world-point ...] [CombineRegions=Yes|No] [Output=Curves|Surfaces] [SimplifyCurves=Yes|No] [DeleteInput=None|All|Used]";
fn failure(message: &'static str) -> CommandError {
    CommandError::Usage(message)
}

/// Prepared intersection graph, reusable across picks and option changes.
pub struct Session {
    sources: Vec<viboceros_document::Object>,
    whole: Vec<sources::Whole>,
    projected_whole: Vec<NurbsCurve>,
    arrangement: PlanarCurveArrangement,
    selected: Vec<bool>,
    tolerance: Tolerance,
    layer: viboceros_document::LayerId,
    plane: Frame3,
    postselected: bool,
}
impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CurveBooleanSession")
            .field("sources", &self.sources.len())
            .field("selected", &self.selected)
            .finish()
    }
}
pub fn prepare(doc: &Document, ids: &[ObjectId], cplane: Frame3) -> Result<Session, CommandError> {
    prepare_with_inputs(
        doc,
        &ids.iter().copied().map(Source::Whole).collect::<Vec<_>>(),
        cplane,
    )
}

pub fn prepare_with_inputs(
    doc: &Document,
    inputs: &[Source],
    cplane: Frame3,
) -> Result<Session, CommandError> {
    let resolved = sources::resolve(doc, inputs)?;
    let curves = resolved.inputs;
    let sources = resolved.parents;
    // Coplanar inputs retain their common plane. Noncoplanar sources are
    // projected onto the active construction plane, preserving parameter charts.
    let points = curves
        .iter()
        .flat_map(|c| c.control_points().iter().map(|p| p.point()))
        .collect::<Vec<_>>();
    let plane = if let Some(plane) =
        viboceros_geometry::try_control_polygon_plane(&points, doc.tolerance())?
    {
        Frame3::try_from_unit_normal(plane.origin(), plane.normal())?
    } else {
        cplane
    };
    let projection =
        AffineTransform3::try_planar_projection(Plane::new(plane.origin(), plane.z_axis()))?;
    let projected = curves
        .iter()
        .map(|c| c.transformed(projection))
        .collect::<Result<Vec<_>, _>>()?;
    let whole = resolved
        .whole
        .into_iter()
        .map(|(parent, curve)| sources::Whole { parent, curve })
        .collect::<Vec<_>>();
    let projected_whole = whole
        .iter()
        .map(|w| w.curve.transformed(projection))
        .collect::<Result<Vec<_>, _>>()?;
    let arrangement = PlanarCurveArrangement::try_new(&projected, plane, doc.tolerance())?;
    if arrangement.cells().is_empty() {
        return Err(failure("CurveBoolean inputs do not enclose a region"));
    }
    let selected = vec![false; arrangement.cells().len()];
    Ok(Session {
        sources,
        whole,
        projected_whole,
        arrangement,
        selected,
        tolerance: doc.tolerance(),
        layer: doc.current_layer_id(),
        plane,
        postselected: false,
    })
}
impl Session {
    pub fn plane(&self) -> Frame3 {
        self.plane
    }
    /// Command-first getter picks are released before recording deleted inputs.
    pub fn set_postselected(&mut self) {
        self.postselected = true;
    }
    pub fn region_count(&self) -> usize {
        self.selected.len()
    }
    pub fn selected_count(&self) -> usize {
        self.selected.iter().filter(|yes| **yes).count()
    }
    pub fn all_regions(&mut self) {
        self.selected.fill(true);
    }
    pub fn toggle(&mut self, point: Point3) -> Result<bool, CommandError> {
        if let Some(index) = self.arrangement.cell_at(point)? {
            self.selected[index] = !self.selected[index];
            Ok(true)
        } else {
            Ok(false)
        }
    }
    pub fn is_current(&self, doc: &Document) -> bool {
        self.tolerance == doc.tolerance()
            && self.layer == doc.current_layer_id()
            && self.sources.iter().all(|old| {
                doc.is_object_selectable(old.id())
                    && doc.object(old.id()).is_some_and(|new| {
                        old.geometry_snapshot()
                            .shares_storage_with(new.geometry_snapshot())
                            && old.attributes() == new.attributes()
                            && old.group_ids() == new.group_ids()
                            && old.geometry_user_text() == new.geometry_user_text()
                    })
            })
    }
    pub fn stage(&self, options: Options) -> Result<Prepared, CommandError> {
        let regions = self.arrangement.selected(&self.selected, options.combine)?;
        let boundaries = regions
            .iter()
            .flat_map(|r| r.boundaries.iter().cloned())
            .collect::<Vec<_>>();
        let outputs = if options.output == Output::Surfaces {
            regions
                .into_iter()
                .map(|r| Geometry::Brep(r.brep))
                .collect()
        } else {
            boundaries
                .iter()
                .map(|boundary| {
                    let curve = Curve3::NurbsCurve(boundary.clone());
                    if options.simplify {
                        Ok(Geometry::from(
                            simplify_curve::simplified(&curve, self.tolerance)?.curve,
                        ))
                    } else {
                        Ok(Geometry::NurbsCurve(boundary.clone()))
                    }
                })
                .collect::<Result<Vec<_>, CommandError>>()?
        };
        let mut edits = Vec::new();
        if !boundaries.is_empty() && options.delete_input != DeleteInput::None {
            if options.delete_input == DeleteInput::All {
                edits = self
                    .whole
                    .iter()
                    .map(|input| (input.parent, Vec::new()))
                    .collect();
            } else {
                let intervals = curve_overlap_source_intervals(
                    &boundaries,
                    &self.projected_whole,
                    self.tolerance,
                )?;
                for (input, used) in self.whole.iter().zip(intervals) {
                    if used.is_empty() {
                        continue;
                    }
                    let pieces = used::remnants(&input.curve, used)?;
                    edits.push((input.parent, pieces));
                }
            }
        }
        Ok(Prepared {
            sources: self.sources.clone(),
            tolerance: self.tolerance,
            layer: self.layer,
            outputs,
            edits,
            release_before: options.delete_input == DeleteInput::None || self.postselected,
        })
    }
}
#[derive(Debug)]
pub struct Prepared {
    sources: Vec<viboceros_document::Object>,
    tolerance: Tolerance,
    layer: viboceros_document::LayerId,
    outputs: Vec<Geometry>,
    edits: Vec<(usize, Vec<Geometry>)>,
    release_before: bool,
}
impl Prepared {
    pub fn output_count(&self) -> usize {
        self.outputs.len()
    }
    fn apply(&self, doc: &mut Document) -> Result<Vec<ObjectId>, CommandError> {
        if self.tolerance != doc.tolerance()
            || self.layer != doc.current_layer_id()
            || !self.sources.iter().all(|old| {
                doc.is_object_selectable(old.id())
                    && doc.object(old.id()).is_some_and(|new| {
                        old.geometry_snapshot()
                            .shares_storage_with(new.geometry_snapshot())
                            && old.attributes() == new.attributes()
                            && old.group_ids() == new.group_ids()
                            && old.geometry_user_text() == new.geometry_user_text()
                    })
            })
        {
            return Err(failure(
                "CurveBoolean inputs or document settings changed; prepare again",
            ));
        }
        if self.release_before || self.outputs.is_empty() {
            doc.clear_selection();
        }
        for (i, pieces) in &self.edits {
            let source = &self.sources[*i];
            let id = source.id();
            doc.copy_object_pieces_with_metadata_into_source_groups(
                pieces
                    .iter()
                    .cloned()
                    .map(|g| (id, g, source.geometry_user_text().clone())),
            )?;
            doc.delete_object(id)?;
        }
        let outputs = self
            .outputs
            .iter()
            .cloned()
            .map(|g| doc.add_geometry(g))
            .collect::<Result<Vec<_>, _>>()?;
        doc.select_objects_direct(outputs.iter().copied(), SelectionMode::Replace)?;
        Ok(outputs)
    }
    pub fn commit(&self, doc: &mut Document) -> Result<Vec<ObjectId>, CommandError> {
        run_command_transaction(doc, "CurveBoolean", |doc| self.apply(doc))
    }
}
pub(super) struct CurveBooleanCommand(pub(super) std::sync::Arc<remembered::Remembered<Options>>);
impl Command for CurveBooleanCommand {
    fn name(&self) -> &'static str {
        "CurveBoolean"
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(doc, args, CommandContext::default())
    }
    fn run_in_context(
        &self,
        doc: &mut Document,
        args: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        self.execute(doc, args, context, false)
    }
    fn run_postselected(
        &self,
        doc: &mut Document,
        args: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        self.execute(doc, args, context, true)
    }
}
impl CurveBooleanCommand {
    fn execute(
        &self,
        doc: &mut Document,
        args: &[&str],
        context: CommandContext,
        postselected: bool,
    ) -> Result<String, CommandError> {
        let input = parse(args, self.0.get())?;
        let ids = input.sources.unwrap_or_else(|| {
            doc.selected_objects()
                .filter(|o| o.geometry().curve_ref().is_some())
                .map(|o| o.id())
                .collect()
        });
        let mut inputs = ids.into_iter().map(Source::Whole).collect::<Vec<_>>();
        inputs.extend(input.subcurves);
        let mut session = prepare_with_inputs(doc, &inputs, context.construction_plane)?;
        if postselected {
            session.set_postselected();
        }
        if input.all_regions {
            session.all_regions();
        }
        for point in input.points {
            session.toggle(point)?;
        }
        let prepared = session.stage(input.options)?;
        let count = prepared.output_count();
        prepared.apply(doc)?;
        self.0.set(input.options);
        Ok(format!("CurveBoolean: created {count} region object(s)"))
    }
}
impl CommandRegistry {
    pub fn curve_boolean_options(&self) -> Options {
        self.curve_boolean_preferences.get()
    }
    pub fn remember_curve_boolean_options(&self, options: Options) {
        self.curve_boolean_preferences.set(options);
    }
}
