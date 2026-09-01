//! In-memory CAD document model.

use std::collections::BTreeSet;
use std::fmt;

use thiserror::Error;
use uuid::Uuid;
use viboceros_geometry::{BoundingBox3, LineSegment, Point3, Tolerance};

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
}

impl Geometry {
    pub fn bounds(&self) -> BoundingBox3 {
        match self {
            Self::Point(point) => BoundingBox3::from_points([*point]).unwrap(),
            Self::Line(line) => BoundingBox3::from_points([line.start(), line.end()]).unwrap(),
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
        }
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
        self.layers.push(Layer {
            id,
            name: name.to_owned(),
            color,
            visible: true,
            locked: false,
        });
        Ok(id)
    }

    pub fn set_current_layer(&mut self, id: LayerId) -> Result<(), DocumentError> {
        let layer = self.layer(id).ok_or(DocumentError::LayerNotFound(id))?;
        if layer.locked {
            return Err(DocumentError::LayerLocked(id));
        }
        self.current_layer = id;
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
        self.objects.push(Object {
            id,
            geometry,
            attributes,
        });
        Ok(id)
    }

    pub fn delete_object(&mut self, id: ObjectId) -> Result<Object, DocumentError> {
        let index = self
            .objects
            .iter()
            .position(|object| object.id == id)
            .ok_or(DocumentError::ObjectNotFound(id))?;
        let object = self.objects.remove(index);
        for group in &mut self.groups {
            group.members.remove(&id);
        }
        self.groups.retain(|group| !group.members.is_empty());
        Ok(object)
    }

    pub fn clear_objects(&mut self) -> usize {
        let count = self.objects.len();
        self.objects.clear();
        self.groups.clear();
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
        self.groups.push(Group { id, name, members });
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
}
