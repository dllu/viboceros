//! Exact source-space conversion of straight-edged planar shell definitions.
use super::{StepError, Table, read_data_section, reported_trimmed_shell};
use monstertruck::meshing::prelude::ParametricSurface;
use monstertruck::step::load::step_geometry::{Curve2D, Curve3D, ElementarySurface, Surface};
use std::io::Read;
use viboceros_geometry::{
    AffineTransform3, Brep, BrepEdge, BrepFace, BrepLoop, BrepLoopType, BrepTrim, BrepTrimType,
    BrepVertex, LengthUnitSystem, NurbsCurve, NurbsCurve2, NurbsSurface, Point2, Point3,
    SurfaceIso, Tolerance,
};

/// An editable planar shell definition, before assembly placement.
/// Coordinates are source units or the explicit target units of the reader used.
#[derive(Clone, Debug, PartialEq)]
pub struct StepPlanarShell {
    pub source_shell_id: u64,
    pub brep: Brep,
}

/// Reads exact planar, straight-edged source shell definitions in entity-ID order.
/// This low-level API does not instantiate assemblies or convert file units.
/// Unsupported surfaces, curves, missing trims, or shell-conversion losses fail
/// the whole request; no tessellation is used as a geometry substitute.
pub fn read_step_planar_shells<R: Read>(
    reader: R,
    tolerance: Tolerance,
) -> Result<Vec<StepPlanarShell>, StepError> {
    let data = read_data_section(reader)?;
    let table = Table::from_data_section(&data);
    drop(data);
    convert_table(&table, tolerance)
}

/// Reads supported planar shell definitions in explicit target units.
/// Tolerance is in target units. Mixed/missing file units and invalid targets
/// are rejected; UV trims stay in the source surface parameterization.
/// A unitless target preserves source coordinates, matching the mesh reader.
/// Assembly instances and oriented-shell wrappers are not expanded.
pub fn read_step_planar_shells_in_units<R: Read>(
    reader: R,
    target: &LengthUnitSystem,
    tolerance: Tolerance,
) -> Result<Vec<StepPlanarShell>, StepError> {
    let data = read_data_section(reader)?;
    let (scale, source_tolerance) = super::units::conversion_to_target(&data, target, tolerance)?;
    let table = Table::from_data_section(&data);
    drop(data);
    let mut shells = convert_table(&table, source_tolerance)?;
    if scale != 1.0 {
        let transform =
            AffineTransform3::try_uniform_scale(Point3::try_new(0.0, 0.0, 0.0)?, scale)?;
        for shell in &mut shells {
            shell.brep = shell.brep.transformed(transform, tolerance)?;
        }
    }
    Ok(shells)
}

fn convert_table(table: &Table, tolerance: Tolerance) -> Result<Vec<StepPlanarShell>, StepError> {
    let mut ids = table.shell.keys().copied().collect::<Vec<_>>();
    ids.sort_unstable();
    if ids.is_empty() {
        return Err(StepError::NoSupportedGeometry);
    }
    let mut output = Vec::new();
    for id in ids {
        let unsupported = |reason| StepError::UnsupportedPlanarShell { shell: id, reason };
        let (shell, report) = reported_trimmed_shell(table, id)?;
        if report.total_lost() != 0 {
            return Err(unsupported("source shell has topology losses"));
        }
        let vertices = shell
            .vertices
            .iter()
            .map(|p| BrepVertex::try_new(Point3::try_new(p.x, p.y, p.z)?, tolerance.absolute()))
            .collect::<Result<Vec<_>, _>>()?;
        let mut edges = Vec::new();
        for edge in &shell.edges {
            let mut curve = &edge.curve;
            loop {
                match curve {
                    Curve3D::SurfaceCurve(surface_curve) => curve = surface_curve.leader(),
                    Curve3D::IntersectionCurve(intersection)
                        if matches!(
                            intersection.surface0().as_ref(),
                            Surface::ElementarySurface(ElementarySurface::Plane(_))
                        ) && matches!(
                            intersection.surface1().as_ref(),
                            Surface::ElementarySurface(ElementarySurface::Plane(_))
                        ) =>
                    {
                        curve = intersection.leader()
                    }
                    _ => break,
                }
            }
            let (points, knots) = match curve {
                Curve3D::Line(line) => (vec![line.0, line.1], vec![0.0, 0.0, 1.0, 1.0]),
                Curve3D::BsplineCurve(curve)
                    if curve.degree() == 1 && curve.control_points().len() == 2 =>
                {
                    (
                        curve.control_points().clone(),
                        curve.knot_vector().iter().copied().collect(),
                    )
                }
                _ => return Err(unsupported("3D edge is not a single linear span")),
            };
            let curve = NurbsCurve::try_new(
                1,
                points
                    .into_iter()
                    .map(|point| Point3::try_new(point.x, point.y, point.z))
                    .collect::<Result<Vec<_>, _>>()?,
                knots,
            )?;
            edges.push(BrepEdge::try_new(
                [edge.vertices.0, edge.vertices.1],
                curve,
                tolerance.absolute(),
            )?);
        }
        let mut incidence = vec![0; edges.len()];
        for face in &shell.faces {
            for edge in face.boundaries.iter().flatten() {
                incidence[edge.index] += 1;
            }
        }
        if incidence.iter().any(|&count| count > 2) {
            return Err(unsupported("non-manifold edge"));
        }
        let mut faces = Vec::new();
        for face in &shell.faces {
            let Surface::ElementarySurface(ElementarySurface::Plane(plane)) = &face.surface else {
                return Err(unsupported("surface is not a plane"));
            };
            if face.boundaries.len() != 1 {
                return Err(unsupported("multiple boundary loops are not yet supported"));
            }
            let mut bounds = [
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
            ];
            let mut trims = Vec::new();
            for edge_use in &face.boundaries[0] {
                let trim = edge_use
                    .trim_curve
                    .as_ref()
                    .ok_or_else(|| unsupported("missing UV trim"))?;
                let Curve2D::Line(line) = trim.curve().as_ref() else {
                    return Err(unsupported("UV trim is not a line"));
                };
                let start = Point2::try_new(line.0.x, line.0.y)?;
                let end = Point2::try_new(line.1.x, line.1.y)?;
                for point in [start, end] {
                    bounds[0] = bounds[0].min(point.x());
                    bounds[1] = bounds[1].max(point.x());
                    bounds[2] = bounds[2].min(point.y());
                    bounds[3] = bounds[3].max(point.y());
                }
                let endpoints = shell.edges[edge_use.index].vertices;
                let vertices = if edge_use.orientation {
                    [endpoints.0, endpoints.1]
                } else {
                    [endpoints.1, endpoints.0]
                };
                trims.push(BrepTrim::try_new(
                    vertices,
                    Some(edge_use.index),
                    !edge_use.orientation,
                    NurbsCurve2::try_line(start, end)?,
                    if incidence[edge_use.index] == 2 {
                        BrepTrimType::Mated
                    } else {
                        BrepTrimType::Boundary
                    },
                    SurfaceIso::NotIso,
                    [0.0; 2],
                )?);
            }
            let [u0, u1, v0, v1] = bounds;
            let control_points = [(u0, v0), (u1, v0), (u0, v1), (u1, v1)]
                .into_iter()
                .map(|(u, v)| {
                    let point = plane.evaluate(u, v);
                    Point3::try_new(point.x, point.y, point.z)
                })
                .collect::<Result<Vec<_>, _>>()?;
            let surface = NurbsSurface::try_new(
                1,
                1,
                2,
                2,
                control_points,
                vec![u0, u0, u1, u1],
                vec![v0, v0, v1, v1],
            )?;
            faces.push(BrepFace::try_new(
                surface,
                !face.orientation,
                vec![BrepLoop::try_new(BrepLoopType::Outer, trims)?],
            )?);
        }
        output.push(StepPlanarShell {
            source_shell_id: id,
            brep: Brep::try_new(vertices, edges, faces, tolerance)?,
        });
    }
    Ok(output)
}
