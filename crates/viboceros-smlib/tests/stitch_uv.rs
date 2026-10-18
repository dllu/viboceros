#![cfg(feature = "native")]
use std::ops::RangeInclusive;
use viboceros_geometry::{Brep, NurbsSurface, Point3, Tolerance, WeightedPoint3};
use viboceros_smlib::Shape;

fn sheet(height: f64, short: bool) -> NurbsSurface {
    let count = if short { 7 } else { 3 };
    let knots = if short {
        vec![0., 0., 0., 0.5, 0.5, 0.5 + 1e-14, 0.5 + 1e-14, 1., 1., 1.]
    } else {
        vec![0., 0., 0., 1., 1., 1.]
    };
    NurbsSurface::try_new_rational(
        2,
        1,
        count,
        2,
        [0., height]
            .into_iter()
            .flat_map(|z| {
                (0..count).map(move |i| {
                    WeightedPoint3::try_new(
                        Point3::try_new(
                            4. * i as f64,
                            if short {
                                (i % 3) as f64 * 2.
                            } else if i == 1 {
                                2.
                            } else {
                                0.
                            },
                            z,
                        )
                        .unwrap(),
                        if short {
                            [1., 0.75, 1.25, 1., 0.5, 1., 1.5][i]
                        } else {
                            1.
                        },
                    )
                    .unwrap()
                })
            })
            .collect(),
        knots,
        vec![0., 0., 1., 1.],
    )
    .unwrap()
}

fn check(u: RangeInclusive<f64>, v: RangeInclusive<f64>, short: bool, swapped: bool) {
    eprintln!("STITCH_UV_CASE u={u:?} v={v:?} short={short} swapped={swapped}");
    // Keep construction independent of the document's modeling tolerance,
    // as in the retained Join recipes that expose the remote-chart failure.
    let construction = Tolerance::DEFAULT;
    let t = Tolerance::try_new(0.001, 1e-12, 1e-10).unwrap();
    let sources = [sheet(3., short), sheet(-3., short)].map(|s| {
        let s = s.try_reparameterized(u.clone(), v.clone()).unwrap();
        let s = if swapped {
            s.try_swapped_uv().unwrap()
        } else {
            s
        };
        Brep::try_surface_face(s, construction).unwrap()
    });
    let input = Brep::try_combine(sources.to_vec(), construction).unwrap();
    let before = input.clone();
    let native = Shape::from_brep(&input, t).unwrap();
    let native_before = native.to_brep(t).unwrap();
    let result = native.stitch(1e-8).unwrap();
    let raw = result.shape.to_brep(t).unwrap();
    let output = result.to_brep_preserving_source_uv(&input, t).unwrap();
    assert_eq!(output.faces().len(), 2);
    assert_eq!(output.edges().len(), 7);
    assert_eq!(
        output.edge_use_counts().iter().filter(|&&n| n == 2).count(),
        1
    );
    assert!(output.is_manifold());
    assert_eq!(output.vertices(), raw.vertices());
    assert_eq!(output.edges(), raw.edges());
    for (face, &origin) in output.faces().iter().zip(&result.face_origins) {
        let expected = input.faces()[origin].surface();
        assert_eq!(
            face.surface(),
            expected,
            "source={origin} short={short} swapped={swapped}"
        );
    }
    assert_eq!(native.to_brep(t).unwrap(), native_before);
    assert_eq!(input, before);
}

#[test]
fn stitching_preserves_remote_and_anisotropic_source_charts() {
    let charts = [
        (0. ..=1., 0. ..=1.),
        (1e12..=1e12 + 1., 1e12..=1e12 + 1.),
        (1e12..=1e12 + 1., -2e12..=-2e12 + 2.),
        (1e12..=1e12 + 1., 0. ..=2_f64.powi(-40)),
    ];
    let mut checked = 0;
    for (u, v) in charts {
        for swapped in [false, true] {
            check(u.clone(), v.clone(), false, swapped);
            checked += 1;
        }
    }
    assert_eq!(checked, 8);
    eprintln!("STITCH_UV_CHARTS cases={checked}");
}

#[test]
fn normal_unification_retains_narrow_interior_knot_spans() {
    for swapped in [false, true] {
        check(0. ..=1., -2. ..=3., true, swapped);
    }
    eprintln!("STITCH_UV_NARROW cases=2");
}
