//! Polynomial flux integration for point-grid shell orientation. N-point
//! Gauss quadrature is exact through degree 2N-1; a degree-(p,q) polynomial
//! surface has a volume-flux integrand of degrees (3p-1,3q-1).

use super::*;
use crate::nurbs::bspline_basis_values;

struct Station {
    weight: Real,
    basis: Vec<(usize, Real, Real)>,
}

pub(super) fn is_inward(surface: &NurbsSurface) -> Result<Option<bool>, GeometryError> {
    if surface.control_points().iter().any(|c| c.weight() != 1.0) {
        return Ok(None);
    }
    let nu = surface.control_point_count_u();
    let origin = surface.control_points()[0].point().to_array();
    let local = surface
        .control_points()
        .iter()
        .map(|p| {
            let p = p.point().to_array();
            let delta = std::array::from_fn::<_, 3, _>(|i| p[i] - origin[i]);
            require_finite(delta, "point grid orientation coordinates")?;
            Ok(delta)
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    let scale = local.iter().flatten().map(|x| x.abs()).fold(0.0, Real::max);
    if scale == 0.0 {
        return Ok(None);
    }
    let local = local
        .into_iter()
        .map(|p| p.map(|x| x / scale))
        .collect::<Vec<_>>();
    let u = stations(surface.knots_u(), surface.degree_u(), nu)?;
    let v = stations(
        surface.knots_v(),
        surface.degree_v(),
        surface.control_point_count_v(),
    )?;
    let (mut sum, mut correction, mut bound) = (0.0, 0.0, 0.0);
    for b in &v {
        for a in &u {
            let (mut p, mut du, mut dv) = ([0.0; 3], [0.0; 3], [0.0; 3]);
            for &(j, y, dy) in &b.basis {
                for &(i, x, dx) in &a.basis {
                    for (axis, c) in local[j * nu + i].into_iter().enumerate() {
                        p[axis] = (x * y).mul_add(c, p[axis]);
                        du[axis] = (dx * y).mul_add(c, du[axis]);
                        dv[axis] = (x * dy).mul_add(c, dv[axis]);
                    }
                }
            }
            let weight = a.weight * b.weight;
            let mut flux = 0.0;
            for (axis, coordinate) in p.into_iter().enumerate() {
                let (j, k) = ((axis + 1) % 3, (axis + 2) % 3);
                flux = coordinate.mul_add(du[j].mul_add(dv[k], -du[k] * dv[j]), flux);
                bound +=
                    weight * coordinate.abs() * ((du[j] * dv[k]).abs() + (du[k] * dv[j]).abs());
            }
            let term = flux * weight;
            let next = sum + term;
            correction += if sum.abs() >= term.abs() {
                (sum - next) + term
            } else {
                (term - next) + sum
            };
            sum = next;
        }
    }
    let value = sum + correction;
    require_finite([value, bound], "point grid orientation flux")?;
    Ok((value.abs() > 8192.0 * Real::EPSILON * bound).then_some(value < 0.0))
}

fn stations(knots: &[Real], degree: usize, count: usize) -> Result<Vec<Station>, GeometryError> {
    let quadrature = gauss((3 * degree).div_ceil(2));
    let mut result = vec![];
    for span in degree..count {
        let (start, end) = (knots[span], knots[span + 1]);
        if start == end {
            continue;
        }
        let width = end - start;
        for &(node, weight) in &quadrature {
            let t = width.mul_add(node, start);
            let basis = bspline_basis_values(knots, degree, count, t)?;
            let lower = bspline_basis_values(&knots[1..knots.len() - 1], degree - 1, count - 1, t)?;
            let mut derivative = vec![0.0; count];
            for (i, b) in lower.into_iter().enumerate().filter(|(_, b)| *b != 0.0) {
                let value = (width / (knots[i + degree + 1] - knots[i + 1])) * degree as Real * b;
                derivative[i] -= value;
                derivative[i + 1] += value;
            }
            result.push(Station {
                weight,
                basis: basis
                    .into_iter()
                    .zip(derivative)
                    .enumerate()
                    .filter_map(|(i, (a, b))| (a != 0.0 || b != 0.0).then_some((i, a, b)))
                    .collect(),
            });
        }
    }
    Ok(result)
}

fn gauss(count: usize) -> Vec<(Real, Real)> {
    let legendre = |x: Real| {
        let (mut previous, mut current) = (1.0, x);
        for n in 2..=count {
            let next = ((2 * n - 1) as Real * x * current - (n - 1) as Real * previous) / n as Real;
            previous = current;
            current = next;
        }
        (
            current,
            count as Real * (x * current - previous) / (x * x - 1.0),
        )
    };
    let mut result = vec![(0.0, 0.0); count];
    for i in 0..count.div_ceil(2) {
        let mut x = (std::f64::consts::PI * (i as Real + 0.75) / (count as Real + 0.5)).cos();
        for _ in 0..32 {
            let (p, d) = legendre(x);
            let next = x - p / d;
            if next == x {
                break;
            }
            x = next;
        }
        let (_, d) = legendre(x);
        let weight = 1.0 / ((1.0 - x * x) * d * d);
        result[i] = ((1.0 - x) * 0.5, weight);
        result[count - 1 - i] = ((1.0 + x) * 0.5, weight);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folded_degree_eleven_grid_retains_positive_signed_volume_orientation() {
        let count = [13, 12];
        let points = (0..count[1])
            .flat_map(|v| {
                (0..count[0]).map(move |u| {
                    let (u, v) = (u as Real, v as Real);
                    Point3::try_new(u, v, (0.3 * u).sin() * (0.2 * v).cos()).unwrap()
                })
            })
            .collect::<Vec<_>>();
        let s = NurbsSurface::try_through_point_grid(&points, count, [11; 2], [true; 2]).unwrap();
        // Independent 70-digit integration of Z X' Y' (X=X(u), Y=Y(v))
        // gives positive volume 4.7657446554. Rhino's command flips this
        // folded, non-embedded shell; reproducing that flip would invert the
        // true signed volume. Keep the diagnostic explicit, not a golden flip.
        assert_eq!(is_inward(&s).unwrap(), Some(false));
    }

    #[test]
    fn polynomial_orientation_matches_independent_brep_volume_and_global_reversal() {
        let count = [6, 5];
        for degree in [[1, 1], [3, 2]] {
            for reflect in [false, true] {
                let points = (0..count[1])
                    .flat_map(|v| {
                        (0..count[0]).map(move |u| {
                            let a = std::f64::consts::TAU * u as Real / count[0] as Real;
                            let b = std::f64::consts::TAU * v as Real / count[1] as Real;
                            let radius = 3.0 + b.cos();
                            Point3::try_new(
                                radius * a.cos(),
                                radius * a.sin(),
                                if reflect { -b.sin() } else { b.sin() },
                            )
                            .unwrap()
                        })
                    })
                    .collect::<Vec<_>>();
                let surface =
                    NurbsSurface::try_through_point_grid(&points, count, degree, [true; 2])
                        .unwrap();
                let tolerance = Tolerance::DEFAULT;
                let [u, v] = surface
                    .sampled_kink_parameters(0.1_f64.to_radians())
                    .unwrap();
                let raw = Brep::try_surface_grid(&surface, &u, &v, tolerance).unwrap();
                assert!(raw.is_solid());
                let volume = raw.signed_volume(tolerance).unwrap();
                assert!(volume.abs() > 1.0);
                assert_eq!(is_inward(&surface).unwrap(), Some(volume < 0.0));
                assert_eq!(volume < 0.0, reflect);

                let reversed = raw.reversed();
                assert!(reversed.is_solid());
                assert_eq!(reversed.reversed(), raw);
                assert_eq!(reversed.edges(), raw.edges());
                assert_eq!(reversed.vertices(), raw.vertices());
                for (a, b) in raw.faces().iter().zip(reversed.faces()) {
                    assert_eq!(a.surface(), b.surface());
                }
                assert!((reversed.signed_volume(tolerance).unwrap() + volume).abs() < 1e-10);
                let oriented = command_brep(&surface, tolerance).unwrap();
                assert!(oriented.signed_volume(tolerance).unwrap() > 0.0);
            }
        }
    }

    #[test]
    fn gaussian_rule_integrates_every_required_polynomial_degree() {
        for n in 2..=17 {
            for degree in 0..2 * n {
                let actual = gauss(n)
                    .iter()
                    .map(|(x, w)| w * x.powi(degree as i32))
                    .sum::<Real>();
                assert!((actual - 1.0 / (degree + 1) as Real).abs() < 1e-14);
            }
        }
    }
}
