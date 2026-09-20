use super::*;
use crate::brep::mass_properties::tests::{paraboloid, round_trim};
use crate::brep::parameter_frame::tests::translated_face;

fn annulus() -> Brep {
    round_trim(paraboloid(), &[0.5, 0.25], false)
}

#[test]
fn homogeneous_trimmed_bounds_already_preserve_large_uv_translations() {
    let source = annulus();
    for offset in [[0., 0.], [1e12, -2e12], [-1e12, 2e12]] {
        let mut brep = source.clone();
        brep.faces[0] = translated_face(&source.faces[0], offset);
        brep.validate(Tolerance::DEFAULT).unwrap();
        let original = brep.clone();
        let tolerance = Tolerance::try_new(1e-10, 1e-13, 1e-10).unwrap();
        for bounds in [
            brep.tight_bounds(tolerance).unwrap(),
            brep.faces[0].tight_bounds(tolerance).unwrap(),
            brep.faces[0].trim_boundary_bounds(tolerance).unwrap(),
        ] {
            for (point, expected) in [
                (bounds.min(), [-0.5, -0.5, 0.0625]),
                (bounds.max(), [0.5, 0.5, 0.25]),
            ] {
                for (actual, expected) in point.to_array().into_iter().zip(expected) {
                    assert!(
                        (actual - expected).abs() < 1.1e-10,
                        "offset={offset:?}, bounds={bounds:?}"
                    );
                }
            }
        }
        assert_eq!(brep, original);
    }
}

#[test]
fn trimmed_isocurve_geometry_survives_unrepresentable_native_intersections() {
    let source = annulus();
    let fixed = 0.125;
    let outer = (0.5_f64.powi(2) - fixed * fixed).sqrt();
    let inner = (0.25_f64.powi(2) - fixed * fixed).sqrt();
    for offset in [[0., 0.], [1e12, -2e12], [-1e12, 2e12]] {
        let face = translated_face(&source.faces[0], offset);
        let original = face.clone();
        for axis in 0..2 {
            let curves = if axis == 0 {
                face.isocurve_u_segments(offset[1] + fixed, Tolerance::DEFAULT)
            } else {
                face.isocurve_v_segments(offset[0] + fixed, Tolerance::DEFAULT)
            }
            .unwrap();
            assert_eq!(curves.len(), 2, "offset={offset:?}, axis={axis}");
            for (curve, ends) in curves.iter().zip([[-outer, -inner], [inner, outer]]) {
                for fraction in [0., 0.125, 0.3, 0.5, 0.875, 1.] {
                    let p = curve
                        .evaluate(curve.parameter_at(fraction).unwrap())
                        .unwrap();
                    let varying = ends[0].mul_add(1. - fraction, ends[1] * fraction);
                    let mut xy = [fixed; 2];
                    xy[axis] = varying;
                    let expected =
                        Point3::try_new(xy[0], xy[1], varying * varying + fixed * fixed).unwrap();
                    assert!(
                        p.distance_to(expected).unwrap() < 2e-12,
                        "offset={offset:?}, axis={axis}, point={p:?}, expected={expected:?}"
                    );
                }
            }
        }
        assert_eq!(face, original);
    }
}

#[test]
fn wireframe_sampling_keeps_large_uv_offsets_out_of_model_space() {
    let baseline = annulus();
    let mut source = baseline.clone();
    source.faces[0] = translated_face(&source.faces[0], [1e12, -2e12]);
    source.validate(Tolerance::DEFAULT).unwrap();
    let original = source.clone();
    for density in [1, 3, 5] {
        let expected = baseline
            .wireframe_curves(density, Tolerance::DEFAULT)
            .unwrap();
        let actual = source
            .wireframe_curves(density, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(actual.len(), expected.len());
        assert!(actual.len() > source.edges.len());
        for (a, b) in actual.iter().zip(expected) {
            for f in [0., 0.125, 0.3, 0.5, 0.875, 1.] {
                let a = a.evaluate(a.parameter_at(f).unwrap()).unwrap();
                let b = b.evaluate(b.parameter_at(f).unwrap()).unwrap();
                assert!(
                    a.distance_to(b).unwrap() < 2e-12,
                    "density={density}: {a:?} != {b:?}"
                );
            }
        }
    }
    assert_eq!(source, original);
}

#[test]
fn density_extraction_preserves_geometry_and_native_parameter_errors() {
    let source = annulus();
    let face = translated_face(&source.faces[0], [1e12, -2e12]);
    for density in [1, 3, 5] {
        for axis in 0..2 {
            let extract = |f: &BrepFace| {
                if axis == 0 {
                    f.isocurve_u_segments_at_density(density, Tolerance::DEFAULT)
                } else {
                    f.isocurve_v_segments_at_density(density, Tolerance::DEFAULT)
                }
            };
            let a = extract(&source.faces[0]).unwrap();
            let b = extract(&face).unwrap();
            assert_eq!(a.len(), b.len());
            for (a, b) in a.iter().zip(b) {
                // Compare loci independently of the curve's chosen parameter
                // origin; native output domains are retained when exact.
                let a = a.try_reparameterized(0.0..=1.0).unwrap();
                let b = b.try_reparameterized(0.0..=1.0).unwrap();
                for t in [0., 0.1, 0.3, 0.5, 0.9, 1.] {
                    assert!(
                        a.evaluate(t)
                            .unwrap()
                            .distance_to(b.evaluate(t).unwrap())
                            .unwrap()
                            < 2e-12
                    );
                }
            }
        }
    }
    for axis in 0..2 {
        let domain = if axis == 0 {
            face.surface.domain_v()
        } else {
            face.surface.domain_u()
        };
        let query = domain.end().next_up();
        let error = if axis == 0 {
            face.isocurve_u_segments(query, Tolerance::DEFAULT)
        } else {
            face.isocurve_v_segments(query, Tolerance::DEFAULT)
        }
        .unwrap_err();
        assert_eq!(
            error,
            GeometryError::ParameterOutOfDomain {
                parameter: query,
                domain_start: *domain.start(),
                domain_end: *domain.end(),
            }
        );
    }
    assert!(matches!(
        face.isocurve_u_segments(Real::NAN, Tolerance::DEFAULT),
        Err(GeometryError::NonFinite { .. })
    ));
    assert!(matches!(
        face.isocurve_v_segments_at_density(-2, Tolerance::DEFAULT),
        Err(GeometryError::InvalidSurfaceWireDensity(-2))
    ));
}
