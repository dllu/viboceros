//! Immutable face-pack previews keyed by source snapshots and the force toggle.
use super::*;
use std::sync::Arc;
use viboceros_document::{GeometrySnapshot, Object};

#[derive(Clone, Debug)]
struct Entry {
    source: GeometrySnapshot,
    force: bool,
    colors: Arc<[[u8; 3]]>,
}
#[derive(Clone, Debug, Default)]
pub struct FacePackPreview {
    entries: BTreeMap<ObjectId, Entry>,
}
#[derive(Clone, Debug)]
pub struct FacePackView {
    entries: BTreeMap<ObjectId, Entry>,
}
impl FacePackView {
    /// Decline stale geometry rather than coloring unrelated replacement faces.
    pub fn colors(&self, object: &Object) -> Option<Arc<[[u8; 3]]>> {
        self.entries
            .get(&object.id())
            .filter(|e| e.source.shares_storage_with(object.geometry_snapshot()))
            .map(|e| Arc::clone(&e.colors))
    }
}
impl FacePackPreview {
    pub fn view(
        &mut self,
        doc: &Document,
        prompt: Option<&ObjectSelectionPrompt>,
    ) -> Result<Option<FacePackView>, CommandError> {
        let Some(prompt) = prompt.filter(|p| p.command == "ToNURBS") else {
            self.entries.clear();
            return Ok(None);
        };
        let command = prompt.command_line();
        let args = command.split_whitespace().skip(1).collect::<Vec<_>>();
        let (options, _) = parse(&args, Options::default())?;
        if !options.show_packs || !options.packed {
            return Ok(None);
        }
        let mut entries = BTreeMap::new();
        let mut faces = 0usize;
        for object in doc
            .selected_objects()
            .filter(|o| doc.is_object_selectable(o.id()))
        {
            let Geometry::SubD(source) = object.geometry() else {
                continue;
            };
            faces = faces.saturating_add(source.faces().len());
            if faces > 1_000_000 {
                return Err(CommandError::FacePackingResourceLimit);
            }
            let reusable = self.entries.get(&object.id()).filter(|e| {
                e.force == options.force_packs
                    && e.source.shares_storage_with(object.geometry_snapshot())
            });
            let entry = if let Some(entry) = reusable {
                entry.clone()
            } else {
                let colors = source
                    .nurbs_face_pack_ids(options.force_packs)?
                    .into_iter()
                    .map(color)
                    .collect::<Vec<_>>()
                    .into();
                Entry {
                    source: object.geometry_snapshot().clone(),
                    force: options.force_packs,
                    colors,
                }
            };
            entries.insert(object.id(), entry);
        }
        self.entries = entries.clone();
        Ok((!entries.is_empty()).then_some(FacePackView { entries }))
    }
}
fn color(id: u32) -> [u8; 3] {
    // Stable bright colors are local to this preview, never saved as face data.
    let h = id.wrapping_mul(0x9e37_79b9);
    [
        64 + (h & 159) as u8,
        64 + ((h >> 8) & 159) as u8,
        64 + ((h >> 16) & 159) as u8,
    ]
}
