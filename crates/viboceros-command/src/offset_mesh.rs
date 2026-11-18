//! Offset selected polygon meshes by vertex normals or a world vector.

use super::*;
use std::sync::Arc;
use viboceros_document::Object;
use viboceros_geometry::MeshOffsetDirection;

const USAGE: &str = "OffsetMesh distance [DirectionMethod=UseVertexNormals|UserSelectedDirection] [Direction=x,y,z] [AverageNormals=Yes|No] [CPlaneNormal=x,y,z] [FlipAll=Yes|No] [Solid=Yes|No] [BothSides=Yes|No] [AllowDisjoint=Yes|No] [DeleteInput=Yes|No]";

pub(super) struct OffsetMeshCommand;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub distance: Real,
    pub direction: MeshOffsetDirection,
    pub flip_all: bool,
    pub solid: bool,
    pub both_sides: bool,
    pub allow_disjoint: bool,
    pub delete_input: bool,
}

/// Immutable mesh inputs shared by preview and acceptance.
#[derive(Clone, Debug)]
pub struct Sources {
    objects: Arc<[Object]>,
    tolerance: Tolerance,
}

#[derive(Debug)]
pub struct Prepared {
    sources: Sources,
    outputs: Vec<(ObjectId, Geometry)>,
}

impl Sources {
    pub fn capture(document: &Document) -> Result<Self, CommandError> {
        let objects = document.selected_objects().cloned().collect::<Vec<_>>();
        if objects.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        if objects
            .iter()
            .any(|object| !matches!(object.geometry(), Geometry::Mesh(_)))
        {
            return Err(CommandError::UnsupportedOffsetMeshGeometry);
        }
        Ok(Self {
            objects: objects.into(),
            tolerance: document.tolerance(),
        })
    }

    pub fn ids(&self) -> impl Iterator<Item = ObjectId> + '_ {
        self.objects.iter().map(Object::id)
    }

    fn meshes(&self) -> impl Iterator<Item = (ObjectId, &TriangleMesh)> {
        self.objects.iter().map(|object| {
            let Geometry::Mesh(mesh) = object.geometry() else {
                unreachable!("only meshes are captured");
            };
            (object.id(), mesh)
        })
    }

    pub fn bounds(&self) -> Result<BoundingBox3, CommandError> {
        let mut bounds = self.meshes().map(|(_, mesh)| mesh.bounds());
        let first = bounds.next().expect("at least one mesh is captured");
        Ok(bounds.try_fold(first, BoundingBox3::union)?)
    }

    /// Common guide axis for the interactive distance picker.
    pub fn reference_normal(&self, fallback: UnitVector3) -> Result<UnitVector3, CommandError> {
        let mut sum = [0.0; 3];
        let mut count = 0;
        for (_, mesh) in self.meshes() {
            for normal in mesh.computed_vertex_normals()? {
                for (sum, component) in sum.iter_mut().zip(normal.to_array()) {
                    *sum += component;
                }
                count += 1;
            }
        }
        if sum
            .iter()
            .all(|component| component.abs() <= 32.0 * Real::EPSILON * count as Real)
        {
            return Ok(fallback);
        }
        Ok(Vector3::try_from(sum)?.normalized_nonzero()?)
    }

    pub fn is_current(&self, document: &Document) -> bool {
        self.tolerance == document.tolerance()
            && document.selected_object_ids().eq(self.ids())
            && self.objects.iter().all(|old| {
                document.is_object_selectable(old.id())
                    && document.object(old.id()).is_some_and(|new| {
                        old.geometry_snapshot()
                            .shares_storage_with(new.geometry_snapshot())
                            && old.attributes() == new.attributes()
                            && old.group_ids() == new.group_ids()
                            && old.geometry_user_text() == new.geometry_user_text()
                    })
            })
    }

    pub fn prepare(
        &self,
        options: Options,
        flips: &BTreeSet<ObjectId>,
    ) -> Result<Prepared, CommandError> {
        if !options.distance.is_finite() || options.distance.abs() <= self.tolerance.absolute() {
            return Err(CommandError::Usage(USAGE));
        }
        let mut outputs = Vec::new();
        for (id, mesh) in self.meshes() {
            let flipped = options.flip_all ^ flips.contains(&id);
            let distance = if flipped {
                -options.distance
            } else {
                options.distance
            };
            let mut result = mesh.offset_mesh(
                distance,
                options.direction,
                options.both_sides,
                options.solid,
                self.tolerance,
            )?;
            // Solid skin and wall winding is determined by the signed offset.
            // Rhino additionally flips the winding of open skins for Flip.
            if flipped && !options.solid {
                result = result.reversed();
            }
            if options.both_sides && !options.solid {
                let first_skin_faces = (0..mesh.face_count()).collect::<Vec<_>>();
                let (second, first) = result.extract_faces(&first_skin_faces)?.into_parts();
                let second = second.expect("both offset skins exist");
                if !options.allow_disjoint {
                    let (first, second) = if matches!(
                        options.direction,
                        MeshOffsetDirection::AverageNormals { .. }
                    ) {
                        (second, first)
                    } else {
                        (first, second)
                    };
                    push_output(&mut outputs, id, first)?;
                    push_output(&mut outputs, id, second)?;
                    continue;
                }
                result = TriangleMesh::try_append(&[&second, &first])?;
            } else if options.allow_disjoint && !options.solid {
                result = if flipped {
                    TriangleMesh::try_append(&[&result, &mesh.reversed()])?
                } else {
                    TriangleMesh::try_append(&[&result, mesh])?
                };
            }
            push_output(&mut outputs, id, result)?;
        }
        Ok(Prepared {
            sources: self.clone(),
            outputs,
        })
    }
}

impl Prepared {
    pub fn meshes(&self) -> impl Iterator<Item = &TriangleMesh> {
        self.outputs.iter().map(|(_, geometry)| {
            let Geometry::Mesh(mesh) = geometry else {
                unreachable!("only meshes are prepared");
            };
            mesh
        })
    }

    pub fn commit(
        self,
        document: &mut Document,
        delete_input: bool,
        postselected: bool,
    ) -> Result<String, CommandError> {
        if !self.sources.is_current(document) {
            return Err(CommandError::OffsetMeshStale);
        }
        run_command_transaction(document, "OffsetMesh", |doc| {
            if postselected {
                doc.release_command_selection_on_history_replay(self.sources.ids())?;
            }
            doc.clear_selection();
            self.apply(doc, delete_input, false)
        })
    }

    fn apply(
        self,
        document: &mut Document,
        delete_input: bool,
        select_outputs: bool,
    ) -> Result<String, CommandError> {
        let count = self.outputs.len();
        let ids = document.copy_object_pieces_into_source_groups(self.outputs)?;
        if delete_input {
            document.delete_objects(self.sources.ids())?;
        }
        if select_outputs {
            document.select_objects_direct(ids, SelectionMode::Replace)?;
        }
        Ok(format!("Offset {count} mesh(es)"))
    }
}

fn push_output(
    outputs: &mut Vec<(ObjectId, Geometry)>,
    id: ObjectId,
    mesh: TriangleMesh,
) -> Result<(), CommandError> {
    if outputs.len() == MAX_SPAN_OUTPUT_OBJECTS {
        return Err(too_many_span_outputs("OffsetMesh"));
    }
    outputs.push((id, Geometry::Mesh(mesh)));
    Ok(())
}

impl Command for OffsetMeshCommand {
    fn name(&self) -> &'static str {
        "OffsetMesh"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(document, arguments, CommandContext::default())
    }

    fn run_in_context(
        &self,
        document: &mut Document,
        arguments: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let options = parse_positional(
            arguments,
            document.tolerance(),
            context.construction_plane.z_axis(),
        )?;
        let sources = Sources::capture(document)?;
        let prepared = sources.prepare(options, &BTreeSet::new())?;
        prepared.apply(document, options.delete_input, true)
    }

    fn object_selection_prompt(
        &self,
        _arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::Mesh,
            workflow: ObjectSelectionWorkflow::ConfirmAfterSelection,
            menus: vec![],
            choices: vec![],
            options: vec![],
        }))
    }
}

pub fn parse_positional(
    arguments: &[&str],
    tolerance: Tolerance,
    cplane_fallback: UnitVector3,
) -> Result<Options, CommandError> {
    let (first, rest) = arguments.split_first().ok_or(CommandError::Usage(USAGE))?;
    let distance = if let Some((name, value)) = first.split_once('=') {
        if !option_name_eq(name, "Distance") {
            return Err(CommandError::Usage(USAGE));
        }
        parse_finite_real(value)?
    } else {
        parse_finite_real(first)?
    };
    if distance.abs() <= tolerance.absolute() {
        return Err(CommandError::Usage(USAGE));
    }
    let (mut method, mut vector, mut average, mut cplane_normal) = (None, None, None, None);
    let (mut flip_all, mut solid, mut both_sides, mut allow_disjoint, mut delete_input) =
        (None, None, None, None, None);
    for argument in rest {
        let (name, value) = argument.split_once('=').ok_or(CommandError::Usage(USAGE))?;
        if option_name_eq(name, "DirectionMethod") {
            let mode = if option_name_eq(value, "UseVertexNormals") {
                false
            } else if option_name_eq(value, "UserSelectedDirection") {
                true
            } else {
                return Err(CommandError::Usage(USAGE));
            };
            if method.replace(mode).is_some() {
                return Err(CommandError::Usage(USAGE));
            }
        } else if option_name_eq(name, "Direction") || option_name_eq(name, "CPlaneNormal") {
            let parts = value.split(',').collect::<Vec<_>>();
            let [x, y, z] = parts.as_slice() else {
                return Err(CommandError::Usage(USAGE));
            };
            let coordinate =
                |text: &str| text.parse::<Real>().map_err(|_| CommandError::Usage(USAGE));
            let normal = Vector3::try_new(coordinate(x)?, coordinate(y)?, coordinate(z)?)
                .and_then(Vector3::normalized_nonzero)
                .map_err(|_| CommandError::Usage(USAGE))?;
            let slot = if option_name_eq(name, "Direction") {
                &mut vector
            } else {
                &mut cplane_normal
            };
            if slot.replace(normal).is_some() {
                return Err(CommandError::Usage(USAGE));
            }
        } else {
            let value = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
            let slot = if option_name_eq(name, "AverageNormals") {
                &mut average
            } else if option_name_eq(name, "FlipAll") {
                &mut flip_all
            } else if option_name_eq(name, "Solid") {
                &mut solid
            } else if option_name_eq(name, "BothSides") {
                &mut both_sides
            } else if option_name_eq(name, "AllowDisjoint") {
                &mut allow_disjoint
            } else if option_name_eq(name, "DeleteInput") {
                &mut delete_input
            } else {
                return Err(CommandError::Usage(USAGE));
            };
            if slot.replace(value).is_some() {
                return Err(CommandError::Usage(USAGE));
            }
        }
    }
    if method == Some(false) && vector.is_some()
        || method == Some(true) && average == Some(true)
        || method == Some(true) && vector.is_none()
        || method != Some(true) && vector.is_some()
        || cplane_normal.is_some() && average != Some(true)
    {
        return Err(CommandError::Usage(USAGE));
    }
    let fallback = cplane_normal.unwrap_or(cplane_fallback);
    let direction = if let Some(vector) = vector {
        MeshOffsetDirection::Vector(vector)
    } else if average.unwrap_or(false) {
        MeshOffsetDirection::AverageNormals { fallback }
    } else {
        MeshOffsetDirection::VertexNormals
    };
    Ok(Options {
        distance,
        direction,
        flip_all: flip_all.unwrap_or(false),
        solid: solid.unwrap_or(false),
        both_sides: both_sides.unwrap_or(false),
        allow_disjoint: allow_disjoint.unwrap_or(false),
        delete_input: delete_input.unwrap_or(false),
    })
}

#[cfg(test)]
mod tests;
