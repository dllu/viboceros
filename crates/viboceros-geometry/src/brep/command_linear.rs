//! Fresh straight-sweep topology with command rim senses and cap ordering.
use super::*;
#[cfg(test)]
mod tests;
mod weights;
use crate::{CurveContinuity, CurveRef};
impl Brep {
    /// Natural straight-extrusion sheet. Both profile edges follow increasing
    /// U; the west ruling follows the negated path interval. UV loops keep
    /// their surface-axis intervals independently of spatial edge direction.
    pub fn try_command_extrusion_sheet(
        surface: NurbsSurface,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let mut brep = Self::try_surface_face_with_native_edge_parameters(surface, tolerance)?;
        if let Some(edge) = brep.faces[0].loops[0].trims[2].edge {
            brep.edges[edge].curve = brep.edges[edge].curve.reversed()?;
            brep.edges[edge].vertices.reverse();
            for trim in brep.faces[0].loops[0]
                .trims
                .iter_mut()
                .filter(|t| t.edge == Some(edge))
            {
                trim.reversed_3d = !trim.reversed_3d;
            }
        }
        brep.validate(tolerance)?;
        Ok(brep)
    }
    /// Capped G1 profile-first construction; apply G2 to the completed walls.
    /// The exact producer retains profile U
    /// and uses physical path length in V. Geometry never escapes on failure.
    pub fn try_command_linear_solid_with<E: From<GeometryError>>(
        curve: &NurbsCurve,
        start: Vector3,
        end: Vector3,
        split: bool,
        tolerance: Tolerance,
        wall: &mut impl FnMut(&NurbsCurve, Vector3, Vector3) -> Result<NurbsSurface, E>,
    ) -> Result<Self, E> {
        if !curve.is_closed()? {
            return Err(GeometryError::InvalidCappedExtrusionProfile.into());
        }
        let frame = CurveRef::NurbsCurve(curve)
            .supporting_plane_frame(tolerance)?
            .ok_or(GeometryError::InvalidCappedExtrusionProfile)?;
        let path = Vector3::try_from(std::array::from_fn(|i| {
            end.to_array()[i] - start.to_array()[i]
        }))?;
        let normal_distance = path.dot(frame.z_axis().as_vector())?;
        if normal_distance.abs() <= tolerance.absolute() {
            return Err(GeometryError::CoplanarCappedExtrusion.into());
        }
        let height = path.length()?;
        let bezier = curve
            .clamped_to_active_domain()?
            .try_trimmed_with_normalized_end_weights(curve.domain())?;
        let (projected, residual) = project_curve_to_frame(&bezier, frame, tolerance)?;
        let mut bounds = [[Real::INFINITY, Real::NEG_INFINITY]; 2];
        for c in projected.control_points() {
            for (i, v) in c.point().to_array().into_iter().enumerate() {
                bounds[i][0] = bounds[i][0].min(v);
                bounds[i][1] = bounds[i][1].max(v);
            }
        }
        let caps = [
            planar_cap_surface(frame, start, bounds)?,
            planar_cap_surface(frame, end, bounds)?,
        ];
        let cuts = if split {
            curve.discontinuity_parameters(CurveContinuity::G1, 1_f64.to_radians())?
        } else {
            Vec::new()
        };
        if cuts.len() > 4095 {
            return Err(GeometryError::SurfaceGridResourceLimit { maximum: 4096 }.into());
        }
        let mut stops = vec![*curve.domain().start()];
        stops.extend(cuts);
        stops.push(*curve.domain().end());
        let mut pieces = Vec::with_capacity(stops.len() - 1);
        super::command_apex::partition(
            curve.clone(),
            &stops,
            &NurbsCurve::try_trimmed,
            &mut pieces,
        )?;
        let mut trims = Vec::with_capacity(pieces.len());
        super::command_apex::partition(projected, &stops, &NurbsCurve2::try_trimmed, &mut trims)?;
        let count = pieces.len();
        let mut vertices = Vec::with_capacity(2 * count);
        for offset in [start, end] {
            for p in &pieces {
                vertices.push(BrepVertex::try_new(
                    p.evaluate(*p.domain().start())?.translated(offset)?,
                    residual,
                )?);
            }
        }
        let mut edges = Vec::with_capacity(3 * count);
        for (level, offset) in [start, end].into_iter().enumerate() {
            for (i, p) in pieces.iter().enumerate() {
                edges.push(BrepEdge::try_new(
                    [level * count + i, level * count + (i + 1) % count],
                    p.transformed(AffineTransform3::from_translation(offset))?,
                    residual,
                )?);
            }
        }
        let cap_loop = |level: usize| -> Result<BrepLoop, GeometryError> {
            BrepLoop::try_new(
                BrepLoopType::Outer,
                trims
                    .iter()
                    .enumerate()
                    .map(|(i, c)| {
                        BrepTrim::try_new(
                            [level * count + i, level * count + (i + 1) % count],
                            Some(level * count + i),
                            false,
                            c.clone(),
                            BrepTrimType::Mated,
                            trim_iso::classify(c, &caps[level]),
                            [residual; 2],
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            )
        };
        let reverse = normal_distance > 0.;
        let mut faces = vec![BrepFace::try_new(
            caps[0].clone(),
            reverse,
            vec![cap_loop(0)?],
        )?];
        let mut verticals = vec![None; count];
        for (i, piece) in pieces.iter().enumerate() {
            let profile = piece.reversed()?;
            let surface = wall(&profile, start, end)?;
            for vertex in [i, (i + 1) % count] {
                if verticals[vertex].is_none() {
                    let up = vertex == 0;
                    let a = vertex;
                    let b = vertex + count;
                    let (ends, domain) = if up {
                        ([a, b], 0. ..=height)
                    } else {
                        ([b, a], -height..=0.)
                    };
                    let curve = NurbsCurve::try_new(
                        1,
                        ends.map(|j| vertices[j].point()).to_vec(),
                        vec![
                            *domain.start(),
                            *domain.start(),
                            *domain.end(),
                            *domain.end(),
                        ],
                    )?;
                    verticals[vertex] = Some(edges.len());
                    edges.push(BrepEdge::try_new(ends, curve, 0.)?);
                }
            }
            let next = (i + 1) % count;
            let kind = if count == 1 {
                BrepTrimType::Seam
            } else {
                BrepTrimType::Mated
            };
            let mut boundary = rectangular_surface_loop(
                &surface,
                [
                    RectangularTrimSpec::edge([next, i], i, true, BrepTrimType::Mated),
                    RectangularTrimSpec::edge([i, i + count], verticals[i].unwrap(), i != 0, kind),
                    RectangularTrimSpec::edge(
                        [i + count, next + count],
                        i + count,
                        false,
                        BrepTrimType::Mated,
                    ),
                    RectangularTrimSpec::edge(
                        [next + count, next],
                        verticals[next].unwrap(),
                        next == 0,
                        kind,
                    ),
                ],
            )?;
            for (side, t) in boundary.trims.iter_mut().enumerate() {
                let domain = if side % 2 == 0 {
                    surface.domain_u()
                } else {
                    surface.domain_v()
                };
                t.curve = NurbsCurve2::try_new_rational(
                    1,
                    t.curve.control_points().to_vec(),
                    vec![
                        *domain.start(),
                        *domain.start(),
                        *domain.end(),
                        *domain.end(),
                    ],
                )?;
            }
            faces.push(BrepFace::try_new(surface, reverse, vec![boundary])?);
        }
        faces.push(BrepFace::try_new(
            caps[1].clone(),
            !reverse,
            vec![cap_loop(1)?],
        )?);
        Ok(Self::try_new(vertices, edges, faces, tolerance)?)
    }
}
