//! Lossless local UV frames for operations that compose trims with surfaces.

use super::*;
use std::borrow::Cow;

#[cfg(test)]
pub(in crate::brep) mod tests;

pub(super) struct ParameterFrame<'a> {
    pub(super) face: Cow<'a, BrepFace>,
    pub(super) origin: [Real; 2],
}

impl BrepFace {
    pub(super) fn local_parameter_frame(&self) -> Result<ParameterFrame<'_>, GeometryError> {
        let knots = [self.surface.knots_u(), self.surface.knots_v()];
        let domains = [self.surface.domain_u(), self.surface.domain_v()];
        let origin = std::array::from_fn(|axis| {
            let domain = &domains[axis];
            // Only remove offsets from intervals lying wholly on one side of
            // zero. Centered/unit domains need no temporary representation.
            let candidate = if *domain.start() > 0. {
                *domain.start()
            } else if *domain.end() < 0. {
                *domain.end()
            } else {
                return 0.;
            };
            let controls = self
                .loops
                .iter()
                .flat_map(|boundary| &boundary.trims)
                .flat_map(|trim| trim.curve.control_points())
                .map(|c| c.point().to_array()[axis]);
            if knots[axis]
                .iter()
                .copied()
                .chain(controls)
                .all(|value| exact_difference(value, candidate).is_some())
            {
                candidate
            } else {
                // Never round a knot/control, collapse a span, or erase an
                // excursion merely to improve another part of the domain.
                0.
            }
        });
        if origin == [0.; 2] {
            return Ok(ParameterFrame {
                face: Cow::Borrowed(self),
                origin,
            });
        }
        let surface = NurbsSurface::try_new_rational(
            self.surface.degree_u(),
            self.surface.degree_v(),
            self.surface.control_point_count_u(),
            self.surface.control_point_count_v(),
            self.surface.control_points().to_vec(),
            knots[0].iter().map(|k| k - origin[0]).collect(),
            knots[1].iter().map(|k| k - origin[1]).collect(),
        )?;
        let loops = self
            .loops
            .iter()
            .map(|boundary| {
                let trims = boundary
                    .trims
                    .iter()
                    .map(|trim| {
                        let controls = trim
                            .curve
                            .control_points()
                            .iter()
                            .map(|c| {
                                WeightedPoint2::try_new(
                                    Point2::try_new(
                                        c.point().x() - origin[0],
                                        c.point().y() - origin[1],
                                    )?,
                                    c.weight(),
                                )
                            })
                            .collect::<Result<Vec<_>, GeometryError>>()?;
                        Ok(BrepTrim {
                            vertices: trim.vertices,
                            edge: trim.edge,
                            reversed_3d: trim.reversed_3d,
                            curve: NurbsCurve2::try_new_rational(
                                trim.curve.degree(),
                                controls,
                                trim.curve.knots().to_vec(),
                            )?,
                            trim_type: trim.trim_type,
                            iso: trim.iso,
                            tolerance: trim.tolerance,
                        })
                    })
                    .collect::<Result<Vec<_>, GeometryError>>()?;
                Ok(BrepLoop {
                    loop_type: boundary.loop_type,
                    trims,
                })
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        Ok(ParameterFrame {
            face: Cow::Owned(BrepFace {
                surface,
                reversed: self.reversed,
                loops,
            }),
            origin,
        })
    }
}

fn exact_difference(a: Real, b: Real) -> Option<Real> {
    // Error-free TwoDiff. All intermediates must remain finite; otherwise the
    // candidate frame is declined. A nonzero residual means the stored
    // representation would change under translation.
    let difference = a - b;
    let b_virtual = a - difference;
    let a_virtual = difference + b_virtual;
    let b_error = b_virtual - b;
    let a_error = a - a_virtual;
    let error = a_error + b_error;
    [difference, b_virtual, a_virtual, b_error, a_error, error]
        .into_iter()
        .all(Real::is_finite)
        .then_some(difference)
        .filter(|_| error == 0.)
}
