//! Directed wall cuts and the base-capped stock left by a failed solid cut.
use super::*;
use viboceros_geometry::{
    BrepLoop, BrepTrim, BrepTrimType, CurveBrepIntersectionEvent, LineSegment,
};
use viboceros_smlib::KernelCurve;

mod planar_split;

#[derive(Clone, Copy)]
pub(in crate::tapered_boundary) struct WallInput<'a> {
    pub(in crate::tapered_boundary) curve: &'a NurbsCurve,
    pub(in crate::tapered_boundary) kernel: &'a KernelCurve,
    pub(in crate::tapered_boundary) draft: f64,
    pub(in crate::tapered_boundary) preview_distance: Option<f64>,
}

impl WallInput<'_> {
    fn displacement(self, stock: Vector3) -> Result<Vector3, GeometryError> {
        self.preview_distance.map_or(Ok(stock), |length| {
            stock.normalized_nonzero()?.as_vector().scaled(length)
        })
    }

    fn stock(
        self,
        displacement: Vector3,
        cap: bool,
        tolerance: Tolerance,
    ) -> Result<Brep, CommandError> {
        if displacement.length()? <= tolerance.absolute() {
            return Err(REJECTED);
        }
        match Shape::directed_oblique_taper(self.kernel, displacement, self.draft, cap, tolerance) {
            Ok(stock) => Ok(stock.to_brep(tolerance)?),
            Err(viboceros_smlib::Error::Kernel(_)) => Err(REJECTED),
            Err(error) => Err(error.into()),
        }
    }
}

pub(in crate::tapered_boundary) fn trim_open(
    input: WallInput<'_>,
    cutter: &Brep,
    displacement: Vector3,
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    let displacement = input.displacement(displacement)?;
    let stock = input.stock(displacement, false, tolerance)?;
    let components = stock.edge_connected_face_components();
    let component = components.first().ok_or(REJECTED)?;
    let surface = cutter.faces()[0].surface();
    let plane = surface.plane(tolerance)?;
    if let Some(plane) = plane
        && components.len() == stock.faces().len()
        && input.curve.degree() == 1
        && stock.faces().iter().all(|face| {
            let wall = face.surface();
            wall.degree_u() == 1
                && wall.degree_v() == 1
                && wall.control_point_count_u() == 2
                && wall.control_point_count_v() == 2
                && !wall.is_rational()
        })
    {
        return planar_split::retain(
            &stock,
            cutter,
            plane,
            displacement,
            input.curve.is_closed()?,
            tolerance,
        );
    }
    if plane.is_none() && !regular_projected_patch(surface, displacement, tolerance)? {
        return Err(REJECTED);
    }
    let section =
        |wall: &BrepFace| wall_section(wall.surface(), cutter, plane, displacement, tolerance);
    let first = stock.sub_brep(component, tolerance)?;
    let Some(leading) = section(&stock.faces()[0])? else {
        return if plane.is_some() && input.curve.degree() == 1 && component.len() == 1 {
            Ok(first)
        } else {
            Err(REJECTED)
        };
    };
    let mut sections = Vec::with_capacity(stock.faces().len());
    sections.push(leading);
    for wall in &stock.faces()[1..] {
        sections.push(section(wall)?.ok_or(REJECTED)?);
    }
    clip_component(
        &first,
        cutter,
        displacement,
        component.iter().flat_map(|&i| sections[i]),
        tolerance,
    )
}

fn clip_component(
    source: &Brep,
    cutter: &Brep,
    displacement: Vector3,
    section: impl Iterator<Item = Point3>,
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    let tool = boundary_tool(cutter, displacement.scaled(-2.)?, tolerance)?;
    let cut = Shape::from_brep(source, tolerance)?
        .trimmed_inside(&tool)?
        .ok_or(REJECTED)?
        .to_brep(tolerance)?;
    if cut.faces().len() != source.faces().len()
        || cut.edge_connected_face_components().len() != 1
        || cut.vertices().len() != source.vertices().len()
        || cut.edges().len() != source.edges().len()
        || !retains_section_vertices(&cut, section, tolerance)
    {
        return Err(REJECTED);
    }
    Ok(cut)
}

fn retains_section_vertices(
    cut: &Brep,
    section: impl Iterator<Item = Point3>,
    tolerance: Tolerance,
) -> bool {
    let bounds = cut.bounds();
    let min = bounds.min().to_array();
    let max = bounds.max().to_array();
    let axis = (0..3)
        .max_by(|&i, &j| (max[i] - min[i]).total_cmp(&(max[j] - min[j])))
        .unwrap();
    let axis_value = |point: Point3| point.to_array()[axis];
    let scale = min
        .into_iter()
        .chain(max)
        .fold(1_f64, |scale, v| scale.max(v.abs()));
    let span = tolerance.absolute().max(tolerance.relative() * scale) * 2.;
    let mut vertices = cut.vertices().iter().map(|v| v.point()).collect::<Vec<_>>();
    vertices.sort_unstable_by(|&a, &b| axis_value(a).total_cmp(&axis_value(b)));
    section.into_iter().all(|point| {
        let coordinate = axis_value(point);
        let first = vertices.partition_point(|&v| axis_value(v) < coordinate - span);
        vertices[first..]
            .iter()
            .take_while(|&&v| axis_value(v) <= coordinate + span)
            .any(|&v| point.is_near(v, tolerance))
    })
}

/// Return the base ends and the corresponding hits on the finite cutter.
/// The native ruled wall runs across the sweep in U, with the original
/// traversal in V. Monotonic rails along the profile chord exclude a
/// longitudinal reversal anywhere in the nonrational ruled strip.
fn wall_section(
    wall: &NurbsSurface,
    cutter: &Brep,
    plane: Option<Plane>,
    displacement: Vector3,
    tolerance: Tolerance,
) -> Result<Option<[Point3; 4]>, CommandError> {
    if wall.degree_u() != 1 || wall.control_point_count_u() != 2 || wall.is_rational() {
        return Err(REJECTED);
    }
    let u = wall.domain_u();
    let v = wall.domain_v();
    let start = wall.evaluate(*u.start(), *v.start())?;
    let direction = start
        .direction_to(wall.evaluate(*u.start(), *v.end())?)?
        .as_vector();
    let planar_cut = plane
        .map(|plane| {
            Ok::<_, GeometryError>((
                plane,
                displacement.dot(plane.normal().as_vector())?.signum(),
            ))
        })
        .transpose()?;
    for i in 0..2 {
        let sign = if i == 0 { -1. } else { 1. };
        let mut controls =
            (0..wall.control_point_count_v()).map(|v| wall.control_point(i, v).unwrap().point());
        let mut previous = None;
        for point in &mut controls {
            if let Some(prior) = previous
                && point.vector_to(prior)?.dot(direction)? > 0.
            {
                return Err(REJECTED);
            }
            if let Some((plane, span)) = planar_cut
                && plane.signed_distance_to(point)? * span * sign <= tolerance.absolute()
            {
                return Err(REJECTED);
            }
            previous = Some(point);
        }
    }
    let mut ends = [start; 4];
    let mut covered = true;
    for (i, v) in [*v.start(), *v.end()].into_iter().enumerate() {
        let start = wall.evaluate(*u.start(), v)?;
        let end = wall.evaluate(*u.end(), v)?;
        ends[i] = start;
        let hit = if let Some((plane, sign)) = planar_cut {
            let a = plane.signed_distance_to(start)? * sign;
            let b = plane.signed_distance_to(end)? * sign;
            let point = start.translated(start.vector_to(end)?.scaled(a / (a - b))?)?;
            let face = &cutter.faces()[0];
            let (u, v) = face.surface().closest_parameters(point, tolerance)?;
            (face.surface().evaluate(u, v)?.is_near(point, tolerance)
                && face.contains_parameters(u, v, tolerance)?)
            .then_some(point)
        } else {
            let ruling = LineSegment::try_new(start, end, tolerance)?.to_nurbs()?;
            let events =
                viboceros_smlib::curve_brep_intersection_events(&ruling, cutter, None, tolerance)?;
            let [CurveBrepIntersectionEvent::Point(hit)] = events.as_slice() else {
                return Ok(None);
            };
            let point = hit.point();
            (!point.is_near(start, tolerance) && !point.is_near(end, tolerance)).then_some(point)
        };
        if let Some(hit) = hit {
            ends[i + 2] = hit;
        } else {
            covered = false;
        }
    }
    Ok(covered.then_some(ends))
}

pub(super) fn uncut_stock(
    input: WallInput<'_>,
    source: &Brep,
    cutter: &Brep,
    displacement: Vector3,
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    if input.curve.degree() != 1 {
        return Err(REJECTED);
    }
    let displacement = input.displacement(displacement)?;
    let stock = input.stock(displacement, true, tolerance)?;
    let source_face = &source.faces()[0];
    let surface = source_face.surface();
    let base = surface.plane(tolerance)?.ok_or(REJECTED)?;
    check_stock_reaches_cutter(&stock, source, base, cutter, displacement, tolerance)?;
    let normal = surface.normal_at(surface.parameter_at_u(0.5)?, surface.parameter_at_v(0.5)?)?;
    let reversed = source_face.loops()[0].trims()[0].is_reversed_3d();
    if (normal.as_vector().dot(displacement)? > 0.) == reversed {
        return Ok(stock);
    }
    // With the base facing away from a following pull, Rhino's failed solid
    // cut retains one attached base edge at the second profile segment.
    let (a, b) = input.curve.spans().nth(1).ok_or(REJECTED)?;
    let ends = [input.curve.evaluate(a)?, input.curve.evaluate(b)?];
    let cap = stock
        .faces()
        .iter()
        .position(|face| on_plane(face.surface(), base, tolerance))
        .ok_or(REJECTED)?;
    let base_edges = stock.faces()[cap].loops()[0]
        .trims()
        .iter()
        .filter_map(|trim| trim.edge())
        .collect::<Vec<_>>();
    let mate = base_edges
        .iter()
        .copied()
        .find(|&i| {
            let vertices = stock.edges()[i]
                .vertices()
                .map(|v| stock.vertices()[v].point());
            (vertices[0].is_near(ends[0], tolerance) && vertices[1].is_near(ends[1], tolerance))
                || (vertices[0].is_near(ends[1], tolerance)
                    && vertices[1].is_near(ends[0], tolerance))
        })
        .ok_or(REJECTED)?;
    separate_stock_cap(&stock, cap, &base_edges, mate, tolerance)
}

fn check_stock_reaches_cutter(
    stock: &Brep,
    source: &Brep,
    base: Plane,
    cutter: &Brep,
    displacement: Vector3,
    tolerance: Tolerance,
) -> Result<(), CommandError> {
    let support = cutter.faces()[0].surface();
    if let Some(plane) = support.plane(tolerance)? {
        let sign = displacement.dot(plane.normal().as_vector())?.signum();
        let before = source.edges().iter().all(|edge| {
            edge.curve().control_points().iter().all(|control| {
                plane
                    .signed_distance_to(control.point())
                    .is_ok_and(|distance| distance * sign < -tolerance.absolute())
            })
        });
        let reached = stock.vertices().iter().any(|vertex| {
            plane
                .signed_distance_to(vertex.point())
                .is_ok_and(|distance| distance * sign > tolerance.absolute())
        });
        return if before && reached {
            Ok(())
        } else {
            Err(REJECTED)
        };
    }
    if regular_projected_patch(support, displacement, tolerance)? {
        for face in stock.faces() {
            if !on_plane(face.surface(), base, tolerance)
                && wall_section(face.surface(), cutter, None, displacement, tolerance)?.is_some()
            {
                return Ok(());
            }
        }
    }
    Err(REJECTED)
}

fn separate_stock_cap(
    stock: &Brep,
    cap: usize,
    base_edges: &[usize],
    mate: usize,
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    let mut vertices = stock.vertices().to_vec();
    let mut remap = (0..vertices.len()).collect::<Vec<_>>();
    let attached = stock.edges()[mate].vertices();
    for v in base_edges.iter().flat_map(|&i| stock.edges()[i].vertices()) {
        if !attached.contains(&v) && remap[v] == v {
            remap[v] = vertices.len();
            vertices.push(stock.vertices()[v]);
        }
    }
    let mut copy = vec![None; stock.edges().len()];
    let mut edges = stock.edges().to_vec();
    for &i in base_edges {
        if i != mate {
            copy[i] = Some(edges.len());
            let edge = &stock.edges()[i];
            edges.push(
                edge.clone()
                    .with_vertices(edge.vertices().map(|v| remap[v])),
            );
        }
    }
    let mut is_base = vec![false; stock.edges().len()];
    for &edge in base_edges {
        is_base[edge] = true;
    }
    for (i, edge) in edges[..stock.edges().len()].iter_mut().enumerate() {
        if !is_base[i] {
            *edge = edge
                .clone()
                .with_vertices(edge.vertices().map(|v| remap[v]));
        }
    }
    let mut faces = Vec::with_capacity(stock.faces().len());
    for (i, face) in stock.faces().iter().enumerate() {
        let mut loops = Vec::with_capacity(face.loops().len());
        for boundary in face.loops() {
            let mut trims = Vec::with_capacity(boundary.trims().len());
            for trim in boundary.trims() {
                let edge = trim.edge().ok_or(REJECTED)?;
                let verts = if i == cap {
                    trim.vertices()
                } else {
                    trim.vertices().map(|v| remap[v])
                };
                trims.push(BrepTrim::try_new(
                    verts,
                    Some(if i == cap {
                        edge
                    } else {
                        copy[edge].unwrap_or(edge)
                    }),
                    trim.is_reversed_3d(),
                    trim.curve().clone(),
                    if copy[edge].is_some() {
                        BrepTrimType::Boundary
                    } else {
                        trim.trim_type()
                    },
                    trim.iso(),
                    trim.tolerance(),
                )?);
            }
            loops.push(BrepLoop::try_new(boundary.loop_type(), trims)?);
        }
        faces.push(BrepFace::try_new(
            face.surface().clone(),
            face.is_reversed(),
            loops,
        )?);
    }
    Ok(Brep::try_new(vertices, edges, faces, tolerance)?)
}

#[cfg(test)]
mod tests;
