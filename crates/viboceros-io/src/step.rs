use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::path::Path;
mod export_geometry;
mod export_plane;
use export_geometry::{ExportLine, ExportPoint};
mod units;
use export_plane::ExportPlane;

use monstertruck::core::cgmath64::{Matrix4, SquareMatrix, Transform};
use monstertruck::meshing::prelude::{
    BoundedCurve, MeshedShape, ParametricCurve, ParametricSurface, PolygonMesh, RobustMeshableShape,
};
use monstertruck::modeling::Point3 as TruckPoint3;
use monstertruck::step::load::convert::StepCompressedTrimmedShell;
use monstertruck::step::load::step_p21::{ast::Name, tables::PlaceHolder};
use monstertruck::step::load::{LoadError, LossCategory, ShellLoadReport, Table};
use monstertruck::step::save::{
    CompleteStepDisplay, StepHeaderDescriptor, StepMeasurementContext, StepModels,
};
use monstertruck::topology::compress::{
    CompressedEdge, CompressedEdgeIndex, CompressedFace, CompressedShell, CompressedTrimmedSolid,
};
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
    let source = LengthUnitSystem::Custom {
        name: "STEP file units".into(),
        meters_per_unit: units::uniform_meters_per_unit(&data)?,
    };
    let scale = source.scale_to(target)?;
    let source_tolerance = Tolerance::try_new(
        tolerance.absolute() / scale,
        tolerance.relative(),
        tolerance.angular(),
    )?;
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

/// Writes validated triangle meshes as STEP shell-based surface models. Mesh
/// edges are shared topologically and every triangle is an oriented planar
/// `ADVANCED_FACE`; coordinates are interpreted as millimetres.
/// Open meshes remain open shells rather than being
/// misrepresented as solids.
pub fn write_step<W: Write>(writer: W, meshes: &[TriangleMesh]) -> Result<(), StepError> {
    write_step_with_accuracy(
        writer,
        meshes,
        StepMeasurementContext::default().distance_accuracy_value,
    )
}

fn write_step_with_accuracy<W: Write>(
    mut writer: W,
    meshes: &[TriangleMesh],
    accuracy: f64,
) -> Result<(), StepError> {
    if meshes.is_empty() {
        return Err(StepError::NoMeshesToWrite);
    }
    let shells = meshes
        .iter()
        .map(mesh_to_shell)
        .collect::<Result<Vec<_>, _>>()?;
    let models = StepModels::from_iter(&shells).with_measurement_context(StepMeasurementContext {
        distance_accuracy_value: accuracy,
        ..Default::default()
    });
    let display = CompleteStepDisplay::new(
        models,
        StepHeaderDescriptor {
            organization_system: "Viboceros".to_owned(),
            ..Default::default()
        },
    );
    write!(writer, "{display}")?;
    Ok(())
}

pub fn write_step_file(path: impl AsRef<Path>, meshes: &[TriangleMesh]) -> Result<(), StepError> {
    write_step_staged(path.as_ref(), |file| write_step(file, meshes))
}

/// Converts source coordinates to millimetres to match the STEP unit declaration.
/// The source meshes are unchanged; tolerance is expressed in source units.
pub fn write_step_in_units<W: Write>(
    writer: W,
    meshes: &[TriangleMesh],
    units: &LengthUnitSystem,
    tolerance: Tolerance,
) -> Result<(), StepError> {
    if matches!(units, LengthUnitSystem::None) {
        return Err(StepError::UnitlessExport);
    }
    let scale = units.scale_to(&LengthUnitSystem::Millimeters)?;
    if scale == 1.0 {
        return write_step_with_accuracy(writer, meshes, tolerance.absolute());
    }
    let target_tolerance = Tolerance::try_new(
        tolerance.absolute() * scale,
        tolerance.relative(),
        tolerance.angular(),
    )?;
    let transform = AffineTransform3::try_uniform_scale(Point3::try_new(0.0, 0.0, 0.0)?, scale)?;
    let converted = meshes
        .iter()
        .map(|mesh| mesh.transformed(transform, Tolerance::MESH_VALIDATION))
        .collect::<Result<Vec<_>, _>>()?;
    write_step_with_accuracy(writer, &converted, target_tolerance.absolute())
}

/// Atomically writes STEP in millimetres, converting from explicit source units.
pub fn write_step_file_in_units(
    path: impl AsRef<Path>,
    meshes: &[TriangleMesh],
    units: &LengthUnitSystem,
    tolerance: Tolerance,
) -> Result<(), StepError> {
    write_step_staged(path.as_ref(), |file| {
        write_step_in_units(file, meshes, units, tolerance)
    })
}

fn write_step_staged(
    destination: &Path,
    write: impl FnOnce(&std::fs::File) -> Result<(), StepError>,
) -> Result<(), StepError> {
    let staged = crate::staged_file::StagedFile::new(destination, ".step.tmp")?;
    write(staged.file())?;
    staged.commit()?;
    Ok(())
}

fn mesh_to_shell(
    mesh: &TriangleMesh,
) -> Result<CompressedShell<ExportPoint, ExportLine, ExportPlane>, StepError> {
    let vertices = mesh
        .vertices()
        .iter()
        .map(|point| TruckPoint3::new(point.x(), point.y(), point.z()))
        .collect::<Vec<_>>();
    let mut edge_indices = BTreeMap::<(u32, u32), usize>::new();
    let mut edges = Vec::new();
    let mut faces = Vec::with_capacity(mesh.triangles().len());

    for (face, triangle) in mesh.triangles().iter().enumerate() {
        let plane = ExportPlane::from_triangle(mesh, face)?;
        let directed_edges = [
            (triangle[0], triangle[1]),
            (triangle[1], triangle[2]),
            (triangle[2], triangle[0]),
        ];
        let boundary = directed_edges
            .into_iter()
            .map(|(start, end)| {
                let key = if start < end {
                    (start, end)
                } else {
                    (end, start)
                };
                let index = if let Some(&index) = edge_indices.get(&key) {
                    index
                } else {
                    let index = edges.len();
                    let endpoints = (key.0 as usize, key.1 as usize);
                    let curve = ExportLine::try_new(
                        mesh.vertices()[endpoints.0],
                        mesh.vertices()[endpoints.1],
                    )
                    .map_err(|_| StepError::InvalidExportDirections { face })?;
                    edges.push(CompressedEdge {
                        vertices: endpoints,
                        curve,
                    });
                    edge_indices.insert(key, index);
                    index
                };
                Ok(CompressedEdgeIndex {
                    index,
                    orientation: (start, end) == key,
                })
            })
            .collect::<Result<Vec<_>, StepError>>()?;
        faces.push(CompressedFace {
            boundaries: vec![boundary],
            orientation: true,
            surface: plane,
        });
    }

    Ok(CompressedShell {
        vertices: vertices.into_iter().map(ExportPoint).collect(),
        edges,
        faces,
        vertex_stable_ids: None,
        edge_stable_ids: None,
        face_stable_ids: None,
    })
}

fn import_table(table: &Table, tolerance: Tolerance) -> Result<StepImport, StepError> {
    let mut report = StepImportReport {
        swallowed_entity_count: table.entity_report.total(),
        ..Default::default()
    };
    let mut instances = Vec::new();
    let mut placed_shapes = BTreeSet::new();

    match table.step_assy() {
        Ok(assembly) => {
            for top in assembly.top_nodes() {
                for path in assembly.paths_iter(top.index()) {
                    let node = path.terminal_node();
                    let transform = path.edges().iter().try_fold(
                        Matrix4::identity(),
                        |accumulated, edge| {
                            Matrix4::try_from(edge.matrix())
                                .map(|matrix| accumulated * matrix)
                                .map_err(|error| {
                                    StepError::InvalidAssemblyTransform(error.to_string())
                                })
                        },
                    )?;
                    let mut shape_ids = node.shape().iter().copied().collect::<BTreeSet<_>>();
                    if let Some(representation) = table.shape_representation_of_node(node.entity())
                    {
                        let (_, relationship_report) =
                            table.solids_via_shape_relationship(representation);
                        record_relationship_report(&mut report, &relationship_report);
                        let (related_shapes, skipped) =
                            related_supported_shapes(table, representation);
                        shape_ids.extend(related_shapes);
                        report.skipped_representation_item_count += skipped;
                    }
                    let name = path
                        .edges()
                        .last()
                        .and_then(|edge| nonempty_name(&edge.attributes().name))
                        .or_else(|| nonempty_name(&node.attributes().name));
                    for shape_id in shape_ids {
                        if table.manifold_solid_brep.contains_key(&shape_id)
                            || table.shell_based_surface_model.contains_key(&shape_id)
                        {
                            placed_shapes.insert(shape_id);
                            instances.push((shape_id, transform, name.clone()));
                        } else if !is_placement_item(table, shape_id) {
                            report.skipped_representation_item_count += 1;
                        }
                    }
                }
            }
        }
        Err(error) => {
            report.assembly_warning = Some(error.to_string());
        }
    }

    let mut supported_shape_ids = table
        .manifold_solid_brep
        .keys()
        .chain(table.shell_based_surface_model.keys())
        .copied()
        .collect::<Vec<_>>();
    supported_shape_ids.sort_unstable();
    supported_shape_ids.dedup();
    for shape_id in supported_shape_ids {
        if placed_shapes.contains(&shape_id) {
            continue;
        }
        report.unplaced_shape_count += 1;
        instances.push((shape_id, Matrix4::identity(), None));
    }

    // Retain source polygons only until their final placement, rather than
    // keeping a second copy of every unique shape throughout the import.
    let mut remaining = BTreeMap::<u64, usize>::new();
    for (shape, _, _) in &instances {
        *remaining.entry(*shape).or_default() += 1;
    }
    let mut tessellations = BTreeMap::new();
    let mut objects = Vec::with_capacity(instances.len());
    for (shape_id, transform, name) in instances {
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

fn related_supported_shapes(table: &Table, source_representation: u64) -> (BTreeSet<u64>, usize) {
    let mut relationships = table
        .shape_representation_relationship
        .iter()
        .filter(|(_, relationship)| {
            matches!(
                &relationship.rep_1,
                PlaceHolder::Ref(Name::Entity(id)) if *id == source_representation
            )
        })
        .collect::<Vec<_>>();
    relationships.sort_unstable_by_key(|(id, _)| **id);

    let mut shapes = BTreeSet::new();
    let mut skipped = 0;
    for (_, relationship) in relationships {
        let PlaceHolder::Ref(Name::Entity(target_id)) = &relationship.rep_2 else {
            continue;
        };
        let Some(target) = table.shape_representation.get(target_id) else {
            continue;
        };
        for item in &target.items {
            let PlaceHolder::Ref(Name::Entity(item_id)) = item else {
                skipped += 1;
                continue;
            };
            if table.manifold_solid_brep.contains_key(item_id)
                || table.shell_based_surface_model.contains_key(item_id)
            {
                shapes.insert(*item_id);
            } else if !is_placement_item(table, *item_id) {
                skipped += 1;
            }
        }
    }
    (shapes, skipped)
}

fn is_placement_item(table: &Table, id: u64) -> bool {
    table.placement.contains_key(&id)
        || table.axis1_placement.contains_key(&id)
        || table.axis2_placement_2d.contains_key(&id)
        || table.axis2_placement_3d.contains_key(&id)
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

fn nonempty_name(name: &str) -> Option<String> {
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_owned())
}

#[cfg(test)]
mod tests;
