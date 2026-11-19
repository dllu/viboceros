//! Construct the section of a shallow apex sweep on a planar boundary.
use super::*;
use crate::{
    CurveBrepIntersectionEvent, curve_brep_intersection_events,
    intersection::curve_plane_root_parameters,
};

mod apertures;

const INCOMPLETE_SECTION: &str =
    "the boundary must span the complete extrusion between the source and the apex";
const PROJECTIVE_POLE: &str = "the projected source carrier crosses the apex plane";

impl Brep {
    /// Trim a coplanar or very shallow sweep to a picked planar face. Perspective
    /// projection retains the source's rim curves and p-curves on the section.
    /// Returns `None` if the native volumetric operation should handle the cut.
    pub fn try_shallow_apex_to_planar_boundary(
        &self,
        apex: Point3,
        boundary: &Self,
        split_at_tangents: bool,
        tolerance: Tolerance,
    ) -> Result<Option<Self>, GeometryError> {
        let Some((source_plane, height)) = self.shallow_apex_plane(apex, tolerance)? else {
            return Ok(None);
        };
        let [boundary_face] = boundary.faces.as_slice() else {
            return Ok(None);
        };
        let Some(plane) = boundary_face.surface.plane(tolerance)? else {
            return Ok(None);
        };
        let distance = plane.signed_distance_to(apex)?;
        if distance.abs() <= tolerance.absolute() {
            return invalid(INCOMPLETE_SECTION);
        }
        let projection = SectionProjection {
            plane,
            apex,
            distance,
        };

        let mut sweep = self.with_apex_wall_splits(split_at_tangents, tolerance)?;
        let edge_uses = sweep.edge_use_counts();
        let vertex_count = sweep.vertices.len();
        let edge_count = sweep.edges.len();
        let cuts = sweep
            .edges
            .iter()
            .zip(&edge_uses)
            .map(|(edge, &uses)| {
                if uses == 1 {
                    projection.check_source_side(&edge.curve, tolerance)?;
                }
                let cut = projection.curve(&edge.curve)?;
                if uses == 1 && !covers_curve(boundary, &cut, tolerance)? {
                    return invalid(INCOMPLETE_SECTION);
                }
                BrepEdge::try_new(
                    edge.vertices.map(|vertex| vertex + vertex_count),
                    cut,
                    edge.tolerance,
                )
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let mut cap = projected_cap(&sweep, &projection, tolerance)?;
        let mut base = sweep.faces.pop().expect("one source face");
        base.reversed = height >= 0.;

        for index in 0..vertex_count {
            let vertex = sweep.vertices[index];
            let point = projection.point(vertex.point)?;
            sweep
                .vertices
                .push(BrepVertex::try_new(point, vertex.tolerance)?);
        }
        sweep.edges.extend(cuts);
        cap.reversed = !base.reversed;
        for trim in cap
            .loops
            .iter_mut()
            .flat_map(|boundary| &mut boundary.trims)
        {
            trim.vertices = trim.vertices.map(|vertex| vertex + vertex_count);
            trim.edge = trim.edge.map(|edge| edge + edge_count);
        }
        if height != 0. {
            let inverse = SectionProjection {
                plane: source_plane,
                apex,
                distance: height,
            };
            let holes = apertures::enclosed(&self.faces[0], boundary, inverse, tolerance)?;
            if !apertures::append(&mut sweep, &mut cap, boundary, &holes, tolerance)? {
                return Ok(None);
            }
        }

        let mut radials = vec![None; vertex_count];
        let mut faces = Vec::new();
        for trim in base.loops.iter().flat_map(|boundary| &boundary.trims) {
            let Some(index) = trim.edge.filter(|&edge| edge_uses[edge] == 1) else {
                continue;
            };
            let cut = index + edge_count;
            let [start, end] = sweep.edges[index].vertices;
            let [cut_start, cut_end] = sweep.edges[cut].vertices;
            let wall = NurbsSurface::try_ruled_from_curves(
                &sweep.edges[index].curve,
                &sweep.edges[cut].curve,
            )?;
            for vertex in [start, end] {
                if radials[vertex].is_none() {
                    let cut_vertex = vertex + vertex_count;
                    radials[vertex] = Some(sweep.edges.len());
                    sweep.edges.push(BrepEdge::try_new(
                        [vertex, cut_vertex],
                        NurbsCurve::try_new(
                            1,
                            vec![
                                sweep.vertices[vertex].point,
                                sweep.vertices[cut_vertex].point,
                            ],
                            vec![0., 0., 1., 1.],
                        )?,
                        sweep.vertices[vertex]
                            .tolerance
                            .max(sweep.vertices[cut_vertex].tolerance),
                    )?);
                }
            }
            let wall_loop = rectangular_surface_loop(
                &wall,
                [
                    RectangularTrimSpec::edge(
                        [start, cut_start],
                        radials[start].unwrap(),
                        false,
                        BrepTrimType::Mated,
                    ),
                    RectangularTrimSpec::edge(
                        [cut_start, cut_end],
                        cut,
                        false,
                        BrepTrimType::Mated,
                    ),
                    RectangularTrimSpec::edge(
                        [cut_end, end],
                        radials[end].unwrap(),
                        true,
                        BrepTrimType::Mated,
                    ),
                    RectangularTrimSpec::edge([end, start], index, true, BrepTrimType::Mated),
                ],
            )?;
            faces.push(BrepFace::try_new(
                wall,
                trim.reversed_3d ^ base.reversed,
                vec![wall_loop],
            )?);
        }
        faces.insert(0, base);
        faces.push(cap);
        let subset = sweep.compact_face_topology(faces);
        Ok(Some(Self::try_new(
            subset.vertices,
            subset.edges,
            subset.faces,
            tolerance,
        )?))
    }
}

struct SectionProjection {
    plane: Plane,
    apex: Point3,
    distance: Real,
}

impl SectionProjection {
    fn check_source_side(
        &self,
        curve: &NurbsCurve,
        tolerance: Tolerance,
    ) -> Result<(), GeometryError> {
        let opposite = |point| {
            let distance = self.plane.signed_distance_to(point)?;
            Ok::<_, GeometryError>(
                distance.abs() > tolerance.absolute()
                    && distance.is_sign_positive() != self.distance.is_sign_positive(),
            )
        };
        if !opposite(curve.evaluate(*curve.domain().start())?)? {
            return invalid(INCOMPLETE_SECTION);
        }
        let weight_sign = curve.control_points()[0].weight().is_sign_positive();
        for control in curve.control_points() {
            // A same-sign hull on the source side certifies most inputs.
            if control.weight().is_sign_positive() != weight_sign || !opposite(control.point())? {
                let (roots, overlap) = curve_plane_root_parameters(curve, self.plane)?;
                if overlap || !roots.is_empty() {
                    return invalid(INCOMPLETE_SECTION);
                }
                break;
            }
        }
        Ok(())
    }

    fn weight_factor(&self, point: Point3) -> Result<Real, GeometryError> {
        let factor = 1. - self.plane.signed_distance_to(point)? / self.distance;
        if !factor.is_finite() || factor <= 0. {
            return invalid(PROJECTIVE_POLE);
        }
        Ok(factor)
    }

    fn point(&self, point: Point3) -> Result<Point3, GeometryError> {
        self.project(point, self.weight_factor(point)?)
    }

    fn project(&self, point: Point3, factor: Real) -> Result<Point3, GeometryError> {
        self.apex
            .translated(self.apex.vector_to(point)?.scaled(1. / factor)?)
    }

    fn controls(&self, points: &[WeightedPoint3]) -> Result<Vec<WeightedPoint3>, GeometryError> {
        // Multiplying the weights by the perspective denominator preserves
        // correspondence in the original rational curve or surface chart.
        points
            .iter()
            .map(|control| {
                let point = control.point();
                let factor = self.weight_factor(point)?;
                WeightedPoint3::try_new(self.project(point, factor)?, control.weight() * factor)
            })
            .collect()
    }

    fn curve(&self, curve: &NurbsCurve) -> Result<NurbsCurve, GeometryError> {
        NurbsCurve::try_new_rational(
            curve.degree(),
            self.controls(curve.control_points())?,
            curve.knots().to_vec(),
        )
    }

    fn surface(&self, surface: &NurbsSurface) -> Result<NurbsSurface, GeometryError> {
        NurbsSurface::try_new_rational(
            surface.degree_u(),
            surface.degree_v(),
            surface.control_point_count_u(),
            surface.control_point_count_v(),
            self.controls(surface.control_points())?,
            surface.knots_u().to_vec(),
            surface.knots_v().to_vec(),
        )
    }
}

fn projected_cap(
    source: &Brep,
    projection: &SectionProjection,
    tolerance: Tolerance,
) -> Result<BrepFace, GeometryError> {
    let mut face = source.faces[0].clone();
    face.surface = match projection.surface(&face.surface) {
        // A carrier can extend beyond the apex even when its trimmed region
        // is entirely before the cut. Restrict only the section carrier;
        // retain the exact original source surface as the base cap.
        Err(GeometryError::InvalidBrepTopology {
            context: PROJECTIVE_POLE,
        }) => {
            let mut shrunk =
                source.try_shrunk_surfaces(BrepSurfaceShrinkMode::ToEdge, tolerance)?;
            face = shrunk.faces.pop().expect("one source face");
            projection.surface(&face.surface)?
        }
        result => result?,
    };
    Ok(face)
}

fn covers_curve(
    boundary: &Brep,
    curve: &NurbsCurve,
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    let first = curve.control_points()[0];
    if curve.control_points().iter().all(|control| {
        control.weight().is_sign_positive() == first.weight().is_sign_positive()
            && control.point().is_near(first.point(), tolerance)
    }) {
        let point = first.point();
        let Some((u, v)) = boundary.closest_parameters_on_face(0, point, tolerance)? else {
            return Ok(false);
        };
        return Ok(boundary.faces[0]
            .surface
            .evaluate(u, v)?
            .is_near(point, tolerance));
    }
    let curve = curve.try_reparameterized(0. ..=1.)?;
    let mut end = 0.0_f64;
    let rounding = 256. * Real::EPSILON;
    for event in curve_brep_intersection_events(&curve, boundary, tolerance)? {
        if let CurveBrepIntersectionEvent::Overlap(overlap) = event {
            if overlap.start().curve_parameter() > end + rounding {
                return Ok(false);
            }
            end = end.max(overlap.end().curve_parameter());
        }
    }
    Ok(end >= 1. - rounding)
}
