//! Recipes attached to the geometry version produced by a joined edge blend.
use super::*;
use std::sync::Arc;
use viboceros_geometry::EdgeBlendSizes;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeBlendMode {
    Fillet,
    BallFillet,
    ChamferRollingBall,
    ChamferBetweenRails,
    BlendRollingBall,
    BlendBetweenRails,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EdgeBlendRecipe {
    pub mode: EdgeBlendMode,
    pub source: GeometrySnapshot,
    pub edges: Vec<(usize, EdgeBlendSizes)>,
}

impl Document {
    /// Assign a joined result and its recipe as one geometry edit.
    pub fn replace_edge_blend_results(
        &mut self,
        replacements: impl IntoIterator<Item = (ObjectId, Geometry, EdgeBlendRecipe)>,
    ) -> Result<usize, DocumentError> {
        let mut replacements = replacements
            .into_iter()
            .map(|(id, geometry, recipe)| (id, (geometry, recipe)))
            .collect::<BTreeMap<_, _>>();
        let indices = self.resolve_object_indices(replacements.keys().copied())?;
        let staged = indices
            .into_iter()
            .map(|index| {
                let object = &self.objects[index];
                self.ensure_object_editable(object)?;
                let (geometry, recipe) = replacements.remove(&object.id()).unwrap();
                let snapshot =
                    GeometrySnapshot::from(object_admission::normalize_geometry(geometry)?)
                        .with_edge_blend_recipe(Arc::new(recipe));
                Ok((index, snapshot))
            })
            .collect::<Result<Vec<_>, DocumentError>>()?;
        self.commit_object_geometry_snapshots(
            staged,
            "Replace edge blend",
            "Replace edge blend",
            ReplacementHistory::ChangesOnly,
            true,
            |_, _| false,
        )
    }

    /// Restore the input of the latest joined edge set, including any earlier recipe.
    pub fn restore_edge_blend_source(&mut self, id: ObjectId) -> Result<bool, DocumentError> {
        let index = self.resolve_object_indices([id])?[0];
        let object = &self.objects[index];
        self.ensure_object_editable(object)?;
        let Some(recipe) = object.geometry_snapshot().edge_blend_recipe() else {
            return Ok(false);
        };
        let source = recipe.source.clone();
        self.commit_object_geometry_snapshots(
            vec![(index, source)],
            "Restore edge blend source",
            "Restore edge blend source",
            ReplacementHistory::ChangesOnly,
            true,
            |_, _| true,
        )
        .map(|count| count != 0)
    }
}
