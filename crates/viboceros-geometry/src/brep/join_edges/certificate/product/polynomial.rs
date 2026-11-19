//! A whole-span interval certificate for cubic curves with constant weights.
use super::*;
use crate::interval::I;

type Point = [I; 3];
type Cubic = [Point; 4];

struct Spline {
    knots: Vec<I>,
    points: Vec<Point>,
}

pub(super) struct Pair([Spline; 2]);

impl Pair {
    pub(super) fn new(
        a: &extract::Spline<'_>,
        b: &extract::Spline<'_>,
        charge: &mut impl FnMut(usize) -> Result<(), GeometryError>,
    ) -> Result<Option<Self>, GeometryError> {
        if a.degree() != 3 || b.degree() != 3 {
            return Ok(None);
        }
        charge(2 * (a.knots.len() + b.knots.len()))?;
        Ok(Spline::new(a)
            .zip(Spline::new(b))
            .map(|(a, b)| Self([a, b])))
    }

    pub(super) fn bound(
        &self,
        spans: [usize; 2],
        ends: [&Rational; 2],
        limit: Real,
        charge: &mut impl FnMut(usize) -> Result<(), GeometryError>,
    ) -> Result<Option<Real>, GeometryError> {
        // Each cubic extraction has 16 vector blends. The remaining work
        // covers the knot intervals and the difference's four hull vertices.
        charge(2 * 16 * 3 + 40)?;
        let result = (|| {
            let ends = [I::exact(ends[0])?, I::exact(ends[1])?];
            let a = self.0[0].extract(spans[0], ends)?;
            let b = self.0[1].extract(spans[1], ends)?;
            let origin = Point3::try_new(0., 0., 0.).ok()?;
            let mut upper: Real = 0.;
            for (a, b) in a.into_iter().zip(b) {
                let vertex = Point3::try_new(
                    a[0].sub(b[0])?.abs()?.hi,
                    a[1].sub(b[1])?.abs()?.hi,
                    a[2].sub(b[2])?.abs()?.hi,
                )
                .ok()?;
                upper = upper.max(point_bound(origin, vertex, limit)?);
            }
            // Matching Bernstein bases make the curve difference a convex
            // combination of these coefficient differences.
            Some(upper)
        })();
        Ok(result)
    }
}

impl Spline {
    fn new(spline: &extract::Spline<'_>) -> Option<Self> {
        Some(Self {
            points: spline
                .polynomial_controls()?
                .map(|p| Some([I::point(p.x())?, I::point(p.y())?, I::point(p.z())?]))
                .collect::<Option<_>>()?,
            knots: spline.knots.iter().map(I::exact).collect::<Option<_>>()?,
        })
    }

    fn extract(&self, span: usize, ends: [I; 2]) -> Option<Cubic> {
        let source: Cubic = self.points[span - 3..=span].try_into().ok()?;
        let mut work = [source; 4];
        // After each level the rows hold the polar evaluations with that
        // many right arguments. Share intermediate rows across coefficients.
        for level in 1..=3 {
            work[level] = work[level - 1];
            for j in (level..=3).rev() {
                let k = span - 3 + j;
                let low = self.knots[k];
                let width = self.knots[k + 4 - level].sub(low)?;
                let fraction = |end: I| {
                    let t = end.sub(low)?.div(width)?;
                    // The exact parameter lies in the active span, which
                    // is contained in this pair of local knots.
                    Some(I {
                        lo: t.lo.max(0.),
                        hi: t.hi.min(1.),
                    })
                };
                let left = fraction(ends[0])?;
                for row in &mut work[..level] {
                    row[j] = blend(row[j - 1], row[j], left)?;
                }
                let row = &mut work[level];
                row[j] = blend(row[j - 1], row[j], fraction(ends[1])?)?;
            }
        }
        Some(work.map(|row| row[3]))
    }
}

fn blend(a: Point, b: Point, t: I) -> Option<Point> {
    let axis = |i: usize| a[i].add(t.mul(b[i].sub(a[i])?)?);
    Some([axis(0)?, axis(1)?, axis(2)?])
}
