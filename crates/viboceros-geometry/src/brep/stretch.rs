//! Affine source faces become ruled cubic profiles in an axial UV chart.
use super::*;
use crate::{PointMorph, StretchPointMorph};
#[cfg(test)]
mod tests;

#[derive(Clone)]
struct Chart {
    origin: Point3,
    axes: [Vector3; 2],
    domains: [[Real; 2]; 2],
    offset: Real,
    gradient: [Real; 2],
    pivot: usize,
    sign: Real,
    bounds: [[Real; 2]; 2],
}
impl Chart {
    fn new(surface: &NurbsSurface, morph: &StretchPointMorph) -> Option<Self> {
        let (origin, u, v) = surface.affine_axes()?;
        let axes = [u, v];
        u.cross(v).ok()?.normalized_nonzero().ok()?;
        let (start, direction) = morph.axis();
        let n = direction.as_vector();
        let offset = n.dot_point_difference(origin, start) / morph.length();
        let gradient = [
            n.dot(u).ok()? / morph.length(),
            n.dot(v).ok()? / morph.length(),
        ];
        let pivot = usize::from(gradient[1].abs() >= gradient[0].abs());
        if gradient[pivot] == 0. || !gradient[pivot].is_normal() {
            return None;
        }
        // det(d(new UV)/d(old UV)) stays positive; face sense and loops survive.
        let sign = if pivot == 1 {
            gradient[pivot].signum()
        } else {
            -gradient[pivot].signum()
        };
        let domains = [surface.domain_u(), surface.domain_v()].map(|d| [*d.start(), *d.end()]);
        if domains.iter().any(|d| !(d[1] - d[0]).is_normal()) {
            return None;
        }
        let mut chart = Self {
            origin,
            axes,
            domains,
            offset,
            gradient,
            pivot,
            sign,
            bounds: [
                [sign.min(0.), sign.max(0.)],
                [Real::INFINITY, Real::NEG_INFINITY],
            ],
        };
        for u in [0., 1.] {
            for v in [0., 1.] {
                let t = chart.normalized_uv([u, v]).ok()?.y();
                chart.bounds[1][0] = chart.bounds[1][0].min(t);
                chart.bounds[1][1] = chart.bounds[1][1].max(t);
            }
        }
        if chart.bounds[1][0] >= chart.bounds[1][1]
            || chart.bounds.iter().flatten().any(|x| !x.is_finite())
        {
            return None;
        }
        Some(chart)
    }
    fn normalized_uv(&self, old: [Real; 2]) -> Result<Point2, GeometryError> {
        Point2::try_new(
            self.sign * old[1 - self.pivot],
            self.gradient[0].mul_add(old[0], self.gradient[1].mul_add(old[1], self.offset)),
        )
    }
    fn uv(&self, old: Point2) -> Result<Point2, GeometryError> {
        use crate::exact_scalar::{rational, scalar};
        let p = [old.x(), old.y()];
        let normalized = [0, 1].map(|i| {
            scalar(
                &((rational(p[i]) - rational(self.domains[i][0]))
                    / (rational(self.domains[i][1]) - rational(self.domains[i][0]))),
            )
        });
        let [u, v] = normalized;
        self.normalized_uv([u?, v?])
    }
    fn source(&self, u: Real, v: Real) -> Result<Point3, GeometryError> {
        let free = self.sign * u;
        let pivot =
            (v - self.offset - self.gradient[1 - self.pivot] * free) / self.gradient[self.pivot];
        self.origin
            .translated(self.axes[1 - self.pivot].scaled(free)?)?
            .translated(self.axes[self.pivot].scaled(pivot)?)
    }
    fn derivative(&self, v: Real, morph: &StretchPointMorph) -> Result<Vector3, GeometryError> {
        let linear = self.axes[self.pivot].scaled(1. / self.gradient[self.pivot])?;
        let blend = if v > 0. && v < 1. {
            6. * v * (1. - v)
        } else {
            0.
        };
        let axial = morph
            .axis()
            .1
            .as_vector()
            .scaled((morph.new_length() - morph.length()) * blend)?;
        Vector3::try_from(std::array::from_fn(|i| {
            linear.to_array()[i] + axial.to_array()[i]
        }))
    }
    fn surface(&self, morph: &StretchPointMorph) -> Result<NurbsSurface, GeometryError> {
        let [lo, hi] = self.bounds[1];
        let cuts = std::iter::once(lo)
            .chain([0., 1.].into_iter().filter(|t| *t > lo && *t < hi))
            .chain(std::iter::once(hi))
            .collect::<Vec<_>>();
        let mut profiles = [Vec::new(), Vec::new()];
        for (i, u) in self.bounds[0].into_iter().enumerate() {
            for (j, span) in cuts.windows(2).enumerate() {
                let p = morph.morph_point(self.source(u, span[0])?)?;
                let q = morph.morph_point(self.source(u, span[1])?)?;
                let a = p.translated(
                    self.derivative(span[0], morph)?
                        .scaled((span[1] - span[0]) / 3.)?,
                )?;
                let b = q.translated(
                    self.derivative(span[1], morph)?
                        .scaled(-(span[1] - span[0]) / 3.)?,
                )?;
                profiles[i].extend([p, a, b, q].into_iter().skip(usize::from(j > 0)));
            }
        }
        let knots = std::iter::repeat_n(lo, 4)
            .chain(
                cuts[1..cuts.len() - 1]
                    .iter()
                    .flat_map(|&v| std::iter::repeat_n(v, 3)),
            )
            .chain(std::iter::repeat_n(hi, 4))
            .collect();
        let controls = (0..profiles[0].len())
            .flat_map(|v| [profiles[0][v], profiles[1][v]])
            .map(|p| WeightedPoint3::try_new(p, 1.))
            .collect::<Result<Vec<_>, _>>()?;
        let mut surface = NurbsSurface::try_new_rational(
            1,
            3,
            2,
            profiles[0].len(),
            controls,
            vec![
                self.bounds[0][0],
                self.bounds[0][0],
                self.bounds[0][1],
                self.bounds[0][1],
            ],
            knots,
        )?;
        for &t in &cuts[1..cuts.len() - 1] {
            if let Ok(simpler) = surface.try_remove_knot_v_near(t) {
                surface = simpler;
            }
        }
        Ok(surface)
    }
    fn face(
        &self,
        source: &BrepFace,
        morph: &StretchPointMorph,
        fitting: Tolerance,
    ) -> Result<BrepFace, GeometryError> {
        let surface = self.surface(morph)?;
        // Guard chart conditioning before assembling any document geometry.
        for j in 0..=8 {
            for i in 0..=8 {
                let u = source.surface.parameter_at_u(i as Real / 8.)?;
                let v = source.surface.parameter_at_v(j as Real / 8.)?;
                let mapped = self.uv(Point2::try_new(u, v)?)?;
                let expected = morph.morph_point(source.surface.evaluate(u, v)?)?;
                if surface
                    .evaluate(mapped.x(), mapped.y())?
                    .distance_to(expected)?
                    > fitting.absolute() * 0.125
                {
                    return Err(GeometryError::Degenerate {
                        context: "ill-conditioned stretch face chart",
                    });
                }
            }
        }
        let loops = source
            .loops
            .iter()
            .map(|boundary| {
                let trims = boundary
                    .trims
                    .iter()
                    .map(|trim| {
                        let mut next = trim.clone();
                        next.curve = NurbsCurve2::try_new_rational(
                            trim.curve.degree(),
                            trim.curve
                                .control_points()
                                .iter()
                                .map(|c| WeightedPoint2::try_new(self.uv(c.point())?, c.weight()))
                                .collect::<Result<Vec<_>, GeometryError>>()?,
                            trim.curve.knots().to_vec(),
                        )?;
                        let p = next.curve.control_points()[0].point();
                        next.iso = if next
                            .curve
                            .control_points()
                            .iter()
                            .all(|c| c.point().x() == p.x())
                        {
                            if p.x() == self.bounds[0][0] {
                                SurfaceIso::West
                            } else if p.x() == self.bounds[0][1] {
                                SurfaceIso::East
                            } else {
                                SurfaceIso::InteriorUConstant
                            }
                        } else if next
                            .curve
                            .control_points()
                            .iter()
                            .all(|c| c.point().y() == p.y())
                        {
                            if p.y() == self.bounds[1][0] {
                                SurfaceIso::South
                            } else if p.y() == self.bounds[1][1] {
                                SurfaceIso::North
                            } else {
                                SurfaceIso::InteriorVConstant
                            }
                        } else {
                            SurfaceIso::NotIso
                        };
                        let old = [0, 1]
                            .map(|i| trim.tolerance[i] / (self.domains[i][1] - self.domains[i][0]));
                        next.tolerance = [
                            old[1 - self.pivot],
                            self.gradient[0].abs() * old[0] + self.gradient[1].abs() * old[1],
                        ];
                        Ok(next)
                    })
                    .collect::<Result<Vec<_>, GeometryError>>()?;
                Ok(BrepLoop {
                    loop_type: boundary.loop_type,
                    trims,
                })
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        BrepFace::try_new(surface, source.reversed, loops)
    }
}

impl Brep {
    pub(crate) fn try_stretch_affine_faces(
        &self,
        morph: &StretchPointMorph,
        tolerance: Tolerance,
    ) -> Result<Option<Self>, GeometryError> {
        let charts = self
            .faces
            .iter()
            .map(|f| Chart::new(&f.surface, morph))
            .collect::<Vec<_>>();
        if charts.iter().all(Option::is_none) {
            return Ok(None);
        }
        let fitting = Tolerance::try_new(
            tolerance.absolute() * 0.25,
            tolerance.relative(),
            tolerance.angular(),
        )?;
        let vertices = self
            .vertices
            .iter()
            .map(|v| BrepVertex::try_new(morph.morph_point(v.point)?, tolerance.absolute()))
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let edges = self
            .edges
            .iter()
            .map(|e| {
                BrepEdge::try_new(
                    e.vertices,
                    morph.morph_nurbs_curve(&e.curve, fitting)?,
                    tolerance.absolute(),
                )
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let faces = self
            .faces
            .iter()
            .zip(charts)
            .map(|(face, chart)| {
                if let Some(chart) = chart {
                    chart.face(face, morph, fitting)
                } else {
                    BrepFace::try_new(
                        morph.morph_nurbs_surface(&face.surface, fitting)?,
                        face.reversed,
                        face.loops.clone(),
                    )
                }
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        Self::try_new(vertices, edges, faces, tolerance).map(Some)
    }
}
