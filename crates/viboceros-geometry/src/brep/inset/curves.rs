//! Smooth planar trim loops, analytic offsets and shared NURBS fitting.
use super::*;
use crate::{CurveSegment3, ParameterSide, PolyCurve3};
mod collinear;
mod groups;
mod producer;
use producer::{aligned, offset, shifted_start};

fn oriented(brep: &Brep, trim: &BrepTrim) -> Result<NurbsCurve, GeometryError> {
    let edge = trim
        .edge
        .ok_or_else(|| invalid("inset does not support poles"))?;
    if !matches!(trim.trim_type, BrepTrimType::Boundary | BrepTrimType::Mated) {
        return Err(invalid("inset does not support seam trims"));
    }
    if trim.reversed_3d {
        brep.edges[edge].curve.reversed()
    } else {
        Ok(brep.edges[edge].curve.clone())
    }
}
pub(super) fn joined(brep: &Brep, trims: &[BrepTrim]) -> Result<NurbsCurve, GeometryError> {
    let segments = trims
        .iter()
        .map(|t| oriented(brep, t).map(CurveSegment3::NurbsCurve))
        .collect::<Result<Vec<_>, GeometryError>>()?;
    PolyCurve3::try_new(segments)?.to_nurbs()
}

fn gap(a: UnitVector3, b: UnitVector3) -> Result<Real, GeometryError> {
    Ok(a.as_vector()
        .cross(b.as_vector())?
        .length()?
        .atan2(a.as_vector().dot(b.as_vector())?))
}

impl Brep {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn inset_smooth_boundary(
        &mut self,
        face: &BrepFace,
        boundary: &BrepLoop,
        normal: UnitVector3,
        options: PlanarInsetOptions,
        tolerance: Tolerance,
        operations: PlanarInsetOperations,
        work: &mut Work,
    ) -> Result<Boundary, GeometryError> {
        let PlanarInsetOptions {
            distance,
            crease_corners: crease,
            loose,
        } = options;
        let old = &boundary.trims;
        let groups = groups::collect(self, old, loose, crease, tolerance)?;
        let sources = groups.iter().map(|g| &g.curve).collect::<Vec<_>>();
        for (j, curve) in sources.iter().enumerate() {
            let next = &sources[(j + 1) % sources.len()];
            if curve.has_full_multiplicity_kink(tolerance)?
                || gap(
                    curve.tangent_at_on_side(*curve.domain().end(), ParameterSide::Left)?,
                    next.tangent_at_on_side(*next.domain().start(), ParameterSide::Right)?,
                )? > tolerance.angular()
            {
                return Err(invalid(
                    "curved inset currently requires smooth boundary joins",
                ));
            }
        }
        let goals = sources
            .iter()
            .map(|c| shifted_start(c, normal, distance))
            .collect::<Result<Vec<_>, _>>()?;
        let mut offsets = Vec::with_capacity(sources.len());
        for (j, source) in sources.iter().enumerate() {
            let (curve, preserve_ends) =
                match offset(source, normal, distance, tolerance, loose, operations) {
                    Ok(result) => result,
                    Err(GeometryError::Degenerate {
                        context: "NURBS offset cusp",
                    }) if !loose && !crease && sources.len() > 1 => {
                        // A cusp can cross original edge boundaries. Offset the
                        // complete smooth loop before its reversed ears are cut.
                        offsets.clear();
                        let full = joined(self, old)?;
                        let (curve, _) =
                            offset(&full, normal, distance, tolerance, false, operations)?;
                        work.charge_curve(&curve)?;
                        offsets.push(curve);
                        break;
                    }
                    Err(error) => return Err(error),
                };
            // A periodic Greville window need not include the seam parameter.
            // Its cyclic interpolation closes naturally; forcing normal-shot
            // endpoints would change that interpolated curve.
            let curve = if !preserve_ends || loose && source.is_periodic() {
                curve
            } else {
                aligned(curve, goals[j], goals[(j + 1) % goals.len()], tolerance)?
            };
            work.charge_curve(&curve)?;
            offsets.push(curve);
        }
        let strips = crease && groups.len() > 1;
        if !strips && offsets.len() > 1 {
            let joined = PolyCurve3::try_new(
                offsets
                    .into_iter()
                    .map(CurveSegment3::NurbsCurve)
                    .collect::<Vec<_>>(),
            )?
            .to_nurbs()?;
            work.charge_curve(&joined)?;
            offsets = vec![joined];
        }
        let offsets = if strips {
            collinear::simplify(offsets, tolerance)?
        } else {
            offsets.into_iter().map(Some).collect()
        };
        let mut group_starts = Vec::with_capacity(offsets.len());
        let mut pieces = Vec::new();
        let kink_tolerance = Tolerance::try_new(
            tolerance.absolute(),
            tolerance.relative(),
            std::f64::consts::PI / 180.,
        )?;
        for curve in offsets {
            group_starts.push(pieces.len());
            let Some(curve) = curve else {
                continue;
            };
            let kinks = curve.full_multiplicity_kink_parameters(kink_tolerance)?;
            if kinks.is_empty() {
                pieces.push(curve);
            } else {
                // B-rep splitting retains the stored seam as a vertex, even
                // when it is smooth. The curve split API instead merges the
                // two seam-adjacent pieces of a closed curve.
                let mut remainder = curve;
                for parameter in kinks {
                    let (piece, right) = remainder.try_split(parameter)?;
                    work.charge_curve(&piece)?;
                    pieces.push(piece);
                    remainder = right;
                }
                work.charge_curve(&remainder)?;
                pieces.push(remainder);
            }
        }
        let offsets = pieces;
        let vertex_start = self.vertices.len();
        for curve in &offsets {
            self.vertices.push(BrepVertex::try_new(
                curve.evaluate(*curve.domain().start())?,
                0.,
            )?);
        }
        let mut inset = Vec::with_capacity(offsets.len());
        let count = offsets.len();
        for (j, curve) in offsets.into_iter().enumerate() {
            let uv = face.surface.try_pullback_affine_curve(&curve, tolerance)?;
            let vertices = [vertex_start + j, vertex_start + (j + 1) % count];
            let edge = self.edges.len();
            self.edges.push(BrepEdge::try_new(vertices, curve, 0.)?);
            let iso = trim_iso::classify(&uv, &face.surface);
            inset.push(BrepTrim::try_new(
                vertices,
                Some(edge),
                false,
                uv,
                BrepTrimType::Mated,
                iso,
                [0.; 2],
            )?);
        }
        let mut borders = Vec::new();
        if strips {
            let mut connectors = Vec::with_capacity(groups.len());
            for (j, group) in groups.iter().enumerate() {
                let trim = &group.trims[0];
                let cap = vertex_start + group_starts[j] % count;
                connectors.push(self.inset_line_trim(
                    &face.surface,
                    [trim.vertices[0], cap],
                    [
                        self.vertices[trim.vertices[0]].point,
                        self.vertices[cap].point,
                    ],
                    tolerance,
                )?);
            }
            for (j, group) in groups.iter().enumerate() {
                let k = (j + 1) % groups.len();
                let end = group_starts.get(j + 1).copied().unwrap_or(inset.len());
                let mut trims = group.trims.clone();
                trims.push(connectors[k].clone());
                for trim in inset[group_starts[j]..end].iter().rev() {
                    trims.push(reverse_trim(trim)?);
                }
                trims.push(reverse_trim(&connectors[j])?);
                borders.push(BrepFace::try_new(
                    face.surface.clone(),
                    face.reversed,
                    vec![BrepLoop::try_new(BrepLoopType::Outer, trims)?],
                )?);
            }
        } else {
            let reversed = inset
                .iter()
                .rev()
                .map(reverse_trim)
                .collect::<Result<Vec<_>, _>>()?;
            let (outer, inner) = if boundary.loop_type == BrepLoopType::Outer {
                (old.clone(), reversed)
            } else {
                (reversed, old.clone())
            };
            borders.push(BrepFace::try_new(
                face.surface.clone(),
                face.reversed,
                vec![
                    BrepLoop::try_new(BrepLoopType::Outer, outer)?,
                    BrepLoop::try_new(BrepLoopType::Inner, inner)?,
                ],
            )?);
        }
        Ok(Boundary { inset, borders })
    }
}
