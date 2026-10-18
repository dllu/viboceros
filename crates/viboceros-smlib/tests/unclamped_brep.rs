#![cfg(feature = "native")]
use viboceros_geometry::{
    Brep, BrepEdge, BrepFace, BrepLoop, BrepTrim, Frame3, NurbsCurve, NurbsCurve2, NurbsSurface,
    Point3, Tolerance, Vector3, WeightedPoint3,
};
use viboceros_smlib::{BooleanOperation, Shape, Solid};

fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

fn patch(u: &[f64], v: &[f64], scale: f64) -> NurbsSurface {
    let nu = u.len() - 3;
    let nv = v.len() - 3;
    let controls = (0..nv)
        .flat_map(|j| {
            (0..nu).map(move |i| {
                let (x, y) = (i as f64, j as f64);
                WeightedPoint3::try_new(
                    point(x, y, 0.125 * (x * x + x * y)),
                    1. + 0.125 * ((i + 2 * j) % 3) as f64,
                )
                .unwrap()
            })
        })
        .collect();
    NurbsSurface::try_new_rational(
        2,
        2,
        nu,
        nv,
        controls,
        u.iter().map(|k| k * scale).collect(),
        v.iter().map(|k| k * scale).collect(),
    )
    .unwrap()
}

fn check_surface(source: &NurbsSurface, output: &NurbsSurface) -> f64 {
    assert_eq!(source.domain_u(), output.domain_u());
    assert_eq!(source.domain_v(), output.domain_v());
    assert_eq!(source.degree_u(), output.degree_u());
    assert_eq!(source.degree_v(), output.degree_v());
    // Independent Rust knot insertion supplies the complete expected tensor
    // definition, as well as the point samples below. Common weight gauges
    // carry the same rational geometry.
    let expected = source.try_clamped_to_active_domain().unwrap();
    assert_eq!(expected.knots_u(), output.knots_u());
    assert_eq!(expected.knots_v(), output.knots_v());
    assert_eq!(
        expected.control_points().len(),
        output.control_points().len()
    );
    let ew = expected.control_points()[0].weight();
    let ow = output.control_points()[0].weight();
    for (a, b) in expected
        .control_points()
        .iter()
        .zip(output.control_points())
    {
        assert!(a.point().distance_to(b.point()).unwrap() < 1e-11);
        assert!((a.weight() / ew - b.weight() / ow).abs() < 1e-12);
    }
    let mut maximum = 0_f64;
    for i in 0..=16 {
        for j in 0..=16 {
            let u = source.parameter_at_u(i as f64 / 16.).unwrap();
            let v = source.parameter_at_v(j as f64 / 16.).unwrap();
            let gap = source
                .evaluate(u, v)
                .unwrap()
                .distance_to(output.evaluate(u, v).unwrap())
                .unwrap();
            maximum = maximum.max(gap);
            assert!(gap < 1e-10, "u={u:e} v={v:e} gap={gap:e}");
        }
    }
    maximum
}

fn unclamped_lines(source: &Brep, edges: bool, uv: bool) -> Brep {
    let padding =
        |start: f64, end: f64| vec![start - (end - start), start, end, end + (end - start)];
    let edge_data = source
        .edges()
        .iter()
        .map(|edge| {
            let c = edge.curve();
            assert_eq!(c.degree(), 1);
            let curve = if edges {
                NurbsCurve::try_new_rational(
                    1,
                    c.control_points().to_vec(),
                    padding(*c.domain().start(), *c.domain().end()),
                )
                .unwrap()
            } else {
                c.clone()
            };
            BrepEdge::try_new(edge.vertices(), curve, edge.tolerance()).unwrap()
        })
        .collect();
    let faces = source
        .faces()
        .iter()
        .map(|face| {
            let loops = face
                .loops()
                .iter()
                .map(|boundary| {
                    BrepLoop::try_new(
                        boundary.loop_type(),
                        boundary
                            .trims()
                            .iter()
                            .map(|t| {
                                let c = t.curve();
                                assert_eq!(c.degree(), 1);
                                let curve = if uv {
                                    NurbsCurve2::try_new_rational(
                                        1,
                                        c.control_points().to_vec(),
                                        padding(*c.domain().start(), *c.domain().end()),
                                    )
                                    .unwrap()
                                } else {
                                    c.clone()
                                };
                                BrepTrim::try_new(
                                    t.vertices(),
                                    t.edge(),
                                    t.is_reversed_3d(),
                                    curve,
                                    t.trim_type(),
                                    t.iso(),
                                    t.tolerance(),
                                )
                                .unwrap()
                            })
                            .collect(),
                    )
                    .unwrap()
                })
                .collect();
            BrepFace::try_new(face.surface().clone(), face.is_reversed(), loops).unwrap()
        })
        .collect();
    Brep::try_new(
        source.vertices().to_vec(),
        edge_data,
        faces,
        Tolerance::DEFAULT,
    )
    .unwrap()
}

#[test]
fn native_sheet_import_clamps_both_active_axes_without_moving_the_surface() {
    let knots: [&[f64]; 5] = [
        &[-1., -1., -1., 0., 1., 1., 1.],
        &[-3., -2., -1., 0., 1., 2., 3.],
        &[-3., -2., -1., 0., 1., 1., 1.],
        &[-1., -1., -1., 0., 1., 2., 3.],
        &[-5., -5., 0., 0., 1., 1., 5., 5.],
    ];
    let mut checked = 0;
    let mut maximum = 0_f64;
    for u in knots {
        for v in knots {
            for exponent in [-40, 0, 40] {
                let surface = patch(u, v, 2_f64.powi(exponent));
                let source = Brep::try_surface_face(surface.clone(), Tolerance::DEFAULT).unwrap();
                let before = source.clone();
                let output = Shape::from_brep(&source, Tolerance::DEFAULT)
                    .unwrap_or_else(|e| panic!("u={u:?}, v={v:?}, exponent={exponent}: {e}"))
                    .to_brep(Tolerance::DEFAULT)
                    .unwrap();
                assert_eq!(output.faces().len(), 1);
                assert_eq!(output.edge_use_counts(), vec![1; 4]);
                maximum = maximum.max(check_surface(&surface, output.faces()[0].surface()));
                assert_eq!(source, before);
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 75);
    eprintln!("UNCLAMPED_SHEET cases={checked} maximum_point_error={maximum:e}");
}

#[test]
fn native_brep_import_accepts_unclamped_spatial_edges_and_uv_trims() {
    let source = Brep::try_surface_face(
        NurbsSurface::try_bilinear([
            point(0., 0., 0.),
            point(4., 0., 0.),
            point(4., 3., 0.),
            point(0., 3., 0.),
        ])
        .unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    for edges in [false, true] {
        for uv in [false, true] {
            let input = unclamped_lines(&source, edges, uv);
            let before = input.clone();
            let output = Shape::from_brep(&input, Tolerance::DEFAULT)
                .unwrap()
                .to_brep(Tolerance::DEFAULT)
                .unwrap();
            assert!((output.area(Tolerance::DEFAULT).unwrap() - 12.).abs() < 1e-10);
            assert_eq!(output.edge_use_counts(), vec![1; 4]);
            for (a, b) in input.edges().iter().zip(output.edges()) {
                assert_eq!(a.vertices(), b.vertices());
                assert_eq!(a.curve().domain(), b.curve().domain());
                for i in 0..=16 {
                    let t = a.curve().parameter_at(i as f64 / 16.).unwrap();
                    assert!(
                        a.curve()
                            .evaluate(t)
                            .unwrap()
                            .distance_to(b.curve().evaluate(t).unwrap())
                            .unwrap()
                            < 1e-11
                    );
                }
            }
            assert_eq!(input, before);
        }
    }
}

#[test]
fn unclamped_periodic_sheet_retains_its_seam_and_active_surface() {
    let ring = [[2., 0.], [0., 2.], [-2., 0.], [0., -2.], [2., 0.], [0., 2.]];
    let surface = NurbsSurface::try_new_rational(
        2,
        1,
        6,
        2,
        [0., 3.]
            .into_iter()
            .flat_map(|z| {
                ring.into_iter().enumerate().map(move |(i, p)| {
                    WeightedPoint3::try_new(
                        point(p[0], p[1], z),
                        if i % 2 == 0 { 1. } else { 0.75 },
                    )
                    .unwrap()
                })
            })
            .collect(),
        vec![-2., -1., 0., 1., 2., 3., 4., 5., 6.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    assert!(surface.is_periodic_u());
    let input = Brep::try_surface_face(surface.clone(), Tolerance::DEFAULT).unwrap();
    let before = input.clone();
    let output = Shape::from_brep(&input, Tolerance::DEFAULT)
        .unwrap()
        .to_brep(Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(output.faces().len(), 1);
    assert_eq!(output.edge_use_counts(), input.edge_use_counts());
    assert!(output.edge_use_counts().contains(&2));
    check_surface(&surface, output.faces()[0].surface());
    assert_eq!(input, before);
}

#[test]
fn unclamped_solid_import_preserves_shared_topology_and_followup_boolean() {
    let t = Tolerance::DEFAULT;
    let frame =
        Frame3::try_from_normal(point(0., 0., 0.), Vector3::try_new(0., 0., 1.).unwrap(), t)
            .unwrap();
    let cube = Brep::try_box(frame, [[0., 2.], [0., 3.], [0., 4.]], t).unwrap();
    let faces = cube
        .faces()
        .iter()
        .map(|face| {
            let s = face.surface();
            assert_eq!((s.degree_u(), s.degree_v()), (1, 1));
            let padding = |a: f64, b: f64| vec![a - (b - a), a, b, b + (b - a)];
            let surface = NurbsSurface::try_new_rational(
                1,
                1,
                2,
                2,
                s.control_points().to_vec(),
                padding(*s.domain_u().start(), *s.domain_u().end()),
                padding(*s.domain_v().start(), *s.domain_v().end()),
            )
            .unwrap();
            BrepFace::try_new(surface, face.is_reversed(), face.loops().to_vec()).unwrap()
        })
        .collect();
    let input = Brep::try_new(cube.vertices().to_vec(), cube.edges().to_vec(), faces, t).unwrap();
    let input = unclamped_lines(&input, true, true);
    let before = input.clone();
    let native = Solid::from_brep(&input, t).unwrap();
    let output = native.to_brep(t).unwrap();
    assert!(output.is_solid());
    assert_eq!(output.vertices().len(), 8);
    assert_eq!(output.edges().len(), 12);
    assert_eq!(output.edge_use_counts(), vec![2; 12]);
    assert_eq!(output.faces().len(), 6);
    assert!((output.signed_volume(t).unwrap() - 24.).abs() < 1e-9);
    let cutter = Solid::box_solid(point(-1., -1., -1.), [2., 5., 6.]).unwrap();
    let cut = native
        .boolean(&cutter, BooleanOperation::Difference)
        .unwrap()
        .to_brep(t)
        .unwrap();
    assert!(cut.is_solid());
    assert!((cut.signed_volume(t).unwrap() - 12.).abs() < 1e-9);
    assert!((native.to_brep(t).unwrap().signed_volume(t).unwrap() - 24.).abs() < 1e-9);
    assert_eq!(input, before);
}
