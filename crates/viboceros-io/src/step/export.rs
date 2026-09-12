//! Mesh-to-STEP construction, unit conversion, and staged destination writes.
use std::collections::{BTreeMap, btree_map::Entry};
use std::io::{BufWriter, Write};
use std::path::Path;

use monstertruck::step::save::{
    CompleteStepDisplay, StepHeaderDescriptor, StepMeasurementContext, StepModels,
};
use monstertruck::topology::compress::{
    CompressedEdge, CompressedEdgeIndex, CompressedFace, CompressedShell,
};
use viboceros_geometry::{AffineTransform3, LengthUnitSystem, Point3, Tolerance, TriangleMesh};

use super::export_geometry::{ExportLine, ExportPoint};
use super::export_plane::ExportPlane;
use super::{StepError, TruckPoint3};
mod components;

#[cfg(test)]
mod tests;

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
    let mut shells = Vec::new();
    for mesh in meshes {
        shells.extend(components::partition(mesh_to_shell(mesh)?));
    }
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
    write: impl FnOnce(&mut BufWriter<&std::fs::File>) -> Result<(), StepError>,
) -> Result<(), StepError> {
    let staged = crate::staged_file::StagedFile::new(destination, ".step.tmp")?;
    write_buffered(staged.file(), write)?;
    staged.commit()?;
    Ok(())
}

fn write_buffered<W: Write>(
    destination: W,
    write: impl FnOnce(&mut BufWriter<W>) -> Result<(), StepError>,
) -> Result<(), StepError> {
    let mut writer = BufWriter::new(destination);
    write(&mut writer)?;
    // Drop ignores flush failures. Propagate them before the staged file can
    // be synchronized and committed over the destination.
    writer.flush()?;
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
                let index = match edge_indices.entry(key) {
                    Entry::Occupied(entry) => *entry.get(),
                    Entry::Vacant(entry) => {
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
                        entry.insert(index);
                        index
                    }
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
