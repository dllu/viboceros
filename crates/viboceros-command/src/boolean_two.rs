//! Prepared two-object Boolean choices and atomic document acceptance.
use super::*;
use std::sync::Arc;
use viboceros_geometry::{
    BrepBooleanOperation, BrepPolyhedralBooleanComponent, BrepPolyhedralBooleanPlan,
};

#[cfg(test)]
mod tests;

const USAGE: &str = "Boolean2Objects [Mode=Union|DifferenceAB|DifferenceBA|Intersection|InverseIntersection] [DeleteInput=Yes|No] [Sources=id,id]";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Union,
    DifferenceAB,
    DifferenceBA,
    Intersection,
    InverseIntersection,
}
impl Mode {
    pub const ALL: [Self; 5] = [
        Self::Union,
        Self::Intersection,
        Self::DifferenceAB,
        Self::DifferenceBA,
        Self::InverseIntersection,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Union => "Union",
            Self::DifferenceAB => "DifferenceAB",
            Self::DifferenceBA => "DifferenceBA",
            Self::Intersection => "Intersection",
            Self::InverseIntersection => "InverseIntersection",
        }
    }
    pub fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % 5]
    }
    fn index(self) -> usize {
        Self::ALL.iter().position(|&m| m == self).unwrap()
    }
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|m| m.name().eq_ignore_ascii_case(s.trim_start_matches('_')))
    }
}
#[derive(Clone, Debug)]
pub struct Piece {
    pub brep: Brep,
    pub owner: usize,
    /// Geometry-root user text survives only a single connected output branch.
    pub retain_geometry_user_text: bool,
}
#[derive(Clone, Debug)]
pub struct Candidates {
    values: [Arc<Vec<Piece>>; 5],
}
impl Candidates {
    pub fn get(&self, mode: Mode) -> Arc<Vec<Piece>> {
        self.values[mode.index()].clone()
    }
}
/// One exact arrangement, shared by every cyclic result. Output rounding never
/// supplies operands to another choice; source-face seams remain distinct.
pub fn prepare(
    first: &Brep,
    second: &Brep,
    tolerance: Tolerance,
) -> Result<Candidates, GeometryError> {
    let sheets = !first.is_solid() || !second.is_solid();
    let mut plan = if sheets {
        BrepPolyhedralBooleanPlan::try_with_planar_sheets(&[first, second], tolerance)?
    } else {
        if boolean_solids::interactions(&[first, second], tolerance, false)?
            .1
            .is_empty()
        {
            return Err(GeometryError::UnsupportedPolyhedralBrepBoolean {
                context: "two boundary-crossing operands required",
            });
        }
        BrepPolyhedralBooleanPlan::try_new(&[first, second], tolerance)?
    };
    if sheets && plan.inputs_are_coplanar(0, 1)? {
        let agrees = plan.coplanar_input_normals_agree(0, 1)?;
        let partition = plan
            .export_coplanar_sheet_partition(0, 1)?
            .into_iter()
            .map(|p| finish(p, 0, tolerance))
            .collect::<Result<Vec<_>, _>>()?;
        let remainder = plan
            .export_coplanar_sheet_boolean(BrepBooleanOperation::Difference, 0, 1)?
            .into_iter()
            .map(|p| finish(p, 0, tolerance))
            .collect::<Result<Vec<_>, _>>()?;
        let (common, difference) = if agrees {
            (partition, remainder)
        } else {
            (remainder, partition)
        };
        let inverse = difference
            .iter()
            .cloned()
            .chain(difference.iter().cloned())
            .collect();
        return Ok(Candidates {
            values: [
                common.clone(),
                common,
                difference.clone(),
                difference,
                inverse,
            ]
            .map(Arc::new),
        });
    }
    if sheets {
        // Rhino's no-intersection open workflow retains the finite sheets in
        // each mode, dropping closed operands and duplicating for inverse.
        let mut sheet_crossing = false;
        for (i, b) in [first, second].into_iter().enumerate() {
            if !b.is_solid() {
                sheet_crossing |= plan.input_boundary_crosses_region(i, 1 - i)?;
            }
        }
        if !sheet_crossing {
            let mut pieces = Vec::new();
            for b in [first, second].into_iter().filter(|b| !b.is_solid()) {
                for faces in b.edge_connected_face_components() {
                    pieces.push(Piece {
                        brep: b.duplicate_faces(&faces, tolerance)?,
                        owner: 0,
                        retain_geometry_user_text: true,
                    });
                }
            }
            let retain = pieces.len() == 1;
            for p in &mut pieces {
                p.retain_geometry_user_text = retain;
            }
            let inverse = pieces
                .iter()
                .cloned()
                .chain(pieces.iter().cloned())
                .collect();
            return Ok(Candidates {
                values: [
                    pieces.clone(),
                    pieces.clone(),
                    pieces.clone(),
                    pieces,
                    inverse,
                ]
                .map(Arc::new),
            });
        }
    }
    if sheets {
        for (index, brep) in [first, second].into_iter().enumerate() {
            if !brep.is_solid()
                && plan.input_boundary_crosses_region(index, 1 - index)?
                && !plan.planar_sheet_covers_input_section(index, 1 - index)?
            {
                return Err(GeometryError::UnsupportedPolyhedralBrepBoolean {
                    context: "complete physical sheet crossing required",
                });
            }
        }
    }
    let a = plan.input(0)?;
    let b = plan.input(1)?;
    let union = plan.combine(BrepBooleanOperation::Union, &[&a, &b])?;
    let ab = plan.combine(BrepBooleanOperation::Difference, &[&a, &b])?;
    let ba = plan.combine(BrepBooleanOperation::Difference, &[&b, &a])?;
    let common = plan.combine(BrepBooleanOperation::Intersection, &[&a, &b])?;
    if plan.is_empty(&common)? {
        return Err(GeometryError::UnsupportedPolyhedralBrepBoolean {
            context: "positive shared volume required",
        });
    }
    let mut values: [Vec<Piece>; 5] = std::array::from_fn(|_| Vec::new());
    for (index, region, owner) in [(0, &union, 0), (1, &common, 0), (2, &ab, 0), (3, &ba, 0)] {
        let output = if sheets {
            plan.export_physical_boundary(region)?
        } else {
            plan.export(region)?
        };
        for result in output {
            values[index].push(finish(result, owner, tolerance)?);
        }
    }
    for (region, owner) in [(&ba, 0), (&ab, 0)] {
        let output = if sheets {
            plan.export_physical_boundary(region)?
        } else {
            plan.export(region)?
        };
        for result in output {
            values[4].push(finish(result, owner, tolerance)?);
        }
    }
    Ok(Candidates {
        values: values.map(Arc::new),
    })
}
fn finish(
    piece: BrepPolyhedralBooleanComponent,
    owner: usize,
    tolerance: Tolerance,
) -> Result<Piece, GeometryError> {
    Ok(Piece {
        brep: boolean_solids::merged(piece.brep, &piece.face_sources, tolerance)?,
        owner,
        retain_geometry_user_text: true,
    })
}
pub(super) struct BooleanTwoCommand {
    delete_input: remembered::Remembered<bool>,
}
impl Default for BooleanTwoCommand {
    fn default() -> Self {
        Self {
            delete_input: remembered::Remembered::new(true),
        }
    }
}
#[derive(Clone, Copy)]
struct Options {
    mode: Mode,
    delete_input: bool,
    sources: Option<[ObjectId; 2]>,
}
impl BooleanTwoCommand {
    fn parse(&self, args: &[&str]) -> Result<Options, CommandError> {
        let mut options = Options {
            mode: Mode::Union,
            delete_input: self.delete_input.get(),
            sources: None,
        };
        let (mut index, mut mode_seen, mut delete_seen) = (0, false, false);
        while index < args.len() {
            let (name, value, consumed) = orient_option(args, index, USAGE)?;
            if option_name_eq(name, "Mode") && !mode_seen {
                options.mode = Mode::parse(value).ok_or(CommandError::Usage(USAGE))?;
                mode_seen = true;
            } else if option_name_eq(name, "DeleteInput") && !delete_seen {
                options.delete_input = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
                delete_seen = true;
            } else if option_name_eq(name, "Sources") && options.sources.is_none() {
                options.sources = Some(
                    boolean_solids::parse_ids(value, USAGE)?
                        .try_into()
                        .map_err(|_| CommandError::Usage(USAGE))?,
                );
            } else {
                return Err(CommandError::Usage(USAGE));
            }
            index += consumed;
        }
        Ok(options)
    }
}
impl Command for BooleanTwoCommand {
    fn name(&self) -> &'static str {
        "Boolean2Objects"
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let options = self.parse(args)?;
        self.delete_input.set(options.delete_input);
        let ids = if let Some(ids) = options.sources {
            ids
        } else {
            doc.selected_objects()
                .filter(|o| ObjectSelectionFilter::SurfaceComponents.accepts_object(o))
                .map(|o| o.id())
                .collect::<Vec<_>>()
                .try_into()
                .map_err(|_| CommandError::Usage(USAGE))?
        };
        let shapes = ids
            .iter()
            .map(|&id| -> Result<Brep, CommandError> {
                let object = doc
                    .object(id)
                    .filter(|_| doc.is_object_selectable(id))
                    .ok_or(DocumentError::ObjectNotFound(id))?;
                match object.geometry() {
                    Geometry::Brep(b) => Ok(b.clone()),
                    Geometry::NurbsSurface(s) => {
                        Ok(Brep::try_surface_face(s.clone(), doc.tolerance())?)
                    }
                    _ => Err(GeometryError::UnsupportedPolyhedralBrepBoolean {
                        context: "surface or polysurface input required",
                    }
                    .into()),
                }
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        let candidates = prepare(&shapes[0], &shapes[1], doc.tolerance())?;
        let pieces = candidates.get(options.mode);
        if pieces.is_empty() {
            return Err(CommandError::Usage("Selected Boolean result is empty"));
        }
        accept(doc, ids, &pieces, options.delete_input)?;
        Ok(format!("Boolean2Objects accepted {}", options.mode.name()))
    }
    fn cleanup_failed_selection(
        &self,
        doc: &mut Document,
        error: &CommandError,
        _postselected: bool,
    ) {
        if matches!(
            error,
            CommandError::Geometry(GeometryError::UnsupportedPolyhedralBrepBoolean { .. })
        ) {
            doc.clear_selection();
        }
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let options = self.parse(args)?;
        if options.sources.is_some()
            || args.iter().any(|s| {
                s.split('=')
                    .next()
                    .is_some_and(|k| option_name_eq(k, "Mode"))
            })
        {
            return Ok(None);
        }
        Ok(Some(ObjectSelectionPrompt {
            command: "Boolean2Objects",
            filter: ObjectSelectionFilter::SurfaceComponents,
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
            menus: vec![],
            choices: vec![],
            options: vec![BooleanSelectionOption {
                name: "DeleteInput",
                value: options.delete_input,
                aliases: &[],
            }],
        }))
    }
    fn accept_object_selection_options(&self, args: &[&str]) -> Result<(), CommandError> {
        self.delete_input.set(self.parse(args)?.delete_input);
        Ok(())
    }
}
/// Accept prepared choices without recomputing the exact arrangement.
/// The caller must own an active transaction and roll it back on failure.
pub fn accept(
    doc: &mut Document,
    ids: [ObjectId; 2],
    pieces: &[Piece],
    delete_input: bool,
) -> Result<(), CommandError> {
    if pieces.is_empty() || ids[0] == ids[1] || pieces.iter().any(|p| p.owner >= 2) {
        return Err(CommandError::Usage(
            "Select a nonempty two-object Boolean result",
        ));
    }
    let mut used = BTreeSet::new();
    let mut replacements = Vec::new();
    let mut copies = Vec::new();
    for piece in pieces {
        let id = ids[piece.owner];
        if delete_input && used.insert(id) {
            replacements.push((
                id,
                Geometry::Brep(piece.brep.clone()),
                piece.retain_geometry_user_text,
            ));
        } else {
            copies.push((
                id,
                Geometry::Brep(piece.brep.clone()),
                if piece.retain_geometry_user_text {
                    doc.object(id)
                        .ok_or(DocumentError::ObjectNotFound(id))?
                        .geometry_user_text()
                        .clone()
                } else {
                    BTreeMap::new()
                },
            ));
        }
    }
    doc.release_command_selection_on_history_replay(ids)?;
    if !copies.is_empty() {
        doc.copy_object_pieces_with_metadata_into_source_groups(copies)?;
    }
    if !replacements.is_empty() {
        let metadata = replacements
            .iter()
            .map(|(id, _, retain)| {
                (
                    *id,
                    *retain,
                    doc.object(*id).unwrap().geometry_user_text().clone(),
                )
            })
            .collect::<Vec<_>>();
        doc.replace_object_geometries(replacements.into_iter().map(|(id, g, _)| (id, g)))?;
        for (id, retain, text) in metadata {
            for (key, value) in text {
                doc.set_object_geometry_user_text([id], &key, retain.then_some(value.as_str()))?;
            }
        }
    }
    if delete_input {
        doc.delete_objects(ids.into_iter().filter(|id| !used.contains(id)))?;
    }
    doc.clear_selection();
    Ok(())
}
