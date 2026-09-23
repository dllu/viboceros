//! Exact-edge planar B-rep STEP export without display tessellation.
use std::io::Write;
use std::path::Path;

use monstertruck::step::save::{
    CompleteStepDisplay, StepHeaderDescriptor, StepMeasurementContext, StepModels,
};
use monstertruck::topology::compress::{
    CompressedEdge, CompressedEdgeIndex, CompressedFace, CompressedShell, CompressedSolid,
};
use viboceros_geometry::{Brep, LengthUnitSystem, Point3, Tolerance};

use super::super::export_geometry::{ExportLine, ExportPoint};
use super::super::export_plane::ExportPlane;
use super::super::{StepError, TruckPoint3};
use super::components;
mod boxes;

type PlanarShell = CompressedShell<ExportPoint, ExportLine, ExportPlane>;
type PlanarSolid = CompressedSolid<ExportPoint, ExportLine, ExportPlane>;

enum NativeModel {
    Shell(PlanarShell),
    Solid(PlanarSolid),
}

/// Writes straight-edged planar B-reps as editable STEP shell models in millimetres.
/// Certified convex planar polyhedra and their contained cavities retain solid structure.
/// Other edge-disconnected shells are emitted separately. Curved edges,
/// singular trims, and nonplanar faces are rejected before writing output.
pub fn write_step_planar_breps<'a, W: Write>(
    writer: W,
    breps: impl IntoIterator<Item = &'a Brep>,
) -> Result<(), StepError> {
    write_with_accuracy(
        writer,
        breps,
        Tolerance::DEFAULT,
        1.0,
        StepMeasurementContext::default().distance_accuracy_value,
    )
}

fn write_with_accuracy<'a, W: Write>(
    mut writer: W,
    breps: impl IntoIterator<Item = &'a Brep>,
    tolerance: Tolerance,
    scale: f64,
    accuracy: f64,
) -> Result<(), StepError> {
    let mut items = Vec::new();
    for (index, brep) in breps.into_iter().enumerate() {
        let shells = components::partition(brep_to_shell(brep, index, tolerance, scale)?);
        if let Some(order) = brep
            .certified_convex_solid_shell_order()
            .or_else(|| boxes::solid_shell_order(brep))
            && order.len() == shells.len()
        {
            let mut slots = shells.into_iter().map(Some).collect::<Vec<_>>();
            let boundaries = order
                .into_iter()
                .map(|shell| {
                    slots[shell]
                        .take()
                        .expect("component order is a permutation")
                })
                .collect();
            items.push(NativeModel::Solid(PlanarSolid {
                boundaries,
                id_allocator: None,
                attributes: None,
            }));
        } else {
            items.extend(shells.into_iter().map(NativeModel::Shell));
        }
    }
    if items.is_empty() {
        return Err(StepError::NoBrepsToWrite);
    }
    let mut models = StepModels::default().with_measurement_context(StepMeasurementContext {
        distance_accuracy_value: accuracy,
        ..Default::default()
    });
    for item in &items {
        match item {
            NativeModel::Shell(shell) => models.push_shell(shell),
            NativeModel::Solid(solid) => models.push_solid(solid),
        }
    }
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

/// Atomically writes native planar STEP in millimetres.
pub fn write_step_planar_breps_file<'a>(
    path: impl AsRef<Path>,
    breps: impl IntoIterator<Item = &'a Brep>,
) -> Result<(), StepError> {
    super::write_step_staged(path.as_ref(), move |file| {
        write_step_planar_breps(file, breps)
    })
}

/// Converts source units to STEP millimetres and writes native planar shells.
pub fn write_step_planar_breps_in_units<'a, W: Write>(
    writer: W,
    breps: impl IntoIterator<Item = &'a Brep>,
    units: &LengthUnitSystem,
    tolerance: Tolerance,
) -> Result<(), StepError> {
    if matches!(units, LengthUnitSystem::None) {
        return Err(StepError::UnitlessExport);
    }
    let scale = units.scale_to(&LengthUnitSystem::Millimeters)?;
    if scale == 1.0 {
        return write_with_accuracy(writer, breps, tolerance, scale, tolerance.absolute());
    }
    let target_tolerance = Tolerance::try_new(
        tolerance.absolute() * scale,
        tolerance.relative(),
        tolerance.angular(),
    )?;
    // Uniform scaling does not change incidence or trim ownership. Rebuilding
    // the B-rep only to serialize it can round a p-curve off its 3D edge.
    write_with_accuracy(writer, breps, tolerance, scale, target_tolerance.absolute())
}

/// Atomically writes native planar STEP from physical source units.
pub fn write_step_planar_breps_file_in_units<'a>(
    path: impl AsRef<Path>,
    breps: impl IntoIterator<Item = &'a Brep>,
    units: &LengthUnitSystem,
    tolerance: Tolerance,
) -> Result<(), StepError> {
    super::write_step_staged(path.as_ref(), move |file| {
        write_step_planar_breps_in_units(file, breps, units, tolerance)
    })
}

fn brep_to_shell(
    brep: &Brep,
    index: usize,
    tolerance: Tolerance,
    scale: f64,
) -> Result<PlanarShell, StepError> {
    let unsupported = |reason| StepError::UnsupportedNativeBrep {
        brep: index,
        reason,
    };
    let scaled = |point: Point3| -> Result<Point3, StepError> {
        Ok(Point3::try_new(
            point.x() * scale,
            point.y() * scale,
            point.z() * scale,
        )?)
    };
    let vertices = brep
        .vertices()
        .iter()
        .map(|vertex| {
            let point = scaled(vertex.point())?;
            Ok(ExportPoint(TruckPoint3::new(
                point.x(),
                point.y(),
                point.z(),
            )))
        })
        .collect::<Result<Vec<_>, StepError>>()?;
    let mut edges = Vec::with_capacity(brep.edges().len());
    for edge in brep.edges() {
        let controls = edge.curve().control_points();
        let [start, end] = edge.vertices();
        if edge.curve().degree() != 1
            || controls.len() != 2
            || controls[0].weight().is_sign_positive() != controls[1].weight().is_sign_positive()
            || controls[0].point() != brep.vertices()[start].point()
            || controls[1].point() != brep.vertices()[end].point()
        {
            return Err(unsupported(
                "edge is not an endpoint-exact straight segment",
            ));
        }
        let curve = ExportLine::try_new(scaled(controls[0].point())?, scaled(controls[1].point())?)
            .map_err(|_| unsupported("edge direction is not representable"))?;
        edges.push(CompressedEdge {
            vertices: (start, end),
            curve,
        });
    }
    let mut faces = Vec::with_capacity(brep.faces().len());
    for face in brep.faces() {
        let plane = ExportPlane::from_surface(face.surface(), tolerance)?
            .ok_or_else(|| unsupported("face is not planar"))?
            .scaled(scale)?;
        let mut boundaries = Vec::with_capacity(face.loops().len());
        for face_loop in face.loops() {
            let mut boundary = Vec::with_capacity(face_loop.trims().len());
            for trim in face_loop.trims() {
                let edge = trim
                    .edge()
                    .ok_or_else(|| unsupported("singular trim has no STEP edge"))?;
                boundary.push(CompressedEdgeIndex {
                    index: edge,
                    orientation: !trim.is_reversed_3d(),
                });
            }
            boundaries.push(boundary);
        }
        faces.push(CompressedFace {
            boundaries,
            orientation: !face.is_reversed(),
            surface: plane,
        });
    }
    Ok(CompressedShell {
        vertices,
        edges,
        faces,
        vertex_stable_ids: None,
        edge_stable_ids: None,
        face_stable_ids: None,
    })
}
