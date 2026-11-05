//! Command-specific cap frame, profile-first walls and sampled path charts.
use super::*;
use crate::{CurveContinuity, CurveRef};
impl Brep {
    /// A capped command extrusion. The producer returns an exact surface whose
    /// U direction runs from the supplied reversed profile to the apex. Native
    /// builds can delegate this geometry to usd-brep; topology stays shared.
    pub fn try_command_curve_apex_solid_with<E: From<GeometryError>>(
        curve: &NurbsCurve,
        apex: Point3,
        split: bool,
        tolerance: Tolerance,
        wall: &mut impl FnMut(&NurbsCurve, Point3) -> Result<NurbsSurface, E>,
    ) -> Result<Self, E> {
        if !curve.is_closed()? {
            return Err(GeometryError::InvalidCappedExtrusionProfile.into());
        }
        let frame = CurveRef::NurbsCurve(curve)
            .supporting_plane_frame(tolerance)?
            .ok_or(GeometryError::InvalidCappedExtrusionProfile)?;
        let distance = frame
            .origin()
            .vector_to(apex)?
            .dot(frame.z_axis().as_vector())?;
        if distance.abs() <= tolerance.absolute() {
            return Err(GeometryError::CoplanarCappedExtrusion.into());
        }
        let bezier = curve
            .clamped_to_active_domain()?
            .try_trimmed_with_normalized_end_weights(curve.domain())?;
        let (projected, residual) = project_curve_to_frame(&bezier, frame, tolerance)?;
        let mut bounds = [[Real::INFINITY, Real::NEG_INFINITY]; 2];
        for control in projected.control_points() {
            for (i, v) in control.point().to_array().into_iter().enumerate() {
                bounds[i][0] = bounds[i][0].min(v);
                bounds[i][1] = bounds[i][1].max(v);
            }
        }
        let cap = planar_cap_surface(frame, Vector3::try_new(0., 0., 0.)?, bounds)?;
        let cuts = if split {
            curve.discontinuity_parameters(CurveContinuity::G2, 1_f64.to_radians())?
        } else {
            Vec::new()
        };
        if cuts.len() > 4095 {
            return Err(GeometryError::SurfaceGridResourceLimit { maximum: 4096 }.into());
        }
        let mut stops = vec![*curve.domain().start()];
        stops.extend(cuts.iter().copied());
        stops.push(*curve.domain().end());
        let mut pieces = Vec::with_capacity(stops.len() - 1);
        partition(curve.clone(), &stops, &NurbsCurve::try_trimmed, &mut pieces)?;
        let mut cap_pieces = Vec::with_capacity(pieces.len());
        partition(
            projected,
            &stops,
            &NurbsCurve2::try_trimmed,
            &mut cap_pieces,
        )?;
        let count = pieces.len();
        let tip = count;
        let mut vertices = pieces
            .iter()
            .map(|c| BrepVertex::try_new(c.evaluate(*c.domain().start())?, residual))
            .collect::<Result<Vec<_>, GeometryError>>()?;
        vertices.push(BrepVertex::try_new(apex, 0.)?);
        let mut edges = pieces
            .iter()
            .enumerate()
            .map(|(i, c)| BrepEdge::try_new([i, (i + 1) % count], c.clone(), residual))
            .collect::<Result<Vec<_>, _>>()?;
        let mut cap_trims = Vec::new();
        for (i, trim) in cap_pieces.into_iter().enumerate() {
            cap_trims.push(BrepTrim::try_new(
                [i, (i + 1) % count],
                Some(i),
                false,
                trim.clone(),
                BrepTrimType::Mated,
                trim_iso::classify(&trim, &cap),
                [residual; 2],
            )?);
        }
        let reversed = distance > 0.;
        let mut faces = vec![BrepFace::try_new(
            cap,
            reversed,
            vec![BrepLoop::try_new(BrepLoopType::Outer, cap_trims)?],
        )?];
        let mut radials = vec![None; count];
        for (i, piece) in pieces.iter().enumerate() {
            let mut height = 0_f64;
            for j in 0..=16 {
                height = height.max(
                    piece
                        .evaluate(piece.parameter_at(j as f64 / 16.)?)?
                        .distance_to(apex)?,
                );
            }
            let profile = piece.reversed()?;
            let surface = wall(&profile, apex)?
                .try_swapped_uv()?
                .try_reparameterized(profile.domain(), 0. ..=height)?;
            for vertex in [i, (i + 1) % count] {
                if radials[vertex].is_none() {
                    let point = vertices[vertex].point();
                    let up = vertex == 0;
                    let (endpoints, controls, domain) = if up {
                        ([vertex, tip], vec![point, apex], 0. ..=height)
                    } else {
                        ([tip, vertex], vec![apex, point], -height..=0.)
                    };
                    let curve = NurbsCurve::try_new(
                        1,
                        controls,
                        vec![
                            *domain.start(),
                            *domain.start(),
                            *domain.end(),
                            *domain.end(),
                        ],
                    )?;
                    radials[vertex] = Some(edges.len());
                    edges.push(BrepEdge::try_new(endpoints, curve, 0.)?);
                }
            }
            let next = (i + 1) % count;
            let kind = if count == 1 {
                BrepTrimType::Seam
            } else {
                BrepTrimType::Mated
            };
            let specs = [
                RectangularTrimSpec::edge([next, i], i, true, BrepTrimType::Mated),
                RectangularTrimSpec::edge([i, tip], radials[i].unwrap(), i != 0, kind),
                RectangularTrimSpec::singular(tip),
                RectangularTrimSpec::edge([tip, next], radials[next].unwrap(), next == 0, kind),
            ];
            let mut boundary = rectangular_surface_loop(&surface, specs)?;
            // Rhino uses the corresponding surface-axis interval for all four
            // p-curves, including north/west traversal in the reverse sense.
            for (side, trim) in boundary.trims.iter_mut().enumerate() {
                let domain = if side % 2 == 0 {
                    surface.domain_u()
                } else {
                    surface.domain_v()
                };
                trim.curve = NurbsCurve2::try_new_rational(
                    1,
                    trim.curve.control_points().to_vec(),
                    vec![
                        *domain.start(),
                        *domain.start(),
                        *domain.end(),
                        *domain.end(),
                    ],
                )?;
            }
            faces.push(BrepFace::try_new(surface, reversed, vec![boundary])?);
        }
        Ok(Self::try_new(vertices, edges, faces, tolerance)?)
    }
}

// Balanced extraction keeps total copied control data O(n log k) for k pieces,
// instead of trimming every interval from the complete source control net.
pub(super) fn partition<T>(
    curve: T,
    stops: &[Real],
    trim: &impl Fn(&T, std::ops::RangeInclusive<Real>) -> Result<T, GeometryError>,
    pieces: &mut Vec<T>,
) -> Result<(), GeometryError> {
    if stops.len() == 2 {
        pieces.push(curve);
        return Ok(());
    }
    let middle = stops.len() / 2;
    let left = trim(&curve, stops[0]..=stops[middle])?;
    let right = trim(&curve, stops[middle]..=*stops.last().unwrap())?;
    partition(left, &stops[..=middle], trim, pieces)?;
    partition(right, &stops[middle..], trim, pieces)
}
