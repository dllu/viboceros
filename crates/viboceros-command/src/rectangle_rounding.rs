//! Rectangle corner construction uses the shared fillet and rational kernels.
use crate::rectangle_input::RectangleDefinition;
use viboceros_document::Geometry;
use viboceros_geometry::{CurveSegment3, GeometryError, Point3, PolyCurve3, Real, Tolerance};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Kind {
    #[default]
    Arc,
    Conic,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub kind: Kind,
    pub radius: Real,
    pub rho: Real,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            kind: Kind::Arc,
            radius: 1.,
            rho: 0.5,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Corner {
    Arc(Real),
    Conic(Real),
}
impl Corner {
    pub fn arguments(self) -> String {
        match self {
            Self::Arc(r) => format!("Corner=Arc {r}"),
            Self::Conic(rho) => format!("Corner=Conic {rho}"),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Input {
    pub options: Options,
    pub enabled: bool,
    pending: Option<[Point3; 4]>,
}
impl Input {
    pub fn new(options: Options) -> Self {
        Self {
            options,
            enabled: false,
            pending: None,
        }
    }
    pub fn option(&mut self, word: &str) -> bool {
        let word = word.trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Rounded") && self.pending.is_none() {
            self.enabled = true;
            return true;
        }
        if !self.enabled {
            return false;
        }
        let value = word
            .split_once('=')
            .and_then(|(key, value)| key.eq_ignore_ascii_case("Corner").then_some(value))
            .unwrap_or(word);
        if value.eq_ignore_ascii_case("Arc") {
            self.options.kind = Kind::Arc;
            true
        } else if value.eq_ignore_ascii_case("Conic") {
            self.options.kind = Kind::Conic;
            true
        } else if word.eq_ignore_ascii_case("Corner") {
            self.options.kind = match self.options.kind {
                Kind::Arc => Kind::Conic,
                Kind::Conic => Kind::Arc,
            };
            true
        } else {
            false
        }
    }
    pub fn pending(self) -> bool {
        self.pending.is_some()
    }
    pub fn complete(&mut self, rectangle: RectangleDefinition) -> Option<RectangleDefinition> {
        if self.enabled {
            self.pending = Some(rectangle.corners);
            None
        } else {
            Some(rectangle)
        }
    }
    pub fn default_value(self) -> Real {
        match self.options.kind {
            Kind::Arc => self.options.radius,
            Kind::Conic => self.options.rho,
        }
    }
    pub fn number(&mut self, value: Real) -> Result<RectangleDefinition, GeometryError> {
        let corners = self.pending.ok_or(invalid("rectangle corner input"))?;
        if !value.is_finite() {
            return Err(GeometryError::NonFinite {
                context: "rectangle corner",
            });
        }
        let corner = match self.options.kind {
            Kind::Arc => {
                let maximum = corners[0]
                    .distance_to(corners[1])?
                    .min(corners[0].distance_to(corners[3])?)
                    * 0.5;
                let value = value.abs().min(maximum);
                self.options.radius = value;
                Corner::Arc(value)
            }
            Kind::Conic => {
                if value <= 0. || value >= 1. {
                    return Err(invalid("rectangle conic rho must be between zero and one"));
                }
                self.options.rho = value;
                Corner::Conic(value)
            }
        };
        Ok(RectangleDefinition {
            corners,
            rounding: Some(corner),
        })
    }
    pub fn point(&mut self, point: Point3) -> Result<RectangleDefinition, GeometryError> {
        let c = self.pending.ok_or(invalid("rectangle corner input"))?;
        let x = c[0].vector_to(c[1])?;
        let y = c[0].vector_to(c[3])?;
        let width = x.length()?;
        let height = y.length()?;
        let frame = viboceros_geometry::Frame3::try_from_directions(
            c[0],
            x,
            y,
            Tolerance::NUMERICAL_VALIDATION,
        )?;
        let [x, y, _] = frame.coordinates_of(point)?;
        let dx = x.abs().min((width - x).abs());
        let dy = y.abs().min((height - y).abs());
        let value = match self.options.kind {
            Kind::Arc => dx + dy + std::f64::consts::SQRT_2 * dx.sqrt() * dy.sqrt(),
            Kind::Conic => {
                let u = dx / (width * 0.5);
                let v = dy / (height * 0.5);
                let a = 1. - u - v;
                let b = 2. * (u * v).sqrt();
                let rho = a / (a + b);
                if rho.is_finite() && rho > 0. && rho < 1. {
                    rho
                } else {
                    self.options.rho
                }
            }
        };
        self.number(value)
    }
    pub fn plane(self) -> Option<viboceros_geometry::Frame3> {
        let c = self.pending?;
        viboceros_geometry::Frame3::try_from_directions(
            c[0],
            c[0].vector_to(c[1]).ok()?,
            c[0].vector_to(c[3]).ok()?,
            Tolerance::NUMERICAL_VALIDATION,
        )
        .ok()
    }
}
fn invalid(context: &'static str) -> GeometryError {
    GeometryError::Degenerate { context }
}
impl RectangleDefinition {
    pub fn geometry(self, tolerance: Tolerance) -> Result<Geometry, GeometryError> {
        match self.rounding {
            None => Ok(Geometry::Polyline(self.polyline(tolerance)?)),
            Some(Corner::Arc(radius)) if (0. ..=2.3283064365386963e-10).contains(&radius) => {
                let mut lines = Vec::with_capacity(4);
                for i in 0..4 {
                    lines.push(CurveSegment3::Line(
                        viboceros_geometry::LineSegment::try_new(
                            self.corners[i],
                            self.corners[(i + 1) % 4],
                            Tolerance::NUMERICAL_VALIDATION,
                        )?,
                    ));
                }
                Ok(Geometry::PolyCurve(PolyCurve3::try_new(lines)?))
            }
            Some(Corner::Arc(radius)) => {
                let source = self.polyline(Tolerance::NUMERICAL_VALIDATION)?;
                let curve = match source.try_fillet_corners(radius, Tolerance::NUMERICAL_VALIDATION)
                {
                    Ok(curve) => curve,
                    Err(GeometryError::InvalidPolyCurve {
                        context:
                            "appended parameter span loses significant precision at this offset",
                    }) => source.try_fillet_corners_with_uniform_parameters(
                        radius,
                        Tolerance::NUMERICAL_VALIDATION,
                    )?,
                    Err(error) => return Err(error),
                };
                // Rectangle's seam starts on edge zero, after its first corner.
                let mut pieces = curve.segments().to_vec();
                pieces.rotate_left(1);
                let mut merged: Vec<CurveSegment3> = Vec::with_capacity(pieces.len());
                for piece in pieces {
                    if let (Some(CurveSegment3::Arc(a)), CurveSegment3::Arc(b)) =
                        (merged.last(), &piece)
                    {
                        let start = a.evaluate(*a.domain().start())?;
                        let normal = a.normal()?;
                        let circle = viboceros_geometry::Circle3::try_from_frame(
                            a.center(),
                            a.radius(),
                            a.center().vector_to(start)?.normalized_nonzero()?,
                            normal,
                            Tolerance::NUMERICAL_VALIDATION,
                        )?;
                        let sweep = ((a.sweep_radians() + b.sweep_radians())
                            / std::f64::consts::FRAC_PI_2)
                            .round()
                            * std::f64::consts::FRAC_PI_2;
                        let arc =
                            viboceros_geometry::CircularArc3::try_from_circle_sweep(circle, sweep)?;
                        *merged.last_mut().unwrap() = CurveSegment3::Arc(arc);
                    } else {
                        merged.push(piece);
                    }
                }
                if let [only] = merged.as_slice() {
                    return Ok(Geometry::Arc(
                        match only {
                            CurveSegment3::Arc(arc) => *arc,
                            _ => return Err(invalid("rounded rectangle circle")),
                        }
                        .try_reparameterized(0. ..=only.length(tolerance)?)?,
                    ));
                }
                let pieces = merged
                    .into_iter()
                    .map(|s| s.try_reparameterized(0. ..=s.length(tolerance)?))
                    .collect::<Result<Vec<_>, GeometryError>>()?;
                let curve = match PolyCurve3::try_new(pieces.clone()) {
                    Ok(curve) => curve,
                    Err(GeometryError::InvalidPolyCurve {
                        context:
                            "appended parameter span loses significant precision at this offset",
                    }) => {
                        // Tiny corners cannot retain length-sized outer spans
                        // at a large accumulated offset. Uniform outer spans
                        // retain the exact leaves without that precision loss.
                        let parameters = (0..=pieces.len()).map(|i| i as Real).collect();
                        PolyCurve3::try_with_segment_domains(pieces, parameters)?
                    }
                    Err(error) => return Err(error),
                };
                Ok(Geometry::PolyCurve(curve))
            }
            Some(Corner::Conic(rho)) => {
                if !rho.is_finite() || rho <= 0. || rho >= 1. {
                    return Err(invalid("rectangle conic rho"));
                }
                let midpoint = |i: usize| self.corners[i].midpoint(self.corners[(i + 1) % 4]);
                let mut pieces = Vec::with_capacity(4);
                for i in 0..4 {
                    let corner = self.corners[(i + 1) % 4];
                    let start = midpoint(i)?;
                    let end = midpoint((i + 1) % 4)?;
                    pieces.push(CurveSegment3::NurbsCurve(
                        viboceros_geometry::NurbsCurve::try_conic(start, corner, end, rho)?,
                    ));
                }
                Ok(Geometry::PolyCurve(PolyCurve3::try_new(pieces)?))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CommandContext, CommandRegistry};
    use viboceros_document::Document;
    #[test]
    fn tiny_rounded_corner_has_stable_geometry_even_when_length_domains_lose_precision() {
        let definition = RectangleDefinition::from_corners(
            CommandContext::default().construction_plane,
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(10., 6., 0.).unwrap(),
        )
        .unwrap();
        for radius in [1e-8, 1e-9] {
            let shape = RectangleDefinition {
                rounding: Some(Corner::Arc(radius)),
                ..definition
            };
            let geometry = shape.geometry(Tolerance::DEFAULT).unwrap();
            let curve = geometry.curve_ref().unwrap();
            assert!(curve.is_closed().unwrap());
            let expected = 32. - (8. - std::f64::consts::TAU) * radius;
            assert!(
                (curve.length(Tolerance::NUMERICAL_VALIDATION).unwrap() - expected).abs() < 1e-12
            );
            let nurbs = curve.to_nurbs().unwrap();
            assert_eq!(nurbs.degree(), 2);
            let Geometry::PolyCurve(poly) = geometry else {
                panic!()
            };
            assert_eq!(poly.segments().len(), 8);
        }
    }
    #[test]
    fn rounded_input_errors_are_atomic_and_preferences_belong_to_the_registry() {
        let registry = CommandRegistry::with_builtins();
        let mut doc = Document::default();
        for text in [
            "Rectangle Rounded 0,0,0 10,6,0 Corner=Conic 0",
            "Rectangle Rounded 0,0,0 10,6,0 Corner=Conic 1",
            "Rectangle Rounded 0,0,0 10,6,0 Corner=Arc NaN",
        ] {
            let before = format!("{doc:?}");
            assert!(registry.execute(&mut doc, text).is_err());
            assert_eq!(format!("{doc:?}"), before);
        }
        registry
            .execute(&mut doc, "Rectangle Rounded 0,0,0 10,6,0 Corner=Arc -9")
            .unwrap();
        assert_eq!(registry.rectangle_options().radius, 3.);
        doc.undo().unwrap();
        registry
            .execute(&mut doc, "Rectangle Rounded 0,0,0 10,6,0 Enter")
            .unwrap();
        assert_eq!(registry.rectangle_options().radius, 3.);
        assert_eq!(
            CommandRegistry::with_builtins().rectangle_options(),
            Options::default()
        );
    }
}
