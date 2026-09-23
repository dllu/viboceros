//! Native STEP serialization of curved B-reps with explicit face-local p-curves.
use std::collections::BTreeMap;
use std::io::Write;

use monstertruck::modeling::{
    BsplineCurve, BsplineSurface, KnotVector, NurbsCurve as TruckNurbsCurve,
    NurbsSurface as TruckNurbsSurface, ParameterCurve, Vector3, Vector4,
};
use monstertruck::step::save::{
    CompleteStepDisplay, StepCurve, StepFormat, StepHeaderDescriptor, StepLength,
    StepMeasurementContext, StepModels, StepSurface,
};
use monstertruck::topology::compress::{
    CompressedEdge, CompressedEdgeUse, CompressedTrimmedFace, CompressedTrimmedShell,
    CompressedTrimmedSolid,
};
use viboceros_geometry::{
    Brep, LengthUnitSystem, NurbsCurve, NurbsCurve2, NurbsSurface, Point3, Tolerance,
};

use super::super::super::export_geometry::{ExportLine, ExportPoint};
use super::super::super::export_plane::ExportPlane;
use super::super::super::{StepError, TruckPoint3};
use std::fmt;

type NurbsCurve3 = TruckNurbsCurve<Vector4>;
type Curve2 = TruckNurbsCurve<Vector3>;
type NurbsSurface3 = TruckNurbsSurface<Vector4>;

enum Curve3 {
    Line(ExportLine),
    Nurbs(NurbsCurve3),
}

#[derive(Clone)]
enum Surface3 {
    Plane(ExportPlane),
    Nurbs(NurbsSurface3),
}

impl StepFormat for Curve3 {
    fn fmt(&self, index: usize, writer: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Line(value) => StepFormat::fmt(value, index, writer),
            Self::Nurbs(value) => StepFormat::fmt(value, index, writer),
        }
    }
}

impl StepLength for Curve3 {
    fn step_length(&self) -> usize {
        match self {
            Self::Line(value) => value.step_length(),
            Self::Nurbs(value) => value.step_length(),
        }
    }
}
impl StepCurve for Curve3 {}

impl StepFormat for Surface3 {
    fn fmt(&self, index: usize, writer: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Plane(value) => StepFormat::fmt(value, index, writer),
            Self::Nurbs(value) => StepFormat::fmt(value, index, writer),
        }
    }
}

impl StepLength for Surface3 {
    fn step_length(&self) -> usize {
        match self {
            Self::Plane(value) => value.step_length(),
            Self::Nurbs(value) => value.step_length(),
        }
    }
}
impl StepSurface for Surface3 {}
type Trim = ParameterCurve<Curve2, Surface3>;
type Shell = CompressedTrimmedShell<ExportPoint, Curve3, Surface3, Trim>;
type Solid = CompressedTrimmedSolid<ExportPoint, Curve3, Surface3, Trim>;

enum Model {
    Shell(Shell),
    Solid(Solid),
}

/// Writes native edges, surfaces, and UV trims as editable STEP shells.
/// Singular trims are unsupported. Convex planar solids retain their certified
/// outer/void structure; other shells remain surface models.
pub fn write_step_nurbs_breps<'a, W: Write>(
    writer: W,
    breps: impl IntoIterator<Item = &'a Brep>,
) -> Result<(), StepError> {
    write_with_scale(
        writer,
        breps,
        1.0,
        Tolerance::DEFAULT,
        StepMeasurementContext::default().distance_accuracy_value,
    )
}

/// Converts source coordinates to STEP millimetres without rebuilding B-reps.
pub fn write_step_nurbs_breps_in_units<'a, W: Write>(
    writer: W,
    breps: impl IntoIterator<Item = &'a Brep>,
    units: &LengthUnitSystem,
    tolerance: Tolerance,
) -> Result<(), StepError> {
    if matches!(units, LengthUnitSystem::None) {
        return Err(StepError::UnitlessExport);
    }
    let scale = units.scale_to(&LengthUnitSystem::Millimeters)?;
    let accuracy = tolerance.absolute() * scale;
    if !accuracy.is_finite() || accuracy <= 0.0 {
        return Err(StepError::UnsupportedNativeBrep {
            brep: 0,
            reason: "scaled STEP distance accuracy is not finite and positive",
        });
    }
    write_with_scale(writer, breps, scale, tolerance, accuracy)
}

fn write_with_scale<'a, W: Write>(
    mut writer: W,
    breps: impl IntoIterator<Item = &'a Brep>,
    scale: f64,
    tolerance: Tolerance,
    accuracy: f64,
) -> Result<(), StepError> {
    let mut items = Vec::new();
    for (index, brep) in breps.into_iter().enumerate() {
        let mut shells = brep
            .edge_connected_face_components()
            .iter()
            .map(|faces| shell(brep, faces, index, scale, tolerance))
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(order) = brep.certified_convex_solid_shell_order()
            && order.len() == shells.len()
        {
            let mut slots = shells.into_iter().map(Some).collect::<Vec<_>>();
            let boundaries = order
                .into_iter()
                .map(|component| slots[component].take().expect("component permutation"))
                .collect();
            items.push(Model::Solid(Solid { boundaries }));
        } else {
            items.extend(shells.drain(..).map(Model::Shell));
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
            Model::Shell(shell) => models.push_trimmed_shell(shell),
            Model::Solid(solid) => models.push_trimmed_solid(solid),
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

fn shell(
    brep: &Brep,
    faces: &[usize],
    index: usize,
    scale: f64,
    tolerance: Tolerance,
) -> Result<Shell, StepError> {
    let unsupported = |reason| StepError::UnsupportedNativeBrep {
        brep: index,
        reason,
    };
    let mut vertices = Vec::new();
    let mut edges = Vec::new();
    let mut output_faces = Vec::new();
    let mut vertex_map = BTreeMap::new();
    let mut edge_map = BTreeMap::new();
    let surfaces = faces
        .iter()
        .map(|&face_index| {
            let surface = brep.faces()[face_index].surface();
            let converted = if surface.degree_u() == 1
                && surface.degree_v() == 1
                && surface.control_point_count_u() == 2
                && surface.control_point_count_v() == 2
                && surface
                    .control_points()
                    .iter()
                    .all(|control| control.weight() == surface.control_points()[0].weight())
            {
                match ExportPlane::from_surface(surface, tolerance)? {
                    Some(plane) => Surface3::Plane(plane.scaled(scale)?),
                    None => Surface3::Nurbs(nurbs_surface3(surface, scale, index)?),
                }
            } else {
                Surface3::Nurbs(nurbs_surface3(surface, scale, index)?)
            };
            Ok(converted)
        })
        .collect::<Result<Vec<_>, StepError>>()?;
    let curved_shell = surfaces
        .iter()
        .any(|surface| matches!(surface, Surface3::Nurbs(_)));
    for (&face_index, surface) in faces.iter().zip(surfaces) {
        let face = &brep.faces()[face_index];
        let mut boundaries = Vec::new();
        for boundary in face.loops() {
            let mut uses = Vec::new();
            for trim in boundary.trims() {
                let source_edge = trim
                    .edge()
                    .ok_or_else(|| unsupported("singular UV trim has no STEP edge"))?;
                let local_edge = if let Some(&local) = edge_map.get(&source_edge) {
                    local
                } else {
                    let edge = &brep.edges()[source_edge];
                    let mut local_vertices = [0; 2];
                    for (destination, source) in local_vertices.iter_mut().zip(edge.vertices()) {
                        *destination = *vertex_map.entry(source).or_insert_with(|| {
                            let local = vertices.len();
                            let p = brep.vertices()[source].point();
                            vertices.push(ExportPoint(TruckPoint3::new(
                                p.x() * scale,
                                p.y() * scale,
                                p.z() * scale,
                            )));
                            local
                        });
                    }
                    if local_vertices.iter().any(|&vertex| {
                        let p = vertices[vertex].0;
                        !p.x.is_finite() || !p.y.is_finite() || !p.z.is_finite()
                    }) {
                        return Err(unsupported("scaled STEP vertex is not finite"));
                    }
                    let local = edges.len();
                    edges.push(CompressedEdge {
                        vertices: (local_vertices[0], local_vertices[1]),
                        curve: curve3(
                            edge.curve(),
                            edge.vertices()
                                .map(|vertex| brep.vertices()[vertex].point()),
                            scale,
                            index,
                            curved_shell,
                        )?,
                    });
                    edge_map.insert(source_edge, local);
                    local
                };
                uses.push(CompressedEdgeUse {
                    index: local_edge,
                    orientation: !trim.is_reversed_3d(),
                    trim_curve: match &surface {
                        Surface3::Plane(_) => None,
                        Surface3::Nurbs(_) => {
                            let reversed = trim
                                .is_reversed_3d()
                                .then(|| trim.curve().reversed())
                                .transpose()?;
                            let oriented = reversed.as_ref().unwrap_or_else(|| trim.curve());
                            Some(ParameterCurve::new(
                                curve2(oriented, index)?,
                                surface.clone(),
                            ))
                        }
                    },
                });
            }
            boundaries.push(uses);
        }
        output_faces.push(CompressedTrimmedFace {
            boundaries,
            orientation: !face.is_reversed(),
            surface,
        });
    }
    Ok(Shell {
        vertices,
        edges,
        faces: output_faces,
    })
}

fn weight_divisor(weights: impl Iterator<Item = f64>, index: usize) -> Result<f64, StepError> {
    let unsupported = || StepError::UnsupportedNativeBrep {
        brep: index,
        reason: "NURBS weights must be finite, nonzero, and have one sign",
    };
    let mut sign = None;
    let mut maximum = 0.0_f64;
    for weight in weights {
        if !weight.is_finite() || weight == 0.0 {
            return Err(unsupported());
        }
        let current = if weight > 0.0 { 1.0 } else { -1.0 };
        if sign.is_some_and(|previous| previous != current) {
            return Err(unsupported());
        }
        sign = Some(current);
        maximum = maximum.max(weight.abs());
    }
    Ok(sign.ok_or_else(unsupported)? * maximum)
}

fn homogeneous(point: Point3, weight: f64, scale: f64, index: usize) -> Result<Vector4, StepError> {
    if weight == 0.0 {
        return Err(StepError::UnsupportedNativeBrep {
            brep: index,
            reason: "NURBS weight ratio underflows STEP precision",
        });
    }
    let values = [
        point.x() * scale * weight,
        point.y() * scale * weight,
        point.z() * scale * weight,
        weight,
    ];
    if values.iter().any(|value| !value.is_finite()) {
        return Err(StepError::UnsupportedNativeBrep {
            brep: index,
            reason: "scaled homogeneous NURBS control is not finite",
        });
    }
    Ok(Vector4::new(values[0], values[1], values[2], values[3]))
}

fn curve3(
    curve: &NurbsCurve,
    endpoints: [Point3; 2],
    scale: f64,
    index: usize,
    curved_shell: bool,
) -> Result<Curve3, StepError> {
    let controls = curve.control_points();
    if !curved_shell
        && curve.degree() == 1
        && controls.len() == 2
        && controls[0].weight() == controls[1].weight()
        && controls[0].point() == endpoints[0]
        && controls[1].point() == endpoints[1]
    {
        let scaled =
            endpoints.map(|p| Point3::try_new(p.x() * scale, p.y() * scale, p.z() * scale));
        let [start, end] = scaled;
        let (start, end) = (start?, end?);
        if let Ok(line) = ExportLine::try_new(start, end) {
            return Ok(Curve3::Line(line));
        }
    }
    Ok(Curve3::Nurbs(nurbs_curve3(curve, scale, index)?))
}

fn nurbs_curve3(curve: &NurbsCurve, scale: f64, index: usize) -> Result<NurbsCurve3, StepError> {
    let divisor = weight_divisor(curve.control_points().iter().map(|p| p.weight()), index)?;
    let controls = curve
        .control_points()
        .iter()
        .map(|p| homogeneous(p.point(), p.weight() / divisor, scale, index))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TruckNurbsCurve::new(BsplineCurve::new(
        KnotVector::from(curve.knots().to_vec()),
        controls,
    )))
}

fn curve2(curve: &NurbsCurve2, index: usize) -> Result<Curve2, StepError> {
    let divisor = weight_divisor(curve.control_points().iter().map(|p| p.weight()), index)?;
    let controls = curve
        .control_points()
        .iter()
        .map(|p| {
            let weight = p.weight() / divisor;
            if weight == 0.0 {
                return Err(StepError::UnsupportedNativeBrep {
                    brep: index,
                    reason: "NURBS weight ratio underflows STEP precision",
                });
            }
            let values = [p.point().x() * weight, p.point().y() * weight, weight];
            if values.iter().any(|value| !value.is_finite()) {
                return Err(StepError::UnsupportedNativeBrep {
                    brep: index,
                    reason: "homogeneous STEP UV control is not finite",
                });
            }
            Ok(Vector3::new(values[0], values[1], values[2]))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TruckNurbsCurve::new(BsplineCurve::new(
        KnotVector::from(curve.knots().to_vec()),
        controls,
    )))
}

fn nurbs_surface3(
    surface: &NurbsSurface,
    scale: f64,
    index: usize,
) -> Result<NurbsSurface3, StepError> {
    let divisor = weight_divisor(surface.control_points().iter().map(|p| p.weight()), index)?;
    let controls = (0..surface.control_point_count_u())
        .map(|u| {
            (0..surface.control_point_count_v())
                .map(|v| {
                    let p = surface.control_point(u, v).expect("control index in range");
                    homogeneous(p.point(), p.weight() / divisor, scale, index)
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TruckNurbsSurface::new(BsplineSurface::new(
        (
            KnotVector::from(surface.knots_u().to_vec()),
            KnotVector::from(surface.knots_v().to_vec()),
        ),
        controls,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_geometry::WeightedPoint3;

    #[test]
    fn common_tiny_nurbs_weights_keep_small_euclidean_controls() {
        let coordinates = [[1e-200, 2e-200, 0.], [2e-200, 3e-200, 0.]];
        for sign in [1., -1.] {
            let controls = coordinates
                .into_iter()
                .zip([1e-200, 4e-200])
                .map(|(p, weight)| {
                    WeightedPoint3::try_new(
                        Point3::try_new(p[0], p[1], p[2]).unwrap(),
                        weight * sign,
                    )
                    .unwrap()
                })
                .collect();
            let source = NurbsCurve::try_new_rational(1, controls, vec![0., 0., 1., 1.]).unwrap();
            let converted = nurbs_curve3(&source, 1., 0).unwrap();
            for (control, expected) in converted.control_points().iter().zip(coordinates) {
                assert!(control.w > 0.);
                assert!((control.x / control.w - expected[0]).abs() < 1e-214);
                assert!((control.y / control.w - expected[1]).abs() < 1e-214);
            }
        }
    }
}
