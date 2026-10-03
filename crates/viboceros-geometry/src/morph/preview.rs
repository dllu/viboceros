//! Prepared display approximations. No adaptive fit or tolerance certificate.
use super::interpolation::Axis;
use super::surface_fit::tensor::Grid;
use super::*;
use faer::Mat;

/// Low-degree polynomial B-rep edges use a cubic interpolant of mapped Greville
/// samples. Higher-degree and rational edges retain a mapped control cage.
/// Standalone command curves can use a different preview preparation policy.
pub struct CurvePreviewCage {
    source: NurbsCurve,
    samples: Option<(Axis, Vec<Point3>)>,
}
impl CurvePreviewCage {
    pub fn try_new(source: &NurbsCurve) -> Result<Self, GeometryError> {
        let samples = if source.degree() < 3 && !source.is_rational() {
            if source.control_points().len() > MAX_MORPH_CURVE_CONTROL_POINTS {
                return Err(GeometryError::TooManyMorphCurveControlPoints {
                    maximum: MAX_MORPH_CURVE_CONTROL_POINTS,
                });
            }
            let template = source.try_change_degree(3, false)?;
            if template.control_points().len() > MAX_MORPH_CURVE_CONTROL_POINTS {
                return Err(GeometryError::TooManyMorphCurveControlPoints {
                    maximum: MAX_MORPH_CURVE_CONTROL_POINTS,
                });
            }
            let axis = Axis::new(template.degree(), template.knots().to_vec())?;
            let points = axis
                .stations
                .iter()
                .map(|s| source.evaluate_on_side(s.parameter, s.side))
                .collect::<Result<Vec<_>, _>>()?;
            Some((axis, points))
        } else {
            None
        };
        Ok(Self {
            source: source.clone(),
            samples,
        })
    }
    pub fn morphed_curve(
        &self,
        morph: &(impl PointMorph + ?Sized),
        preserve: bool,
    ) -> Result<NurbsCurve, GeometryError> {
        let Some((axis, samples)) = self.samples.as_ref().filter(|_| !preserve) else {
            return morph.morph_nurbs_curve_controls(&self.source);
        };
        let targets = samples
            .iter()
            .map(|p| morph.morph_point(*p))
            .collect::<Result<Vec<_>, _>>()?;
        let anchor = targets[0].to_array();
        let origin = if targets.iter().all(|p| {
            p.to_array()
                .iter()
                .zip(anchor)
                .all(|(a, b)| (a - b).is_finite())
        }) {
            anchor
        } else {
            [0.; 3]
        };
        let solved = axis.solve(Mat::from_fn(targets.len(), 3, |i, j| {
            targets[i].to_array()[j] - origin[j]
        }))?;
        let controls = axis
            .stations
            .iter()
            .enumerate()
            .map(|(i, s)| {
                if s.fixed {
                    Ok(targets[i])
                } else {
                    Point3::try_from(std::array::from_fn(|j| solved[(i, j)] + origin[j]))
                }
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        NurbsCurve::try_new(axis.degree, controls, axis.knots.clone())
    }
}

/// Polynomial surfaces with a degree below three prepare a cubic Greville grid
/// and collocation axes once. Mouse motion maps that grid and solves one tensor
/// interpolant. Preserved, rational and higher-degree surfaces map controls.
/// These choices describe display approximations, not accepted model geometry.
pub struct SurfacePreviewCage {
    source: NurbsSurface,
    samples: Option<Grid>,
}
impl SurfacePreviewCage {
    pub fn try_new(source: &NurbsSurface) -> Result<Self, GeometryError> {
        let samples = if (source.degree_u() < 3 || source.degree_v() < 3) && !source.is_rational() {
            if source.control_point_count_u() > MAX_MORPH_SURFACE_AXIS_CONTROLS
                || source.control_point_count_v() > MAX_MORPH_SURFACE_AXIS_CONTROLS
            {
                return Err(GeometryError::TooManyMorphSurfaceControlPoints {
                    maximum: MAX_MORPH_SURFACE_AXIS_CONTROLS,
                });
            }
            let template = source.try_change_degree(
                source.degree_u().max(3),
                source.degree_v().max(3),
                false,
            )?;
            if template.control_point_count_u() > MAX_MORPH_SURFACE_AXIS_CONTROLS
                || template.control_point_count_v() > MAX_MORPH_SURFACE_AXIS_CONTROLS
            {
                return Err(GeometryError::TooManyMorphSurfaceControlPoints {
                    maximum: MAX_MORPH_SURFACE_AXIS_CONTROLS,
                });
            }
            Some(Grid::sample(
                Axis::new(template.degree_u(), template.knots_u().to_vec())?,
                Axis::new(template.degree_v(), template.knots_v().to_vec())?,
                &mut |uv, sides| source.evaluate_on_sides(uv[0], uv[1], sides[0], sides[1]),
            )?)
        } else {
            None
        };
        Ok(Self {
            source: source.clone(),
            samples,
        })
    }
    pub fn morphed_surface(
        &self,
        morph: &(impl PointMorph + ?Sized),
        preserve: bool,
    ) -> Result<NurbsSurface, GeometryError> {
        if let Some(samples) = self.samples.as_ref().filter(|_| !preserve) {
            samples.morphed(morph)
        } else {
            morph.morph_nurbs_surface_controls(&self.source)
        }
    }
}
