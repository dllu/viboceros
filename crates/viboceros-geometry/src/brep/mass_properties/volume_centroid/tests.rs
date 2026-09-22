use super::super::tests::{paraboloid, round_trim};
use super::*;

#[test]
fn trimmed_paraboloid_volume_first_moments_match_analytic_radial_integrals() {
    for radius in [0.8_f64, 0.5, 0.01] {
        let brep = round_trim(paraboloid(), &[radius], true);
        // Integral 2*pi*r*(radius^2-r^2) dr, with z first moment.
        let volume = std::f64::consts::PI * radius.powi(4) / 2.;
        let z = 2. * radius * radius / 3.;
        for (b, sign) in [(&brep, 1.), (&brep.reversed(), -1.)] {
            let mass = b
                .volume_mass_properties(Tolerance::try_new(1e-11, 1e-13, 1e-10).unwrap())
                .unwrap();
            assert!(
                (mass.signed_volume().unwrap() - sign * volume).abs() < 1e-11,
                "{:?}",
                mass.signed_volume()
            );
            assert!(
                mass.centroid()
                    .unwrap()
                    .distance_to(Point3::try_new(0., 0., z).unwrap())
                    .unwrap()
                    < 1e-10
            );
        }
    }
}
