use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;
mod export;
mod export_geometry;
mod export_plane;
pub use export::{write_step, write_step_file, write_step_file_in_units, write_step_in_units};
mod instance_plan;
mod native_instances;
mod native_planar;
pub use native_instances::{StepPlanarImport, StepPlanarInstance, read_step_planar_instances};
mod units;
pub use native_planar::{
    StepPlanarShell, read_step_planar_shells, read_step_planar_shells_in_units,
};

use monstertruck::core::cgmath64::{Matrix4, Transform};
use monstertruck::meshing::prelude::{
    BoundedCurve, MeshedShape, ParametricCurve, ParametricSurface, PolygonMesh, RobustMeshableShape,
};
use monstertruck::modeling::Point3 as TruckPoint3;
use monstertruck::step::load::convert::StepCompressedTrimmedShell;
use monstertruck::step::load::step_p21::{ast::Name, tables::PlaceHolder};
use monstertruck::step::load::{LoadError, LossCategory, ShellLoadReport, Table};
use monstertruck::topology::compress::CompressedTrimmedSolid;
use thiserror::Error;
use viboceros_geometry::{
    AffineTransform3, GeometryError, LengthUnitSystem, Point3, Tolerance, TriangleMesh, UnitError,
};

/// Relative chord tolerance used to create a display mesh from exact STEP
/// geometry. The absolute document tolerance and Monstertruck's numerical
/// floor are also respected.
const RELATIVE_MESH_TOLERANCE: f64 = 1.0e-3;

#[derive(Clone, Debug, PartialEq)]
pub struct StepObject {
    pub mesh: TriangleMesh,
    pub name: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StepImportReport {
    /// Entity records the parser recognized but could not retain.
    pub swallowed_entity_count: usize,
    /// Topological items omitted while resolving shells, solids, and shape
    /// relationships. Shell-conversion losses count once per source shape,
    /// independent of its assembly instance count.
    pub lost_topology_item_count: usize,
    /// Non-solid/non-shell items in shape representations, such as placement
    /// records or unsupported wireframe geometry.
    pub skipped_representation_item_count: usize,
    /// Supported shapes not reachable from the product assembly graph and
    /// therefore imported at their file-space coordinates.
    pub unplaced_shape_count: usize,
    /// Explanation when a file had usable B-rep data but no usable product
    /// assembly graph.
    pub assembly_warning: Option<String>,
    /// One-line topology reports from conversions that omitted something.
    pub topology_warnings: Vec<String>,
}

impl StepImportReport {
    pub fn warning_count(&self) -> usize {
        self.swallowed_entity_count
            + self.lost_topology_item_count
            + self.skipped_representation_item_count
            + self.unplaced_shape_count
            + usize::from(self.assembly_warning.is_some())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StepImport {
    pub objects: Vec<StepObject>,
    pub report: StepImportReport,
}

#[derive(Debug, Error)]
pub enum StepError {
    #[error("STEP shell #{shell} cannot be converted to a native planar B-rep: {reason}")]
    UnsupportedPlanarShell { shell: u64, reason: &'static str },
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Load(#[from] LoadError),

    #[error(transparent)]
    Geometry(#[from] GeometryError),

    #[error(transparent)]
    Units(#[from] UnitError),

    #[error("STEP export requires physical length units; the source is unitless")]
    UnitlessExport,

    #[error("invalid or unsupported STEP length units: {0}")]
    InvalidLengthUnits(String),

    #[error("STEP import requires exactly one data section; found {count}")]
    UnsupportedDataSections { count: usize },

    #[error("STEP assembly contains an invalid transform: {0}")]
    InvalidAssemblyTransform(String),

    #[error("STEP tessellation dropped face {face} from shape #{shape}")]
    TessellationFaceDropped { shape: u64, face: usize },

    #[error("STEP shape #{shape} tessellated to no triangles")]
    EmptyTessellation { shape: u64 },

    #[error("STEP shape #{shape} has too many tessellation vertices for 32-bit indices")]
    MeshTooLarge { shape: u64 },

    #[error("STEP file contains no supported solid or shell geometry")]
    NoSupportedGeometry,

    #[error("at least one triangle mesh is required for STEP export")]
    NoMeshesToWrite,

    #[error(
        "STEP exporter cannot represent directions for mesh triangle {face} at this coordinate scale"
    )]
    InvalidExportDirections { face: usize },
}

pub fn read_step<R: Read>(reader: R, tolerance: Tolerance) -> Result<StepImport, StepError> {
    let data = read_data_section(reader)?;
    let table = Table::from_data_section(&data);
    drop(data);
    import_table(&table, tolerance)
}

pub fn read_step_file(
    path: impl AsRef<Path>,
    tolerance: Tolerance,
) -> Result<StepImport, StepError> {
    read_step(std::fs::File::open(path)?, tolerance)
}

// Share structural validation and UTF-8/Latin-1 decoding between both readers.
fn read_data_section<R: Read>(
    mut reader: R,
) -> Result<monstertruck::step::load::step_p21::ast::DataSection, StepError> {
    use monstertruck::step::load::step_p21::parser;
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes)?;
    let text = match std::str::from_utf8(&bytes) {
        Ok(text) => std::borrow::Cow::Borrowed(text),
        Err(_) => {
            std::borrow::Cow::Owned(bytes.iter().map(|byte| *byte as char).collect::<String>())
        }
    };
    let mut exchange = parser::parse(&text).map_err(LoadError::from)?;
    if exchange.data.len() != 1 {
        return Err(StepError::UnsupportedDataSections {
            count: exchange.data.len(),
        });
    }
    Ok(exchange.data.pop().expect("checked single data section"))
}

/// Imports a uniform-unit STEP file into explicit target units. Mixed-unit
/// contexts are rejected until per-representation assembly scaling is supported.
pub fn read_step_in_units<R: Read>(
    reader: R,
    target: &LengthUnitSystem,
    tolerance: Tolerance,
) -> Result<StepImport, StepError> {
    let data = read_data_section(reader)?;
    let (scale, source_tolerance) = units::conversion_to_target(&data, target, tolerance)?;
    let table = Table::from_data_section(&data);
    // The table owns its geometry. Do not retain a second parsed copy of a
    // potentially large STEP file while tessellating its shapes.
    drop(data);
    let mut imported = import_table(&table, source_tolerance)?;
    if scale != 1.0 {
        let transform =
            AffineTransform3::try_uniform_scale(Point3::try_new(0.0, 0.0, 0.0)?, scale)?;
        for object in &mut imported.objects {
            object.mesh = object
                .mesh
                .transformed(transform, Tolerance::MESH_VALIDATION)?;
        }
    }
    Ok(imported)
}

pub fn read_step_file_in_units(
    path: impl AsRef<Path>,
    target: &LengthUnitSystem,
    tolerance: Tolerance,
) -> Result<StepImport, StepError> {
    read_step_in_units(std::fs::File::open(path)?, target, tolerance)
}

fn import_table(table: &Table, tolerance: Tolerance) -> Result<StepImport, StepError> {
    let instance_plan::InstancePlan {
        instances,
        mut report,
    } = instance_plan::build(table)?;

    // Retain source polygons only until their final placement, rather than
    // keeping a second copy of every unique shape throughout the import.
    let mut remaining = BTreeMap::<u64, usize>::new();
    for instance in &instances {
        *remaining.entry(instance.shape_id).or_default() += 1;
    }
    let mut tessellations = BTreeMap::new();
    let mut objects = Vec::with_capacity(instances.len());
    for instance_plan::ShapeInstance {
        shape_id,
        transform,
        name,
    } in instances
    {
        if let Some(mesh) = import_shape(
            table,
            shape_id,
            transform,
            tolerance,
            &mut report,
            &mut tessellations,
        )? {
            objects.push(StepObject { mesh, name });
        }
        let count = remaining
            .get_mut(&shape_id)
            .expect("counted shape instance");
        *count -= 1;
        if *count == 0 {
            tessellations.remove(&shape_id);
        }
    }

    if objects.is_empty() {
        Err(StepError::NoSupportedGeometry)
    } else {
        Ok(StepImport { objects, report })
    }
}

fn import_shape(
    table: &Table,
    shape_id: u64,
    transform: Matrix4,
    tolerance: Tolerance,
    report: &mut StepImportReport,
    tessellations: &mut BTreeMap<u64, Option<PolygonMesh>>,
) -> Result<Option<TriangleMesh>, StepError> {
    // Cache source-space tessellation, not a validated native mesh: each
    // instance must still be transformed and validated in document space.
    let polygon = match tessellations.entry(shape_id) {
        std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(tessellate_shape(table, shape_id, tolerance, report)?)
        }
    };
    polygon
        .as_ref()
        .map(|polygon| polygon_to_mesh(shape_id, polygon, transform))
        .transpose()
}

fn tessellate_shape(
    table: &Table,
    shape_id: u64,
    tolerance: Tolerance,
    report: &mut StepImportReport,
) -> Result<Option<PolygonMesh>, StepError> {
    if let Some(solid) = table.manifold_solid_brep.get(&shape_id) {
        let outer_id = referenced_entity(&solid.outer, "failed to reference `solid.outer`")?;
        let (outer, outer_report) = reported_trimmed_shell(table, outer_id)?;
        record_topology_report(report, &outer_report);
        let mut boundaries = vec![outer];
        for inner in &solid.voids {
            let inner_id =
                referenced_entity(inner, "failed to reference an element of `solid.voids`")?;
            let (inner, inner_report) = reported_trimmed_shell(table, inner_id)?;
            record_topology_report(report, &inner_report);
            boundaries.push(inner);
        }
        let solid = CompressedTrimmedSolid { boundaries };
        let mesh_tolerance = tessellation_tolerance(solid.boundaries.iter(), tolerance)?;
        let tessellation = solid.robust_triangulation(mesh_tolerance);
        reject_dropped_faces(
            shape_id,
            tessellation
                .boundaries
                .iter()
                .flat_map(|shell| shell.faces.iter().map(|face| face.surface.is_some())),
        )?;
        return Ok(Some(tessellation.to_polygon()));
    }

    if let Some(surface_model) = table.shell_based_surface_model.get(&shape_id) {
        let mut shells = Vec::with_capacity(surface_model.sbsm_boundary.len());
        for shell in &surface_model.sbsm_boundary {
            let shell_id =
                referenced_entity(shell, "failed to reference an element of `sbsm_boundary`")?;
            let (shell, shell_report) = reported_trimmed_shell(table, shell_id)?;
            record_topology_report(report, &shell_report);
            shells.push(shell);
        }
        let mesh_tolerance = tessellation_tolerance(shells.iter(), tolerance)?;
        let mut polygon = PolygonMesh::default();
        let mut face_offset = 0;
        for shell in shells {
            let tessellation = shell.robust_triangulation(mesh_tolerance);
            reject_dropped_faces(
                shape_id,
                tessellation.faces.iter().map(|face| face.surface.is_some()),
            )
            .map_err(|error| match error {
                StepError::TessellationFaceDropped { shape, face } => {
                    StepError::TessellationFaceDropped {
                        shape,
                        face: face + face_offset,
                    }
                }
                other => other,
            })?;
            face_offset += tessellation.faces.len();
            polygon.merge(tessellation.to_polygon());
        }
        return Ok(Some(polygon));
    }

    Ok(None)
}

fn referenced_entity<T>(holder: &PlaceHolder<T>, context: &str) -> Result<u64, LoadError> {
    match holder {
        PlaceHolder::Ref(Name::Entity(id)) => Ok(*id),
        _ => Err(LoadError::Conversion(context.to_owned())),
    }
}

fn reported_trimmed_shell(
    table: &Table,
    shell_id: u64,
) -> Result<(StepCompressedTrimmedShell, ShellLoadReport), StepError> {
    if let Some(shell) = table.shell.get(&shell_id) {
        Ok(table.to_compressed_trimmed_shell_reported(shell)?)
    } else if let Some(shell) = table.oriented_shell.get(&shell_id) {
        Ok(table.to_compressed_trimmed_shell_reported(shell)?)
    } else {
        Err(StepError::Load(LoadError::Conversion(format!(
            "failed to resolve shell #{shell_id}"
        ))))
    }
}

fn tessellation_tolerance<'a>(
    shells: impl Iterator<Item = &'a StepCompressedTrimmedShell>,
    document_tolerance: Tolerance,
) -> Result<f64, StepError> {
    let mut extent = SampledExtent::default();
    for shell in shells {
        for point in &shell.vertices {
            extent.push(*point)?;
        }
        for edge in &shell.edges {
            let (start, end) = edge.curve.range_tuple();
            for sample in 0..=4 {
                let parameter = sample_parameter(start, end, sample);
                extent.push(edge.curve.evaluate(parameter))?;
            }
        }
        for face in &shell.faces {
            let (Some((u_start, u_end)), Some((v_start, v_end))) = face.surface.try_range_tuple()
            else {
                continue;
            };
            for u_sample in 0..=4 {
                let u = sample_parameter(u_start, u_end, u_sample);
                for v_sample in 0..=4 {
                    let v = sample_parameter(v_start, v_end, v_sample);
                    extent.push(face.surface.evaluate(u, v))?;
                }
            }
        }
    }
    Ok(extent
        .relative_diameter()
        .max(document_tolerance.absolute())
        .max(monstertruck::core::tolerance::TOLERANCE))
}

// The five sampling stations include the exact endpoints. Opposite-sign
// finite bounds may have an unrepresentable difference, even though every
// interpolated parameter is representable.
fn sample_parameter(start: f64, end: f64, sample: u32) -> f64 {
    debug_assert!(sample <= 4);
    match sample {
        0 => start,
        4 => end,
        _ => {
            let fraction = f64::from(sample) / 4.0;
            let span = end - start;
            if span.is_finite() {
                start + span * fraction
            } else {
                let midpoint = start * 0.5 + end * 0.5;
                match sample {
                    1 => start * 0.5 + midpoint * 0.5,
                    2 => midpoint,
                    3 => midpoint * 0.5 + end * 0.5,
                    _ => unreachable!("interior quarter station"),
                }
            }
        }
    }
}

struct SampledExtent {
    minimum: [f64; 3],
    maximum: [f64; 3],
    count: usize,
}

impl Default for SampledExtent {
    fn default() -> Self {
        Self {
            minimum: [f64::INFINITY; 3],
            maximum: [f64::NEG_INFINITY; 3],
            count: 0,
        }
    }
}

impl SampledExtent {
    fn push(&mut self, point: monstertruck::core::cgmath64::Point3) -> Result<(), GeometryError> {
        // f64::min/max ignore NaNs. Validate the complete point before
        // updating any axis so a failed sample cannot silently shrink bounds.
        Point3::try_new(point.x, point.y, point.z)?;
        self.minimum[0] = self.minimum[0].min(point.x);
        self.minimum[1] = self.minimum[1].min(point.y);
        self.minimum[2] = self.minimum[2].min(point.z);
        self.maximum[0] = self.maximum[0].max(point.x);
        self.maximum[1] = self.maximum[1].max(point.y);
        self.maximum[2] = self.maximum[2].max(point.z);
        self.count += 1;
        Ok(())
    }

    fn relative_diameter(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            let spans = std::array::from_fn::<_, 3, _>(|axis| {
                let span = self.maximum[axis] - self.minimum[axis];
                if span.is_finite() {
                    span * RELATIVE_MESH_TOLERANCE
                } else {
                    self.maximum[axis] * RELATIVE_MESH_TOLERANCE
                        - self.minimum[axis] * RELATIVE_MESH_TOLERANCE
                }
            });
            // Scale before both subtraction overflow and diagonal overflow.
            spans[0].hypot(spans[1]).hypot(spans[2])
        }
    }
}

fn reject_dropped_faces(shape: u64, faces: impl Iterator<Item = bool>) -> Result<(), StepError> {
    for (face, retained) in faces.enumerate() {
        if !retained {
            return Err(StepError::TessellationFaceDropped { shape, face });
        }
    }
    Ok(())
}

fn polygon_to_mesh(
    shape: u64,
    polygon: &PolygonMesh,
    transform: Matrix4,
) -> Result<TriangleMesh, StepError> {
    let vertices = polygon
        .positions()
        .iter()
        .map(|point| {
            let point = transform.transform_point(*point);
            Point3::try_new(point.x, point.y, point.z)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let triangles = polygon
        .faces()
        .triangle_iter()
        .map(|face| {
            Ok([
                u32::try_from(face[0].pos).map_err(|_| StepError::MeshTooLarge { shape })?,
                u32::try_from(face[1].pos).map_err(|_| StepError::MeshTooLarge { shape })?,
                u32::try_from(face[2].pos).map_err(|_| StepError::MeshTooLarge { shape })?,
            ])
        })
        .collect::<Result<Vec<_>, StepError>>()?;
    if triangles.is_empty() {
        return Err(StepError::EmptyTessellation { shape });
    }
    // Tessellation accuracy and a mesh's numerical validity are different
    // policies. A coarse document tolerance must not reject finite small faces.
    Ok(TriangleMesh::try_new(
        vertices,
        triangles,
        Tolerance::MESH_VALIDATION,
    )?)
}

fn record_topology_report(report: &mut StepImportReport, shell_report: &ShellLoadReport) {
    let lost = shell_report.total_lost();
    if lost > 0 {
        report.lost_topology_item_count += lost;
        report.topology_warnings.push(shell_report.to_string());
    }
}

fn record_relationship_report(report: &mut StepImportReport, shell_report: &ShellLoadReport) {
    let lost = shell_report.lost(LossCategory::Representation);
    if lost > 0 {
        report.lost_topology_item_count += lost;
        report.topology_warnings.push(shell_report.to_string());
    }
}

#[cfg(test)]
mod tests;
