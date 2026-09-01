//! In-memory CAD document model.

mod history;

use std::collections::BTreeSet;
use std::fmt;

use thiserror::Error;
use uuid::Uuid;
use viboceros_geometry::{BoundingBox3, LineSegment, NurbsCurve, Point3, Tolerance};

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
}

impl Geometry {
    pub fn bounds(&self) -> BoundingBox3 {
        match self {
            Self::Point(point) => BoundingBox3::from_points([*point]).unwrap(),
            Self::Line(line) => BoundingBox3::from_points([line.start(), line.end()]).unwrap(),
            Self::NurbsCurve(curve) => curve.control_point_bounds(),
        }
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
        for edit in transaction.edits.iter_mut().rev() {
            edit.undo(self)?;
        }
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

    pub fn objects(&self) -> impl ExactSizeIterator<Item = &Object> {
        self.objects.iter()
    }

    pub fn object(&self, id: ObjectId) -> Option<&Object> {
        self.objects.iter().find(|object| object.id == id)
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

    #[error("object {0} was not found")]
    ObjectNotFound(ObjectId),

    #[error("a group must contain at least one object")]
    EmptyGroup,

    #[error("a document edit transaction is already active")]
    TransactionAlreadyActive,

    #[error("there is no active document edit transaction")]
    NoActiveTransaction,

    #[error("undo or redo cannot run during a document edit transaction")]
    TransactionInProgress,

    #[error("document history invariant failed: {0}")]
    HistoryInvariant(&'static str),
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
        document
            .add_layer("Temporary", ColorRgb::new(1, 2, 3))
            .unwrap();
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
