use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::path::Path;

use monstertruck::core::cgmath64::{Matrix4, SquareMatrix, Transform};
use monstertruck::meshing::prelude::{
    BoundedCurve, MeshedShape, ParametricCurve, ParametricSurface, PolygonMesh, RobustMeshableShape,
};
use monstertruck::modeling::{Curve, Line, Plane, Point3 as TruckPoint3, Surface};
use monstertruck::step::load::convert::StepCompressedTrimmedShell;
use monstertruck::step::load::step_p21::{ast::Name, tables::PlaceHolder};
use monstertruck::step::load::{LoadError, LossCategory, ShellLoadReport, Table};
use monstertruck::step::save::{CompleteStepDisplay, StepHeaderDescriptor, StepModels};
use monstertruck::topology::compress::{
    CompressedEdge, CompressedEdgeIndex, CompressedFace, CompressedShell, CompressedTrimmedSolid,
};
use thiserror::Error;
use viboceros_geometry::{GeometryError, Point3, Tolerance, TriangleMesh};

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
    /// Topological items omitted while resolving shells and solids.
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
}

pub fn read_step<R: Read>(mut reader: R, tolerance: Tolerance) -> Result<StepImport, StepError> {
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes)?;
    let table = Table::from_step_bytes(&bytes)?;
    import_table(&table, tolerance)
}

pub fn read_step_file(
    path: impl AsRef<Path>,
    tolerance: Tolerance,
) -> Result<StepImport, StepError> {
    read_step(std::fs::File::open(path)?, tolerance)
}

/// Writes validated triangle meshes as STEP shell-based surface models. Mesh
/// edges are shared topologically and every triangle is an oriented planar
/// `ADVANCED_FACE`; open meshes remain open shells rather than being
/// misrepresented as solids.
pub fn write_step<W: Write>(mut writer: W, meshes: &[TriangleMesh]) -> Result<(), StepError> {
    if meshes.is_empty() {
        return Err(StepError::NoMeshesToWrite);
    }
    let shells = meshes.iter().map(mesh_to_shell).collect::<Vec<_>>();
    let models = StepModels::from_iter(&shells);
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
    let destination = path.as_ref();
    let parent = destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let staged = tempfile::Builder::new()
        .prefix(".viboceros-")
        .suffix(".step.tmp")
        .tempfile_in(parent)?;
    write_step(staged.as_file(), meshes)?;
    staged.as_file().sync_all()?;
    staged
        .persist(destination)
        .map_err(|error| StepError::Io(error.error))?;
    Ok(())
}

fn mesh_to_shell(mesh: &TriangleMesh) -> CompressedShell<TruckPoint3, Curve, Surface> {
    let vertices = mesh
        .vertices()
        .iter()
        .map(|point| TruckPoint3::new(point.x(), point.y(), point.z()))
        .collect::<Vec<_>>();
    let mut edge_indices = BTreeMap::<(u32, u32), usize>::new();
    let mut edges = Vec::new();
    let mut faces = Vec::with_capacity(mesh.triangles().len());

    for triangle in mesh.triangles() {
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
                let index = *edge_indices.entry(key).or_insert_with(|| {
                    let index = edges.len();
                    let endpoints = (key.0 as usize, key.1 as usize);
                    edges.push(CompressedEdge {
                        vertices: endpoints,
                        curve: Curve::Line(Line(vertices[endpoints.0], vertices[endpoints.1])),
                    });
                    index
                });
                CompressedEdgeIndex {
                    index,
                    orientation: (start, end) == key,
                }
            })
            .collect();
        let points = [
            vertices[triangle[0] as usize],
            vertices[triangle[1] as usize],
            vertices[triangle[2] as usize],
        ];
        faces.push(CompressedFace {
            boundaries: vec![boundary],
            orientation: true,
            surface: Surface::Plane(Plane::new(points[0], points[1], points[2])),
        });
    }

    CompressedShell {
        vertices,
        edges,
        faces,
        vertex_stable_ids: None,
        edge_stable_ids: None,
        face_stable_ids: None,
    }
}

fn import_table(table: &Table, tolerance: Tolerance) -> Result<StepImport, StepError> {
    let mut report = StepImportReport {
        swallowed_entity_count: table.entity_report.total(),
        ..Default::default()
    };
    let mut objects = Vec::new();
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
                        if let Some(mesh) =
                            import_shape(table, shape_id, transform, tolerance, &mut report)?
                        {
                            placed_shapes.insert(shape_id);
                            objects.push(StepObject {
                                mesh,
                                name: name.clone(),
                            });
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
        if let Some(mesh) =
            import_shape(table, shape_id, Matrix4::identity(), tolerance, &mut report)?
        {
            report.unplaced_shape_count += 1;
            objects.push(StepObject { mesh, name: None });
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
) -> Result<Option<TriangleMesh>, StepError> {
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
        let mesh_tolerance = tessellation_tolerance(solid.boundaries.iter(), tolerance);
        let tessellation = solid.robust_triangulation(mesh_tolerance);
        reject_dropped_faces(
            shape_id,
            tessellation
                .boundaries
                .iter()
                .flat_map(|shell| shell.faces.iter().map(|face| face.surface.is_some())),
        )?;
        return polygon_to_mesh(shape_id, tessellation.to_polygon(), transform, tolerance)
            .map(Some);
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
        let mesh_tolerance = tessellation_tolerance(shells.iter(), tolerance);
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
        return polygon_to_mesh(shape_id, polygon, transform, tolerance).map(Some);
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
) -> f64 {
    let mut extent = SampledExtent::default();
    for shell in shells {
        for point in &shell.vertices {
            extent.push(*point);
        }
        for edge in &shell.edges {
            let (start, end) = edge.curve.range_tuple();
            for sample in 0..=4 {
                let parameter = start + (end - start) * f64::from(sample) / 4.0;
                extent.push(edge.curve.evaluate(parameter));
            }
        }
        for face in &shell.faces {
            let (Some((u_start, u_end)), Some((v_start, v_end))) = face.surface.try_range_tuple()
            else {
                continue;
            };
            for u_sample in 0..=4 {
                let u = u_start + (u_end - u_start) * f64::from(u_sample) / 4.0;
                for v_sample in 0..=4 {
                    let v = v_start + (v_end - v_start) * f64::from(v_sample) / 4.0;
                    extent.push(face.surface.evaluate(u, v));
                }
            }
        }
    }
    (extent.diameter() * RELATIVE_MESH_TOLERANCE)
        .max(document_tolerance.absolute())
        .max(monstertruck::core::tolerance::TOLERANCE)
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
    fn push(&mut self, point: monstertruck::core::cgmath64::Point3) {
        self.minimum[0] = self.minimum[0].min(point.x);
        self.minimum[1] = self.minimum[1].min(point.y);
        self.minimum[2] = self.minimum[2].min(point.z);
        self.maximum[0] = self.maximum[0].max(point.x);
        self.maximum[1] = self.maximum[1].max(point.y);
        self.maximum[2] = self.maximum[2].max(point.z);
        self.count += 1;
    }

    fn diameter(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            (self.maximum[0] - self.minimum[0])
                .hypot(self.maximum[1] - self.minimum[1])
                .hypot(self.maximum[2] - self.minimum[2])
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
    polygon: PolygonMesh,
    transform: Matrix4,
    tolerance: Tolerance,
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
    Ok(TriangleMesh::try_new(vertices, triangles, tolerance)?)
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
mod tests {
    use std::io::Cursor;

    use super::*;
    use monstertruck::modeling::{BoundingBox, Point3 as TruckPoint3, primitive};
    use monstertruck::step::save::{
        CompleteStepDisplay, StepHeaderDescriptor, StepModel as TruckStepModel,
    };

    fn cube_step() -> String {
        let cube: monstertruck::modeling::Solid = primitive::cuboid(BoundingBox::from_iter([
            TruckPoint3::new(-1.0, -2.0, -3.0),
            TruckPoint3::new(4.0, 5.0, 6.0),
        ]));
        let compressed = cube.compress();
        CompleteStepDisplay::new(
            TruckStepModel::from(&compressed),
            StepHeaderDescriptor {
                organization_system: "Viboceros test".to_owned(),
                ..Default::default()
            },
        )
        .to_string()
    }

    #[test]
    fn imports_an_analytic_step_solid_as_a_validated_mesh() {
        let model = read_step(Cursor::new(cube_step()), Tolerance::DEFAULT).unwrap();

        assert_eq!(model.objects.len(), 1);
        let mesh = &model.objects[0].mesh;
        assert_eq!(mesh.triangles().len(), 12);
        assert!(mesh.bounds().min().is_near(
            Point3::try_new(-1.0, -2.0, -3.0).unwrap(),
            Tolerance::DEFAULT
        ));
        assert!(
            mesh.bounds()
                .max()
                .is_near(Point3::try_new(4.0, 5.0, 6.0).unwrap(), Tolerance::DEFAULT)
        );
        assert_eq!(model.report.swallowed_entity_count, 0);
        assert_eq!(model.report.lost_topology_item_count, 0);
    }

    #[test]
    fn exported_mesh_round_trips_as_a_shared_edge_step_shell() {
        let mesh = TriangleMesh::try_new(
            vec![
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(2.0, 0.0, 0.0).unwrap(),
                Point3::try_new(2.0, 3.0, 0.0).unwrap(),
                Point3::try_new(0.0, 3.0, 0.0).unwrap(),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let mut bytes = Vec::new();
        write_step(&mut bytes, std::slice::from_ref(&mesh)).unwrap();
        let text = std::str::from_utf8(&bytes).unwrap();
        assert!(text.starts_with("ISO-10303-21;"));
        assert!(text.contains("SHELL_BASED_SURFACE_MODEL"));
        assert_eq!(text.matches("EDGE_CURVE(").count(), 5);
        assert_eq!(text.matches("ADVANCED_FACE(").count(), 2);

        let imported = read_step(Cursor::new(bytes), Tolerance::DEFAULT).unwrap();
        assert_eq!(imported.objects.len(), 1);
        assert_eq!(imported.objects[0].mesh.triangles().len(), 2);
        assert_eq!(imported.objects[0].mesh.bounds(), mesh.bounds());
    }

    #[test]
    fn failed_step_export_does_not_replace_an_existing_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("existing.step");
        std::fs::write(&path, b"keep me").unwrap();

        assert!(matches!(
            write_step_file(&path, &[]),
            Err(StepError::NoMeshesToWrite)
        ));
        assert_eq!(std::fs::read(path).unwrap(), b"keep me");
    }

    #[test]
    fn rejects_invalid_and_geometry_free_step_data() {
        assert!(matches!(
            read_step(Cursor::new(b"not STEP"), Tolerance::DEFAULT),
            Err(StepError::Load(_))
        ));

        let empty = b"ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION((''),'2;1');\nFILE_NAME('','','',(''),(''),'','');\nFILE_SCHEMA(('AUTOMOTIVE_DESIGN'));\nENDSEC;\nDATA;\nENDSEC;\nEND-ISO-10303-21;\n";
        assert!(matches!(
            read_step(Cursor::new(empty), Tolerance::DEFAULT),
            Err(StepError::NoSupportedGeometry)
        ));
    }
}
