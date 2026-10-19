//! Focal geometry of a whole conic locus, independent of its parameterization.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ConicFoci {
    Circle { center: Point3 },
    Ellipse { foci: [Point3; 2] },
    Hyperbola { foci: [Point3; 2] },
    Parabola { vertex: Point3, focus: Point3 },
}

impl NurbsCurve {
    /// Recognizes a planar conic using homogeneous Bernstein coefficients.
    /// Every original span must pass plane and implicit-locus bounds; a sampled
    /// fit alone cannot accept the curve. Degenerate and numerically ambiguous
    /// conics return `None`. Same-sign weights within each span are required.
    pub fn conic_foci(&self, tolerance: Tolerance) -> Result<Option<ConicFoci>, GeometryError> {
        if self.degree() < 2 {
            return Ok(None);
        }
        if let Some(locus) = self.elliptical_locus(tolerance)? {
            let major = usize::from(locus.radii[1] > locus.radii[0]);
            let a = locus.radii[major];
            let b = locus.radii[1 - major];
            if a - b <= 64. * Real::EPSILON * a {
                return Ok(Some(ConicFoci::Circle {
                    center: locus.center,
                }));
            }
            let distance = a * (((a - b) / a) * (1. + b / a)).sqrt();
            let offset = locus.axes[major].scaled(distance)?;
            return Ok(Some(ConicFoci::Ellipse {
                foci: [
                    locus.center.translated(offset)?,
                    locus.center.translated(offset.scaled(-1.)?)?,
                ],
            }));
        }
        let spans = self.try_bezier_spans()?;
        let Some(fit) = coefficient_fit(&spans, tolerance).ok().flatten() else {
            return Ok(None);
        };
        let [a, b, c, d, e, f] = fit.coefficients;
        let frame = &fit.frame;
        let scale = frame.scales[0].max(frame.scales[1]);
        let ratios = Vector2::new(scale / frame.scales[0], scale / frame.scales[1]);
        let quadratic = Matrix2::new(
            a * ratios.x * ratios.x,
            b * ratios.x * ratios.y,
            b * ratios.x * ratios.y,
            c * ratios.y * ratios.y,
        );
        let linear = Vector2::new(d * ratios.x, e * ratios.y);
        if quadratic
            .iter()
            .chain(linear.iter())
            .any(|v| !v.is_finite())
        {
            return Ok(None);
        }
        let eigen = quadratic.symmetric_eigen();
        let small = usize::from(eigen.eigenvalues[1].abs() < eigen.eigenvalues[0].abs());
        let large = 1 - small;
        let noise = fit.noise * ratios.max().powi(2);
        let lambda = eigen.eigenvalues[large];
        if !noise.is_finite() || lambda.abs() <= 4. * noise {
            return Ok(None);
        }
        let axis = |i| frame.vector([eigen.eigenvectors[(0, i)], eigen.eigenvectors[(1, i)]]);
        let point = |v: Vector2<Real>| {
            frame
                .origin
                .translated(frame.vector([scale * v.x, scale * v.y])?)
        };
        if eigen.eigenvalues[small].abs() <= 2. * noise {
            let u = eigen.eigenvectors.column(large);
            let v = eigen.eigenvectors.column(small);
            let lu = linear.dot(&u);
            let lv = linear.dot(&v);
            if lv.abs() <= 4. * noise {
                return Ok(None);
            }
            let x = -lu / lambda;
            let y = -(f - lu * lu / lambda) / (2. * lv);
            let focal = -lv / (2. * lambda);
            // Sensitivity grows with extrapolation to the vertex. Reject an
            // unresolved near-parabola rather than returning distant markers.
            let sensitivity = scale * noise * (1. + x.abs() + y.abs() + focal.abs()).powi(2)
                / lambda.abs().min(lv.abs());
            if !sensitivity.is_finite() || sensitivity > tolerance.absolute() {
                return Ok(None);
            }
            let vertex = point(u * x + v * y)?;
            let direction = axis(small)?;
            let focus = vertex.translated(direction.scaled(scale * focal)?)?;
            let locus = OpenConic {
                origin: vertex,
                axes: [axis(large)?, direction],
                normal: frame.normal,
                shape: Shape::Parabola {
                    focal: scale * focal,
                },
            };
            if locus.contains(&spans, tolerance)? {
                return Ok(Some(ConicFoci::Parabola { vertex, focus }));
            }
            return Ok(None);
        }
        if eigen.eigenvalues[0].signum() == eigen.eigenvalues[1].signum() {
            return Ok(None);
        }
        let Some(inverse) = quadratic.try_inverse() else {
            return Ok(None);
        };
        let center = -(inverse * linear);
        let level = -f - linear.dot(&center);
        let transverse = usize::from(level / eigen.eigenvalues[1] > 0.);
        let aa = level / eigen.eigenvalues[transverse];
        let bb = -level / eigen.eigenvalues[1 - transverse];
        if !aa.is_finite() || !bb.is_finite() || aa <= 0. || bb <= 0. {
            return Ok(None);
        }
        let a = scale * aa.sqrt();
        let b = scale * bb.sqrt();
        let sensitivity = scale * noise * (1. + center.norm() + aa.sqrt() + bb.sqrt()).powi(2)
            / eigen.eigenvalues[small].abs();
        if !sensitivity.is_finite() || sensitivity > tolerance.absolute() {
            return Ok(None);
        }
        let center = point(center)?;
        let axes = [axis(transverse)?, axis(1 - transverse)?];
        let locus = OpenConic {
            origin: center,
            axes,
            normal: frame.normal,
            shape: Shape::Hyperbola { a, b },
        };
        if !locus.contains(&spans, tolerance)? {
            return Ok(None);
        }
        let offset = axes[0].scaled(a.hypot(b))?;
        Ok(Some(ConicFoci::Hyperbola {
            foci: [
                center.translated(offset)?,
                center.translated(offset.scaled(-1.)?)?,
            ],
        }))
    }
}

enum Shape {
    Hyperbola { a: Real, b: Real },
    Parabola { focal: Real },
}
struct OpenConic {
    origin: Point3,
    axes: [Vector3; 2],
    normal: Vector3,
    shape: Shape,
}
impl OpenConic {
    fn contains(&self, spans: &[NurbsCurve], tolerance: Tolerance) -> Result<bool, GeometryError> {
        for span in spans {
            let controls = span.control_points();
            let gauge = controls
                .iter()
                .map(|c| c.weight().abs())
                .fold(0., Real::max);
            let sign = controls[0].weight().signum();
            let mut minimum = 1.0_f64;
            let mut q = Vec::with_capacity(controls.len());
            for c in controls {
                let w = c.weight() / gauge * sign;
                if w <= 0. {
                    return Ok(false);
                }
                minimum = minimum.min(w);
                let p = self.origin.vector_to(c.point())?;
                if p.dot(self.normal)?.abs() + 32. * Real::EPSILON * p.length()?
                    > tolerance.absolute() * 0.5
                {
                    return Ok(false);
                }
                let x = p.dot(self.axes[0])?;
                let y = p.dot(self.axes[1])?;
                let (x, y) = match self.shape {
                    Shape::Hyperbola { a, b } => (x / a, y / b),
                    Shape::Parabola { focal } => (x / (4. * focal.abs()), y / (4. * focal)),
                };
                q.push([x * w, y * w, w]);
            }
            let n = span.degree();
            let b = binomial(n);
            let doubled = binomial(2 * n);
            let limit = match self.shape {
                Shape::Hyperbola { a, .. } => tolerance.absolute() * 0.5 / a,
                Shape::Parabola { focal } => tolerance.absolute() * 0.5 / (4. * focal.abs()),
            };
            for (k, denominator) in doubled.iter().enumerate() {
                let mut value = 0.;
                let mut magnitude = 0.;
                for i in k.saturating_sub(n)..=k.min(n) {
                    let j = k - i;
                    let factor = b[i] * b[j] / denominator;
                    let [xi, yi, wi] = q[i];
                    let [xj, yj, wj] = q[j];
                    let terms = match self.shape {
                        Shape::Hyperbola { .. } => [xi * xj, -yi * yj, -wi * wj],
                        Shape::Parabola { .. } => [xi * xj, -0.5 * yi * wj, -0.5 * wi * yj],
                    };
                    for term in terms {
                        value += factor * term;
                        magnitude += factor * term.abs();
                    }
                }
                // Positive Bernstein weights give W >= minimum. Hyperbola
                // |x²-y²-1| bounds horizontal distance because sqrt(1+y²)>=1;
                // parabola |x²-y| is exactly vertical distance in scaled units.
                if !value.is_finite()
                    || (value.abs() + 64. * (n + 1) as Real * Real::EPSILON * magnitude)
                        > limit * minimum * minimum
                {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
}
