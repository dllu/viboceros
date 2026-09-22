use super::*;
use crate::TriangleMesh;
use num_traits::Signed;

impl TriangleMesh {
    /// Area and centroid of the face triangles, splitting quads along their
    /// shorter spatial diagonal (A-C on a tie). This is neither a volume
    /// centroid nor a vertex average. Unused vertices contribute no mass.
    pub fn area_mass_properties(&self) -> Result<AreaMassProperties, GeometryError> {
        let mut result = AreaMassProperties::default();
        let mut areas = crate::FiniteSum::default();
        let mut first: [products::Products; 3] =
            std::array::from_fn(|_| products::Products::default());
        for points in self.mass_triangles() {
            let fast_area = points[0]
                .vector_to(points[1])
                .and_then(|a| a.half_cross_length(points[0].vector_to(points[2])?));
            if let Ok(area) = fast_area
                && area > 0.
            {
                areas.add(area)?;
                for point in points {
                    for (sum, value) in first.iter_mut().zip(point.to_array()) {
                        sum.add(area, value)?;
                    }
                }
                continue;
            }
            let exact = points.map(|p| p.to_array().map(rational));
            let delta = |end: usize| -> [Rational; 3] {
                std::array::from_fn(|axis| &exact[end][axis] - &exact[0][axis])
            };
            let (a, b) = (delta(1), delta(2));
            let cross: [Rational; 3] = std::array::from_fn(|axis| {
                let (j, k) = ((axis + 1) % 3, (axis + 2) % 3);
                &a[j] * &b[k] - &a[k] * &b[j]
            });
            let scale = cross.iter().map(Signed::abs).max().unwrap();
            if scale.is_zero() {
                continue;
            }
            // Normalize only the final exact cross product: extreme aspect
            // ratios and overflowing coordinate differences cannot erase it.
            let norm = scalar(&(&cross[0] / &scale))?
                .hypot(scalar(&(&cross[1] / &scale))?)
                .hypot(scalar(&(&cross[2] / &scale))?);
            let area = scale * rational(norm * 0.5);
            result.area += &area;
            for (axis, first) in result.first.iter_mut().enumerate() {
                let sum = &exact[0][axis] + &exact[1][axis] + &exact[2][axis];
                *first += sum * &area / Rational::from_integer(3.into());
            }
        }
        result.area += areas.exact_total();
        for (sum, values) in result.first.iter_mut().zip(first) {
            *sum += values.total() / Rational::from_integer(3.into());
        }
        result.centroid()?;
        Ok(result)
    }
}
