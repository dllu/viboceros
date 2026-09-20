//! Lossless local UV frames for operations that compose trims with surfaces.

use super::*;
use crate::parameter::exact_difference;
use std::borrow::Cow;

#[cfg(test)]
pub(in crate::brep) mod tests;

pub(super) struct ParameterFrame<'a> {
    pub(super) face: Cow<'a, BrepFace>,
    pub(super) origin: [Real; 2],
}

impl ParameterFrame<'_> {
    pub(super) fn restore_curve_parameter_origin(
        &self,
        curve: NurbsCurve,
        axis: usize,
    ) -> Result<NurbsCurve, GeometryError> {
        let origin = self.origin[axis];
        if origin == 0. {
            return Ok(curve);
        }
        let Some(knots) = curve
            .knots()
            .iter()
            .map(|&k| exact_difference(k, -origin))
            .collect::<Option<Vec<_>>>()
        else {
            // Newly computed trim intersections need not fit on the native
            // parameter grid. Keep their local parameterization and geometry.
            return Ok(curve);
        };
        NurbsCurve::try_new_rational(curve.degree(), curve.control_points().to_vec(), knots)
    }
}

impl BrepFace {
    pub(super) fn local_parameter_frame(&self) -> Result<ParameterFrame<'_>, GeometryError> {
        let coordinates = self
            .loops
            .iter()
            .flat_map(|boundary| &boundary.trims)
            .flat_map(|trim| trim.curve.control_points())
            .map(|c| c.point().to_array());
        let surface_frame = self.surface.local_parameter_frame(coordinates)?;
        let origin = surface_frame.origin;
        if origin == [0.; 2] {
            return Ok(ParameterFrame {
                face: Cow::Borrowed(self),
                origin,
            });
        }
        let surface = surface_frame.surface.into_owned();
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
