//! Immediate edge-driven Untrim, staged before atomic document/history edits.
use super::*;
use viboceros_document::{GeometrySnapshot, HistoryGroup, ReplacementHistory};
use viboceros_geometry::BrepLoopType;

#[cfg(test)]
mod tests;

const USAGE: &str = "Untrim object-id edge-index [AllSimilar=Yes|No] [KeepTrimObjects=Yes|No]";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UntrimOptions {
    pub all_similar: bool,
    pub keep_trim_objects: bool,
}
impl UntrimOptions {
    pub fn updated(self, input: &str) -> Result<Self, CommandError> {
        parse_options(&input.split_whitespace().collect::<Vec<_>>(), self)
    }
    pub fn command_line(self) -> String {
        format!(
            "Untrim AllSimilar={} KeepTrimObjects={}",
            if self.all_similar { "Yes" } else { "No" },
            if self.keep_trim_objects { "Yes" } else { "No" }
        )
    }
}
fn parse_options(
    arguments: &[&str],
    mut options: UntrimOptions,
) -> Result<UntrimOptions, CommandError> {
    let mut index = 0;
    while index < arguments.len() {
        let (name, value, consumed) = orient_option(arguments, index, USAGE)?;
        let value = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
        if option_name_eq(name, "AllSimilar") {
            options.all_similar = value;
        } else if option_name_eq(name, "KeepTrimObjects") {
            options.keep_trim_objects = value;
        } else {
            return Err(CommandError::Usage(USAGE));
        }
        index += consumed;
    }
    Ok(options)
}

#[derive(Clone, Debug)]
pub struct UntrimSelection {
    object: ObjectId,
    source: GeometrySnapshot,
    tolerance: Tolerance,
    replacement: Option<Brep>,
    retained: Vec<Geometry>,
    boundaries: usize,
    walls: usize,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UntrimResult {
    pub changed: bool,
    pub retained: Vec<ObjectId>,
    pub restored_boundaries: usize,
    pub removed_faces: usize,
}
impl UntrimSelection {
    pub fn prepare(
        document: &Document,
        object: ObjectId,
        edge: usize,
        options: UntrimOptions,
    ) -> Result<Self, CommandError> {
        if !document.is_object_selectable(object) {
            return Err(CommandError::UntrimUnavailable);
        }
        let source = document
            .object(object)
            .ok_or(CommandError::UntrimUnavailable)?
            .geometry_snapshot()
            .clone();
        let tolerance = document.tolerance();
        let converted;
        let brep = match &*source {
            Geometry::Brep(brep) => brep,
            Geometry::NurbsSurface(surface) => {
                converted =
                    Brep::try_surface_face_with_native_edge_parameters(surface.clone(), tolerance)?;
                &converted
            }
            _ => return Err(CommandError::UntrimUnavailable),
        };
        if edge >= brep.edges().len() {
            return Err(CommandError::UntrimUnavailable);
        }
        let mut uses = brep
            .faces()
            .iter()
            .enumerate()
            .flat_map(|(face, record)| {
                record
                    .loops()
                    .iter()
                    .enumerate()
                    .flat_map(move |(boundary, ring)| {
                        ring.trims()
                            .iter()
                            .enumerate()
                            .filter_map(move |(trim, record)| {
                                (record.edge() == Some(edge)).then_some((face, boundary, trim))
                            })
                    })
            })
            .collect::<Vec<_>>();
        // A joined hole rim also bounds its wall. The opening determines the
        // operation; neighboring walls are found through exact shared topology.
        uses.sort_by_key(|&(face, boundary, trim)| {
            (
                brep.faces()[face].loops()[boundary].loop_type() != BrepLoopType::Inner,
                face,
                boundary,
                trim,
            )
        });
        let &(face, boundary, trim) = uses.first().ok_or(CommandError::UntrimUnavailable)?;
        let restoration =
            brep.try_untrim_boundary(face, boundary, trim, options.all_similar, tolerance)?;
        let mut retained = Vec::new();
        let mut boundaries = 0;
        let mut walls = 0;
        let replacement = if let Some(restoration) = restoration {
            boundaries = restoration.removed_boundaries().len();
            walls = restoration.removed_faces().len();
            if options.keep_trim_objects {
                let wall_edges = restoration
                    .removed_faces()
                    .iter()
                    .flat_map(|&face| {
                        brep.faces()[face]
                            .loops()
                            .iter()
                            .flat_map(|ring| ring.trims())
                            .filter_map(|trim| trim.edge())
                    })
                    .collect::<BTreeSet<_>>();
                for &(face, boundary) in restoration.removed_boundaries() {
                    if brep.faces()[face].loops()[boundary]
                        .trims()
                        .iter()
                        .any(|trim| trim.edge().is_some_and(|edge| wall_edges.contains(&edge)))
                    {
                        continue;
                    }
                    if brep.faces()[face].loops()[boundary].loop_type() == BrepLoopType::Inner {
                        retained
                            .extend(untrim_holes::retained_hole_geometry(brep, face, boundary)?);
                    } else {
                        for mut component in brep.loop_boundary_curve_components(face, boundary)? {
                            if !brep.faces()[face].is_reversed() {
                                component = component
                                    .into_iter()
                                    .rev()
                                    .map(|curve| curve.reversed())
                                    .collect::<Result<_, _>>()?;
                            }
                            retained.push(border::assemble(component, tolerance)?);
                        }
                    }
                }
                if walls != 0 {
                    let detached = brep.sub_brep(restoration.removed_faces(), tolerance)?;
                    for component in detached.edge_connected_face_components() {
                        retained.push(Geometry::Brep(detached.sub_brep(&component, tolerance)?));
                    }
                }
                if retained.len() > MAX_SPAN_OUTPUT_OBJECTS {
                    return Err(too_many_span_outputs("Untrim"));
                }
            }
            Some(restoration.into_brep())
        } else {
            None
        };
        Ok(Self {
            object,
            source,
            tolerance,
            replacement,
            retained,
            boundaries,
            walls,
        })
    }
    pub fn changes_geometry(&self) -> bool {
        self.replacement.is_some()
    }
    pub fn validate_source(&self, document: &Document) -> Result<(), CommandError> {
        if document.tolerance() != self.tolerance
            || !document.is_object_selectable(self.object)
            || document
                .object(self.object)
                .is_none_or(|object| object.geometry_snapshot() != &self.source)
        {
            return Err(CommandError::UntrimStale);
        }
        Ok(())
    }
    pub fn commit(&self, document: &mut Document) -> Result<UntrimResult, CommandError> {
        run_command_transaction(document, "Untrim", |document| self.apply(document))
    }
    pub fn commit_in_group(
        &self,
        document: &mut Document,
        group: &mut HistoryGroup,
    ) -> Result<UntrimResult, CommandError> {
        document.begin_group_transaction(group)?;
        match self.apply(document) {
            Ok(result) => {
                document.commit_group_transaction(group)?;
                Ok(result)
            }
            Err(error) => {
                document.rollback_transaction()?;
                Err(error)
            }
        }
    }
    fn apply(&self, document: &mut Document) -> Result<UntrimResult, CommandError> {
        self.validate_source(document)?;
        let mut result = UntrimResult {
            changed: false,
            retained: vec![],
            restored_boundaries: self.boundaries,
            removed_faces: self.walls,
        };
        let Some(replacement) = &self.replacement else {
            return Ok(result);
        };
        document.clear_selection();
        for geometry in &self.retained {
            result
                .retained
                .push(document.add_geometry(geometry.clone())?);
        }
        document.replace_object_geometries_with_history(
            [(self.object, Geometry::Brep(replacement.clone()))],
            ReplacementHistory::EveryReplacement,
        )?;
        document
            .move_objects_to_end_in_order(result.retained.iter().copied().chain([self.object]))?;
        result.changed = true;
        Ok(result)
    }
}

#[derive(Default)]
pub(super) struct UntrimEdgeCommand {
    options: remembered::Remembered<UntrimOptions>,
}
impl Command for UntrimEdgeCommand {
    fn name(&self) -> &'static str {
        "Untrim"
    }
    fn component_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ComponentSelectionPrompt>, CommandError> {
        if arguments
            .first()
            .is_some_and(|argument| argument.parse::<ObjectId>().is_ok())
        {
            return Ok(None);
        }
        let options = parse_options(arguments, self.options.get())?;
        Ok(Some(ComponentSelectionPrompt {
            command: self.name(),
            kind: ComponentSelectionKind::BrepEdge,
            options: vec![
                BooleanSelectionOption {
                    name: "AllSimilar",
                    aliases: &[],
                    value: options.all_similar,
                },
                BooleanSelectionOption {
                    name: "KeepTrimObjects",
                    aliases: &[],
                    value: options.keep_trim_objects,
                },
            ],
            numbers: vec![],
        }))
    }
    fn accept_object_selection_options(&self, arguments: &[&str]) -> Result<(), CommandError> {
        self.options
            .set(parse_options(arguments, self.options.get())?);
        Ok(())
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if arguments.len() < 2 {
            return Err(CommandError::Usage(USAGE));
        }
        let object = arguments[0]
            .parse()
            .map_err(|_| CommandError::Usage(USAGE))?;
        let edge = arguments[1]
            .parse()
            .map_err(|_| CommandError::Usage(USAGE))?;
        let options = parse_options(&arguments[2..], self.options.get())?;
        let result = UntrimSelection::prepare(document, object, edge, options)?.apply(document)?;
        self.options.set(options);
        Ok(format!(
            "Restored {} boundary loop(s) and removed {} wall face(s); retained {} trim object(s)",
            result.restored_boundaries,
            result.removed_faces,
            result.retained.len()
        ))
    }
}
