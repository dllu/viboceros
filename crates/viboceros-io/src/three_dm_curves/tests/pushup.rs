//! Retained VIB-02 fixture: an exact rational UV ellipse on a warped support.
use super::*;
use viboceros_geometry::{
    Brep, BrepEdge, BrepFace, BrepLoop, BrepLoopType, BrepTrim, BrepTrimType, BrepVertex,
    NurbsCurve2, NurbsSurface, Point2, SurfaceIso, Vector3, WeightedPoint2,
};

fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

fn uv(x: f64, y: f64) -> Point2 {
    Point2::try_new(x, y).unwrap()
}

fn ellipse() -> NurbsCurve2 {
    NurbsCurve2::try_new_rational(
        2,
        [
            (0.75, 0.5),
            (0.75, 0.625),
            (0.5, 0.625),
            (0.25, 0.625),
            (0.25, 0.5),
            (0.25, 0.375),
            (0.5, 0.375),
            (0.75, 0.375),
            (0.75, 0.5),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (x, y))| {
            WeightedPoint2::try_new(
                uv(x, y),
                if i % 2 == 0 {
                    1.
                } else {
                    std::f64::consts::FRAC_1_SQRT_2
                },
            )
            .unwrap()
        })
        .collect(),
        vec![0., 0., 0., 1., 1., 2., 2., 3., 3., 4., 4., 4.],
    )
    .unwrap()
}

fn support(offset: f64) -> NurbsSurface {
    NurbsSurface::try_bilinear([
        point(0., 0., offset),
        point(1., 0., offset),
        point(1., 1., 1. + offset),
        point(0., 1., offset),
    ])
    .unwrap()
}

// The fixture's wall has its own compatible support. Here the exact endpoint
// controls AND weights coincide, so sharing them needs no gauge adjustment,
// fitting, or knot removal. Export must leave this support unchanged.
fn wall_profile(source: &NurbsCurve) -> NurbsCurve {
    let pieces = source.try_split_at_full_order_knots().unwrap();
    let degree = source.degree();
    let mut controls = pieces[0].control_points().to_vec();
    let mut knots = vec![*source.domain().start(); degree + 1];
    for piece in &pieces[1..] {
        assert_eq!(controls.last(), piece.control_points().first());
        knots.extend(std::iter::repeat_n(*piece.domain().start(), degree));
        controls.extend_from_slice(&piece.control_points()[1..]);
    }
    knots.extend(std::iter::repeat_n(*source.domain().end(), degree + 1));
    NurbsCurve::try_new_rational(degree, controls, knots).unwrap()
}

fn trim(
    vertices: [usize; 2],
    edge: usize,
    reversed: bool,
    curve: NurbsCurve2,
    kind: BrepTrimType,
    iso: SurfaceIso,
) -> BrepTrim {
    BrepTrim::try_new(vertices, Some(edge), reversed, curve, kind, iso, [1e-8; 2]).unwrap()
}

fn line(a: Point2, b: Point2) -> NurbsCurve2 {
    NurbsCurve2::try_new(1, vec![a, b], vec![0., 0., 1., 1.]).unwrap()
}

fn panel() -> Brep {
    let tolerance = Tolerance::DEFAULT;
    let ellipse_uv = ellipse();
    let top = support(0.);
    let bottom = support(-0.125);
    let (top_edge, top_bound) = top
        .try_pushup_curve_certified_with_bound(&ellipse_uv, tolerance)
        .unwrap();
    let (bottom_edge, bottom_bound) = bottom
        .try_pushup_curve_certified_with_bound(&ellipse_uv, tolerance)
        .unwrap();
    assert!(top_bound <= tolerance.absolute() && bottom_bound <= tolerance.absolute());
    assert_eq!(
        top_edge.full_order_knots().collect::<Vec<_>>(),
        [1., 2., 3.]
    );
    assert_eq!(
        bottom_edge.full_order_knots().collect::<Vec<_>>(),
        [1., 2., 3.]
    );
    let wall = NurbsSurface::try_extruded_curve(
        &wall_profile(&top_edge),
        Vector3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., -0.125).unwrap(),
    )
    .unwrap();
    let a = top_edge.evaluate(0.).unwrap();
    let b = bottom_edge.evaluate(0.).unwrap();
    let vertices = vec![
        BrepVertex::try_new(a, 1e-8).unwrap(),
        BrepVertex::try_new(b, 1e-8).unwrap(),
    ];
    let edges = vec![
        BrepEdge::try_new([0, 0], top_edge, 1e-8).unwrap(),
        BrepEdge::try_new([1, 1], bottom_edge, 1e-8).unwrap(),
        BrepEdge::try_new(
            [0, 1],
            NurbsCurve::try_new(1, vec![a, b], vec![0., 0., 0.125, 0.125]).unwrap(),
            1e-8,
        )
        .unwrap(),
    ];
    let loop_for = |trims| BrepLoop::try_new(BrepLoopType::Outer, trims).unwrap();
    let faces = vec![
        BrepFace::try_new(
            top,
            false,
            vec![loop_for(vec![trim(
                [0, 0],
                0,
                false,
                ellipse_uv.clone(),
                BrepTrimType::Mated,
                SurfaceIso::NotIso,
            )])],
        )
        .unwrap(),
        BrepFace::try_new(
            bottom,
            true,
            vec![loop_for(vec![trim(
                [1, 1],
                1,
                false,
                ellipse_uv,
                BrepTrimType::Mated,
                SurfaceIso::NotIso,
            )])],
        )
        .unwrap(),
        BrepFace::try_new(
            wall,
            true,
            vec![loop_for(vec![
                trim(
                    [0, 0],
                    0,
                    false,
                    line(uv(0., 0.), uv(4., 0.)),
                    BrepTrimType::Mated,
                    SurfaceIso::South,
                ),
                trim(
                    [0, 1],
                    2,
                    false,
                    line(uv(4., 0.), uv(4., 0.125)),
                    BrepTrimType::Seam,
                    SurfaceIso::East,
                ),
                trim(
                    [1, 1],
                    1,
                    true,
                    line(uv(4., 0.125), uv(0., 0.125)),
                    BrepTrimType::Mated,
                    SurfaceIso::North,
                ),
                trim(
                    [1, 0],
                    2,
                    true,
                    line(uv(0., 0.125), uv(0., 0.)),
                    BrepTrimType::Seam,
                    SurfaceIso::West,
                ),
            ])],
        )
        .unwrap(),
    ];
    Brep::try_new(vertices, edges, faces, tolerance).unwrap()
}

#[test]
fn certified_ellipse_pushup_trimmed_solid_exports_without_refitting() {
    let source = panel();
    assert!(source.is_solid());
    let original = model(ThreeDmGeometry::Brep(source.clone()));
    let preserved = original.clone();
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("certified-pushup-panel.3dm");
    let adapted = prepare(&original.objects[0].geometry, original.tolerance).unwrap();
    let ThreeDmGeometry::Brep(adapted) = adapted[0].as_ref() else {
        panic!("expected adapted B-rep");
    };
    for (a, b) in adapted.faces().iter().zip(source.faces()) {
        assert_eq!(a.surface(), b.surface());
    }
    let report = write_3dm_file(&path, &original).unwrap();
    assert_eq!(report.source_object_count, 1);
    assert_eq!(report.written_object_count, 1);
    assert_eq!(report.adapted_curve_count, 1);
    assert_eq!(original, preserved);
    if let Some(destination) = std::env::var_os("VIBOCEROS_PUSHUP_FIXTURE") {
        std::fs::copy(&path, destination).unwrap();
    }
    let decoded = read_3dm_file(&path, Tolerance::DEFAULT).unwrap();
    assert_eq!(decoded.unsupported_object_count(), 0);
    assert_eq!(decoded.objects.len(), 1);
    let mut attributes = decoded.objects[0].clone();
    attributes.geometry = original.objects[0].geometry.clone();
    assert_eq!(attributes, original.objects[0]);
    let ThreeDmGeometry::Brep(actual) = &decoded.objects[0].geometry else {
        panic!("expected B-rep");
    };
    check_geometry(actual, &source);
}

fn check_geometry(actual: &Brep, source: &Brep) {
    assert!(actual.is_solid());
    assert_eq!(actual.faces().len(), source.faces().len());
    assert_eq!(actual.edges().len(), 9);
    assert!(actual.edge_use_counts().iter().all(|&count| count == 2));
    for (a, b) in actual.faces().iter().zip(source.faces()) {
        assert_eq!(a.surface().knots_u(), b.surface().knots_u());
        assert_eq!(a.surface().knots_v(), b.surface().knots_v());
        assert_eq!(a.surface().degree_u(), b.surface().degree_u());
        assert_eq!(a.surface().degree_v(), b.surface().degree_v());
        assert_eq!(
            a.surface().control_points().len(),
            b.surface().control_points().len()
        );
        for (a, b) in a
            .surface()
            .control_points()
            .iter()
            .zip(b.surface().control_points())
        {
            assert!(a.point().distance_to(b.point()).unwrap() < 1e-14);
            assert!((a.weight() - b.weight()).abs() < 1e-14);
        }
        assert_eq!(a.is_reversed(), b.is_reversed());
    }
    for edge in actual.edges() {
        assert!(edge.curve().full_order_knots().next().is_none());
        let original_edge = if edge.curve().degree() == 1 {
            &source.edges()[2]
        } else if edge
            .curve()
            .evaluate(*edge.curve().domain().start())
            .unwrap()
            .distance_to(
                source.edges()[0]
                    .curve()
                    .evaluate(*edge.curve().domain().start())
                    .unwrap(),
            )
            .unwrap()
            < 1e-12
        {
            &source.edges()[0]
        } else {
            &source.edges()[1]
        };
        for i in 0..=32 {
            let t = edge.curve().parameter_at(i as f64 / 32.).unwrap();
            assert!(
                edge.curve()
                    .evaluate(t)
                    .unwrap()
                    .distance_to(original_edge.curve().evaluate(t).unwrap())
                    .unwrap()
                    < 1e-12
            );
        }
    }
}

#[test]
fn independent_rational_edge_gauges_and_reversed_faces_survive_export() {
    let original = panel();
    let edges = original
        .edges()
        .iter()
        .map(|edge| {
            let curve = edge.curve();
            if curve.full_order_knots().next().is_none() {
                return edge.clone();
            }
            let controls = curve
                .control_points()
                .iter()
                .enumerate()
                .map(|(index, control)| {
                    let piece = index / (curve.degree() + 1);
                    let scale = match piece {
                        0 => 2.0_f64.powi(-700),
                        1 => -2.0_f64.powi(700),
                        2 => 2.0_f64.powi(700),
                        _ => -2.0_f64.powi(-700),
                    };
                    WeightedPoint3::try_new(control.point(), control.weight() * scale).unwrap()
                })
                .collect();
            BrepEdge::try_new(
                edge.vertices(),
                NurbsCurve::try_new_rational(curve.degree(), controls, curve.knots().to_vec())
                    .unwrap(),
                edge.tolerance(),
            )
            .unwrap()
        })
        .collect();
    let source = Brep::try_new(
        original.vertices().to_vec(),
        edges,
        original.faces().to_vec(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    for source in [source.clone(), source.reversed()] {
        let original = model(ThreeDmGeometry::Brep(source.clone()));
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("independent-edge-gauges.3dm");
        write_3dm_file(&path, &original).unwrap();
        let decoded = read_3dm_file(&path, Tolerance::DEFAULT).unwrap();
        assert_eq!(decoded.unsupported_object_count(), 0);
        assert_eq!(decoded.objects.len(), 1);
        let ThreeDmGeometry::Brep(actual) = &decoded.objects[0].geometry else {
            panic!("expected B-rep");
        };
        check_geometry(actual, &source);
    }
}

#[test]
fn ordinary_breps_need_no_topology_conversion() {
    let brep = Brep::try_box(
        viboceros_geometry::Frame3::try_from_normal(
            point(0., 0., 0.),
            Vector3::try_new(0., 0., 1.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap(),
        [[0., 1.], [0., 2.], [0., 3.]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let source = ThreeDmGeometry::Brep(brep);
    assert!(matches!(
        &prepare(&source, Tolerance::DEFAULT).unwrap()[0],
        Cow::Borrowed(_)
    ));
}
