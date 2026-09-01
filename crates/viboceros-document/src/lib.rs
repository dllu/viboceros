//! In-memory CAD document model.

mod history;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use thiserror::Error;
use uuid::Uuid;
use viboceros_geometry::{
    AffineTransform3, BoundingBox3, GeometryError, LineSegment, NurbsCurve, NurbsSurface, Point3,
    Tolerance, TriangleMesh,
};

use history::{Edit, HISTORY_LIMIT, History, HistoryEntry, PendingTransaction};

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(Uuid);

        impl $name {
            fn new() -> Self {
                Self(Uuid::new_v4())
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }
    };
}

id_type!(ObjectId);
id_type!(LayerId);
id_type!(GroupId);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectionMode {
    Replace,
    Add,
    Toggle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ColorRgb {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl ColorRgb {
    pub const BLACK: Self = Self::new(0, 0, 0);

    pub const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Geometry {
    Point(Point3),
    Line(LineSegment),
    NurbsCurve(NurbsCurve),
    NurbsSurface(NurbsSurface),
    Mesh(TriangleMesh),
}

impl Geometry {
    pub fn bounds(&self) -> BoundingBox3 {
        match self {
            Self::Point(point) => BoundingBox3::from_points([*point]).unwrap(),
            Self::Line(line) => BoundingBox3::from_points([line.start(), line.end()]).unwrap(),
            Self::NurbsCurve(curve) => curve.control_point_bounds(),
            Self::NurbsSurface(surface) => surface.control_point_bounds(),
            Self::Mesh(mesh) => mesh.bounds(),
        }
    }

    pub fn transformed(
        &self,
        transform: AffineTransform3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        Ok(match self {
            Self::Point(point) => Self::Point(transform.transform_point(*point)?),
            Self::Line(line) => Self::Line(line.transformed(transform, tolerance)?),
            Self::NurbsCurve(curve) => Self::NurbsCurve(curve.transformed(transform)?),
            Self::NurbsSurface(surface) => Self::NurbsSurface(surface.transformed(transform)?),
            Self::Mesh(mesh) => Self::Mesh(mesh.transformed(transform, tolerance)?),
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ObjectAttributes {
    name: Option<String>,
    layer_id: LayerId,
    visible: bool,
    locked: bool,
}

impl ObjectAttributes {
    pub fn on_layer(layer_id: LayerId) -> Self {
        Self {
            name: None,
            layer_id,
            visible: true,
            locked: false,
        }
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        let name = name.into();
        let name = name.trim();
        self.name = (!name.is_empty()).then(|| name.to_owned());
        self
    }

    pub const fn with_visibility(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    pub const fn with_locked(mut self, locked: bool) -> Self {
        self.locked = locked;
        self
    }

    pub const fn layer_id(&self) -> LayerId {
        self.layer_id
    }

    pub const fn is_visible(&self) -> bool {
        self.visible
    }

    pub const fn is_locked(&self) -> bool {
        self.locked
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Object {
    id: ObjectId,
    geometry: Geometry,
    attributes: ObjectAttributes,
}

impl Object {
    pub const fn id(&self) -> ObjectId {
        self.id
    }

    pub const fn geometry(&self) -> &Geometry {
        &self.geometry
    }

    pub const fn attributes(&self) -> &ObjectAttributes {
        &self.attributes
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    id: LayerId,
    name: String,
    color: ColorRgb,
    visible: bool,
    locked: bool,
}

impl Layer {
    pub const fn id(&self) -> LayerId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn color(&self) -> ColorRgb {
        self.color
    }

    pub const fn is_visible(&self) -> bool {
        self.visible
    }

    pub const fn is_locked(&self) -> bool {
        self.locked
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    id: GroupId,
    name: Option<String>,
    members: BTreeSet<ObjectId>,
}

impl Group {
    pub const fn id(&self) -> GroupId {
        self.id
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn members(&self) -> impl ExactSizeIterator<Item = ObjectId> + '_ {
        self.members.iter().copied()
    }
}

#[derive(Clone, Debug)]
pub struct Document {
    tolerance: Tolerance,
    layers: Vec<Layer>,
    current_layer: LayerId,
    objects: Vec<Object>,
    groups: Vec<Group>,
    selection: BTreeSet<ObjectId>,
    history: History,
}

impl Document {
    pub fn new(tolerance: Tolerance) -> Self {
        let default_layer = Layer {
            id: LayerId::new(),
            name: "Default".to_owned(),
            color: ColorRgb::BLACK,
            visible: true,
            locked: false,
        };
        let current_layer = default_layer.id;
        Self {
            tolerance,
            layers: vec![default_layer],
            current_layer,
            objects: Vec::new(),
            groups: Vec::new(),
            selection: BTreeSet::new(),
            history: History::default(),
        }
    }

    /// Starts an atomic edit transaction. Successful commands commit all edits
    /// as one undo step; failed commands roll them all back.
    pub fn begin_transaction(&mut self, label: impl Into<String>) -> Result<(), DocumentError> {
        if self.history.active.is_some() {
            return Err(DocumentError::TransactionAlreadyActive);
        }
        let label = label.into();
        let label = if label.trim().is_empty() {
            "Edit".to_owned()
        } else {
            label
        };
        self.history.active = Some(PendingTransaction {
            label,
            edits: Vec::new(),
            selection_before: self.selection.clone(),
        });
        Ok(())
    }

    /// Commits the active transaction, returning whether it contained edits.
    pub fn commit_transaction(&mut self) -> Result<bool, DocumentError> {
        let transaction = self
            .history
            .active
            .take()
            .ok_or(DocumentError::NoActiveTransaction)?;
        if transaction.edits.is_empty() {
            return Ok(false);
        }
        self.push_new_undo(HistoryEntry {
            label: transaction.label,
            edits: transaction.edits,
        });
        Ok(true)
    }

    /// Reverses every edit in the active transaction without adding history.
    pub fn rollback_transaction(&mut self) -> Result<bool, DocumentError> {
        let mut transaction = self
            .history
            .active
            .take()
            .ok_or(DocumentError::NoActiveTransaction)?;
        let changed = !transaction.edits.is_empty();
        let result = transaction
            .edits
            .iter_mut()
            .rev()
            .try_for_each(|edit| edit.undo(self));
        self.selection = transaction.selection_before;
        self.prune_selection();
        result?;
        Ok(changed)
    }

    pub fn can_undo(&self) -> bool {
        !self.history.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.history.redo.is_empty()
    }

    pub fn undo_label(&self) -> Option<&str> {
        self.history.undo.last().map(|entry| entry.label.as_str())
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.history.redo.last().map(|entry| entry.label.as_str())
    }

    /// Reverses the latest committed transaction and returns its label.
    pub fn undo(&mut self) -> Result<Option<String>, DocumentError> {
        self.ensure_no_transaction()?;
        let Some(mut entry) = self.history.undo.pop() else {
            return Ok(None);
        };
        for index in (0..entry.edits.len()).rev() {
            if let Err(error) = entry.edits[index].undo(self) {
                for restore in index + 1..entry.edits.len() {
                    entry.edits[restore].redo(self)?;
                }
                self.history.undo.push(entry);
                return Err(error);
            }
        }
        let label = entry.label.clone();
        self.history.redo.push(entry);
        self.prune_selection();
        Ok(Some(label))
    }

    /// Reapplies the latest undone transaction and returns its label.
    pub fn redo(&mut self) -> Result<Option<String>, DocumentError> {
        self.ensure_no_transaction()?;
        let Some(mut entry) = self.history.redo.pop() else {
            return Ok(None);
        };
        for index in 0..entry.edits.len() {
            if let Err(error) = entry.edits[index].redo(self) {
                for restore in (0..index).rev() {
                    entry.edits[restore].undo(self)?;
                }
                self.history.redo.push(entry);
                return Err(error);
            }
        }
        let label = entry.label.clone();
        self.push_replayed_undo(entry);
        self.prune_selection();
        Ok(Some(label))
    }

    pub const fn tolerance(&self) -> Tolerance {
        self.tolerance
    }

    pub const fn current_layer_id(&self) -> LayerId {
        self.current_layer
    }

    pub fn layers(&self) -> impl ExactSizeIterator<Item = &Layer> {
        self.layers.iter()
    }

    pub fn layer(&self, id: LayerId) -> Option<&Layer> {
        self.layers.iter().find(|layer| layer.id == id)
    }

    pub fn layer_by_name(&self, name: &str) -> Option<&Layer> {
        self.layers
            .iter()
            .find(|layer| layer.name.eq_ignore_ascii_case(name.trim()))
    }

    pub fn add_layer(
        &mut self,
        name: impl Into<String>,
        color: ColorRgb,
    ) -> Result<LayerId, DocumentError> {
        let name = name.into();
        let name = name.trim();
        if name.is_empty() {
            return Err(DocumentError::EmptyLayerName);
        }
        if self
            .layers
            .iter()
            .any(|layer| layer.name.eq_ignore_ascii_case(name))
        {
            return Err(DocumentError::DuplicateLayerName(name.to_owned()));
        }

        let id = LayerId::new();
        let index = self.layers.len();
        self.layers.push(Layer {
            id,
            name: name.to_owned(),
            color,
            visible: true,
            locked: false,
        });
        self.record_edit(
            "Add layer",
            Edit::LayerInserted {
                index,
                id,
                stored: None,
            },
        );
        Ok(id)
    }

    pub fn set_current_layer(&mut self, id: LayerId) -> Result<(), DocumentError> {
        let layer = self.layer(id).ok_or(DocumentError::LayerNotFound(id))?;
        if !layer.visible {
            return Err(DocumentError::LayerHidden(id));
        }
        if layer.locked {
            return Err(DocumentError::LayerLocked(id));
        }
        if self.current_layer == id {
            return Ok(());
        }
        let before = self.current_layer;
        self.current_layer = id;
        self.record_edit(
            "Set current layer",
            Edit::CurrentLayerChanged { before, after: id },
        );
        Ok(())
    }

    pub fn rename_layer(
        &mut self,
        id: LayerId,
        name: impl Into<String>,
    ) -> Result<bool, DocumentError> {
        let name = name.into();
        let name = name.trim();
        if name.is_empty() {
            return Err(DocumentError::EmptyLayerName);
        }
        let index = self.layer_index(id)?;
        if self
            .layers
            .iter()
            .any(|layer| layer.id != id && layer.name.eq_ignore_ascii_case(name))
        {
            return Err(DocumentError::DuplicateLayerName(name.to_owned()));
        }
        if self.layers[index].name == name {
            return Ok(false);
        }
        let before = self.layers[index].clone();
        self.layers[index].name = name.to_owned();
        self.record_layer_change("Rename layer", index, before);
        Ok(true)
    }

    pub fn set_layer_color(&mut self, id: LayerId, color: ColorRgb) -> Result<bool, DocumentError> {
        let index = self.layer_index(id)?;
        if self.layers[index].color == color {
            return Ok(false);
        }
        let before = self.layers[index].clone();
        self.layers[index].color = color;
        self.record_layer_change("Set layer color", index, before);
        Ok(true)
    }

    pub fn set_layer_visibility(
        &mut self,
        id: LayerId,
        visible: bool,
    ) -> Result<bool, DocumentError> {
        let index = self.layer_index(id)?;
        if self.layers[index].visible == visible {
            return Ok(false);
        }
        if !visible && self.current_layer == id {
            return Err(DocumentError::CurrentLayerCannotBeHidden(id));
        }
        let before = self.layers[index].clone();
        self.layers[index].visible = visible;
        self.record_layer_change("Set layer visibility", index, before);
        self.prune_selection();
        Ok(true)
    }

    pub fn set_layer_locked(&mut self, id: LayerId, locked: bool) -> Result<bool, DocumentError> {
        let index = self.layer_index(id)?;
        if self.layers[index].locked == locked {
            return Ok(false);
        }
        if locked && self.current_layer == id {
            return Err(DocumentError::CurrentLayerCannotBeLocked(id));
        }
        let before = self.layers[index].clone();
        self.layers[index].locked = locked;
        self.record_layer_change("Set layer lock", index, before);
        self.prune_selection();
        Ok(true)
    }

    pub fn delete_layer(&mut self, id: LayerId) -> Result<(), DocumentError> {
        let index = self.layer_index(id)?;
        if self.current_layer == id {
            return Err(DocumentError::CurrentLayerCannotBeDeleted(id));
        }
        if self
            .objects
            .iter()
            .any(|object| object.attributes.layer_id == id)
        {
            return Err(DocumentError::LayerNotEmpty(id));
        }
        let layer = self.layers.remove(index);
        self.record_edit(
            "Delete layer",
            Edit::LayerRemoved {
                index,
                id,
                stored: Some(layer),
            },
        );
        Ok(())
    }

    pub fn objects(&self) -> impl ExactSizeIterator<Item = &Object> {
        self.objects.iter()
    }

    pub fn object(&self, id: ObjectId) -> Option<&Object> {
        self.objects.iter().find(|object| object.id == id)
    }

    pub fn is_object_selectable(&self, id: ObjectId) -> bool {
        let Some(object) = self.object(id) else {
            return false;
        };
        let attributes = object.attributes();
        attributes.visible
            && !attributes.locked
            && self
                .layer(attributes.layer_id)
                .is_some_and(|layer| layer.visible && !layer.locked)
    }

    pub fn selected_object_ids(&self) -> impl ExactSizeIterator<Item = ObjectId> + '_ {
        self.selection.iter().copied()
    }

    pub fn selected_objects(&self) -> impl Iterator<Item = &Object> {
        self.selection.iter().filter_map(|id| self.object(*id))
    }

    pub fn selected_object_count(&self) -> usize {
        self.selection.len()
    }

    pub fn is_selected(&self, id: ObjectId) -> bool {
        self.selection.contains(&id)
    }

    pub fn clear_selection(&mut self) -> usize {
        let count = self.selection.len();
        self.selection.clear();
        count
    }

    pub fn select_all(&mut self) -> usize {
        self.selection = self
            .objects
            .iter()
            .filter(|object| self.is_object_selectable(object.id))
            .map(|object| object.id)
            .collect();
        self.selection.len()
    }

    pub fn invert_selection(&mut self) -> usize {
        self.selection = self
            .objects
            .iter()
            .filter(|object| {
                self.is_object_selectable(object.id) && !self.selection.contains(&object.id)
            })
            .map(|object| object.id)
            .collect();
        self.selection.len()
    }

    pub fn select_object(
        &mut self,
        id: ObjectId,
        mode: SelectionMode,
    ) -> Result<usize, DocumentError> {
        if self.object(id).is_none() {
            return Err(DocumentError::ObjectNotFound(id));
        }
        if !self.is_object_selectable(id) {
            return Err(DocumentError::ObjectNotSelectable(id));
        }
        let cluster = self.selectable_group_cluster(id);
        match mode {
            SelectionMode::Replace => {
                self.selection = cluster;
            }
            SelectionMode::Add => {
                self.selection.extend(cluster);
            }
            SelectionMode::Toggle => {
                if cluster.iter().all(|member| self.selection.contains(member)) {
                    self.selection.retain(|member| !cluster.contains(member));
                } else {
                    self.selection.extend(cluster);
                }
            }
        }
        Ok(self.selection.len())
    }

    pub fn transform_objects(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
        transform: AffineTransform3,
    ) -> Result<usize, DocumentError> {
        let staged = self
            .stage_transformed_objects(ids, transform)?
            .into_iter()
            .filter(|(_, before, after)| before != after)
            .collect::<Vec<_>>();
        if staged.is_empty() {
            return Ok(0);
        }
        let transformed_count = staged.len();

        let owns_transaction = self.history.active.is_none();
        if owns_transaction {
            self.begin_transaction("Transform objects")?;
        }
        for (index, before, after) in staged {
            let id = before.id;
            self.objects[index] = after.clone();
            self.record_edit(
                "Transform object",
                Edit::ObjectChanged { id, before, after },
            );
        }
        if owns_transaction {
            self.commit_transaction()?;
        }
        Ok(transformed_count)
    }

    pub fn copy_objects_transformed(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
        transform: AffineTransform3,
    ) -> Result<Vec<ObjectId>, DocumentError> {
        let staged = self.stage_transformed_objects(ids, transform)?;
        if staged.is_empty() {
            return Ok(Vec::new());
        }
        let originals: BTreeSet<_> = staged.iter().map(|(_, object, _)| object.id).collect();
        let copied_groups = self
            .groups
            .iter()
            .filter_map(|group| {
                let members: Vec<_> = group
                    .members
                    .iter()
                    .filter(|member| originals.contains(member))
                    .copied()
                    .collect();
                (!members.is_empty()).then(|| (group.name.clone(), members))
            })
            .collect::<Vec<_>>();

        let owns_transaction = self.history.active.is_none();
        if owns_transaction {
            self.begin_transaction("Copy objects")?;
        }
        let mut copied_ids = Vec::with_capacity(staged.len());
        let mut copied_by_original = BTreeMap::new();
        for (_, original, transformed) in staged {
            let id = ObjectId::new();
            let index = self.objects.len();
            self.objects.push(Object {
                id,
                geometry: transformed.geometry,
                attributes: original.attributes,
            });
            self.record_edit(
                "Copy object",
                Edit::ObjectInserted {
                    index,
                    id,
                    stored: None,
                },
            );
            copied_by_original.insert(original.id, id);
            copied_ids.push(id);
        }
        for (name, original_members) in copied_groups {
            let members = original_members
                .into_iter()
                .map(|member| copied_by_original[&member])
                .collect();
            let name = name.map(|name| self.next_group_copy_name(&name));
            let id = GroupId::new();
            let index = self.groups.len();
            self.groups.push(Group { id, name, members });
            self.record_edit(
                "Copy group",
                Edit::GroupInserted {
                    index,
                    id,
                    stored: None,
                },
            );
        }
        self.selection = copied_ids.iter().copied().collect();
        self.prune_selection();
        if owns_transaction {
            self.commit_transaction()?;
        }
        Ok(copied_ids)
    }

    pub fn add_geometry(&mut self, geometry: Geometry) -> Result<ObjectId, DocumentError> {
        self.add_geometry_with_attributes(geometry, ObjectAttributes::on_layer(self.current_layer))
    }

    pub fn add_geometry_with_attributes(
        &mut self,
        geometry: Geometry,
        attributes: ObjectAttributes,
    ) -> Result<ObjectId, DocumentError> {
        let layer = self
            .layer(attributes.layer_id)
            .ok_or(DocumentError::LayerNotFound(attributes.layer_id))?;
        if layer.locked {
            return Err(DocumentError::LayerLocked(layer.id));
        }

        let id = ObjectId::new();
        let index = self.objects.len();
        self.objects.push(Object {
            id,
            geometry,
            attributes,
        });
        self.record_edit(
            "Add object",
            Edit::ObjectInserted {
                index,
                id,
                stored: None,
            },
        );
        Ok(id)
    }

    pub fn delete_object(&mut self, id: ObjectId) -> Result<(), DocumentError> {
        let owns_transaction = self.history.active.is_none();
        if owns_transaction {
            self.begin_transaction("Delete object")?;
        }
        let result = self.delete_object_recorded(id);
        if owns_transaction {
            match result {
                Ok(()) => {
                    self.commit_transaction()?;
                    Ok(())
                }
                Err(error) => {
                    self.rollback_transaction()?;
                    Err(error)
                }
            }
        } else {
            result
        }
    }

    fn delete_object_recorded(&mut self, id: ObjectId) -> Result<(), DocumentError> {
        let index = self
            .objects
            .iter()
            .position(|object| object.id == id)
            .ok_or(DocumentError::ObjectNotFound(id))?;
        for group_index in (0..self.groups.len()).rev() {
            if !self.groups[group_index].members.contains(&id) {
                continue;
            }
            if self.groups[group_index].members.len() == 1 {
                let group = self.groups.remove(group_index);
                let group_id = group.id;
                self.record_edit(
                    "Delete object",
                    Edit::GroupRemoved {
                        index: group_index,
                        id: group_id,
                        stored: Some(group),
                    },
                );
            } else {
                let group_id = self.groups[group_index].id;
                self.groups[group_index].members.remove(&id);
                self.record_edit(
                    "Delete object",
                    Edit::GroupMemberRemoved {
                        group_id,
                        object_id: id,
                    },
                );
            }
        }
        let object = self.objects.remove(index);
        self.selection.remove(&id);
        self.record_edit(
            "Delete object",
            Edit::ObjectRemoved {
                index,
                id,
                stored: Some(object),
            },
        );
        Ok(())
    }

    pub fn clear_objects(&mut self) -> usize {
        let count = self.objects.len();
        if count == 0 && self.groups.is_empty() {
            return 0;
        }
        let stored_objects = std::mem::take(&mut self.objects);
        let stored_groups = std::mem::take(&mut self.groups);
        self.selection.clear();
        self.record_edit(
            "Clear objects",
            Edit::ObjectsCleared {
                stored_objects,
                stored_groups,
            },
        );
        count
    }

    pub fn add_group(
        &mut self,
        name: Option<String>,
        members: impl IntoIterator<Item = ObjectId>,
    ) -> Result<GroupId, DocumentError> {
        let name = name
            .map(|name| name.trim().to_owned())
            .filter(|name| !name.is_empty());
        if let Some(name) = &name
            && self.groups.iter().any(|group| {
                group
                    .name
                    .as_ref()
                    .is_some_and(|candidate| candidate.eq_ignore_ascii_case(name))
            })
        {
            return Err(DocumentError::DuplicateGroupName(name.clone()));
        }
        let members: BTreeSet<_> = members.into_iter().collect();
        if members.is_empty() {
            return Err(DocumentError::EmptyGroup);
        }
        if let Some(missing) = members.iter().find(|id| self.object(**id).is_none()) {
            return Err(DocumentError::ObjectNotFound(*missing));
        }

        let id = GroupId::new();
        let index = self.groups.len();
        self.groups.push(Group { id, name, members });
        self.record_edit(
            "Add group",
            Edit::GroupInserted {
                index,
                id,
                stored: None,
            },
        );
        Ok(id)
    }

    pub fn groups(&self) -> impl ExactSizeIterator<Item = &Group> {
        self.groups.iter()
    }

    pub fn group(&self, id: GroupId) -> Option<&Group> {
        self.groups.iter().find(|group| group.id == id)
    }

    pub fn group_by_name(&self, name: &str) -> Option<&Group> {
        self.groups.iter().find(|group| {
            group
                .name
                .as_ref()
                .is_some_and(|candidate| candidate.eq_ignore_ascii_case(name.trim()))
        })
    }

    pub fn remove_group(&mut self, id: GroupId) -> Result<usize, DocumentError> {
        let index = self
            .groups
            .iter()
            .position(|group| group.id == id)
            .ok_or(DocumentError::GroupNotFound(id))?;
        let group = self.groups.remove(index);
        let member_count = group.members.len();
        self.record_edit(
            "Remove group",
            Edit::GroupRemoved {
                index,
                id,
                stored: Some(group),
            },
        );
        Ok(member_count)
    }

    pub fn bounds(&self) -> Option<BoundingBox3> {
        self.objects
            .iter()
            .map(|object| object.geometry.bounds())
            .reduce(|left, right| left.union(right).expect("finite object bounds"))
    }

    fn ensure_no_transaction(&self) -> Result<(), DocumentError> {
        if self.history.active.is_some() {
            Err(DocumentError::TransactionInProgress)
        } else {
            Ok(())
        }
    }

    fn layer_index(&self, id: LayerId) -> Result<usize, DocumentError> {
        self.layers
            .iter()
            .position(|layer| layer.id == id)
            .ok_or(DocumentError::LayerNotFound(id))
    }

    fn selectable_group_cluster(&self, id: ObjectId) -> BTreeSet<ObjectId> {
        let mut connected = BTreeSet::from([id]);
        loop {
            let previous_len = connected.len();
            for group in &self.groups {
                if group
                    .members
                    .iter()
                    .any(|member| connected.contains(member))
                {
                    connected.extend(group.members.iter().copied());
                }
            }
            if connected.len() == previous_len {
                break;
            }
        }
        connected.retain(|member| self.is_object_selectable(*member));
        connected
    }

    fn stage_transformed_objects(
        &self,
        ids: impl IntoIterator<Item = ObjectId>,
        transform: AffineTransform3,
    ) -> Result<Vec<(usize, Object, Object)>, DocumentError> {
        let ids: BTreeSet<_> = ids.into_iter().collect();
        if let Some(missing) = ids.iter().find(|id| self.object(**id).is_none()) {
            return Err(DocumentError::ObjectNotFound(*missing));
        }

        let mut staged = Vec::with_capacity(ids.len());
        for (index, object) in self.objects.iter().enumerate() {
            if !ids.contains(&object.id) {
                continue;
            }
            if object.attributes.locked {
                return Err(DocumentError::ObjectLocked(object.id));
            }
            let layer = self
                .layer(object.attributes.layer_id)
                .ok_or(DocumentError::LayerNotFound(object.attributes.layer_id))?;
            if layer.locked {
                return Err(DocumentError::LayerLocked(layer.id));
            }
            let mut transformed = object.clone();
            transformed.geometry = object.geometry.transformed(transform, self.tolerance)?;
            staged.push((index, object.clone(), transformed));
        }
        Ok(staged)
    }

    fn next_group_copy_name(&self, original: &str) -> String {
        let root = format!("{original} copy");
        if self.group_by_name(&root).is_none() {
            return root;
        }
        for suffix in 2_u64..=u64::MAX {
            let candidate = format!("{root} {suffix}");
            if self.group_by_name(&candidate).is_none() {
                return candidate;
            }
        }
        loop {
            let candidate = format!("{root} {}", GroupId::new());
            if self.group_by_name(&candidate).is_none() {
                return candidate;
            }
        }
    }

    fn prune_selection(&mut self) {
        let selection = std::mem::take(&mut self.selection);
        self.selection = selection
            .into_iter()
            .filter(|id| self.is_object_selectable(*id))
            .collect();
    }

    fn record_layer_change(&mut self, label: &'static str, index: usize, before: Layer) {
        let after = self.layers[index].clone();
        let id = after.id;
        self.record_edit(label, Edit::LayerChanged { id, before, after });
    }

    fn record_edit(&mut self, label: &'static str, edit: Edit) {
        if let Some(transaction) = &mut self.history.active {
            transaction.edits.push(edit);
            return;
        }
        self.push_new_undo(HistoryEntry {
            label: label.to_owned(),
            edits: vec![edit],
        });
    }

    fn push_new_undo(&mut self, entry: HistoryEntry) {
        self.history.redo.clear();
        self.push_replayed_undo(entry);
    }

    fn push_replayed_undo(&mut self, entry: HistoryEntry) {
        if self.history.undo.len() == HISTORY_LIMIT {
            self.history.undo.remove(0);
        }
        self.history.undo.push(entry);
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new(Tolerance::DEFAULT)
    }
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum DocumentError {
    #[error("layer name cannot be empty")]
    EmptyLayerName,

    #[error("a layer named '{0}' already exists")]
    DuplicateLayerName(String),

    #[error("layer {0} was not found")]
    LayerNotFound(LayerId),

    #[error("layer {0} is locked")]
    LayerLocked(LayerId),

    #[error("layer {0} is hidden")]
    LayerHidden(LayerId),

    #[error("the current layer {0} cannot be hidden")]
    CurrentLayerCannotBeHidden(LayerId),

    #[error("the current layer {0} cannot be locked")]
    CurrentLayerCannotBeLocked(LayerId),

    #[error("the current layer {0} cannot be deleted")]
    CurrentLayerCannotBeDeleted(LayerId),

    #[error("layer {0} contains objects and cannot be deleted")]
    LayerNotEmpty(LayerId),

    #[error("object {0} was not found")]
    ObjectNotFound(ObjectId),

    #[error("object {0} is hidden or locked and cannot be selected")]
    ObjectNotSelectable(ObjectId),

    #[error("object {0} is locked and cannot be edited")]
    ObjectLocked(ObjectId),

    #[error("a group must contain at least one object")]
    EmptyGroup,

    #[error("a group named '{0}' already exists")]
    DuplicateGroupName(String),

    #[error("group {0} was not found")]
    GroupNotFound(GroupId),

    #[error("a document edit transaction is already active")]
    TransactionAlreadyActive,

    #[error("there is no active document edit transaction")]
    NoActiveTransaction,

    #[error("undo or redo cannot run during a document edit transaction")]
    TransactionInProgress,

    #[error("document history invariant failed: {0}")]
    HistoryInvariant(&'static str),

    #[error(transparent)]
    Geometry(#[from] GeometryError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_with_one_current_default_layer() {
        let document = Document::default();
        assert_eq!(document.layers().len(), 1);
        assert_eq!(
            document.layer(document.current_layer_id()).unwrap().name(),
            "Default"
        );
    }

    #[test]
    fn layer_names_are_unique_case_insensitively() {
        let mut document = Document::default();
        document
            .add_layer("Construction", ColorRgb::new(0, 128, 255))
            .unwrap();
        assert_eq!(
            document.add_layer("construction", ColorRgb::BLACK),
            Err(DocumentError::DuplicateLayerName("construction".to_owned()))
        );
    }

    #[test]
    fn layer_properties_are_atomic_and_reversible() {
        let mut document = Document::default();
        let layer = document
            .add_layer("Construction", ColorRgb::new(10, 20, 30))
            .unwrap();
        document.begin_transaction("Edit layer").unwrap();
        assert!(document.rename_layer(layer, "Reference").unwrap());
        assert!(
            document
                .set_layer_color(layer, ColorRgb::new(80, 90, 100))
                .unwrap()
        );
        assert!(document.set_layer_visibility(layer, false).unwrap());
        assert!(document.set_layer_locked(layer, true).unwrap());
        document.commit_transaction().unwrap();

        let changed = document.layer(layer).unwrap();
        assert_eq!(changed.name(), "Reference");
        assert_eq!(changed.color(), ColorRgb::new(80, 90, 100));
        assert!(!changed.is_visible());
        assert!(changed.is_locked());

        document.undo().unwrap();
        let original = document.layer(layer).unwrap();
        assert_eq!(original.name(), "Construction");
        assert_eq!(original.color(), ColorRgb::new(10, 20, 30));
        assert!(original.is_visible());
        assert!(!original.is_locked());

        document.redo().unwrap();
        let replayed = document.layer(layer).unwrap();
        assert_eq!(replayed.name(), "Reference");
        assert!(!replayed.is_visible());
        assert!(replayed.is_locked());
    }

    #[test]
    fn current_layer_stays_visible_unlocked_and_present() {
        let mut document = Document::default();
        let current = document.current_layer_id();
        assert_eq!(
            document.set_layer_visibility(current, false),
            Err(DocumentError::CurrentLayerCannotBeHidden(current))
        );
        assert_eq!(
            document.set_layer_locked(current, true),
            Err(DocumentError::CurrentLayerCannotBeLocked(current))
        );
        assert_eq!(
            document.delete_layer(current),
            Err(DocumentError::CurrentLayerCannotBeDeleted(current))
        );

        let other = document.add_layer("Other", ColorRgb::new(1, 2, 3)).unwrap();
        document.set_layer_visibility(other, false).unwrap();
        assert_eq!(
            document.set_current_layer(other),
            Err(DocumentError::LayerHidden(other))
        );
        document.set_layer_visibility(other, true).unwrap();
        document.set_layer_locked(other, true).unwrap();
        assert_eq!(
            document.set_current_layer(other),
            Err(DocumentError::LayerLocked(other))
        );
    }

    #[test]
    fn deleting_an_empty_layer_is_reversible_but_nonempty_layers_are_protected() {
        let mut document = Document::default();
        let default = document.current_layer_id();
        let occupied = document
            .add_layer("Occupied", ColorRgb::new(1, 2, 3))
            .unwrap();
        document.set_current_layer(occupied).unwrap();
        document
            .add_geometry(Geometry::Point(Point3::try_new(0.0, 0.0, 0.0).unwrap()))
            .unwrap();
        document.set_current_layer(default).unwrap();
        assert_eq!(
            document.delete_layer(occupied),
            Err(DocumentError::LayerNotEmpty(occupied))
        );

        let empty = document.add_layer("Empty", ColorRgb::new(4, 5, 6)).unwrap();
        document.delete_layer(empty).unwrap();
        assert!(document.layer(empty).is_none());
        document.undo().unwrap();
        assert_eq!(document.layer(empty).unwrap().name(), "Empty");
        document.redo().unwrap();
        assert!(document.layer(empty).is_none());
    }

    #[test]
    fn deleting_an_object_prunes_empty_groups() {
        let mut document = Document::default();
        let object = document
            .add_geometry(Geometry::Point(Point3::try_new(0.0, 0.0, 0.0).unwrap()))
            .unwrap();
        document.add_group(None, [object]).unwrap();
        document.delete_object(object).unwrap();
        assert_eq!(document.objects().len(), 0);
        assert_eq!(document.groups().len(), 0);
    }

    #[test]
    fn named_groups_are_unique_and_removal_is_reversible() {
        let mut document = Document::default();
        let first = document
            .add_geometry(Geometry::Point(Point3::try_new(0.0, 0.0, 0.0).unwrap()))
            .unwrap();
        let second = document
            .add_geometry(Geometry::Point(Point3::try_new(1.0, 0.0, 0.0).unwrap()))
            .unwrap();
        let group = document
            .add_group(Some("Pair".to_owned()), [first, second])
            .unwrap();
        assert_eq!(document.group_by_name("pair").unwrap().id(), group);
        assert_eq!(
            document.add_group(Some("PAIR".to_owned()), [first]),
            Err(DocumentError::DuplicateGroupName("PAIR".to_owned()))
        );

        assert_eq!(document.remove_group(group).unwrap(), 2);
        assert!(document.group(group).is_none());
        document.undo().unwrap();
        assert_eq!(document.group(group).unwrap().members().len(), 2);
        document.redo().unwrap();
        assert!(document.group(group).is_none());
    }

    #[test]
    fn selection_expands_connected_groups_and_skips_locked_members() {
        let mut document = Document::default();
        let default = document.current_layer_id();
        let first = document
            .add_geometry(Geometry::Point(Point3::try_new(0.0, 0.0, 0.0).unwrap()))
            .unwrap();
        let locked_layer = document
            .add_layer("Locked", ColorRgb::new(10, 20, 30))
            .unwrap();
        document.set_current_layer(locked_layer).unwrap();
        let bridge = document
            .add_geometry(Geometry::Point(Point3::try_new(1.0, 0.0, 0.0).unwrap()))
            .unwrap();
        document.set_current_layer(default).unwrap();
        let last = document
            .add_geometry(Geometry::Point(Point3::try_new(2.0, 0.0, 0.0).unwrap()))
            .unwrap();
        document.add_group(None, [first, bridge]).unwrap();
        document.add_group(None, [bridge, last]).unwrap();
        document.set_layer_locked(locked_layer, true).unwrap();

        assert_eq!(
            document.select_object(bridge, SelectionMode::Replace),
            Err(DocumentError::ObjectNotSelectable(bridge))
        );
        assert_eq!(
            document
                .select_object(first, SelectionMode::Replace)
                .unwrap(),
            2
        );
        assert!(document.is_selected(first));
        assert!(!document.is_selected(bridge));
        assert!(document.is_selected(last));

        assert_eq!(
            document
                .select_object(first, SelectionMode::Toggle)
                .unwrap(),
            0
        );
    }

    #[test]
    fn selection_is_transient_inverted_and_pruned_by_model_state() {
        let mut document = Document::default();
        let first = document
            .add_geometry(Geometry::Point(Point3::try_new(0.0, 0.0, 0.0).unwrap()))
            .unwrap();
        let second = document
            .add_geometry(Geometry::Point(Point3::try_new(1.0, 0.0, 0.0).unwrap()))
            .unwrap();
        let history_label = document.undo_label().map(str::to_owned);

        assert_eq!(
            document
                .select_object(first, SelectionMode::Replace)
                .unwrap(),
            1
        );
        assert_eq!(document.invert_selection(), 1);
        assert!(!document.is_selected(first));
        assert!(document.is_selected(second));
        assert_eq!(document.undo_label(), history_label.as_deref());

        document.undo().unwrap();
        assert!(document.object(second).is_none());
        assert_eq!(document.selected_object_count(), 0);
        assert_eq!(document.select_all(), 1);
        assert!(document.is_selected(first));
        document.begin_transaction("Cancelled delete").unwrap();
        document.delete_object(first).unwrap();
        assert_eq!(document.selected_object_count(), 0);
        document.rollback_transaction().unwrap();
        assert!(document.object(first).is_some());
        assert!(document.is_selected(first));
        assert_eq!(document.clear_selection(), 1);
        assert_eq!(document.clear_selection(), 0);
    }

    #[test]
    fn object_transforms_are_atomic_identity_preserving_and_reversible() {
        let mut document = Document::default();
        let first = document
            .add_geometry(Geometry::Point(Point3::try_new(1.0, 2.0, 3.0).unwrap()))
            .unwrap();
        let second = document
            .add_geometry(Geometry::Line(
                LineSegment::try_new(
                    Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                    Point3::try_new(2.0, 0.0, 0.0).unwrap(),
                    document.tolerance(),
                )
                .unwrap(),
            ))
            .unwrap();
        document
            .select_object(first, SelectionMode::Replace)
            .unwrap();
        let transform = AffineTransform3::from_translation(
            viboceros_geometry::Vector3::try_new(10.0, -2.0, 4.0).unwrap(),
        );
        assert_eq!(
            document
                .transform_objects([second, first, first], transform)
                .unwrap(),
            2
        );
        assert_eq!(document.undo_label(), Some("Transform objects"));
        assert!(document.is_selected(first));
        assert!(matches!(
            document.object(first).unwrap().geometry(),
            Geometry::Point(point) if *point == Point3::try_new(11.0, 0.0, 7.0).unwrap()
        ));

        document.undo().unwrap();
        assert!(matches!(
            document.object(first).unwrap().geometry(),
            Geometry::Point(point) if *point == Point3::try_new(1.0, 2.0, 3.0).unwrap()
        ));
        assert_eq!(document.object(first).unwrap().id(), first);
        assert_eq!(document.object(second).unwrap().id(), second);
        document.redo().unwrap();
        assert!(matches!(
            document.object(second).unwrap().geometry(),
            Geometry::Line(line)
                if line.start() == Point3::try_new(10.0, -2.0, 4.0).unwrap()
                    && line.end() == Point3::try_new(12.0, -2.0, 4.0).unwrap()
        ));

        let history_label = document.undo_label().map(str::to_owned);
        assert_eq!(
            document
                .transform_objects([first, second], AffineTransform3::identity())
                .unwrap(),
            0
        );
        assert_eq!(document.undo_label(), history_label.as_deref());

        document.begin_transaction("Two transforms").unwrap();
        document
            .transform_objects(
                [first],
                AffineTransform3::from_translation(
                    viboceros_geometry::Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
                ),
            )
            .unwrap();
        document
            .transform_objects(
                [first],
                AffineTransform3::from_translation(
                    viboceros_geometry::Vector3::try_new(2.0, 0.0, 0.0).unwrap(),
                ),
            )
            .unwrap();
        document.commit_transaction().unwrap();
        assert!(matches!(
            document.object(first).unwrap().geometry(),
            Geometry::Point(point) if *point == Point3::try_new(14.0, 0.0, 7.0).unwrap()
        ));
        document.undo().unwrap();
        assert!(matches!(
            document.object(first).unwrap().geometry(),
            Geometry::Point(point) if *point == Point3::try_new(11.0, 0.0, 7.0).unwrap()
        ));
        document.redo().unwrap();
        assert!(matches!(
            document.object(first).unwrap().geometry(),
            Geometry::Point(point) if *point == Point3::try_new(14.0, 0.0, 7.0).unwrap()
        ));
    }

    #[test]
    fn copying_objects_preserves_layers_and_recreates_group_topology() {
        let mut document = Document::default();
        let layer = document
            .add_layer("Parts", ColorRgb::new(12, 34, 56))
            .unwrap();
        document.set_current_layer(layer).unwrap();
        let first = document
            .add_geometry(Geometry::Point(Point3::try_new(0.0, 0.0, 0.0).unwrap()))
            .unwrap();
        let second = document
            .add_geometry(Geometry::Point(Point3::try_new(2.0, 0.0, 0.0).unwrap()))
            .unwrap();
        document
            .add_group(Some("Pair".to_owned()), [first, second])
            .unwrap();
        document
            .select_object(first, SelectionMode::Replace)
            .unwrap();
        let copies = document
            .copy_objects_transformed(
                document.selected_object_ids().collect::<Vec<_>>(),
                AffineTransform3::from_translation(
                    viboceros_geometry::Vector3::try_new(0.0, 5.0, 0.0).unwrap(),
                ),
            )
            .unwrap();

        assert_eq!(copies.len(), 2);
        assert_eq!(document.objects().len(), 4);
        assert_eq!(document.groups().len(), 2);
        assert_eq!(
            document.group_by_name("Pair copy").unwrap().members().len(),
            2
        );
        assert!(copies.iter().all(|id| document.is_selected(*id)));
        assert!(!document.is_selected(first));
        for copy in &copies {
            assert_eq!(
                document.object(*copy).unwrap().attributes().layer_id(),
                layer
            );
        }
        let copied_points = copies
            .iter()
            .map(|id| match document.object(*id).unwrap().geometry() {
                Geometry::Point(point) => *point,
                _ => panic!("expected copied points"),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            copied_points,
            vec![
                Point3::try_new(0.0, 5.0, 0.0).unwrap(),
                Point3::try_new(2.0, 5.0, 0.0).unwrap(),
            ]
        );

        document.undo().unwrap();
        assert_eq!(document.objects().len(), 2);
        assert_eq!(document.groups().len(), 1);
        assert_eq!(document.selected_object_count(), 0);
        document.redo().unwrap();
        assert!(copies.iter().all(|id| document.object(*id).is_some()));
        assert_eq!(document.groups().len(), 2);
    }

    #[test]
    fn failed_transform_staging_leaves_geometry_history_and_selection_unchanged() {
        let mut document = Document::default();
        let object = document
            .add_geometry(Geometry::Point(
                Point3::try_new(viboceros_geometry::Real::MAX, 0.0, 0.0).unwrap(),
            ))
            .unwrap();
        document
            .select_object(object, SelectionMode::Replace)
            .unwrap();
        let history_label = document.undo_label().map(str::to_owned);
        let transform = AffineTransform3::from_translation(
            viboceros_geometry::Vector3::try_new(viboceros_geometry::Real::MAX, 0.0, 0.0).unwrap(),
        );

        assert!(document.transform_objects([object], transform).is_err());
        assert!(matches!(
            document.object(object).unwrap().geometry(),
            Geometry::Point(point) if point.x() == viboceros_geometry::Real::MAX
        ));
        assert!(document.is_selected(object));
        assert_eq!(document.undo_label(), history_label.as_deref());
    }

    #[test]
    fn undo_and_redo_preserve_object_identity() {
        let mut document = Document::default();
        let id = document
            .add_geometry(Geometry::Point(Point3::try_new(1.0, 2.0, 3.0).unwrap()))
            .unwrap();
        assert_eq!(document.undo_label(), Some("Add object"));

        assert_eq!(document.undo().unwrap(), Some("Add object".to_owned()));
        assert!(document.object(id).is_none());
        assert!(document.can_redo());

        assert_eq!(document.redo().unwrap(), Some("Add object".to_owned()));
        assert_eq!(document.object(id).unwrap().id(), id);
    }

    #[test]
    fn transaction_groups_layer_edits_into_one_step() {
        let mut document = Document::default();
        let default_layer = document.current_layer_id();
        document.begin_transaction("Layer").unwrap();
        let new_layer = document
            .add_layer("Curves", ColorRgb::new(10, 20, 30))
            .unwrap();
        document.set_current_layer(new_layer).unwrap();
        assert!(document.commit_transaction().unwrap());

        assert_eq!(document.layers().len(), 2);
        assert_eq!(document.current_layer_id(), new_layer);
        document.undo().unwrap();
        assert_eq!(document.layers().len(), 1);
        assert_eq!(document.current_layer_id(), default_layer);
        document.redo().unwrap();
        assert_eq!(document.layers().len(), 2);
        assert_eq!(document.current_layer_id(), new_layer);
    }

    #[test]
    fn rollback_reverses_all_edits_without_creating_history() {
        let mut document = Document::default();
        document.begin_transaction("Failing command").unwrap();
        document
            .add_geometry(Geometry::Point(Point3::try_new(0.0, 0.0, 0.0).unwrap()))
            .unwrap();
        let temporary = document
            .add_layer("Temporary", ColorRgb::new(1, 2, 3))
            .unwrap();
        document.rename_layer(temporary, "Discarded").unwrap();
        document
            .set_layer_color(temporary, ColorRgb::new(4, 5, 6))
            .unwrap();
        document.set_layer_visibility(temporary, false).unwrap();
        document.set_layer_locked(temporary, true).unwrap();
        assert!(document.rollback_transaction().unwrap());

        assert_eq!(document.objects().len(), 0);
        assert_eq!(document.layers().len(), 1);
        assert!(!document.can_undo());
    }

    #[test]
    fn deleting_and_undoing_restores_group_membership() {
        let mut document = Document::default();
        let first = document
            .add_geometry(Geometry::Point(Point3::try_new(0.0, 0.0, 0.0).unwrap()))
            .unwrap();
        let second = document
            .add_geometry(Geometry::Point(Point3::try_new(1.0, 0.0, 0.0).unwrap()))
            .unwrap();
        let group = document
            .add_group(Some("Pair".to_owned()), [first, second])
            .unwrap();

        document.delete_object(first).unwrap();
        assert_eq!(document.groups().next().unwrap().members().len(), 1);
        document.undo().unwrap();
        let restored = document
            .groups()
            .find(|candidate| candidate.id() == group)
            .unwrap();
        assert_eq!(
            restored.members().collect::<BTreeSet<_>>(),
            BTreeSet::from([first, second])
        );
        document.redo().unwrap();
        assert!(document.object(first).is_none());
        assert_eq!(document.groups().next().unwrap().members().len(), 1);
    }

    #[test]
    fn clear_undo_restores_objects_and_groups() {
        let mut document = Document::default();
        let object = document
            .add_geometry(Geometry::Point(Point3::try_new(0.0, 0.0, 0.0).unwrap()))
            .unwrap();
        let group = document.add_group(None, [object]).unwrap();
        assert_eq!(document.clear_objects(), 1);
        assert_eq!(document.objects().len(), 0);

        document.undo().unwrap();
        assert!(document.object(object).is_some());
        assert_eq!(document.groups().next().unwrap().id(), group);
        document.redo().unwrap();
        assert_eq!(document.objects().len(), 0);
        assert_eq!(document.groups().len(), 0);
    }

    #[test]
    fn a_new_edit_invalidates_redo_history() {
        let mut document = Document::default();
        document
            .add_geometry(Geometry::Point(Point3::try_new(0.0, 0.0, 0.0).unwrap()))
            .unwrap();
        document.undo().unwrap();
        assert!(document.can_redo());
        document
            .add_geometry(Geometry::Point(Point3::try_new(2.0, 0.0, 0.0).unwrap()))
            .unwrap();
        assert!(!document.can_redo());
    }

    #[test]
    fn undo_restores_a_group_removed_with_its_last_object() {
        let mut document = Document::default();
        let object = document
            .add_geometry(Geometry::Point(Point3::try_new(0.0, 0.0, 0.0).unwrap()))
            .unwrap();
        let group = document
            .add_group(Some("Solo".to_owned()), [object])
            .unwrap();
        document.delete_object(object).unwrap();
        assert_eq!(document.groups().len(), 0);

        document.undo().unwrap();
        assert_eq!(document.groups().next().unwrap().id(), group);
        assert_eq!(
            document.groups().next().unwrap().members().next(),
            Some(object)
        );
        document.redo().unwrap();
        assert_eq!(document.groups().len(), 0);
    }

    #[test]
    fn clear_then_add_is_replayed_in_transaction_order() {
        let mut document = Document::default();
        let original = document
            .add_geometry(Geometry::Point(Point3::try_new(1.0, 0.0, 0.0).unwrap()))
            .unwrap();
        document.begin_transaction("Replace").unwrap();
        document.clear_objects();
        let replacement = document
            .add_geometry(Geometry::Point(Point3::try_new(2.0, 0.0, 0.0).unwrap()))
            .unwrap();
        document.commit_transaction().unwrap();

        document.undo().unwrap();
        assert!(document.object(original).is_some());
        assert!(document.object(replacement).is_none());
        document.redo().unwrap();
        assert!(document.object(original).is_none());
        assert!(document.object(replacement).is_some());
    }

    #[test]
    fn history_retains_the_most_recent_bounded_number_of_edits() {
        let mut document = Document::default();
        for index in 0..=HISTORY_LIMIT {
            document
                .add_geometry(Geometry::Point(
                    Point3::try_new(index as f64, 0.0, 0.0).unwrap(),
                ))
                .unwrap();
        }
        for _ in 0..HISTORY_LIMIT {
            assert!(document.undo().unwrap().is_some());
        }
        assert_eq!(document.objects().len(), 1);
        assert!(!document.can_undo());
    }

    #[test]
    fn nested_transactions_and_undo_during_transaction_are_rejected() {
        let mut document = Document::default();
        document.begin_transaction("Outer").unwrap();
        assert_eq!(
            document.begin_transaction("Inner"),
            Err(DocumentError::TransactionAlreadyActive)
        );
        assert_eq!(document.undo(), Err(DocumentError::TransactionInProgress));
        assert!(!document.rollback_transaction().unwrap());
    }
}
