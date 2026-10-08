//! Exact surface composition proposals and certified adaptive image fitting.
use super::*;
use crate::{NurbsCurveParameterSampler, WeightedPoint3};

#[cfg(test)]
mod tests;

impl NurbsSurface {
    /// Constructs a spatial image of a UV spline and certifies its complete
    /// normalized-parameter correspondence at absolute model tolerance.
    /// The returned curve retains the UV spline's native parameter domain.
    /// See [`Self::try_pushup_curve_certified_with_bound`] for limits.
    pub fn try_pushup_curve_certified(
        &self,
        uv: &NurbsCurve2,
        tolerance: Tolerance,
    ) -> Result<NurbsCurve, GeometryError> {
        self.try_pushup_curve_certified_with_bound(uv, tolerance)
            .map(|(curve, _)| curve)
    }

    /// Returns a spatial image and a continuous model-space deviation bound.
    /// Exact rational Bernstein composition is tried on original UV spans and
    /// rational linear tensor-knot crossings. Rounded controls, weights and
    /// restored knot times receive an independent certificate. Otherwise an
    /// adaptive polynomial cubic interpolant is certified against the original
    /// UV spline, retaining all original UV knots in every proof interval.
    /// No sampled-only proposal is returned. Sources remain unchanged.
    ///
    /// The certificate's degree, weight, domain and arithmetic limits apply.
    /// Fitting has depth 20 and at most `MAX_CURVE_DIVISION_POINTS` controls.
    /// Unsupported inputs or inconclusive fits fail explicitly. This certifies
    /// correspondence, not trim topology, smoothness or injectivity.
    pub fn try_pushup_curve_certified_with_bound(
        &self,
        uv: &NurbsCurve2,
        tolerance: Tolerance,
    ) -> Result<(NurbsCurve, Real), GeometryError> {
        let limit = tolerance.absolute();
        let failed = || GeometryError::SurfacePushupDidNotConverge { tolerance: limit };
        if let Some(result) = affine::image(self, uv, limit)? {
            return Ok(result);
        }
        let mut certificate = ImageCertificate::new(self, uv)?.ok_or_else(failed)?;
        if let Some(proposal) = certificate.exact_proposal(uv)?
            && let Some(bound) = self.parameter_curve_deviation_bound(uv, &proposal, limit)?
        {
            return Ok((proposal, bound));
        }

        let frame =
            self.local_parameter_frame(uv.control_points().iter().map(|p| p.point().to_array()))?;
        let source = NurbsCurve::try_new_rational(
            uv.degree(),
            uv.control_points()
                .iter()
                .map(|p| {
                    WeightedPoint3::try_new(
                        Point3::try_new(
                            p.point().x() - frame.origin[0],
                            p.point().y() - frame.origin[1],
                            0.,
                        )?,
                        p.weight(),
                    )
                })
                .collect::<Result<Vec<_>, GeometryError>>()?,
            uv.knots().to_vec(),
        )?;
        let mut fitter = Fitter {
            surface: &frame.surface,
            sampler: source.parameter_sampler()?,
            certificate,
            segments: Vec::new(),
            limit,
        };
        fitter.append([0., 1.], 0)?;
        let segments = fitter
            .segments
            .into_iter()
            .map(|(interval, controls)| {
                Ok((
                    interval
                        .map(|t| source.parameter_at(t))
                        .into_iter()
                        .collect::<Result<Vec<_>, GeometryError>>()?,
                    controls
                        .into_iter()
                        .map(|p| WeightedPoint3::try_new(p, 1.))
                        .collect::<Result<Vec<_>, GeometryError>>()?,
                ))
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let spatial = assemble(3, segments)?;
        let bound = self
            .parameter_curve_deviation_bound(uv, &spatial, limit)?
            .ok_or_else(failed)?;
        Ok((spatial, bound))
    }
}

struct ImageCertificate {
    surface: surface::Surface,
    uv: curve::Spline<3>,
    budget: Budget,
}

impl ImageCertificate {
    fn new(surface: &NurbsSurface, uv: &NurbsCurve2) -> Result<Option<Self>, GeometryError> {
        if !supported(surface, uv.degree(), 3) {
            return Ok(None);
        }
        let mut budget = Budget(MAX_WORK);
        let Some(surface) = surface::Surface::new(surface, &mut budget)? else {
            return Ok(None);
        };
        let Some(uv) = curve::Spline::uv(uv, &mut budget)? else {
            return Ok(None);
        };
        Ok(Some(Self {
            surface,
            uv,
            budget,
        }))
    }

    fn exact_proposal(
        &mut self,
        source: &NurbsCurve2,
    ) -> Result<Option<NurbsCurve>, GeometryError> {
        let mut cuts = self.uv.cuts();
        cuts.sort();
        cuts.dedup();
        if source.degree() == 1 {
            let mut crossings = Vec::new();
            for interval in cuts.windows(2) {
                let uv = self
                    .uv
                    .extract(&interval[0], &interval[1], &mut self.budget)?;
                for t in self.surface.linear_crossings(&uv, &mut self.budget)? {
                    crossings.push(&interval[0] + (&interval[1] - &interval[0]) * t);
                }
            }
            cuts.extend(crossings);
            cuts.sort();
            cuts.dedup();
        }
        let mut pieces = Vec::new();
        let mut degree = 1;
        for interval in cuts.windows(2) {
            let uv = self
                .uv
                .extract(&interval[0], &interval[1], &mut self.budget)?;
            let bounds = curve::bounds(&uv, &mut self.budget)?;
            if !self.surface.in_domain(&bounds) {
                return Ok(None);
            }
            let Some(patch) = self.surface.containing_patch(&bounds, &mut self.budget)? else {
                return Ok(None);
            };
            let image = self.surface.compose(patch, &uv, &mut self.budget)?;
            degree = degree.max(image.len() - 1);
            pieces.push((interval.to_vec(), image));
        }
        let start = rational(*source.domain().start());
        let width = rational(*source.domain().end()) - &start;
        let mut segments = Vec::new();
        for (interval, mut net) in pieces {
            elevate(&mut net, degree, &mut self.budget)?;
            let gauge = net.iter().map(|p| &p[3]).max().unwrap().clone();
            if gauge <= Rational::zero() || net.iter().any(|p| p[3] <= Rational::zero()) {
                return Ok(None);
            }
            let mut controls = Vec::new();
            for p in net {
                let weight = scalar(&(&p[3] / &gauge))?;
                if weight == 0. {
                    return Ok(None);
                }
                controls.push(WeightedPoint3::try_new(
                    Point3::try_new(
                        scalar(&(&p[0] / &p[3]))?,
                        scalar(&(&p[1] / &p[3]))?,
                        scalar(&(&p[2] / &p[3]))?,
                    )?,
                    weight,
                )?);
            }
            let interval = interval
                .iter()
                .map(|t| scalar(&(&start + &width * t)))
                .collect::<Result<Vec<_>, _>>()?;
            if interval[0] >= interval[1] {
                return Ok(None);
            }
            segments.push((interval, controls));
        }
        Ok(Some(assemble(degree, segments)?))
    }

    fn segment(
        &mut self,
        interval: [Real; 2],
        controls: [Point3; 4],
        limit: Real,
    ) -> Result<bool, GeometryError> {
        let [start, end] = interval.map(rational);
        let width = &end - &start;
        let mut cuts = self
            .uv
            .cuts()
            .into_iter()
            .filter(|t| *t > start && *t < end)
            .collect::<Vec<_>>();
        cuts.extend([start.clone(), end.clone()]);
        cuts.sort();
        cuts.dedup();
        let spatial = controls.map(|p| {
            [
                rational(p.x()),
                rational(p.y()),
                rational(p.z()),
                Rational::one(),
            ]
        });
        let knots = [vec![Rational::zero(); 4], vec![Rational::one(); 4]].concat();
        for bounds in cuts.windows(2) {
            let first = (&bounds[0] - &start) / &width;
            let last = (&bounds[1] - &start) / &width;
            let spatial = curve::extract(&knots, 3, 3, &spatial, &first, &last, &mut self.budget)?;
            let uv = self.uv.extract(&bounds[0], &bounds[1], &mut self.budget)?;
            if piece_bound(&mut self.surface, uv, spatial, limit, &mut self.budget)?.is_none() {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

fn elevate(net: &mut Net<4>, degree: usize, budget: &mut Budget) -> Result<(), GeometryError> {
    while net.len() <= degree {
        let n = net.len();
        let mut next = Vec::with_capacity(n + 1);
        next.push(net[0].clone());
        for i in 1..n {
            let t = rational(i as Real) / rational(n as Real);
            let control = std::array::from_fn(|axis| {
                &t * &net[i - 1][axis] + (Rational::one() - &t) * &net[i][axis]
            });
            for r in &control {
                budget.check(r)?;
            }
            next.push(control);
        }
        next.push(net[n - 1].clone());
        *net = next;
    }
    Ok(())
}

type Segment = (Vec<Real>, Vec<WeightedPoint3>);
fn assemble(degree: usize, segments: Vec<Segment>) -> Result<NurbsCurve, GeometryError> {
    let count = segments
        .len()
        .checked_mul(degree + 1)
        .filter(|n| *n <= MAX_CURVE_DIVISION_POINTS)
        .ok_or(GeometryError::TooManySurfacePushupControlPoints {
            maximum: MAX_CURVE_DIVISION_POINTS,
        })?;
    let mut controls = Vec::with_capacity(count);
    let mut knots = Vec::with_capacity(count + degree + 1);
    for (interval, points) in segments {
        knots.extend(std::iter::repeat_n(interval[0], degree + 1));
        controls.extend(points);
        // A full-order boundary keeps independent rational gauges and exact
        // one-sided source limits. The final certificate verifies the join.
        if controls.len() == count {
            knots.extend(std::iter::repeat_n(interval[1], degree + 1));
        }
    }
    NurbsCurve::try_new_rational(degree, controls, knots)
}

struct Fitter<'a> {
    surface: &'a NurbsSurface,
    sampler: NurbsCurveParameterSampler<'a>,
    certificate: ImageCertificate,
    segments: Vec<([Real; 2], [Point3; 4])>,
    limit: Real,
}
impl Fitter<'_> {
    fn node(&self, t: Real) -> Result<Point3, GeometryError> {
        let uv = self.sampler.evaluate(t)?;
        self.surface.evaluate(
            snap_domain_roundoff(
                uv.x(),
                [
                    *self.surface.domain_u().start(),
                    *self.surface.domain_u().end(),
                ],
            ),
            snap_domain_roundoff(
                uv.y(),
                [
                    *self.surface.domain_v().start(),
                    *self.surface.domain_v().end(),
                ],
            ),
        )
    }
    fn append(&mut self, interval: [Real; 2], depth: usize) -> Result<(), GeometryError> {
        let [start, end] = interval;
        let fraction = |t: Real| start.mul_add(1. - t, end * t);
        let points = [
            self.node(start)?,
            self.node(fraction(1. / 3.))?,
            self.node(fraction(2. / 3.))?,
            self.node(end)?,
        ];
        let blend = |weights: [Real; 4]| {
            let anchor = points[0].to_array();
            let coordinates = std::array::from_fn(|axis| {
                points
                    .iter()
                    .zip(weights)
                    .skip(1)
                    .fold(anchor[axis], |sum, (p, w)| {
                        w.mul_add(p.to_array()[axis] - anchor[axis], sum)
                    })
            });
            Point3::try_from(coordinates)
        };
        let controls = [
            points[0],
            blend([-5. / 6., 3., -1.5, 1. / 3.])?,
            blend([1. / 3., -1.5, 3., -5. / 6.])?,
            points[3],
        ];
        if self.certificate.segment(interval, controls, self.limit)? {
            if self.segments.len() >= MAX_CURVE_DIVISION_POINTS / 4 {
                return Err(GeometryError::TooManySurfacePushupControlPoints {
                    maximum: MAX_CURVE_DIVISION_POINTS,
                });
            }
            self.segments.push((interval, controls));
            return Ok(());
        }
        if depth == MAX_PULLBACK_SUBDIVISION_DEPTH {
            return Err(GeometryError::SurfacePushupDidNotConverge {
                tolerance: self.limit,
            });
        }
        let middle = start.mul_add(0.5, end * 0.5);
        self.append([start, middle], depth + 1)?;
        self.append([middle, end], depth + 1)
    }
}
