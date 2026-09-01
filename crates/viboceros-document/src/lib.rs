//! In-memory CAD document model.

mod duplicate;
mod history;
mod object_layer;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use thiserror::Error;
use uuid::Uuid;
use viboceros_geometry::{
    AffineTransform3, BoundingBox3, Circle3, CircularArc3, Ellipse3, GeometryError, LineSegment,
    NurbsCurve, NurbsSurface, Point3, PointCloud3, Polyline3, Tolerance, TriangleMesh,
};

use duplicate::DuplicateGeometryFamily;
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

#[derive(Clone, Debug)]
struct CaseInsensitiveWildcard {
    pattern: Vec<char>,
}

impl CaseInsensitiveWildcard {
    fn new(pattern: &str) -> Self {
        Self {
            pattern: pattern.to_lowercase().chars().collect(),
        }
    }

    fn matches(&self, candidate: &str) -> bool {
        let candidate = candidate.to_lowercase().chars().collect::<Vec<_>>();
        let mut pattern_index = 0;
        let mut candidate_index = 0;
        let mut star_index = None;
        let mut star_candidate_index = 0;
        while candidate_index < candidate.len() {
            if pattern_index < self.pattern.len()
                && (self.pattern[pattern_index] == '?'
                    || self.pattern[pattern_index] == candidate[candidate_index])
            {
                pattern_index += 1;
                candidate_index += 1;
            } else if pattern_index < self.pattern.len() && self.pattern[pattern_index] == '*' {
                star_index = Some(pattern_index);
                pattern_index += 1;
                star_candidate_index = candidate_index;
            } else if let Some(star) = star_index {
                pattern_index = star + 1;
                star_candidate_index += 1;
                candidate_index = star_candidate_index;
            } else {
                return false;
            }
        }
        while pattern_index < self.pattern.len() && self.pattern[pattern_index] == '*' {
            pattern_index += 1;
        }
        pattern_index == self.pattern.len()
    }
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
    PointCloud(PointCloud3),
    Line(LineSegment),
    Circle(Circle3),
    Arc(CircularArc3),
    Ellipse(Ellipse3),
    Polyline(Polyline3),
    NurbsCurve(NurbsCurve),
    NurbsSurface(NurbsSurface),
    Mesh(TriangleMesh),
}

impl Geometry {
    pub fn bounds(&self) -> BoundingBox3 {
        match self {
            Self::Point(point) => BoundingBox3::from_points([*point]).unwrap(),
            Self::PointCloud(cloud) => cloud.bounds(),
            Self::Line(line) => BoundingBox3::from_points([line.start(), line.end()]).unwrap(),
            Self::Circle(circle) => circle.bounds(),
            Self::Arc(arc) => arc.bounds(),
            Self::Ellipse(ellipse) => ellipse.bounds(),
            Self::Polyline(polyline) => polyline.bounds(),
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
            Self::PointCloud(cloud) => Self::PointCloud(cloud.transformed(transform)?),
            Self::Line(line) => Self::Line(line.transformed(transform, tolerance)?),
            Self::Circle(circle) => match circle.transformed_similarity(transform, tolerance)? {
                Some(circle) => Self::Circle(circle),
                None => Self::NurbsCurve(circle.to_nurbs()?.transformed(transform)?),
            },
            Self::Arc(arc) => match arc.transformed_similarity(transform, tolerance)? {
                Some(arc) => Self::Arc(arc),
                None => Self::NurbsCurve(arc.to_nurbs()?.transformed(transform)?),
            },
            Self::Ellipse(ellipse) => {
                match ellipse.transformed_orthogonal(transform, tolerance)? {
                    Some(ellipse) => Self::Ellipse(ellipse),
                    None => Self::NurbsCurve(ellipse.to_nurbs()?.transformed(transform)?),
                }
            }
            Self::Polyline(polyline) => Self::Polyline(polyline.transformed(transform, tolerance)?),
            Self::NurbsCurve(curve) => Self::NurbsCurve(curve.transformed(transform)?),
            Self::NurbsSurface(surface) => Self::NurbsSurface(surface.transformed(transform)?),
            Self::Mesh(mesh) => Self::Mesh(mesh.transformed(transform, tolerance)?),
        })
    }

    /// Returns the defining locations duplicated by Rhino's `ExtractPt`
    /// command. Point objects produce no new points; closed curve seams and
    /// periodic NURBS controls follow Rhino's unique-grip ordering.
    pub fn extract_point_locations(&self) -> Result<Vec<Point3>, GeometryError> {
        Ok(match self {
            Self::Point(_) => Vec::new(),
            Self::PointCloud(cloud) => cloud.points().to_vec(),
            Self::Line(line) => vec![line.start(), line.end()],
            Self::Circle(circle) => circle.to_nurbs()?.extract_point_locations()?,
            Self::Arc(arc) => arc.to_nurbs()?.extract_point_locations()?,
            Self::Ellipse(ellipse) => ellipse.to_nurbs()?.extract_point_locations()?,
            Self::Polyline(polyline) => {
                let mut points = polyline.vertices().to_vec();
                if polyline.is_closed() {
                    points.pop();
                }
                points
            }
            Self::NurbsCurve(curve) => curve.extract_point_locations()?,
            Self::NurbsSurface(surface) => surface.extract_point_locations(),
            Self::Mesh(mesh) => mesh.vertices().to_vec(),
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
        if !visible {
            self.locked = false;
        }
        self
    }

    pub const fn with_locked(mut self, locked: bool) -> Self {
        self.locked = locked;
        if locked {
            self.visible = true;
        }
        self
    }

    pub const fn with_layer(mut self, layer_id: LayerId) -> Self {
        self.layer_id = layer_id;
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
    isolation: ObjectIsolation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ObjectIsolation {
    None,
    Hidden,
    Locked,
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
    selection_order: Vec<ObjectId>,
    previous_selection: BTreeSet<ObjectId>,
    previous_selection_order: Vec<ObjectId>,
    last_changed_objects: BTreeSet<ObjectId>,
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
            selection_order: Vec::new(),
            previous_selection: BTreeSet::new(),
            previous_selection_order: Vec::new(),
            last_changed_objects: BTreeSet::new(),
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
            object_ids: BTreeSet::new(),
            selection_before: self.selection.clone(),
            selection_order_before: self.selection_order.clone(),
            previous_selection_before: self.previous_selection.clone(),
            previous_selection_order_before: self.previous_selection_order.clone(),
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
        if transaction.selection_before.is_subset(&self.selection) {
            self.previous_selection = transaction.previous_selection_before;
            self.previous_selection_order = transaction.previous_selection_order_before;
        } else {
            self.previous_selection = transaction.selection_before.clone();
            self.previous_selection_order = transaction.selection_order_before.clone();
        }
        if transaction.edits.is_empty() {
            return Ok(false);
        }
        self.push_new_undo(HistoryEntry {
            label: transaction.label,
            edits: transaction.edits,
            object_ids: transaction.object_ids,
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
        self.selection_order = transaction.selection_order_before;
        self.previous_selection = transaction.previous_selection_before;
        self.previous_selection_order = transaction.previous_selection_order_before;
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
        self.update_last_changed_objects(&entry.object_ids);
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
        self.update_last_changed_objects(&entry.object_ids);
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

    /// Iterates the selection in user action order. Objects added together by
    /// a filter or group expansion retain document order.
    pub fn selected_object_ids(&self) -> impl ExactSizeIterator<Item = ObjectId> + '_ {
        self.selection_order.iter().copied()
    }

    /// Iterates selected objects in the same order as [`Self::selected_object_ids`].
    pub fn selected_objects(&self) -> impl Iterator<Item = &Object> {
        self.selection_order
            .iter()
            .filter_map(|id| self.object(*id))
    }

    pub fn selected_object_count(&self) -> usize {
        self.selection.len()
    }

    pub fn is_selected(&self, id: ObjectId) -> bool {
        self.selection.contains(&id)
    }

    pub fn clear_selection(&mut self) -> usize {
        let count = self.selection.len();
        self.update_selection(BTreeSet::new());
        count
    }

    pub fn select_all(&mut self) -> usize {
        let selection = self
            .objects
            .iter()
            .filter(|object| self.is_object_selectable(object.id))
            .map(|object| object.id)
            .collect();
        self.update_selection(selection)
    }

    pub fn invert_selection(&mut self) -> usize {
        let selection = self
            .objects
            .iter()
            .filter(|object| {
                self.is_object_selectable(object.id) && !self.selection.contains(&object.id)
            })
            .map(|object| object.id)
            .collect();
        self.update_selection(selection)
    }

    pub fn select_object(
        &mut self,
        id: ObjectId,
        mode: SelectionMode,
    ) -> Result<usize, DocumentError> {
        self.select_objects([id], mode)
    }

    /// Selects complete selectable group clusters as one atomic selection
    /// action. Every requested id is validated before the selection changes.
    pub fn select_objects(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
        mode: SelectionMode,
    ) -> Result<usize, DocumentError> {
        let ids = ids.into_iter().collect::<BTreeSet<_>>();
        if let Some(missing) = ids.iter().find(|id| self.object(**id).is_none()) {
            return Err(DocumentError::ObjectNotFound(*missing));
        }
        if let Some(unselectable) = ids.iter().find(|id| !self.is_object_selectable(**id)) {
            return Err(DocumentError::ObjectNotSelectable(*unselectable));
        }
        let cluster = self.selectable_clusters(ids.iter().copied());
        Ok(self.apply_selection_mode(cluster, mode))
    }

    /// Selects exactly the requested selectable objects without expanding
    /// their groups. Attribute-based Rhino commands use this path so a name,
    /// layer, or group match does not pull in members from other groups.
    pub fn select_objects_direct(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
        mode: SelectionMode,
    ) -> Result<usize, DocumentError> {
        let ids = ids.into_iter().collect::<BTreeSet<_>>();
        if let Some(missing) = ids.iter().find(|id| self.object(**id).is_none()) {
            return Err(DocumentError::ObjectNotFound(*missing));
        }
        if let Some(unselectable) = ids.iter().find(|id| !self.is_object_selectable(**id)) {
            return Err(DocumentError::ObjectNotSelectable(*unselectable));
        }
        Ok(self.apply_selection_mode(ids, mode))
    }

    /// Directly adds selectable objects whose names match Rhino's
    /// case-insensitive `*` and `?` wildcard rules. An empty pattern matches
    /// unnamed objects.
    pub fn select_objects_by_name_pattern(&mut self, pattern: &str) -> usize {
        let pattern = CaseInsensitiveWildcard::new(pattern);
        let matches = self
            .objects
            .iter()
            .filter(|object| self.is_object_selectable(object.id))
            .filter(|object| pattern.matches(object.attributes.name.as_deref().unwrap_or_default()))
            .map(|object| object.id)
            .collect();
        self.apply_selection_mode(matches, SelectionMode::Add)
    }

    /// Directly adds the selectable members of an exact, case-sensitive named
    /// group without expanding any overlapping groups.
    pub fn select_group_objects_by_name(&mut self, name: &str) -> usize {
        let matches = self
            .group_by_name(name)
            .map(|group| group.members.clone())
            .unwrap_or_default()
            .into_iter()
            .filter(|id| self.is_object_selectable(*id))
            .collect();
        self.apply_selection_mode(matches, SelectionMode::Add)
    }

    /// Adds every selectable duplicate class to the selection.
    ///
    /// When `include_originals` is false, the first object in document order
    /// is retained as the deterministic original of each class. Attributes,
    /// layers, and group membership do not participate in geometry equality,
    /// and matching a grouped object does not expand its group.
    pub fn select_duplicate_objects(
        &mut self,
        include_originals: bool,
    ) -> Result<usize, DocumentError> {
        let mut candidate_buckets = BTreeMap::<DuplicateGeometryFamily, Vec<usize>>::new();
        for (index, object) in self.objects.iter().enumerate() {
            if self.is_object_selectable(object.id) {
                candidate_buckets
                    .entry(object.geometry.duplicate_family())
                    .or_default()
                    .push(index);
            }
        }
        let mut matches = BTreeSet::new();
        for candidates in candidate_buckets.into_values() {
            let mut classes = Vec::<Vec<usize>>::new();
            for candidate in candidates {
                let mut matching_class = None;
                for (class_index, class) in classes.iter().enumerate() {
                    if self.objects[candidate]
                        .geometry
                        .geometrically_equals(&self.objects[class[0]].geometry)?
                    {
                        matching_class = Some(class_index);
                        break;
                    }
                }
                if let Some(class_index) = matching_class {
                    classes[class_index].push(candidate);
                } else {
                    classes.push(vec![candidate]);
                }
            }
            for class in classes.into_iter().filter(|class| class.len() > 1) {
                matches.extend(
                    class
                        .into_iter()
                        .skip(usize::from(!include_originals))
                        .map(|index| self.objects[index].id),
                );
            }
        }
        Ok(self.apply_selection_mode(matches, SelectionMode::Add))
    }

    /// Selects every layer whose name matches Rhino's case-insensitive `*`
    /// and `?` wildcard rules.
    pub fn select_layer_objects_by_name_pattern(
        &mut self,
        pattern: &str,
    ) -> Result<usize, DocumentError> {
        let pattern = CaseInsensitiveWildcard::new(pattern);
        let layer_ids = self
            .layers
            .iter()
            .filter(|layer| pattern.matches(&layer.name))
            .map(|layer| layer.id)
            .collect::<Vec<_>>();
        self.select_layer_objects(layer_ids)
    }

    /// Makes the requested layers visible and unlocked, then directly adds
    /// their ordinary objects to the selection. Rhino's `SelLayer` persists
    /// these layer-state changes outside normal undo history; object-level
    /// hidden and locked states are retained.
    pub fn select_layer_objects(
        &mut self,
        layer_ids: impl IntoIterator<Item = LayerId>,
    ) -> Result<usize, DocumentError> {
        self.ensure_no_transaction()?;
        let layer_ids = layer_ids.into_iter().collect::<BTreeSet<_>>();
        let indices = layer_ids
            .iter()
            .map(|id| self.layer_index(*id))
            .collect::<Result<Vec<_>, _>>()?;
        for index in indices {
            self.layers[index].visible = true;
            self.layers[index].locked = false;
        }
        let matches = self
            .objects
            .iter()
            .filter(|object| {
                layer_ids.contains(&object.attributes.layer_id)
                    && self.is_object_selectable(object.id)
            })
            .map(|object| object.id)
            .collect();
        Ok(self.apply_selection_mode(matches, SelectionMode::Add))
    }

    /// Selects the objects affected by the latest object-editing transaction.
    /// Rhino's default replaces the current selection; setting
    /// `deselect_others` to false adds the objects instead.
    pub fn select_last_changed(&mut self, deselect_others: bool) -> usize {
        let targets = self.selectable_clusters(self.last_changed_objects.iter().copied());
        self.apply_selection_mode(
            targets,
            if deselect_others {
                SelectionMode::Replace
            } else {
                SelectionMode::Add
            },
        )
    }

    /// Re-selects the previous selection and makes the current selection the
    /// next previous set, so repeated replacement calls toggle between them.
    pub fn select_previous(&mut self, deselect_others: bool) -> usize {
        let targets = self.selectable_clusters(self.previous_selection.iter().copied());
        let current = self.selection.clone();
        let current_order = self.selection_order.clone();
        let target_order =
            self.previous_selection_order
                .iter()
                .copied()
                .filter(|id| targets.contains(id))
                .chain(self.objects.iter().map(|object| object.id).filter(|id| {
                    targets.contains(id) && !self.previous_selection_order.contains(id)
                }))
                .collect::<Vec<_>>();
        let next = if deselect_others {
            targets
        } else {
            let mut next = current.clone();
            next.extend(targets);
            next
        };
        self.selection_order = if deselect_others {
            target_order
        } else {
            current_order
                .iter()
                .copied()
                .chain(target_order.into_iter().filter(|id| !current.contains(id)))
                .collect()
        };
        self.selection = next;
        self.previous_selection = current;
        self.previous_selection_order = current_order;
        self.selection.len()
    }

    pub fn selectable_last_changed_object_count(&self) -> usize {
        self.selectable_clusters(self.last_changed_objects.iter().copied())
            .len()
    }

    pub fn selectable_previous_object_count(&self) -> usize {
        self.selectable_clusters(self.previous_selection.iter().copied())
            .len()
    }

    fn apply_selection_mode(&mut self, cluster: BTreeSet<ObjectId>, mode: SelectionMode) -> usize {
        let mut next = self.selection.clone();
        match mode {
            SelectionMode::Replace => {
                next = cluster;
            }
            SelectionMode::Add => {
                next.extend(cluster);
            }
            SelectionMode::Toggle => {
                if cluster.iter().all(|member| next.contains(member)) {
                    next.retain(|member| !cluster.contains(member));
                } else {
                    next.extend(cluster);
                }
            }
        }
        self.update_selection(next)
    }

    /// Atomically sets object-level visibility without changing geometry,
    /// identity, layers, or group membership. Hidden objects are removed from
    /// the transient selection.
    pub fn set_objects_visibility(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
        visible: bool,
    ) -> Result<usize, DocumentError> {
        self.change_objects(ids, "Set object visibility", |object| {
            object.attributes.visible = visible;
            if !visible {
                object.attributes.locked = false;
            }
            object.isolation = ObjectIsolation::None;
        })
    }

    /// Atomically sets object-level locking without changing geometry,
    /// identity, layers, or group membership. Newly locked objects are removed
    /// from the transient selection.
    pub fn set_objects_locked(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
        locked: bool,
    ) -> Result<usize, DocumentError> {
        self.change_objects(ids, "Set object lock", |object| {
            object.attributes.locked = locked;
            if locked {
                object.attributes.visible = true;
            }
            object.isolation = ObjectIsolation::None;
        })
    }

    /// Atomically assigns trimmed optional names while retaining object
    /// identity, geometry, layers, groups, and selection.
    pub fn set_object_names(
        &mut self,
        assignments: impl IntoIterator<Item = (ObjectId, Option<String>)>,
    ) -> Result<usize, DocumentError> {
        let assignments = assignments
            .into_iter()
            .map(|(id, name)| {
                let name = name
                    .map(|name| name.trim().to_owned())
                    .filter(|name| !name.is_empty());
                (id, name)
            })
            .collect::<BTreeMap<_, _>>();
        if assignments.is_empty() {
            return Ok(0);
        }
        if let Some(missing) = assignments.keys().find(|id| self.object(**id).is_none()) {
            return Err(DocumentError::ObjectNotFound(*missing));
        }

        let mut staged = Vec::with_capacity(assignments.len());
        for (index, object) in self.objects.iter().enumerate() {
            let Some(name) = assignments.get(&object.id) else {
                continue;
            };
            if object.attributes.locked {
                return Err(DocumentError::ObjectLocked(object.id));
            }
            let layer = self
                .layer(object.attributes.layer_id)
                .ok_or(DocumentError::LayerNotFound(object.attributes.layer_id))?;
            if layer.locked {
                return Err(DocumentError::LayerLocked(layer.id));
            }
            if &object.attributes.name == name {
                continue;
            }
            let before = object.clone();
            let mut after = before.clone();
            after.attributes.name = name.clone();
            staged.push((index, before, after));
        }
        if staged.is_empty() {
            return Ok(0);
        }

        let changed_count = staged.len();
        let owns_transaction = self.history.active.is_none();
        if owns_transaction {
            self.begin_transaction("Set object name")?;
        }
        for (index, before, after) in staged {
            let id = before.id;
            self.objects[index] = after.clone();
            self.record_edit("Set object name", Edit::ObjectChanged { id, before, after });
        }
        if owns_transaction {
            self.commit_transaction()?;
        }
        Ok(changed_count)
    }

    /// Swaps normal and hidden object modes on visible, unlocked layers.
    /// Locked objects and every object on a hidden or locked layer are left
    /// unchanged, matching Rhino's `HideSwap` scope.
    pub fn swap_object_visibility_modes(&mut self) -> Result<usize, DocumentError> {
        let ids = self.eligible_object_ids(|object| !object.attributes.locked)?;
        self.change_objects(ids, "Swap object visibility", |object| {
            if !object.attributes.locked {
                object.attributes.visible = !object.attributes.visible;
                object.isolation = ObjectIsolation::None;
            }
        })
    }

    /// Swaps normal and locked object modes on visible, unlocked layers.
    /// Hidden objects and every object on a hidden or locked layer are left
    /// unchanged, matching Rhino's `LockSwap` scope.
    pub fn swap_object_lock_modes(&mut self) -> Result<usize, DocumentError> {
        let ids = self.eligible_object_ids(|object| object.attributes.visible)?;
        self.change_objects(ids, "Swap object lock", |object| {
            if object.attributes.visible {
                object.attributes.locked = !object.attributes.locked;
                object.isolation = ObjectIsolation::None;
            }
        })
    }

    /// Hides ordinary objects outside the current selection on visible,
    /// unlocked layers and records exactly which objects were changed.
    pub fn isolate_selected_objects(&mut self) -> Result<usize, DocumentError> {
        let ids = self.eligible_object_ids(|object| {
            object.attributes.visible
                && !object.attributes.locked
                && !self.selection.contains(&object.id)
        })?;
        self.change_objects(ids, "Isolate objects", |object| {
            object.attributes.visible = false;
            object.attributes.locked = false;
            object.isolation = ObjectIsolation::Hidden;
        })
    }

    /// Shows only objects hidden by [`Self::isolate_selected_objects`].
    pub fn unisolate_objects(&mut self) -> Result<usize, DocumentError> {
        let ids = self
            .objects
            .iter()
            .filter(|object| object.isolation == ObjectIsolation::Hidden)
            .map(|object| object.id)
            .collect::<Vec<_>>();
        self.change_objects(ids, "Unisolate objects", |object| {
            object.attributes.visible = true;
            object.attributes.locked = false;
            object.isolation = ObjectIsolation::None;
        })
    }

    /// Locks ordinary objects outside the current selection on visible,
    /// unlocked layers and records exactly which objects were changed.
    pub fn isolate_lock_selected_objects(&mut self) -> Result<usize, DocumentError> {
        let ids = self.eligible_object_ids(|object| {
            object.attributes.visible
                && !object.attributes.locked
                && !self.selection.contains(&object.id)
        })?;
        self.change_objects(ids, "Isolate-lock objects", |object| {
            object.attributes.visible = true;
            object.attributes.locked = true;
            object.isolation = ObjectIsolation::Locked;
        })
    }

    /// Unlocks only objects locked by [`Self::isolate_lock_selected_objects`].
    pub fn unisolate_locked_objects(&mut self) -> Result<usize, DocumentError> {
        let ids = self
            .objects
            .iter()
            .filter(|object| object.isolation == ObjectIsolation::Locked)
            .map(|object| object.id)
            .collect::<Vec<_>>();
        self.change_objects(ids, "Unisolate-lock objects", |object| {
            object.attributes.visible = true;
            object.attributes.locked = false;
            object.isolation = ObjectIsolation::None;
        })
    }

    pub fn isolated_hidden_object_count(&self) -> usize {
        self.objects
            .iter()
            .filter(|object| object.isolation == ObjectIsolation::Hidden)
            .count()
    }

    pub fn isolated_locked_object_count(&self) -> usize {
        self.objects
            .iter()
            .filter(|object| object.isolation == ObjectIsolation::Locked)
            .count()
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

    /// Atomically replaces geometry while retaining object identity,
    /// attributes, group membership, and selection.
    pub fn replace_object_geometries(
        &mut self,
        replacements: impl IntoIterator<Item = (ObjectId, Geometry)>,
    ) -> Result<usize, DocumentError> {
        let replacements = replacements.into_iter().collect::<BTreeMap<_, _>>();
        if let Some(missing) = replacements.keys().find(|id| self.object(**id).is_none()) {
            return Err(DocumentError::ObjectNotFound(*missing));
        }

        let mut staged = Vec::with_capacity(replacements.len());
        for (index, object) in self.objects.iter().enumerate() {
            let Some(geometry) = replacements.get(&object.id) else {
                continue;
            };
            if object.attributes.locked {
                return Err(DocumentError::ObjectLocked(object.id));
            }
            let layer = self
                .layer(object.attributes.layer_id)
                .ok_or(DocumentError::LayerNotFound(object.attributes.layer_id))?;
            if layer.locked {
                return Err(DocumentError::LayerLocked(layer.id));
            }
            if &object.geometry == geometry {
                continue;
            }
            let before = object.clone();
            let mut after = before.clone();
            after.geometry = geometry.clone();
            staged.push((index, before, after));
        }
        if staged.is_empty() {
            return Ok(0);
        }

        let replacement_count = staged.len();
        let owns_transaction = self.history.active.is_none();
        if owns_transaction {
            self.begin_transaction("Replace object geometry")?;
        }
        for (index, before, after) in staged {
            let id = before.id;
            self.objects[index] = after.clone();
            self.record_edit(
                "Replace object geometry",
                Edit::ObjectChanged { id, before, after },
            );
        }
        if owns_transaction {
            self.commit_transaction()?;
        }
        Ok(replacement_count)
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
                isolation: ObjectIsolation::None,
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
        self.update_selection(copied_ids.iter().copied().collect());
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
            isolation: ObjectIsolation::None,
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
        self.selection_order.retain(|selected| *selected != id);
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
        self.update_selection(BTreeSet::new());
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
        self.insert_group(name, members, false)
    }

    /// Adds an empty group definition, as needed when importing file tables.
    pub fn add_empty_group(&mut self, name: Option<String>) -> Result<GroupId, DocumentError> {
        self.insert_group(name, BTreeSet::new(), true)
    }

    fn insert_group(
        &mut self,
        name: Option<String>,
        members: BTreeSet<ObjectId>,
        allow_empty: bool,
    ) -> Result<GroupId, DocumentError> {
        let name = name
            .map(|name| name.trim().to_owned())
            .filter(|name| !name.is_empty());
        if let Some(name) = &name
            && self.groups.iter().any(|group| {
                group
                    .name
                    .as_ref()
                    .is_some_and(|candidate| candidate == name)
            })
        {
            return Err(DocumentError::DuplicateGroupName(name.clone()));
        }
        if members.is_empty() && !allow_empty {
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
        let name = name.trim();
        self.groups.iter().find(|group| {
            group
                .name
                .as_ref()
                .is_some_and(|candidate| candidate == name)
        })
    }

    /// Returns Rhino's next unused automatic group name (`Group01`,
    /// `Group02`, ...). Group-name comparisons are case-sensitive.
    pub fn next_unused_group_name(&self) -> String {
        for number in 1_u64..=u64::MAX {
            let candidate = format!("Group{number:02}");
            if self.group_by_name(&candidate).is_none() {
                return candidate;
            }
        }
        loop {
            let candidate = format!("Group{}", GroupId::new());
            if self.group_by_name(&candidate).is_none() {
                return candidate;
            }
        }
    }

    /// Adds existing objects to an existing group as one undoable edit.
    pub fn add_group_members(
        &mut self,
        group_id: GroupId,
        members: impl IntoIterator<Item = ObjectId>,
    ) -> Result<usize, DocumentError> {
        let group_index = self
            .groups
            .iter()
            .position(|group| group.id == group_id)
            .ok_or(DocumentError::GroupNotFound(group_id))?;
        let members = members.into_iter().collect::<BTreeSet<_>>();
        if let Some(missing) = members.iter().find(|id| self.object(**id).is_none()) {
            return Err(DocumentError::ObjectNotFound(*missing));
        }
        let additions = members
            .into_iter()
            .filter(|member| !self.groups[group_index].members.contains(member))
            .collect::<Vec<_>>();
        if additions.is_empty() {
            return Ok(0);
        }

        let owns_transaction = self.history.active.is_none();
        if owns_transaction {
            self.begin_transaction("Add group members")?;
        }
        for object_id in &additions {
            let inserted = self.groups[group_index].members.insert(*object_id);
            debug_assert!(inserted, "new group members were filtered in advance");
            self.record_edit(
                "Add group member",
                Edit::GroupMemberInserted {
                    group_id,
                    object_id: *object_id,
                },
            );
        }
        if owns_transaction {
            self.commit_transaction()?;
        }
        Ok(additions.len())
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

    fn selectable_clusters(&self, ids: impl IntoIterator<Item = ObjectId>) -> BTreeSet<ObjectId> {
        let mut clusters = BTreeSet::new();
        for id in ids {
            if self.is_object_selectable(id) {
                clusters.extend(self.selectable_group_cluster(id));
            }
        }
        clusters
    }

    fn update_selection(&mut self, next: BTreeSet<ObjectId>) -> usize {
        if self.selection != next {
            if !self.selection.is_subset(&next) {
                self.previous_selection = self.selection.clone();
                self.previous_selection_order = self.selection_order.clone();
            }
            let mut ordered = self
                .selection_order
                .iter()
                .copied()
                .filter(|id| next.contains(id))
                .collect::<Vec<_>>();
            let retained = ordered.iter().copied().collect::<BTreeSet<_>>();
            ordered.extend(
                self.objects
                    .iter()
                    .map(|object| object.id)
                    .filter(|id| next.contains(id) && !retained.contains(id)),
            );
            self.selection = next;
            self.selection_order = ordered;
        }
        self.selection.len()
    }

    fn change_objects(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
        label: &'static str,
        change: impl Fn(&mut Object),
    ) -> Result<usize, DocumentError> {
        let mut remaining = ids.into_iter().collect::<BTreeSet<_>>();
        if remaining.is_empty() {
            return Ok(0);
        }
        let mut staged = Vec::with_capacity(remaining.len());
        for (index, object) in self.objects.iter().enumerate() {
            if !remaining.remove(&object.id) {
                continue;
            }
            let before = object.clone();
            let mut after = before.clone();
            change(&mut after);
            if before != after {
                staged.push((index, before, after));
            }
            if remaining.is_empty() {
                break;
            }
        }
        if let Some(missing) = remaining.first().copied() {
            return Err(DocumentError::ObjectNotFound(missing));
        }
        if staged.is_empty() {
            return Ok(0);
        }

        let changed_count = staged.len();
        let owns_transaction = self.history.active.is_none();
        if owns_transaction {
            self.begin_transaction(label)?;
        }
        for (index, before, after) in staged {
            let id = before.id;
            self.objects[index] = after.clone();
            self.record_edit(label, Edit::ObjectChanged { id, before, after });
        }
        self.prune_selection();
        if owns_transaction {
            self.commit_transaction()?;
        }
        Ok(changed_count)
    }

    fn eligible_object_ids(
        &self,
        include: impl Fn(&Object) -> bool,
    ) -> Result<Vec<ObjectId>, DocumentError> {
        let layer_is_eligible = self
            .layers
            .iter()
            .map(|layer| (layer.id, layer.visible && !layer.locked))
            .collect::<BTreeMap<_, _>>();
        let mut ids = Vec::with_capacity(self.objects.len());
        for object in &self.objects {
            let eligible = layer_is_eligible
                .get(&object.attributes.layer_id)
                .copied()
                .ok_or(DocumentError::LayerNotFound(object.attributes.layer_id))?;
            if eligible && include(object) {
                ids.push(object.id);
            }
        }
        Ok(ids)
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
        let selection = self
            .selection
            .iter()
            .copied()
            .filter(|id| self.is_object_selectable(*id))
            .collect();
        self.update_selection(selection);
    }

    fn record_layer_change(&mut self, label: &'static str, index: usize, before: Layer) {
        let after = self.layers[index].clone();
        let id = after.id;
        self.record_edit(label, Edit::LayerChanged { id, before, after });
    }

    fn single_affected_object_id(edit: &Edit) -> Option<ObjectId> {
        match edit {
            Edit::ObjectInserted { id, .. }
            | Edit::ObjectRemoved { id, .. }
            | Edit::ObjectChanged { id, .. } => Some(*id),
            Edit::GroupMemberRemoved { object_id, .. }
            | Edit::GroupMemberInserted { object_id, .. } => Some(*object_id),
            _ => None,
        }
    }

    fn affected_object_ids(&self, edit: &Edit) -> BTreeSet<ObjectId> {
        match edit {
            Edit::GroupInserted { id, .. } => self
                .group(*id)
                .map(|group| group.members.clone())
                .unwrap_or_default(),
            Edit::GroupRemoved { stored, .. } => stored
                .as_ref()
                .map(|group| group.members.clone())
                .unwrap_or_default(),
            Edit::ObjectsCleared { stored_objects, .. } => {
                stored_objects.iter().map(|object| object.id).collect()
            }
            Edit::ObjectInserted { .. }
            | Edit::ObjectRemoved { .. }
            | Edit::ObjectChanged { .. }
            | Edit::LayerInserted { .. }
            | Edit::LayerRemoved { .. }
            | Edit::LayerChanged { .. }
            | Edit::GroupMemberRemoved { .. }
            | Edit::GroupMemberInserted { .. }
            | Edit::CurrentLayerChanged { .. } => BTreeSet::new(),
        }
    }

    fn record_edit(&mut self, label: &'static str, edit: Edit) {
        if let Some(object_id) = Self::single_affected_object_id(&edit) {
            if let Some(transaction) = &mut self.history.active {
                transaction.object_ids.insert(object_id);
                transaction.edits.push(edit);
                return;
            }
            self.push_new_undo(HistoryEntry {
                label: label.to_owned(),
                edits: vec![edit],
                object_ids: BTreeSet::from([object_id]),
            });
            return;
        }
        let object_ids = self.affected_object_ids(&edit);
        if let Some(transaction) = &mut self.history.active {
            transaction.object_ids.extend(object_ids);
            transaction.edits.push(edit);
            return;
        }
        self.push_new_undo(HistoryEntry {
            label: label.to_owned(),
            edits: vec![edit],
            object_ids,
        });
    }

    fn push_new_undo(&mut self, entry: HistoryEntry) {
        self.update_last_changed_objects(&entry.object_ids);
        self.history.redo.clear();
        self.push_replayed_undo(entry);
    }

    fn update_last_changed_objects(&mut self, ids: &BTreeSet<ObjectId>) {
        self.last_changed_objects = ids
            .iter()
            .copied()
            .filter(|id| self.object(*id).is_some())
            .collect();
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

    fn point(x: f64, y: f64, z: f64) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    #[test]
    fn geometric_equality_is_scale_aware_and_direction_independent() {
        let tolerance = Tolerance::DEFAULT;
        let start = point(0.0, 0.0, 0.0);
        let end = point(4.0, 2.0, 0.0);
        let line = Geometry::Line(LineSegment::try_new(start, end, tolerance).unwrap());
        let reversed_line = Geometry::Line(LineSegment::try_new(end, start, tolerance).unwrap());
        assert!(line.geometrically_equals(&reversed_line).unwrap());

        let zero = point(0.0, 0.0, 0.0);
        let five = point(5.0, 0.0, 0.0);
        let reference_line = Geometry::Line(LineSegment::try_new(zero, five, tolerance).unwrap());
        let near_line = Geometry::Line(
            LineSegment::try_new(zero, point(5.0 + 1.0e-7, 0.0, 0.0), tolerance).unwrap(),
        );
        let far_line = Geometry::Line(
            LineSegment::try_new(zero, point(5.0 + 1.0e-6, 0.0, 0.0), tolerance).unwrap(),
        );
        assert!(reference_line.geometrically_equals(&near_line).unwrap());
        assert!(!reference_line.geometrically_equals(&far_line).unwrap());

        let weighted_line = Geometry::NurbsCurve(
            NurbsCurve::try_new_rational(
                1,
                vec![
                    viboceros_geometry::WeightedPoint3::try_new(start, 2.0).unwrap(),
                    viboceros_geometry::WeightedPoint3::try_new(end, 5.0).unwrap(),
                ],
                vec![2.0, 2.0, 8.0, 8.0],
            )
            .unwrap(),
        );
        assert!(line.geometrically_equals(&weighted_line).unwrap());

        let open = Geometry::Polyline(
            Polyline3::try_new(vec![start, point(2.0, 0.0, 0.0), end], tolerance).unwrap(),
        );
        let open_reversed = match &open {
            Geometry::Polyline(polyline) => Geometry::Polyline(polyline.reversed()),
            _ => unreachable!(),
        };
        assert!(open.geometrically_equals(&open_reversed).unwrap());

        let closed = Geometry::Polyline(
            Polyline3::try_new(
                vec![start, point(2.0, 0.0, 0.0), point(2.0, 2.0, 0.0), start],
                tolerance,
            )
            .unwrap(),
        );
        let shifted_seam = Geometry::Polyline(
            Polyline3::try_new(
                vec![
                    point(2.0, 0.0, 0.0),
                    point(2.0, 2.0, 0.0),
                    start,
                    point(2.0, 0.0, 0.0),
                ],
                tolerance,
            )
            .unwrap(),
        );
        assert!(!closed.geometrically_equals(&shifted_seam).unwrap());

        let up = viboceros_geometry::UnitVector3::try_new(0.0, 0.0, 1.0, tolerance).unwrap();
        let down = up.opposite();
        let circle = Geometry::Circle(Circle3::try_new(start, 3.0, up, tolerance).unwrap());
        let opposite_circle =
            Geometry::Circle(Circle3::try_new(start, 3.0, down, tolerance).unwrap());
        assert!(circle.geometrically_equals(&opposite_circle).unwrap());
        let radius_five = Geometry::Circle(Circle3::try_new(start, 5.0, up, tolerance).unwrap());
        let near_radius =
            Geometry::Circle(Circle3::try_new(start, 5.0 + 1.0e-7, up, tolerance).unwrap());
        let far_radius =
            Geometry::Circle(Circle3::try_new(start, 5.0 + 1.0e-6, up, tolerance).unwrap());
        assert!(radius_five.geometrically_equals(&near_radius).unwrap());
        assert!(!radius_five.geometrically_equals(&far_radius).unwrap());

        let arc = CircularArc3::try_from_three_points(
            point(1.0, 0.0, 0.0),
            point(0.0, 1.0, 0.0),
            point(-1.0, 0.0, 0.0),
            tolerance,
        )
        .unwrap();
        assert!(
            Geometry::Arc(arc)
                .geometrically_equals(&Geometry::Arc(arc.reversed(tolerance).unwrap()))
                .unwrap()
        );

        assert!(
            Geometry::Point(start)
                .geometrically_equals(&Geometry::Point(start))
                .unwrap()
        );
        assert!(
            !Geometry::Point(start)
                .geometrically_equals(&Geometry::Point(point(f64::EPSILON, 0.0, 0.0)))
                .unwrap()
        );

        let cloud = Geometry::PointCloud(
            PointCloud3::try_new(vec![point(1.0, 2.0, 3.0), point(4.0, 5.0, 6.0)]).unwrap(),
        );
        let near_cloud = Geometry::PointCloud(
            PointCloud3::try_new(vec![point(1.0 + 1.0e-8, 2.0, 3.0), point(4.0, 5.0, 6.0)])
                .unwrap(),
        );
        let far_cloud = Geometry::PointCloud(
            PointCloud3::try_new(vec![point(1.0 + 1.0e-7, 2.0, 3.0), point(4.0, 5.0, 6.0)])
                .unwrap(),
        );
        let reversed_cloud = Geometry::PointCloud(
            PointCloud3::try_new(vec![point(4.0, 5.0, 6.0), point(1.0, 2.0, 3.0)]).unwrap(),
        );
        assert!(cloud.geometrically_equals(&near_cloud).unwrap());
        assert!(!cloud.geometrically_equals(&far_cloud).unwrap());
        assert!(!cloud.geometrically_equals(&reversed_cloud).unwrap());
    }

    #[test]
    fn point_cloud_duplicate_selection_is_ordered_and_scale_aware() {
        let mut document = Document::default();
        for points in [
            vec![point(1.0, 2.0, 3.0), point(4.0, 5.0, 6.0)],
            vec![point(1.0 + 1.0e-8, 2.0, 3.0), point(4.0, 5.0, 6.0)],
            vec![point(4.0, 5.0, 6.0), point(1.0, 2.0, 3.0)],
        ] {
            document
                .add_geometry(Geometry::PointCloud(PointCloud3::try_new(points).unwrap()))
                .unwrap();
        }
        assert_eq!(document.select_duplicate_objects(true).unwrap(), 2);
        assert_eq!(
            document.selected_object_ids().collect::<Vec<_>>(),
            document
                .objects()
                .take(2)
                .map(|object| object.id())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn duplicate_selection_is_additive_attribute_free_and_document_ordered() {
        let tolerance = Tolerance::DEFAULT;
        let mut document = Document::default();
        let layer = document.current_layer_id();
        let unrelated = document
            .add_geometry(Geometry::Point(point(30.0, 0.0, 0.0)))
            .unwrap();
        let first = document
            .add_geometry(Geometry::Point(point(0.0, 0.0, 0.0)))
            .unwrap();
        let second = document
            .add_geometry_with_attributes(
                Geometry::Point(point(0.0, 0.0, 0.0)),
                ObjectAttributes::on_layer(layer).with_name("different"),
            )
            .unwrap();
        let hidden = document
            .add_geometry_with_attributes(
                Geometry::Point(point(0.0, 0.0, 0.0)),
                ObjectAttributes::on_layer(layer).with_visibility(false),
            )
            .unwrap();
        let locked = document
            .add_geometry_with_attributes(
                Geometry::Point(point(0.0, 0.0, 0.0)),
                ObjectAttributes::on_layer(layer).with_locked(true),
            )
            .unwrap();
        let near = document
            .add_geometry(Geometry::Point(point(tolerance.absolute() * 0.5, 0.0, 0.0)))
            .unwrap();
        let group_peer = document
            .add_geometry(Geometry::Point(point(20.0, 0.0, 0.0)))
            .unwrap();
        document
            .add_group(Some("Pair".to_owned()), [second, group_peer])
            .unwrap();

        let line_start = point(0.0, 10.0, 0.0);
        let line_end = point(5.0, 10.0, 0.0);
        let line_first = document
            .add_geometry(Geometry::Line(
                LineSegment::try_new(line_start, line_end, tolerance).unwrap(),
            ))
            .unwrap();
        let line_second = document
            .add_geometry(Geometry::Line(
                LineSegment::try_new(line_end, line_start, tolerance).unwrap(),
            ))
            .unwrap();
        let line_nurbs = document
            .add_geometry(Geometry::NurbsCurve(
                NurbsCurve::try_new(1, vec![line_start, line_end], vec![0.0, 0.0, 1.0, 1.0])
                    .unwrap(),
            ))
            .unwrap();

        document
            .select_object(unrelated, SelectionMode::Replace)
            .unwrap();
        assert_eq!(document.select_duplicate_objects(true).unwrap(), 6);
        assert_eq!(
            document.selected_object_ids().collect::<BTreeSet<_>>(),
            BTreeSet::from([
                unrelated,
                first,
                second,
                line_first,
                line_second,
                line_nurbs,
            ])
        );
        assert!(!document.is_selected(hidden));
        assert!(!document.is_selected(locked));
        assert!(!document.is_selected(near));
        assert!(!document.is_selected(group_peer));

        document.clear_selection();
        document
            .select_object(unrelated, SelectionMode::Replace)
            .unwrap();
        assert_eq!(document.select_duplicate_objects(false).unwrap(), 4);
        assert_eq!(
            document.selected_object_ids().collect::<BTreeSet<_>>(),
            BTreeSet::from([unrelated, second, line_second, line_nurbs])
        );
        assert!(!document.is_selected(first));
        assert!(!document.is_selected(line_first));
    }

    #[test]
    fn extracts_rhino_defining_points_across_supported_geometry() {
        let tolerance = Tolerance::DEFAULT;
        assert!(
            Geometry::Point(point(1.0, 2.0, 3.0))
                .extract_point_locations()
                .unwrap()
                .is_empty()
        );

        let line =
            LineSegment::try_new(point(0.0, 0.0, 0.0), point(2.0, 3.0, 4.0), tolerance).unwrap();
        assert_eq!(
            Geometry::Line(line).extract_point_locations().unwrap(),
            vec![point(0.0, 0.0, 0.0), point(2.0, 3.0, 4.0)]
        );

        let circle = Circle3::try_new(
            point(0.0, 0.0, 0.0),
            2.0,
            viboceros_geometry::UnitVector3::try_new(0.0, 0.0, 1.0, tolerance).unwrap(),
            tolerance,
        )
        .unwrap();
        let circle_points = Geometry::Circle(circle).extract_point_locations().unwrap();
        assert_eq!(circle_points.len(), 8);
        assert_eq!(circle_points[0], point(2.0, 0.0, 0.0));
        assert_ne!(circle_points.first(), circle_points.last());

        let closed_polyline = Polyline3::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(2.0, 2.0, 0.0),
                point(0.0, 0.0, 0.0),
            ],
            tolerance,
        )
        .unwrap();
        assert_eq!(
            Geometry::Polyline(closed_polyline)
                .extract_point_locations()
                .unwrap(),
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(2.0, 2.0, 0.0),
            ]
        );

        let surface_corners = [
            point(0.0, 0.0, 0.0),
            point(2.0, 0.0, 1.0),
            point(2.0, 3.0, 3.0),
            point(0.0, 3.0, 2.0),
        ];
        let surface = NurbsSurface::try_bilinear(surface_corners).unwrap();
        assert_eq!(
            Geometry::NurbsSurface(surface)
                .extract_point_locations()
                .unwrap(),
            vec![
                surface_corners[0],
                surface_corners[3],
                surface_corners[1],
                surface_corners[2],
            ]
        );

        let mesh = TriangleMesh::try_new(
            vec![
                point(99.0, 99.0, 99.0),
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(0.0, 2.0, 0.0),
            ],
            vec![[1, 2, 3]],
            tolerance,
        )
        .unwrap();
        assert_eq!(
            Geometry::Mesh(mesh).extract_point_locations().unwrap(),
            vec![
                point(99.0, 99.0, 99.0),
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(0.0, 2.0, 0.0),
            ]
        );
    }

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
    fn deleting_an_object_prunes_groups_losing_their_last_member() {
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
    fn empty_group_definitions_are_reversible_and_can_gain_members() {
        let mut document = Document::default();
        let object = document
            .add_geometry(Geometry::Point(Point3::try_new(0.0, 0.0, 0.0).unwrap()))
            .unwrap();
        let group = document
            .add_empty_group(Some("File Group".to_owned()))
            .unwrap();
        assert_eq!(document.group(group).unwrap().members().len(), 0);
        assert_eq!(document.add_group_members(group, [object]).unwrap(), 1);
        assert_eq!(document.group(group).unwrap().members().len(), 1);
        document.undo().unwrap();
        assert_eq!(document.group(group).unwrap().members().len(), 0);
        document.undo().unwrap();
        assert!(document.group(group).is_none());
        document.redo().unwrap();
        assert_eq!(document.group(group).unwrap().members().len(), 0);
    }

    #[test]
    fn adding_group_members_is_deduplicated_and_reversible() {
        let mut document = Document::default();
        let first = document
            .add_geometry(Geometry::Point(Point3::try_new(0.0, 0.0, 0.0).unwrap()))
            .unwrap();
        let second = document
            .add_geometry(Geometry::Point(Point3::try_new(1.0, 0.0, 0.0).unwrap()))
            .unwrap();
        let group = document
            .add_group(Some("Pair".to_owned()), [first])
            .unwrap();

        assert_eq!(
            document.add_group_members(group, [first, second]).unwrap(),
            1
        );
        assert_eq!(document.undo_label(), Some("Add group members"));
        assert_eq!(
            document
                .group(group)
                .unwrap()
                .members()
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([first, second])
        );
        assert_eq!(document.add_group_members(group, [second]).unwrap(), 0);
        assert_eq!(document.undo_label(), Some("Add group members"));

        document.undo().unwrap();
        assert_eq!(
            document.group(group).unwrap().members().collect::<Vec<_>>(),
            vec![first]
        );
        document.redo().unwrap();
        assert_eq!(
            document
                .group(group)
                .unwrap()
                .members()
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([first, second])
        );

        let missing = ObjectId::new();
        assert_eq!(
            document.add_group_members(group, [missing]),
            Err(DocumentError::ObjectNotFound(missing))
        );
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
        assert!(document.group_by_name("pair").is_none());
        let case_variant = document
            .add_group(Some("PAIR".to_owned()), [first])
            .unwrap();
        assert_ne!(case_variant, group);
        assert_eq!(document.next_unused_group_name(), "Group01");
        document
            .add_group(Some("Group01".to_owned()), [second])
            .unwrap();
        assert_eq!(document.next_unused_group_name(), "Group02");
        document
            .add_group(Some("group02".to_owned()), [second])
            .unwrap();
        assert_eq!(document.next_unused_group_name(), "Group02");
        assert_eq!(
            document.add_group(Some("Pair".to_owned()), [first]),
            Err(DocumentError::DuplicateGroupName("Pair".to_owned()))
        );

        assert_eq!(document.remove_group(group).unwrap(), 2);
        assert!(document.group(group).is_none());
        document.undo().unwrap();
        assert_eq!(document.group(group).unwrap().members().len(), 2);
        document.redo().unwrap();
        assert!(document.group(group).is_none());
    }

    #[test]
    fn object_names_are_atomic_identity_preserving_and_reversible() {
        let mut document = Document::default();
        let first = document
            .add_geometry(Geometry::Point(point(0.0, 0.0, 0.0)))
            .unwrap();
        let second = document
            .add_geometry(Geometry::Point(point(1.0, 0.0, 0.0)))
            .unwrap();
        let locked = document
            .add_geometry(Geometry::Point(point(2.0, 0.0, 0.0)))
            .unwrap();
        let group = document
            .add_group(Some("Names".to_owned()), [first, second])
            .unwrap();
        document
            .select_object(first, SelectionMode::Replace)
            .unwrap();

        assert_eq!(
            document
                .set_object_names([
                    (first, Some(" First ".to_owned())),
                    (second, Some("Second".to_owned())),
                ])
                .unwrap(),
            2
        );
        assert_eq!(document.undo_label(), Some("Set object name"));
        assert_eq!(
            document.object(first).unwrap().attributes().name(),
            Some("First")
        );
        assert_eq!(
            document.object(second).unwrap().attributes().name(),
            Some("Second")
        );
        assert_eq!(document.selected_object_count(), 2);
        assert_eq!(document.group(group).unwrap().members().len(), 2);

        document.undo().unwrap();
        assert_eq!(document.object(first).unwrap().attributes().name(), None);
        assert_eq!(document.object(second).unwrap().attributes().name(), None);
        assert_eq!(document.selected_object_count(), 2);
        document.redo().unwrap();
        assert_eq!(
            document.object(first).unwrap().attributes().name(),
            Some("First")
        );
        assert_eq!(
            document
                .set_object_names([(first, Some("First".to_owned()))])
                .unwrap(),
            0
        );
        assert_eq!(document.set_object_names([(second, None)]).unwrap(), 1);
        assert_eq!(document.object(second).unwrap().attributes().name(), None);

        document.set_objects_locked([locked], true).unwrap();
        let before = document.objects().cloned().collect::<Vec<_>>();
        assert_eq!(
            document.set_object_names([
                (first, Some("Changed".to_owned())),
                (locked, Some("Locked".to_owned())),
            ]),
            Err(DocumentError::ObjectLocked(locked))
        );
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
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
    fn direct_and_layer_selection_match_attribute_command_scope_without_history() {
        let mut document = Document::default();
        let default = document.current_layer_id();
        let first = document
            .add_geometry(Geometry::Point(point(0.0, 0.0, 0.0)))
            .unwrap();
        let grouped = document
            .add_geometry(Geometry::Point(point(1.0, 0.0, 0.0)))
            .unwrap();
        let locked_object = document
            .add_geometry(Geometry::Point(point(2.0, 0.0, 0.0)))
            .unwrap();
        document
            .add_group(Some("Pair".to_owned()), [first, grouped])
            .unwrap();
        document.set_objects_locked([locked_object], true).unwrap();

        let hidden_layer = document
            .add_layer("Hidden", ColorRgb::new(10, 20, 30))
            .unwrap();
        document.set_current_layer(hidden_layer).unwrap();
        let on_hidden_layer = document
            .add_geometry(Geometry::Point(point(3.0, 0.0, 0.0)))
            .unwrap();
        let hidden_object = document
            .add_geometry_with_attributes(
                Geometry::Point(point(4.0, 0.0, 0.0)),
                ObjectAttributes::on_layer(hidden_layer).with_visibility(false),
            )
            .unwrap();
        document.set_current_layer(default).unwrap();
        document.set_layer_visibility(hidden_layer, false).unwrap();

        let locked_layer = document
            .add_layer("Locked", ColorRgb::new(40, 50, 60))
            .unwrap();
        document.set_current_layer(locked_layer).unwrap();
        let on_locked_layer = document
            .add_geometry(Geometry::Point(point(5.0, 0.0, 0.0)))
            .unwrap();
        let locked_on_layer = document
            .add_geometry_with_attributes(
                Geometry::Point(point(6.0, 0.0, 0.0)),
                ObjectAttributes::on_layer(locked_layer).with_locked(true),
            )
            .unwrap();
        document.set_current_layer(default).unwrap();
        document.set_layer_locked(locked_layer, true).unwrap();
        let history = document.undo_label().map(str::to_owned);

        assert_eq!(
            document
                .select_objects_direct([first], SelectionMode::Replace)
                .unwrap(),
            1
        );
        assert!(document.is_selected(first));
        assert!(!document.is_selected(grouped));
        assert_eq!(
            document.select_objects_direct([locked_object], SelectionMode::Add),
            Err(DocumentError::ObjectNotSelectable(locked_object))
        );
        assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), [first]);

        assert_eq!(
            document
                .select_layer_objects([hidden_layer, locked_layer])
                .unwrap(),
            3
        );
        assert!(document.layer(hidden_layer).unwrap().is_visible());
        assert!(!document.layer(locked_layer).unwrap().is_locked());
        assert_eq!(
            document.selected_object_ids().collect::<BTreeSet<_>>(),
            BTreeSet::from([first, on_hidden_layer, on_locked_layer])
        );
        assert!(!document.is_selected(hidden_object));
        assert!(!document.is_selected(locked_on_layer));
        assert_eq!(document.undo_label(), history.as_deref());

        document.begin_transaction("active").unwrap();
        assert_eq!(
            document.select_layer_objects([hidden_layer]),
            Err(DocumentError::TransactionInProgress)
        );
        document.rollback_transaction().unwrap();
    }

    #[test]
    fn name_wildcards_support_rhino_star_question_and_case_rules() {
        for (pattern, candidate, expected) in [
            ("Bolt*", "bolt assembly", true),
            ("?olt?", "BOLTA", true),
            ("*", "", true),
            ("", "", true),
            ("", "Bolt", false),
            ("Bolt?", "Bolt", false),
            ("Bolt", "bolt", true),
            ("Å*", "ångström", true),
        ] {
            assert_eq!(
                CaseInsensitiveWildcard::new(pattern).matches(candidate),
                expected,
                "pattern {pattern:?}, candidate {candidate:?}"
            );
        }
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
    fn action_order_selection_tracks_transactions_and_toggles_previous_sets() {
        let mut document = Document::default();
        let first = document
            .add_geometry(Geometry::Point(Point3::try_new(0.0, 0.0, 0.0).unwrap()))
            .unwrap();
        let second = document
            .add_geometry(Geometry::Point(Point3::try_new(1.0, 0.0, 0.0).unwrap()))
            .unwrap();
        let third = document
            .add_geometry(Geometry::Point(Point3::try_new(2.0, 0.0, 0.0).unwrap()))
            .unwrap();
        let selected = |document: &Document| {
            document
                .selected_object_ids()
                .collect::<BTreeSet<ObjectId>>()
        };

        assert_eq!(document.selectable_last_changed_object_count(), 1);
        assert_eq!(document.select_last_changed(true), 1);
        assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), [third]);
        document
            .select_objects([first, second], SelectionMode::Replace)
            .unwrap();
        document.clear_selection();
        document.select_object(third, SelectionMode::Add).unwrap();
        assert_eq!(document.selectable_previous_object_count(), 2);
        assert_eq!(document.select_previous(true), 2);
        assert_eq!(selected(&document), BTreeSet::from([first, second]));
        assert_eq!(document.select_previous(true), 1);
        assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), [third]);

        document
            .select_objects([first, second], SelectionMode::Replace)
            .unwrap();
        document.clear_selection();
        document.select_object(third, SelectionMode::Add).unwrap();
        assert_eq!(document.select_previous(false), 3);
        assert_eq!(selected(&document), BTreeSet::from([first, second, third]));
        assert_eq!(document.select_previous(true), 1);
        assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), [third]);

        document.clear_selection();
        document.begin_transaction("Add pair").unwrap();
        let fourth = document
            .add_geometry(Geometry::Point(Point3::try_new(3.0, 0.0, 0.0).unwrap()))
            .unwrap();
        let fifth = document
            .add_geometry(Geometry::Point(Point3::try_new(4.0, 0.0, 0.0).unwrap()))
            .unwrap();
        document.commit_transaction().unwrap();
        assert_eq!(document.selectable_last_changed_object_count(), 2);
        assert_eq!(document.select_last_changed(true), 2);
        assert_eq!(selected(&document), BTreeSet::from([fourth, fifth]));
        document.undo().unwrap();
        assert_eq!(document.selectable_last_changed_object_count(), 0);
        assert_eq!(document.selected_object_count(), 0);
        document.redo().unwrap();
        assert_eq!(document.select_last_changed(true), 2);

        document
            .add_group(Some("First pair".to_owned()), [first, second])
            .unwrap();
        assert_eq!(document.select_last_changed(true), 2);
        assert_eq!(selected(&document), BTreeSet::from([first, second]));
        let missing = ObjectId::new();
        assert_eq!(
            document.select_objects([first, missing], SelectionMode::Replace),
            Err(DocumentError::ObjectNotFound(missing))
        );
        assert_eq!(selected(&document), BTreeSet::from([first, second]));

        document.clear_selection();
        document.select_object(third, SelectionMode::Add).unwrap();
        document.begin_transaction("Cancelled delete").unwrap();
        document.delete_object(third).unwrap();
        document.rollback_transaction().unwrap();
        assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), [third]);
        assert_eq!(document.select_previous(true), 2);
        assert_eq!(selected(&document), BTreeSet::from([first, second]));
    }

    #[test]
    fn selection_iteration_preserves_action_order_across_pruning_and_rollback() {
        let mut document = Document::default();
        let first = document
            .add_geometry(Geometry::Point(point(0.0, 0.0, 0.0)))
            .unwrap();
        let second = document
            .add_geometry(Geometry::Point(point(1.0, 0.0, 0.0)))
            .unwrap();
        let third = document
            .add_geometry(Geometry::Point(point(2.0, 0.0, 0.0)))
            .unwrap();

        document
            .select_object(third, SelectionMode::Replace)
            .unwrap();
        document.select_object(first, SelectionMode::Add).unwrap();
        document.select_object(second, SelectionMode::Add).unwrap();
        assert_eq!(
            document.selected_object_ids().collect::<Vec<_>>(),
            [third, first, second]
        );

        document.begin_transaction("Cancelled delete").unwrap();
        document.delete_object(first).unwrap();
        assert_eq!(
            document.selected_object_ids().collect::<Vec<_>>(),
            [third, second]
        );
        document.rollback_transaction().unwrap();
        assert_eq!(
            document.selected_object_ids().collect::<Vec<_>>(),
            [third, first, second]
        );

        document.clear_selection();
        document.select_object(second, SelectionMode::Add).unwrap();
        assert_eq!(document.select_previous(true), 3);
        assert_eq!(
            document.selected_object_ids().collect::<Vec<_>>(),
            [third, first, second]
        );
        assert_eq!(document.select_previous(true), 1);
        assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), [second]);
    }

    #[test]
    fn object_visibility_and_lock_are_atomic_identity_preserving_and_reversible() {
        let mut document = Document::default();
        let hidden = ObjectAttributes::on_layer(document.current_layer_id())
            .with_locked(true)
            .with_visibility(false);
        assert!(!hidden.is_visible());
        assert!(!hidden.is_locked());
        let locked = hidden.with_locked(true);
        assert!(locked.is_visible());
        assert!(locked.is_locked());

        let first = document
            .add_geometry(Geometry::Point(Point3::try_new(0.0, 0.0, 0.0).unwrap()))
            .unwrap();
        let second = document
            .add_geometry(Geometry::Point(Point3::try_new(1.0, 0.0, 0.0).unwrap()))
            .unwrap();
        let third = document
            .add_geometry(Geometry::Point(Point3::try_new(2.0, 0.0, 0.0).unwrap()))
            .unwrap();
        let group = document
            .add_group(Some("Pair".to_owned()), [first, second])
            .unwrap();
        document
            .select_object(first, SelectionMode::Replace)
            .unwrap();
        assert_eq!(document.selected_object_count(), 2);

        assert_eq!(
            document
                .set_objects_visibility([second, first, first], false)
                .unwrap(),
            2
        );
        assert_eq!(document.undo_label(), Some("Set object visibility"));
        assert_eq!(document.selected_object_count(), 0);
        assert!(!document.object(first).unwrap().attributes().is_visible());
        assert!(!document.object(second).unwrap().attributes().is_visible());
        assert!(document.object(third).unwrap().attributes().is_visible());
        assert_eq!(document.group(group).unwrap().members().len(), 2);
        assert_eq!(
            document
                .set_objects_visibility([first, second], false)
                .unwrap(),
            0
        );

        let missing = ObjectId::new();
        assert_eq!(
            document.set_objects_visibility([third, missing], false),
            Err(DocumentError::ObjectNotFound(missing))
        );
        assert!(document.object(third).unwrap().attributes().is_visible());

        document.undo().unwrap();
        assert!(document.object(first).unwrap().attributes().is_visible());
        assert!(document.object(second).unwrap().attributes().is_visible());
        assert_eq!(document.object(first).unwrap().id(), first);
        document.redo().unwrap();
        assert!(!document.object(first).unwrap().attributes().is_visible());
        assert_eq!(
            document
                .set_objects_visibility([first, second], true)
                .unwrap(),
            2
        );

        document
            .select_object(first, SelectionMode::Replace)
            .unwrap();
        assert_eq!(
            document.set_objects_locked([first, second], true).unwrap(),
            2
        );
        assert_eq!(document.undo_label(), Some("Set object lock"));
        assert_eq!(document.selected_object_count(), 0);
        assert!(document.object(first).unwrap().attributes().is_locked());
        assert!(document.object(second).unwrap().attributes().is_locked());
        assert!(!document.is_object_selectable(first));
        assert_eq!(document.set_objects_locked([first], true).unwrap(), 0);

        document.undo().unwrap();
        assert!(!document.object(first).unwrap().attributes().is_locked());
        assert!(!document.object(second).unwrap().attributes().is_locked());
        document.redo().unwrap();
        assert!(document.object(first).unwrap().attributes().is_locked());
        assert_eq!(
            document.set_objects_locked([first, second], false).unwrap(),
            2
        );
        assert_eq!(document.object(first).unwrap().id(), first);
        assert_eq!(document.group(group).unwrap().members().len(), 2);
    }

    #[test]
    fn object_mode_swaps_match_rhino_layer_scope_and_are_reversible() {
        let mut document = Document::default();
        let default = document.current_layer_id();
        let hidden_layer = document
            .add_layer("Hidden", ColorRgb::new(1, 2, 3))
            .unwrap();
        let locked_layer = document
            .add_layer("Locked", ColorRgb::new(4, 5, 6))
            .unwrap();
        let mut ids = Vec::new();
        for (layer, x) in [(default, 0.0), (hidden_layer, 10.0), (locked_layer, 20.0)] {
            for (offset, attributes) in [
                (0.0, ObjectAttributes::on_layer(layer)),
                (
                    1.0,
                    ObjectAttributes::on_layer(layer).with_visibility(false),
                ),
                (2.0, ObjectAttributes::on_layer(layer).with_locked(true)),
            ] {
                ids.push(
                    document
                        .add_geometry_with_attributes(
                            Geometry::Point(Point3::try_new(x + offset, 0.0, 0.0).unwrap()),
                            attributes,
                        )
                        .unwrap(),
                );
            }
        }
        document.set_layer_visibility(hidden_layer, false).unwrap();
        document.set_layer_locked(locked_layer, true).unwrap();
        let modes = |document: &Document| {
            ids.iter()
                .map(|id| {
                    let attributes = document.object(*id).unwrap().attributes();
                    if !attributes.is_visible() {
                        "hidden"
                    } else if attributes.is_locked() {
                        "locked"
                    } else {
                        "normal"
                    }
                })
                .collect::<Vec<_>>()
        };
        let initial = vec![
            "normal", "hidden", "locked", "normal", "hidden", "locked", "normal", "hidden",
            "locked",
        ];
        assert_eq!(modes(&document), initial);

        document
            .select_object(ids[0], SelectionMode::Replace)
            .unwrap();
        assert_eq!(document.swap_object_visibility_modes().unwrap(), 2);
        assert_eq!(document.undo_label(), Some("Swap object visibility"));
        assert_eq!(document.selected_object_count(), 0);
        assert_eq!(
            modes(&document),
            vec![
                "hidden", "normal", "locked", "normal", "hidden", "locked", "normal", "hidden",
                "locked",
            ]
        );
        document.undo().unwrap();
        assert_eq!(modes(&document), initial);
        document.redo().unwrap();
        assert_eq!(document.swap_object_visibility_modes().unwrap(), 2);
        assert_eq!(modes(&document), initial);

        document
            .select_object(ids[0], SelectionMode::Replace)
            .unwrap();
        assert_eq!(document.swap_object_lock_modes().unwrap(), 2);
        assert_eq!(document.undo_label(), Some("Swap object lock"));
        assert_eq!(document.selected_object_count(), 0);
        assert_eq!(
            modes(&document),
            vec![
                "locked", "hidden", "normal", "normal", "hidden", "locked", "normal", "hidden",
                "locked",
            ]
        );
        document.undo().unwrap();
        assert_eq!(modes(&document), initial);
        document.redo().unwrap();
        assert_eq!(document.swap_object_lock_modes().unwrap(), 2);
        assert_eq!(modes(&document), initial);
        assert!(!document.layer(hidden_layer).unwrap().is_visible());
        assert!(document.layer(locked_layer).unwrap().is_locked());
        assert!(
            ids.iter()
                .enumerate()
                .all(|(index, id)| document.object(*id).unwrap().id() == ids[index])
        );
    }

    #[test]
    fn object_isolation_tracks_only_rhino_changed_modes_and_is_reversible() {
        let mut document = Document::default();
        let default = document.current_layer_id();
        let hidden_layer = document
            .add_layer("Isolation Hidden", ColorRgb::new(1, 2, 3))
            .unwrap();
        let locked_layer = document
            .add_layer("Isolation Locked", ColorRgb::new(4, 5, 6))
            .unwrap();
        let mut ids = Vec::new();
        for (offset, attributes) in [
            (0.0, ObjectAttributes::on_layer(default)),
            (1.0, ObjectAttributes::on_layer(default)),
            (
                2.0,
                ObjectAttributes::on_layer(default).with_visibility(false),
            ),
            (3.0, ObjectAttributes::on_layer(default).with_locked(true)),
            (10.0, ObjectAttributes::on_layer(hidden_layer)),
            (
                11.0,
                ObjectAttributes::on_layer(hidden_layer).with_visibility(false),
            ),
            (
                12.0,
                ObjectAttributes::on_layer(hidden_layer).with_locked(true),
            ),
            (20.0, ObjectAttributes::on_layer(locked_layer)),
            (
                21.0,
                ObjectAttributes::on_layer(locked_layer).with_visibility(false),
            ),
            (
                22.0,
                ObjectAttributes::on_layer(locked_layer).with_locked(true),
            ),
        ] {
            ids.push(
                document
                    .add_geometry_with_attributes(
                        Geometry::Point(Point3::try_new(offset, 0.0, 0.0).unwrap()),
                        attributes,
                    )
                    .unwrap(),
            );
        }
        document.set_layer_visibility(hidden_layer, false).unwrap();
        document.set_layer_locked(locked_layer, true).unwrap();
        document
            .select_object(ids[0], SelectionMode::Replace)
            .unwrap();
        let modes = |document: &Document| {
            ids.iter()
                .map(|id| {
                    let attributes = document.object(*id).unwrap().attributes();
                    if !attributes.is_visible() {
                        "hidden"
                    } else if attributes.is_locked() {
                        "locked"
                    } else {
                        "normal"
                    }
                })
                .collect::<Vec<_>>()
        };
        let initial = vec![
            "normal", "normal", "hidden", "locked", "normal", "hidden", "locked", "normal",
            "hidden", "locked",
        ];
        assert_eq!(modes(&document), initial);

        assert_eq!(document.isolate_selected_objects().unwrap(), 1);
        assert_eq!(document.undo_label(), Some("Isolate objects"));
        assert_eq!(document.isolated_hidden_object_count(), 1);
        assert!(document.is_selected(ids[0]));
        assert_eq!(
            modes(&document),
            vec![
                "normal", "hidden", "hidden", "locked", "normal", "hidden", "locked", "normal",
                "hidden", "locked",
            ]
        );
        document.undo().unwrap();
        assert_eq!(modes(&document), initial);
        assert_eq!(document.isolated_hidden_object_count(), 0);
        document.redo().unwrap();
        assert_eq!(document.isolated_hidden_object_count(), 1);
        assert_eq!(document.unisolate_objects().unwrap(), 1);
        assert_eq!(modes(&document), initial);
        assert_eq!(document.isolated_hidden_object_count(), 0);
        document.undo().unwrap();
        assert_eq!(document.isolated_hidden_object_count(), 1);
        document.redo().unwrap();
        assert_eq!(modes(&document), initial);

        assert_eq!(document.isolate_selected_objects().unwrap(), 1);
        assert_eq!(document.isolate_selected_objects().unwrap(), 0);
        let hidden_ids = document
            .objects()
            .filter(|object| !object.attributes().is_visible())
            .map(|object| object.id())
            .collect::<Vec<_>>();
        assert_eq!(
            document.set_objects_visibility(hidden_ids, true).unwrap(),
            4
        );
        assert_eq!(document.isolated_hidden_object_count(), 0);
        assert_eq!(document.unisolate_objects().unwrap(), 0);
        document
            .set_objects_visibility([ids[2], ids[5], ids[8]], false)
            .unwrap();
        assert_eq!(modes(&document), initial);

        assert_eq!(document.isolate_lock_selected_objects().unwrap(), 1);
        assert_eq!(document.undo_label(), Some("Isolate-lock objects"));
        assert_eq!(document.isolated_locked_object_count(), 1);
        assert!(document.is_selected(ids[0]));
        assert_eq!(
            modes(&document),
            vec![
                "normal", "locked", "hidden", "locked", "normal", "hidden", "locked", "normal",
                "hidden", "locked",
            ]
        );
        document.undo().unwrap();
        assert_eq!(modes(&document), initial);
        assert_eq!(document.isolated_locked_object_count(), 0);
        document.redo().unwrap();
        assert_eq!(document.isolated_locked_object_count(), 1);
        assert_eq!(document.unisolate_locked_objects().unwrap(), 1);
        assert_eq!(modes(&document), initial);
        assert_eq!(document.isolated_locked_object_count(), 0);

        assert_eq!(document.isolate_lock_selected_objects().unwrap(), 1);
        assert_eq!(document.isolate_lock_selected_objects().unwrap(), 0);
        let locked_ids = document
            .objects()
            .filter(|object| object.attributes().is_locked())
            .map(|object| object.id())
            .collect::<Vec<_>>();
        assert_eq!(document.set_objects_locked(locked_ids, false).unwrap(), 4);
        assert_eq!(document.isolated_locked_object_count(), 0);
        assert_eq!(document.unisolate_locked_objects().unwrap(), 0);
        assert!(!document.layer(hidden_layer).unwrap().is_visible());
        assert!(document.layer(locked_layer).unwrap().is_locked());
        assert!(
            ids.iter()
                .enumerate()
                .all(|(index, id)| document.object(*id).unwrap().id() == ids[index])
        );
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
    fn geometry_replacement_retains_identity_groups_selection_and_history() {
        let mut document = Document::default();
        let line = LineSegment::try_new(
            Point3::try_new(1.0, 2.0, 3.0).unwrap(),
            Point3::try_new(7.0, 5.0, 3.0).unwrap(),
            document.tolerance(),
        )
        .unwrap();
        let id = document.add_geometry(Geometry::Line(line)).unwrap();
        let group = document
            .add_group(Some("Direction".to_owned()), [id])
            .unwrap();
        document.select_object(id, SelectionMode::Replace).unwrap();

        assert_eq!(
            document
                .replace_object_geometries([(id, Geometry::Line(line.reversed()))])
                .unwrap(),
            1
        );
        assert_eq!(document.undo_label(), Some("Replace object geometry"));
        assert!(document.is_selected(id));
        assert_eq!(document.group(group).unwrap().members().next(), Some(id));
        assert!(matches!(
            document.object(id).unwrap().geometry(),
            Geometry::Line(reversed) if reversed.start() == line.end() && reversed.end() == line.start()
        ));

        document.undo().unwrap();
        assert!(matches!(
            document.object(id).unwrap().geometry(),
            Geometry::Line(restored) if *restored == line
        ));
        assert!(document.is_selected(id));
        document.redo().unwrap();
        assert!(matches!(
            document.object(id).unwrap().geometry(),
            Geometry::Line(reversed) if *reversed == line.reversed()
        ));

        let missing = ObjectId::new();
        let before = document.object(id).unwrap().geometry().clone();
        assert_eq!(
            document.replace_object_geometries([
                (id, Geometry::Line(line)),
                (
                    missing,
                    Geometry::Point(Point3::try_new(0.0, 0.0, 0.0).unwrap()),
                ),
            ]),
            Err(DocumentError::ObjectNotFound(missing))
        );
        assert_eq!(document.object(id).unwrap().geometry(), &before);
    }

    #[test]
    fn circular_geometry_preserves_analytics_or_promotes_exactly() {
        let mut document = Document::default();
        let tolerance = document.tolerance();
        let normal = viboceros_geometry::UnitVector3::try_new(0.0, 0.0, 1.0, tolerance).unwrap();
        let circle = Circle3::try_new(
            Point3::try_new(1.0, 2.0, 0.0).unwrap(),
            3.0,
            normal,
            tolerance,
        )
        .unwrap();
        let id = document.add_geometry(Geometry::Circle(circle)).unwrap();

        document
            .transform_objects(
                [id],
                AffineTransform3::from_translation(
                    viboceros_geometry::Vector3::try_new(4.0, -1.0, 2.0).unwrap(),
                ),
            )
            .unwrap();
        assert!(matches!(
            document.object(id).unwrap().geometry(),
            Geometry::Circle(circle)
                if circle.center() == Point3::try_new(5.0, 1.0, 2.0).unwrap()
                    && circle.radius() == 3.0
        ));

        document.undo().unwrap();
        let shear = AffineTransform3::try_new(
            [[1.0, 0.5, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            viboceros_geometry::Vector3::try_new(0.0, 0.0, 0.0).unwrap(),
        )
        .unwrap();
        document.transform_objects([id], shear).unwrap();
        let Geometry::NurbsCurve(ellipse) = document.object(id).unwrap().geometry() else {
            panic!("a sheared circle must be promoted to an exact NURBS curve")
        };
        assert_eq!(ellipse.degree(), 2);
        assert_eq!(ellipse.control_points().len(), 9);
        let start = ellipse.evaluate(0.0).unwrap();
        assert!(
            start.is_near(
                shear
                    .transform_point(circle.point_at_angle(0.0).unwrap())
                    .unwrap(),
                tolerance
            )
        );

        document.undo().unwrap();
        assert!(matches!(
            document.object(id).unwrap().geometry(),
            Geometry::Circle(restored) if *restored == circle
        ));
    }

    #[test]
    fn ellipse_preserves_orthogonal_images_and_promotes_shear_exactly() {
        let mut document = Document::default();
        let tolerance = document.tolerance();
        let x_axis = viboceros_geometry::UnitVector3::try_new(1.0, 0.0, 0.0, tolerance).unwrap();
        let y_axis = viboceros_geometry::UnitVector3::try_new(0.0, 1.0, 0.0, tolerance).unwrap();
        let ellipse = Ellipse3::try_new(
            Point3::try_new(1.0, 2.0, 0.0).unwrap(),
            4.0,
            2.0,
            x_axis,
            y_axis,
            tolerance,
        )
        .unwrap();
        let id = document.add_geometry(Geometry::Ellipse(ellipse)).unwrap();
        let orthogonal = AffineTransform3::try_new(
            [[2.0, 0.0, 0.0], [0.0, 3.0, 0.0], [0.0, 0.0, 1.0]],
            viboceros_geometry::Vector3::try_new(5.0, 7.0, 0.0).unwrap(),
        )
        .unwrap();
        document.transform_objects([id], orthogonal).unwrap();
        assert!(matches!(
            document.object(id).unwrap().geometry(),
            Geometry::Ellipse(transformed)
                if transformed.center() == Point3::try_new(7.0, 13.0, 0.0).unwrap()
                    && transformed.radius_x() == 8.0
                    && transformed.radius_y() == 6.0
        ));

        document.undo().unwrap();
        let shear = AffineTransform3::try_new(
            [[1.0, 0.5, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            viboceros_geometry::Vector3::try_new(0.0, 0.0, 0.0).unwrap(),
        )
        .unwrap();
        document.transform_objects([id], shear).unwrap();
        let Geometry::NurbsCurve(curve) = document.object(id).unwrap().geometry() else {
            panic!("a sheared ellipse must be promoted to an exact NURBS curve")
        };
        assert_eq!(curve.degree(), 2);
        assert_eq!(curve.control_points().len(), 9);
        assert!(
            curve.evaluate(0.0).unwrap().is_near(
                shear
                    .transform_point(ellipse.point_at_angle(0.0).unwrap())
                    .unwrap(),
                tolerance
            )
        );
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
