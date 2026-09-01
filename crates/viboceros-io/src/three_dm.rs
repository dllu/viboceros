use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{CStr, CString, c_char, c_double, c_int};
use std::path::Path;
use std::ptr::NonNull;
use std::slice;

use thiserror::Error;
use viboceros_geometry::{
    GeometryError, LineSegment, NurbsCurve, NurbsSurface, Point3, PointCloud3, Tolerance,
    TriangleMesh, WeightedPoint3,
};

const ERROR_CAPACITY: usize = 4096;
const OBJECT_POINT: c_int = 1;
const OBJECT_LINE: c_int = 2;
const OBJECT_NURBS_CURVE: c_int = 3;
const OBJECT_TRIANGLE_MESH: c_int = 4;
const OBJECT_NURBS_SURFACE: c_int = 5;
const OBJECT_POINT_CLOUD: c_int = 6;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThreeDmLayer {
    pub name: String,
    pub color: [u8; 3],
    pub visible: bool,
    pub locked: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThreeDmGroup {
    pub name: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ThreeDmColorSource {
    Layer = 0,
    Object = 1,
    Material = 2,
    Parent = 3,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ThreeDmGeometry {
    Point(Point3),
    PointCloud(PointCloud3),
    Line(LineSegment),
    NurbsCurve(NurbsCurve),
    NurbsSurface(NurbsSurface),
    Mesh(TriangleMesh),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ThreeDmObject {
    pub geometry: ThreeDmGeometry,
    pub layer_index: usize,
    pub name: Option<String>,
    pub visible: bool,
    pub locked: bool,
    pub object_color: [u8; 3],
    pub color_source: ThreeDmColorSource,
    /// Indices into [`ThreeDmModel::groups`] in source-attribute order.
    pub group_indices: Vec<usize>,
}

impl ThreeDmObject {
    pub fn new(geometry: ThreeDmGeometry, layer_index: usize) -> Self {
        Self {
            geometry,
            layer_index,
            name: None,
            visible: true,
            locked: false,
            object_color: [0, 0, 0],
            color_source: ThreeDmColorSource::Layer,
            group_indices: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ThreeDmModel {
    pub layers: Vec<ThreeDmLayer>,
    pub groups: Vec<ThreeDmGroup>,
    pub objects: Vec<ThreeDmObject>,
    unsupported_object_count: usize,
}

impl ThreeDmModel {
    pub fn new(
        layers: Vec<ThreeDmLayer>,
        groups: Vec<ThreeDmGroup>,
        objects: Vec<ThreeDmObject>,
    ) -> Self {
        Self {
            layers,
            groups,
            objects,
            unsupported_object_count: 0,
        }
    }

    pub const fn unsupported_object_count(&self) -> usize {
        self.unsupported_object_count
    }
}

#[derive(Debug, Error)]
pub enum ThreeDmError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Geometry(#[from] GeometryError),

    #[error("3DM path is not valid UTF-8 or contains a nul byte: {0}")]
    InvalidPath(String),

    #[error("3DM text contains an interior nul byte in {field}")]
    InteriorNul { field: &'static str },

    #[error("invalid 3DM model: {0}")]
    InvalidModel(String),

    #[error("malformed data returned by the OpenNURBS bridge: {0}")]
    MalformedBridge(&'static str),

    #[error("OpenNURBS error: {0}")]
    Native(String),
}

pub fn read_3dm_file(
    path: impl AsRef<Path>,
    tolerance: Tolerance,
) -> Result<ThreeDmModel, ThreeDmError> {
    let path = path_to_c_string(path.as_ref())?;
    let mut error = [0 as c_char; ERROR_CAPACITY];
    let mut pointer = std::ptr::null_mut();
    // SAFETY: `path` and `error` are valid terminated buffers and `pointer`
    // points to writable storage. The bridge catches all C++ exceptions.
    let success =
        unsafe { ffi::vibo_3dm_read(path.as_ptr(), &mut pointer, error.as_mut_ptr(), error.len()) };
    if success == 0 {
        return Err(native_error(&error));
    }
    let handle = ModelHandle(
        NonNull::new(pointer).ok_or(ThreeDmError::MalformedBridge("read returned a null model"))?,
    );
    decode_model(&handle, tolerance)
}

pub fn write_3dm_file(path: impl AsRef<Path>, model: &ThreeDmModel) -> Result<(), ThreeDmError> {
    validate_model(model)?;
    let destination = path.as_ref();
    let parent = destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let staged = tempfile::Builder::new()
        .prefix(".viboceros-")
        .suffix(".3dm.tmp")
        .tempfile_in(parent)?;
    let native_path = path_to_c_string(staged.path())?;
    let layer_names = model
        .layers
        .iter()
        .map(|layer| c_string(&layer.name, "layer name"))
        .collect::<Result<Vec<_>, _>>()?;
    let layers = model
        .layers
        .iter()
        .zip(&layer_names)
        .map(|(layer, name)| ffi::ViboWriteLayer {
            name: name.as_ptr(),
            red: layer.color[0],
            green: layer.color[1],
            blue: layer.color[2],
            visible: u8::from(layer.visible),
            locked: u8::from(layer.locked),
        })
        .collect::<Vec<_>>();

    let group_names = model
        .groups
        .iter()
        .map(|group| c_string(&group.name, "group name"))
        .collect::<Result<Vec<_>, _>>()?;
    let groups = group_names
        .iter()
        .map(|name| ffi::ViboWriteGroup {
            name: name.as_ptr(),
        })
        .collect::<Vec<_>>();

    let object_names = model
        .objects
        .iter()
        .map(|object| {
            object
                .name
                .as_deref()
                .map(|name| c_string(name, "object name"))
                .transpose()
        })
        .collect::<Result<Vec<_>, _>>()?;
    let payloads = model
        .objects
        .iter()
        .map(ObjectPayload::from_object)
        .collect::<Vec<_>>();
    let objects = model
        .objects
        .iter()
        .zip(&object_names)
        .zip(&payloads)
        .map(|((object, name), payload)| ffi::ViboWriteObject {
            object_type: payload.object_type,
            layer_index: object.layer_index,
            name: name.as_ref().map_or(std::ptr::null(), |name| name.as_ptr()),
            visible: u8::from(object.visible),
            locked: u8::from(object.locked),
            color_source: object.color_source as u8,
            color_red: object.object_color[0],
            color_green: object.object_color[1],
            color_blue: object.object_color[2],
            degree_u: payload.degree_u,
            degree_v: payload.degree_v,
            control_point_count_u: payload.control_point_count_u,
            control_point_count_v: payload.control_point_count_v,
            coordinates: pointer_or_null(&payload.coordinates),
            coordinate_count: payload.coordinates.len(),
            knots_u: pointer_or_null(&payload.knots_u),
            knot_u_count: payload.knots_u.len(),
            knots_v: pointer_or_null(&payload.knots_v),
            knot_v_count: payload.knots_v.len(),
            indices: pointer_or_null(&payload.indices),
            index_count: payload.indices.len(),
            group_indices: pointer_or_null(&object.group_indices),
            group_index_count: object.group_indices.len(),
        })
        .collect::<Vec<_>>();

    let mut error = [0 as c_char; ERROR_CAPACITY];
    // SAFETY: all pointers reference immutable vectors and C strings retained
    // for the duration of this synchronous call. The bridge catches exceptions.
    let success = unsafe {
        ffi::vibo_3dm_write(
            native_path.as_ptr(),
            pointer_or_null(&layers),
            layers.len(),
            pointer_or_null(&groups),
            groups.len(),
            pointer_or_null(&objects),
            objects.len(),
            error.as_mut_ptr(),
            error.len(),
        )
    };
    if success == 0 {
        Err(native_error(&error))
    } else {
        staged.as_file().sync_all()?;
        staged
            .persist(destination)
            .map_err(|error| ThreeDmError::Io(error.error))?;
        Ok(())
    }
}

fn decode_model(handle: &ModelHandle, tolerance: Tolerance) -> Result<ThreeDmModel, ThreeDmError> {
    // SAFETY: the handle owns a live bridge model.
    let layer_count = unsafe { ffi::vibo_3dm_layer_count(handle.0.as_ptr()) };
    let mut layers = Vec::with_capacity(layer_count.max(1));
    let mut layer_positions = BTreeMap::new();
    for index in 0..layer_count {
        let mut source_index = 0;
        let mut name = std::ptr::null();
        let (mut red, mut green, mut blue, mut visible, mut locked) = (0, 0, 0, 0, 0);
        // SAFETY: all output pointers are valid and the index is in range.
        let success = unsafe {
            ffi::vibo_3dm_layer(
                handle.0.as_ptr(),
                index,
                &mut source_index,
                &mut name,
                &mut red,
                &mut green,
                &mut blue,
                &mut visible,
                &mut locked,
            )
        };
        if success == 0 || name.is_null() {
            return Err(ThreeDmError::MalformedBridge("invalid layer record"));
        }
        if layer_positions.insert(source_index, layers.len()).is_some() {
            return Err(ThreeDmError::MalformedBridge("duplicate layer index"));
        }
        layers.push(ThreeDmLayer {
            name: c_text(name)?,
            color: [red, green, blue],
            visible: visible != 0,
            locked: locked != 0,
        });
    }
    if layers.is_empty() {
        layer_positions.insert(0, 0);
        layers.push(ThreeDmLayer {
            name: "Default".to_owned(),
            color: [0, 0, 0],
            visible: true,
            locked: false,
        });
    }

    // SAFETY: the handle owns a live bridge model.
    let group_count = unsafe { ffi::vibo_3dm_group_count(handle.0.as_ptr()) };
    let mut groups = Vec::with_capacity(group_count);
    let mut group_positions = BTreeMap::new();
    for index in 0..group_count {
        let mut source_index = 0;
        let mut name = std::ptr::null();
        // SAFETY: all output pointers are valid and the index is in range.
        let success =
            unsafe { ffi::vibo_3dm_group(handle.0.as_ptr(), index, &mut source_index, &mut name) };
        if success == 0 || name.is_null() {
            return Err(ThreeDmError::MalformedBridge("invalid group record"));
        }
        if group_positions.insert(source_index, groups.len()).is_some() {
            return Err(ThreeDmError::MalformedBridge("duplicate group index"));
        }
        groups.push(ThreeDmGroup {
            name: c_text(name)?,
        });
    }

    // SAFETY: the handle owns a live bridge model.
    let object_count = unsafe { ffi::vibo_3dm_object_count(handle.0.as_ptr()) };
    let mut objects = Vec::with_capacity(object_count);
    // SAFETY: the handle owns a live bridge model.
    let mut unsupported = unsafe { ffi::vibo_3dm_unsupported_object_count(handle.0.as_ptr()) };
    for index in 0..object_count {
        match decode_object(handle, index, &layer_positions, &group_positions, tolerance) {
            Ok(object) => objects.push(object),
            Err(ThreeDmError::Geometry(_)) => unsupported += 1,
            Err(error) => return Err(error),
        }
    }
    Ok(ThreeDmModel {
        layers,
        groups,
        objects,
        unsupported_object_count: unsupported,
    })
}

fn decode_object(
    handle: &ModelHandle,
    index: usize,
    layer_positions: &BTreeMap<i32, usize>,
    group_positions: &BTreeMap<i32, usize>,
    tolerance: Tolerance,
) -> Result<ThreeDmObject, ThreeDmError> {
    let mut info = ffi::ViboObjectInfo::default();
    let mut coordinates = std::ptr::null();
    let mut knots_u = std::ptr::null();
    let mut knots_v = std::ptr::null();
    let mut indices = std::ptr::null();
    let mut group_indices = std::ptr::null();
    // SAFETY: output pointers are valid, the model is live, and index is in range.
    let success = unsafe {
        ffi::vibo_3dm_object(
            handle.0.as_ptr(),
            index,
            &mut info,
            &mut coordinates,
            &mut knots_u,
            &mut knots_v,
            &mut indices,
            &mut group_indices,
        )
    };
    if success == 0 {
        return Err(ThreeDmError::MalformedBridge("invalid object record"));
    }
    let coordinates = ffi_slice(handle, coordinates, info.coordinate_count)?;
    let knots_u = ffi_slice(handle, knots_u, info.knot_u_count)?;
    let knots_v = ffi_slice(handle, knots_v, info.knot_v_count)?;
    let indices = ffi_slice(handle, indices, info.index_count)?;
    let group_indices = ffi_slice(handle, group_indices, info.group_index_count)?
        .iter()
        .map(|source_index| {
            group_positions.get(source_index).copied().ok_or_else(|| {
                ThreeDmError::InvalidModel(format!(
                    "object {index} references unknown group index {source_index}"
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let layer_index = layer_positions
        .get(&info.source_layer_index)
        .copied()
        // Legacy OpenNURBS archives use a negative unset index for geometry
        // that belongs on the first/default layer.
        .or_else(|| (info.source_layer_index < 0).then_some(0))
        .ok_or_else(|| {
            ThreeDmError::InvalidModel(format!(
                "object {index} references unknown layer index {}",
                info.source_layer_index
            ))
        })?;
    let name = if info.name.is_null() {
        None
    } else {
        let name = c_text(info.name)?;
        (!name.is_empty()).then_some(name)
    };
    let color_source = match info.color_source {
        0 => ThreeDmColorSource::Layer,
        1 => ThreeDmColorSource::Object,
        2 => ThreeDmColorSource::Material,
        3 => ThreeDmColorSource::Parent,
        _ => return Err(ThreeDmError::MalformedBridge("invalid object color source")),
    };

    let geometry = match info.object_type {
        OBJECT_POINT
            if coordinates.len() == 3
                && knots_u.is_empty()
                && knots_v.is_empty()
                && indices.is_empty() =>
        {
            ThreeDmGeometry::Point(point(coordinates)?)
        }
        OBJECT_LINE
            if coordinates.len() == 6
                && knots_u.is_empty()
                && knots_v.is_empty()
                && indices.is_empty() =>
        {
            ThreeDmGeometry::Line(LineSegment::try_new(
                point(&coordinates[..3])?,
                point(&coordinates[3..])?,
                tolerance,
            )?)
        }
        OBJECT_POINT_CLOUD
            if info.degree_u == 0
                && info.degree_v == 0
                && info.control_point_count_u == 0
                && info.control_point_count_v == 0
                && !coordinates.is_empty()
                && coordinates.len() % 3 == 0
                && knots_u.is_empty()
                && knots_v.is_empty()
                && indices.is_empty() =>
        {
            ThreeDmGeometry::PointCloud(PointCloud3::try_new(
                coordinates
                    .chunks_exact(3)
                    .map(point)
                    .collect::<Result<Vec<_>, _>>()?,
            )?)
        }
        OBJECT_NURBS_CURVE
            if info.degree_u > 0
                && info.degree_v == 0
                && info.control_point_count_v == 0
                && info.control_point_count_u
                    > usize::try_from(info.degree_u).unwrap_or(usize::MAX)
                && coordinates.len() == info.control_point_count_u.saturating_mul(4)
                && knots_u.len()
                    == info
                        .control_point_count_u
                        .saturating_add(info.degree_u as usize)
                        .saturating_add(1)
                && knots_v.is_empty()
                && indices.is_empty() =>
        {
            let control_points = coordinates
                .chunks_exact(4)
                .map(|value| WeightedPoint3::try_new(point(value)?, value[3]))
                .collect::<Result<Vec<_>, GeometryError>>()?;
            ThreeDmGeometry::NurbsCurve(NurbsCurve::try_new_rational(
                info.degree_u as usize,
                control_points,
                knots_u.to_vec(),
            )?)
        }
        OBJECT_NURBS_SURFACE
            if info.degree_u > 0
                && info.degree_v > 0
                && info.control_point_count_u
                    > usize::try_from(info.degree_u).unwrap_or(usize::MAX)
                && info.control_point_count_v
                    > usize::try_from(info.degree_v).unwrap_or(usize::MAX)
                && coordinates.len()
                    == info
                        .control_point_count_u
                        .saturating_mul(info.control_point_count_v)
                        .saturating_mul(4)
                && knots_u.len()
                    == info
                        .control_point_count_u
                        .saturating_add(info.degree_u as usize)
                        .saturating_add(1)
                && knots_v.len()
                    == info
                        .control_point_count_v
                        .saturating_add(info.degree_v as usize)
                        .saturating_add(1)
                && indices.is_empty() =>
        {
            let control_points = coordinates
                .chunks_exact(4)
                .map(|value| WeightedPoint3::try_new(point(value)?, value[3]))
                .collect::<Result<Vec<_>, GeometryError>>()?;
            ThreeDmGeometry::NurbsSurface(NurbsSurface::try_new_rational(
                info.degree_u as usize,
                info.degree_v as usize,
                info.control_point_count_u,
                info.control_point_count_v,
                control_points,
                knots_u.to_vec(),
                knots_v.to_vec(),
            )?)
        }
        OBJECT_TRIANGLE_MESH
            if !coordinates.is_empty()
                && coordinates.len() % 3 == 0
                && !indices.is_empty()
                && indices.len() % 3 == 0
                && knots_u.is_empty()
                && knots_v.is_empty() =>
        {
            let vertices = coordinates
                .chunks_exact(3)
                .map(point)
                .collect::<Result<Vec<_>, _>>()?;
            let triangles = indices
                .chunks_exact(3)
                .map(|face| [face[0], face[1], face[2]])
                .collect();
            ThreeDmGeometry::Mesh(TriangleMesh::try_new(vertices, triangles, tolerance)?)
        }
        _ => return Err(ThreeDmError::MalformedBridge("inconsistent object payload")),
    };
    Ok(ThreeDmObject {
        geometry,
        layer_index,
        name,
        visible: info.visible != 0,
        locked: info.locked != 0,
        object_color: [info.color_red, info.color_green, info.color_blue],
        color_source,
        group_indices,
    })
}

fn validate_model(model: &ThreeDmModel) -> Result<(), ThreeDmError> {
    if model.layers.is_empty() && !model.objects.is_empty() {
        return Err(ThreeDmError::InvalidModel(
            "objects require at least one layer".to_owned(),
        ));
    }
    for (index, layer) in model.layers.iter().enumerate() {
        if layer.name.trim().is_empty() {
            return Err(ThreeDmError::InvalidModel(format!(
                "layer {index} has an empty name"
            )));
        }
    }
    let mut group_names = BTreeSet::new();
    for (index, group) in model.groups.iter().enumerate() {
        let name = group.name.trim();
        if name.is_empty() {
            return Err(ThreeDmError::InvalidModel(format!(
                "group {index} has an empty name"
            )));
        }
        if !group_names.insert(name) {
            return Err(ThreeDmError::InvalidModel(format!(
                "group {index} duplicates the name '{name}'"
            )));
        }
    }
    for (index, object) in model.objects.iter().enumerate() {
        if object.layer_index >= model.layers.len() {
            return Err(ThreeDmError::InvalidModel(format!(
                "object {index} references missing layer {}",
                object.layer_index
            )));
        }
        let mut memberships = BTreeSet::new();
        for group_index in &object.group_indices {
            if *group_index >= model.groups.len() {
                return Err(ThreeDmError::InvalidModel(format!(
                    "object {index} references missing group {group_index}"
                )));
            }
            if !memberships.insert(*group_index) {
                return Err(ThreeDmError::InvalidModel(format!(
                    "object {index} repeats group {group_index}"
                )));
            }
        }
    }
    Ok(())
}

struct ObjectPayload {
    object_type: c_int,
    degree_u: u32,
    degree_v: u32,
    control_point_count_u: usize,
    control_point_count_v: usize,
    coordinates: Vec<c_double>,
    knots_u: Vec<c_double>,
    knots_v: Vec<c_double>,
    indices: Vec<u32>,
}

impl ObjectPayload {
    fn from_object(object: &ThreeDmObject) -> Self {
        match &object.geometry {
            ThreeDmGeometry::Point(point) => Self {
                object_type: OBJECT_POINT,
                degree_u: 0,
                degree_v: 0,
                control_point_count_u: 0,
                control_point_count_v: 0,
                coordinates: point.to_array().to_vec(),
                knots_u: Vec::new(),
                knots_v: Vec::new(),
                indices: Vec::new(),
            },
            ThreeDmGeometry::Line(line) => Self {
                object_type: OBJECT_LINE,
                degree_u: 0,
                degree_v: 0,
                control_point_count_u: 0,
                control_point_count_v: 0,
                coordinates: line
                    .start()
                    .to_array()
                    .into_iter()
                    .chain(line.end().to_array())
                    .collect(),
                knots_u: Vec::new(),
                knots_v: Vec::new(),
                indices: Vec::new(),
            },
            ThreeDmGeometry::PointCloud(cloud) => Self {
                object_type: OBJECT_POINT_CLOUD,
                degree_u: 0,
                degree_v: 0,
                control_point_count_u: 0,
                control_point_count_v: 0,
                coordinates: cloud
                    .points()
                    .iter()
                    .flat_map(|point| point.to_array())
                    .collect(),
                knots_u: Vec::new(),
                knots_v: Vec::new(),
                indices: Vec::new(),
            },
            ThreeDmGeometry::NurbsCurve(curve) => Self {
                object_type: OBJECT_NURBS_CURVE,
                degree_u: curve.degree() as u32,
                degree_v: 0,
                control_point_count_u: curve.control_points().len(),
                control_point_count_v: 0,
                coordinates: curve
                    .control_points()
                    .iter()
                    .flat_map(|control| {
                        control
                            .point()
                            .to_array()
                            .into_iter()
                            .chain([control.weight()])
                    })
                    .collect(),
                knots_u: curve.knots().to_vec(),
                knots_v: Vec::new(),
                indices: Vec::new(),
            },
            ThreeDmGeometry::NurbsSurface(surface) => Self {
                object_type: OBJECT_NURBS_SURFACE,
                degree_u: surface.degree_u() as u32,
                degree_v: surface.degree_v() as u32,
                control_point_count_u: surface.control_point_count_u(),
                control_point_count_v: surface.control_point_count_v(),
                coordinates: surface
                    .control_points()
                    .iter()
                    .flat_map(|control| {
                        control
                            .point()
                            .to_array()
                            .into_iter()
                            .chain([control.weight()])
                    })
                    .collect(),
                knots_u: surface.knots_u().to_vec(),
                knots_v: surface.knots_v().to_vec(),
                indices: Vec::new(),
            },
            ThreeDmGeometry::Mesh(mesh) => Self {
                object_type: OBJECT_TRIANGLE_MESH,
                degree_u: 0,
                degree_v: 0,
                control_point_count_u: 0,
                control_point_count_v: 0,
                coordinates: mesh
                    .vertices()
                    .iter()
                    .flat_map(|point| point.to_array())
                    .collect(),
                knots_u: Vec::new(),
                knots_v: Vec::new(),
                indices: mesh.triangles().iter().flatten().copied().collect(),
            },
        }
    }
}

struct ModelHandle(NonNull<ffi::ViboThreeDmModel>);

impl Drop for ModelHandle {
    fn drop(&mut self) {
        // SAFETY: this handle was returned by `vibo_3dm_read` and has not been freed.
        unsafe { ffi::vibo_3dm_free(self.0.as_ptr()) };
    }
}

fn path_to_c_string(path: &Path) -> Result<CString, ThreeDmError> {
    let text = path
        .to_str()
        .ok_or_else(|| ThreeDmError::InvalidPath(path.display().to_string()))?;
    CString::new(text).map_err(|_| ThreeDmError::InvalidPath(path.display().to_string()))
}

fn c_string(value: &str, field: &'static str) -> Result<CString, ThreeDmError> {
    CString::new(value).map_err(|_| ThreeDmError::InteriorNul { field })
}

fn c_text(pointer: *const c_char) -> Result<String, ThreeDmError> {
    // SAFETY: bridge string pointers are nul terminated and valid for the model lifetime.
    unsafe { CStr::from_ptr(pointer) }
        .to_str()
        .map(str::to_owned)
        .map_err(|_| ThreeDmError::MalformedBridge("text is not UTF-8"))
}

fn native_error(buffer: &[c_char]) -> ThreeDmError {
    // SAFETY: the bridge always nul terminates a nonempty error buffer.
    let message = unsafe { CStr::from_ptr(buffer.as_ptr()) }
        .to_string_lossy()
        .into_owned();
    ThreeDmError::Native(if message.is_empty() {
        "operation failed without a diagnostic".to_owned()
    } else {
        message
    })
}

fn point(values: &[c_double]) -> Result<Point3, GeometryError> {
    Point3::try_new(values[0], values[1], values[2])
}

fn pointer_or_null<T>(values: &[T]) -> *const T {
    if values.is_empty() {
        std::ptr::null()
    } else {
        values.as_ptr()
    }
}

fn ffi_slice<T>(
    _owner: &ModelHandle,
    pointer: *const T,
    length: usize,
) -> Result<&[T], ThreeDmError> {
    if length == 0 {
        return Ok(&[]);
    }
    if pointer.is_null() || length > isize::MAX as usize / size_of::<T>() {
        return Err(ThreeDmError::MalformedBridge("invalid array pointer"));
    }
    // SAFETY: the bridge guarantees arrays have `length` initialized elements
    // and remain valid while the owning model handle is alive.
    Ok(unsafe { slice::from_raw_parts(pointer, length) })
}

mod ffi {
    use super::{c_char, c_double, c_int};

    #[repr(C)]
    pub struct ViboThreeDmModel {
        _private: [u8; 0],
    }

    #[repr(C)]
    #[derive(Default)]
    pub struct ViboObjectInfo {
        pub object_type: c_int,
        pub source_layer_index: i32,
        pub name: *const c_char,
        pub visible: u8,
        pub locked: u8,
        pub color_source: u8,
        pub color_red: u8,
        pub color_green: u8,
        pub color_blue: u8,
        pub degree_u: u32,
        pub degree_v: u32,
        pub control_point_count_u: usize,
        pub control_point_count_v: usize,
        pub coordinate_count: usize,
        pub knot_u_count: usize,
        pub knot_v_count: usize,
        pub index_count: usize,
        pub group_index_count: usize,
    }

    #[repr(C)]
    pub struct ViboWriteLayer {
        pub name: *const c_char,
        pub red: u8,
        pub green: u8,
        pub blue: u8,
        pub visible: u8,
        pub locked: u8,
    }

    #[repr(C)]
    pub struct ViboWriteGroup {
        pub name: *const c_char,
    }

    #[repr(C)]
    pub struct ViboWriteObject {
        pub object_type: c_int,
        pub layer_index: usize,
        pub name: *const c_char,
        pub visible: u8,
        pub locked: u8,
        pub color_source: u8,
        pub color_red: u8,
        pub color_green: u8,
        pub color_blue: u8,
        pub degree_u: u32,
        pub degree_v: u32,
        pub control_point_count_u: usize,
        pub control_point_count_v: usize,
        pub coordinates: *const c_double,
        pub coordinate_count: usize,
        pub knots_u: *const c_double,
        pub knot_u_count: usize,
        pub knots_v: *const c_double,
        pub knot_v_count: usize,
        pub indices: *const u32,
        pub index_count: usize,
        pub group_indices: *const usize,
        pub group_index_count: usize,
    }

    unsafe extern "C" {
        pub fn vibo_3dm_read(
            path: *const c_char,
            output: *mut *mut ViboThreeDmModel,
            error: *mut c_char,
            error_capacity: usize,
        ) -> c_int;
        pub fn vibo_3dm_free(model: *mut ViboThreeDmModel);
        pub fn vibo_3dm_layer_count(model: *const ViboThreeDmModel) -> usize;
        pub fn vibo_3dm_layer(
            model: *const ViboThreeDmModel,
            index: usize,
            source_index: *mut i32,
            name: *mut *const c_char,
            red: *mut u8,
            green: *mut u8,
            blue: *mut u8,
            visible: *mut u8,
            locked: *mut u8,
        ) -> c_int;
        pub fn vibo_3dm_group_count(model: *const ViboThreeDmModel) -> usize;
        pub fn vibo_3dm_group(
            model: *const ViboThreeDmModel,
            index: usize,
            source_index: *mut i32,
            name: *mut *const c_char,
        ) -> c_int;
        pub fn vibo_3dm_object_count(model: *const ViboThreeDmModel) -> usize;
        pub fn vibo_3dm_unsupported_object_count(model: *const ViboThreeDmModel) -> usize;
        pub fn vibo_3dm_object(
            model: *const ViboThreeDmModel,
            index: usize,
            info: *mut ViboObjectInfo,
            coordinates: *mut *const c_double,
            knots_u: *mut *const c_double,
            knots_v: *mut *const c_double,
            indices: *mut *const u32,
            group_indices: *mut *const i32,
        ) -> c_int;
        pub fn vibo_3dm_write(
            path: *const c_char,
            layers: *const ViboWriteLayer,
            layer_count: usize,
            groups: *const ViboWriteGroup,
            group_count: usize,
            objects: *const ViboWriteObject,
            object_count: usize,
            error: *mut c_char,
            error_capacity: usize,
        ) -> c_int;
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn sample_model() -> ThreeDmModel {
        let point = Point3::try_new(1.0, 2.0, 3.0).unwrap();
        let line = LineSegment::try_new(
            Point3::try_new(-2.0, 0.0, 1.0).unwrap(),
            Point3::try_new(5.0, 4.0, -1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let curve = NurbsCurve::try_new_rational(
            2,
            vec![
                WeightedPoint3::try_new(Point3::try_new(0.0, 0.0, 0.0).unwrap(), 1.0).unwrap(),
                WeightedPoint3::try_new(Point3::try_new(2.0, 3.0, 0.0).unwrap(), 0.5).unwrap(),
                WeightedPoint3::try_new(Point3::try_new(4.0, 0.0, 0.0).unwrap(), 1.0).unwrap(),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let middle_weight = 0.5_f64.sqrt();
        let mut surface_controls = Vec::new();
        for z in [0.0, 3.0] {
            surface_controls.extend([
                WeightedPoint3::try_new(Point3::try_new(1.0, 0.0, z).unwrap(), 1.0).unwrap(),
                WeightedPoint3::try_new(Point3::try_new(1.0, 1.0, z).unwrap(), middle_weight)
                    .unwrap(),
                WeightedPoint3::try_new(Point3::try_new(0.0, 1.0, z).unwrap(), 1.0).unwrap(),
            ]);
        }
        let surface = NurbsSurface::try_new_rational(
            2,
            1,
            3,
            2,
            surface_controls,
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            vec![0.0, 0.0, 1.0, 1.0],
        )
        .unwrap();
        let mesh = TriangleMesh::try_new(
            vec![
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(1.0, 0.0, 0.0).unwrap(),
                Point3::try_new(0.0, 1.0, 0.0).unwrap(),
            ],
            vec![[0, 1, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let cloud = PointCloud3::try_new(vec![
            Point3::try_new(-4.0, 2.0, 7.0).unwrap(),
            Point3::try_new(8.0, -1.0, 3.0).unwrap(),
            Point3::try_new(-4.0, 2.0, 7.0).unwrap(),
        ])
        .unwrap();
        ThreeDmModel::new(
            vec![
                ThreeDmLayer {
                    name: "Default".to_owned(),
                    color: [0, 0, 0],
                    visible: true,
                    locked: false,
                },
                ThreeDmLayer {
                    name: "Reference 三".to_owned(),
                    color: [12, 34, 56],
                    visible: false,
                    locked: true,
                },
            ],
            vec![
                ThreeDmGroup {
                    name: "Assembly α".to_owned(),
                },
                ThreeDmGroup {
                    name: "Inspection".to_owned(),
                },
            ],
            vec![
                ThreeDmObject {
                    group_indices: vec![0],
                    object_color: [12, 34, 56],
                    color_source: ThreeDmColorSource::Object,
                    ..ThreeDmObject::new(ThreeDmGeometry::Point(point), 0)
                },
                ThreeDmObject {
                    group_indices: vec![0, 1],
                    object_color: [7, 8, 9],
                    ..ThreeDmObject::new(ThreeDmGeometry::PointCloud(cloud), 0)
                },
                ThreeDmObject {
                    geometry: ThreeDmGeometry::Line(line),
                    layer_index: 1,
                    name: Some("guide".to_owned()),
                    visible: true,
                    locked: true,
                    object_color: [90, 80, 70],
                    color_source: ThreeDmColorSource::Parent,
                    group_indices: vec![1],
                },
                ThreeDmObject {
                    object_color: [4, 5, 6],
                    color_source: ThreeDmColorSource::Material,
                    ..ThreeDmObject::new(ThreeDmGeometry::NurbsCurve(curve), 0)
                },
                ThreeDmObject::new(ThreeDmGeometry::NurbsSurface(surface), 0),
                ThreeDmObject::new(ThreeDmGeometry::Mesh(mesh), 0),
            ],
        )
    }

    #[test]
    fn open_nurbs_round_trip_preserves_geometry_layers_and_groups() {
        let path = temporary_path("roundtrip.3dm");
        let original = sample_model();
        fs::write(&path, b"previous valid file remains until replacement").unwrap();
        write_3dm_file(&path, &original).unwrap();
        let bytes = fs::read(&path).unwrap();
        assert!(bytes.starts_with(b"3D Geometry File Format"));

        let decoded = read_3dm_file(&path, Tolerance::DEFAULT).unwrap();
        assert_eq!(decoded.unsupported_object_count(), 0);
        assert_eq!(decoded.layers, original.layers);
        assert_eq!(decoded.groups, original.groups);
        assert_eq!(decoded.objects, original.objects);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn rejects_invalid_layer_and_group_references_before_calling_native_code() {
        let model = ThreeDmModel::new(
            Vec::new(),
            Vec::new(),
            vec![ThreeDmObject::new(
                ThreeDmGeometry::Point(Point3::try_new(0.0, 0.0, 0.0).unwrap()),
                0,
            )],
        );
        assert!(matches!(
            write_3dm_file(temporary_path("invalid.3dm"), &model),
            Err(ThreeDmError::InvalidModel(_))
        ));

        let layer = ThreeDmLayer {
            name: "Default".to_owned(),
            color: [0, 0, 0],
            visible: true,
            locked: false,
        };
        let group = ThreeDmGroup {
            name: "Assembly".to_owned(),
        };
        let invalid_membership = ThreeDmObject {
            group_indices: vec![1],
            ..ThreeDmObject::new(
                ThreeDmGeometry::Point(Point3::try_new(0.0, 0.0, 0.0).unwrap()),
                0,
            )
        };
        assert!(matches!(
            write_3dm_file(
                temporary_path("invalid-group.3dm"),
                &ThreeDmModel::new(
                    vec![layer.clone()],
                    vec![group.clone()],
                    vec![invalid_membership],
                ),
            ),
            Err(ThreeDmError::InvalidModel(_))
        ));

        let repeated_membership = ThreeDmObject {
            group_indices: vec![0, 0],
            ..ThreeDmObject::new(
                ThreeDmGeometry::Point(Point3::try_new(0.0, 0.0, 0.0).unwrap()),
                0,
            )
        };
        assert!(matches!(
            write_3dm_file(
                temporary_path("repeated-group.3dm"),
                &ThreeDmModel::new(vec![layer], vec![group], vec![repeated_membership],),
            ),
            Err(ThreeDmError::InvalidModel(_))
        ));
    }

    #[test]
    fn reports_non_3dm_input_as_a_native_error() {
        let path = temporary_path("not-a-model.3dm");
        fs::write(&path, b"not a 3dm file").unwrap();
        assert!(matches!(
            read_3dm_file(&path, Tolerance::DEFAULT),
            Err(ThreeDmError::Native(_))
        ));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn reads_official_old_archive_curve_surface_and_group_fixtures() {
        let root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/opennurbs/example_files");
        let curves = read_3dm_file(root.join("V2/v2_my_curves.3dm"), Tolerance::DEFAULT).unwrap();
        assert!(!curves.objects.is_empty());
        assert!(curves.objects.iter().all(|object| matches!(
            object.geometry,
            ThreeDmGeometry::Line(_) | ThreeDmGeometry::NurbsCurve(_)
        )));

        let surfaces =
            read_3dm_file(root.join("V7/v7_my_surfaces.3dm"), Tolerance::DEFAULT).unwrap();
        assert!(
            surfaces
                .objects
                .iter()
                .any(|object| matches!(object.geometry, ThreeDmGeometry::NurbsSurface(_)))
        );

        let points = read_3dm_file(root.join("V7/v7_my_points.3dm"), Tolerance::DEFAULT).unwrap();
        let group_index = points
            .groups
            .iter()
            .position(|group| group.name == "group of points")
            .unwrap();
        let grouped = points
            .objects
            .iter()
            .filter(|object| object.group_indices.contains(&group_index))
            .collect::<Vec<_>>();
        assert_eq!(grouped.len(), 2);
        assert!(
            grouped
                .iter()
                .any(|object| matches!(object.geometry, ThreeDmGeometry::Point(_)))
        );
        assert!(
            grouped
                .iter()
                .any(|object| matches!(object.geometry, ThreeDmGeometry::PointCloud(_)))
        );
    }

    fn temporary_path(suffix: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "viboceros-{}-{unique}-{suffix}",
            std::process::id()
        ))
    }
}
