//! Original cutter geometry survives replacement and local command Undo.
use super::*;
use viboceros_document::{GeometrySnapshot, HistoryGroup};

/// Original cutter inputs for successive MeshTrim picks. Capturing retains
/// geometry storage without creating document objects or copying mesh buffers.
/// Current targets and temporary plane definitions are supplied with each pick.
#[derive(Clone, Debug)]
pub struct MeshTrimCutters {
    pub(super) objects: Vec<(ObjectId, GeometrySnapshot)>,
    pub(super) origins: BTreeMap<ObjectId, ObjectId>,
}

impl MeshTrimCutters {
    pub fn capture(doc: &Document, ids: &[ObjectId]) -> Result<Self, CommandError> {
        if ids.len() > 128 || ids.iter().collect::<BTreeSet<_>>().len() != ids.len() {
            return Err(CommandError::Usage(USAGE));
        }
        let objects = ids
            .iter()
            .map(|&id| {
                let object = doc.object(id).ok_or(DocumentError::ObjectNotFound(id))?;
                if !doc.is_object_selectable(id)
                    || !ObjectSelectionFilter::MeshSplitCutters.accepts_object(object)
                {
                    return Err(CommandError::Usage("selectable MeshTrim cutters"));
                }
                Ok((id, object.geometry_snapshot().clone()))
            })
            .collect::<Result<_, CommandError>>()?;
        Ok(Self {
            objects,
            origins: BTreeMap::new(),
        })
    }

    /// Atomically accept one pick in a caller-owned MeshTrim history group.
    /// Failure rolls back only this pick; earlier steps and frozen cutters stay
    /// available. No transaction remains open between calls.
    pub fn trim(
        &mut self,
        doc: &mut Document,
        target: ObjectId,
        point: Point3,
        planes: &[String],
        context: CommandContext,
        group: &mut HistoryGroup,
    ) -> Result<String, CommandError> {
        let origin = self.origins.get(&target).copied().unwrap_or(target);
        doc.begin_group_transaction(group)?;
        match self.execute(doc, target, point, planes, context) {
            Ok((message, outputs)) => {
                doc.commit_group_transaction(group)?;
                for id in outputs {
                    self.origins.insert(id, origin);
                }
                Ok(message)
            }
            Err(error) => {
                doc.rollback_transaction()?;
                Err(error)
            }
        }
    }
}
