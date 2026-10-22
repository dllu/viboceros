//! surfaces split mapping command regressions.
use super::*;

#[test]
fn cutting_object_split_preserves_exact_curved_paths_on_warped_bilinear_surfaces() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let surface = NurbsSurface::try_bilinear([
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Point3::try_new(10.0, 0.0, 0.0).unwrap(),
        Point3::try_new(10.0, 10.0, 10.0).unwrap(),
        Point3::try_new(0.0, 10.0, 0.0).unwrap(),
    ])
    .unwrap()
    .try_reparameterized(0.0..=10.0, 0.0..=10.0)
    .unwrap();
    let source_id = document
        .add_geometry(Geometry::NurbsSurface(surface.clone()))
        .unwrap();
    let cutter = NurbsCurve::try_new(
        3,
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(10.0 / 3.0, 0.0, 0.0).unwrap(),
            Point3::try_new(20.0 / 3.0, 10.0 / 3.0, 0.0).unwrap(),
            Point3::try_new(10.0, 10.0, 10.0).unwrap(),
        ],
        vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let cutter_id = document
        .add_geometry(Geometry::NurbsCurve(cutter.clone()))
        .unwrap();
    document
        .select_objects_direct([source_id, cutter_id], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "Split CuttingObjects=8,7,5.6")
            .unwrap(),
        "Split the selected surface along 1 complete boundary-to-boundary curve into 2 exact B-rep faces"
    );
    assert!(document.object(source_id).is_none());
    assert!(!document.is_selected(cutter_id));
    let outputs = document.selected_objects().collect::<Vec<_>>();
    assert_eq!(outputs.len(), 2);
    let mut forward_trim = None;
    let mut coarse_vertex_count = 0;
    let mut dense_vertex_count = 0;
    for object in outputs {
        let Geometry::Brep(brep) = object.geometry() else {
            panic!("warped surface cutting Split must create B-rep faces")
        };
        assert_eq!(brep.vertices().len(), 3);
        assert_eq!(brep.faces()[0].surface(), &surface);
        let trim = brep.faces()[0].loops()[0]
            .trims()
            .iter()
            .find(|trim| trim.iso() == viboceros_geometry::SurfaceIso::NotIso)
            .unwrap();
        assert_eq!(brep.edges()[trim.edge().unwrap()].curve(), &cutter);
        assert_eq!(trim.curve().degree(), 3);
        assert_eq!(trim.curve().control_points().len(), 4);
        if !trim.is_reversed_3d() {
            forward_trim = Some(trim.curve().clone());
        }
        coarse_vertex_count += brep
            .tessellate(1, document.tolerance())
            .unwrap()
            .vertices()
            .len();
        dense_vertex_count += brep
            .tessellate(4, document.tolerance())
            .unwrap()
            .vertices()
            .len();
    }
    let forward_trim = forward_trim.unwrap();
    assert_eq!(forward_trim.knots(), cutter.knots());
    for (actual, expected) in forward_trim.control_points().iter().zip([
        [0.0, 0.0],
        [10.0 / 3.0, 0.0],
        [20.0 / 3.0, 10.0 / 3.0],
        [10.0, 10.0],
    ]) {
        assert!(
            document
                .tolerance()
                .approx_eq(actual.point().x(), expected[0])
        );
        assert!(
            document
                .tolerance()
                .approx_eq(actual.point().y(), expected[1])
        );
    }
    assert!(dense_vertex_count > coarse_vertex_count);
}

#[test]
fn cutting_object_split_preserves_curved_paths_on_degree_elevated_affine_surfaces() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let surface = NurbsSurface::try_bilinear([
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Point3::try_new(10.0, 0.0, 0.0).unwrap(),
        Point3::try_new(10.0, 10.0, 0.0).unwrap(),
        Point3::try_new(0.0, 10.0, 0.0).unwrap(),
    ])
    .unwrap()
    .try_reparameterized(0.0..=10.0, 0.0..=10.0)
    .unwrap()
    .try_change_degree(2, 2, false)
    .unwrap();
    let source_id = document
        .add_geometry(Geometry::NurbsSurface(surface.clone()))
        .unwrap();
    let cutter = NurbsCurve::try_new(
        2,
        vec![
            Point3::try_new(0.0, 2.0, 0.0).unwrap(),
            Point3::try_new(5.0, 8.0, 0.0).unwrap(),
            Point3::try_new(10.0, 6.0, 0.0).unwrap(),
        ],
        vec![0.0, 0.0, 0.0, 2.0, 2.0, 2.0],
    )
    .unwrap();
    let cutter_id = document
        .add_geometry(Geometry::NurbsCurve(cutter.clone()))
        .unwrap();
    document
        .select_objects_direct([source_id, cutter_id], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "Split CuttingObjects=5,1,0")
            .unwrap(),
        "Split the selected surface along 1 complete boundary-to-boundary curve into 2 exact B-rep faces"
    );
    assert!(document.object(source_id).is_none());
    assert!(!document.is_selected(cutter_id));
    let outputs = document.selected_objects().collect::<Vec<_>>();
    assert_eq!(outputs.len(), 2);
    let mut forward_trim = None;
    let mut area = 0.0;
    for object in outputs {
        let Geometry::Brep(brep) = object.geometry() else {
            panic!("degree-elevated surface cutting Split must create B-rep faces")
        };
        assert_eq!(brep.faces()[0].surface(), &surface);
        assert_eq!(brep.vertices().len(), 4);
        assert_eq!(brep.edges().len(), 4);
        let trim = brep.faces()[0].loops()[0]
            .trims()
            .iter()
            .find(|trim| trim.iso() == viboceros_geometry::SurfaceIso::NotIso)
            .unwrap();
        assert_eq!(brep.edges()[trim.edge().unwrap()].curve(), &cutter);
        if !trim.is_reversed_3d() {
            forward_trim = Some(trim.curve().clone());
        }
        area += brep.area(document.tolerance()).unwrap();
        brep.tessellate(3, document.tolerance()).unwrap();
    }
    assert!(document.tolerance().approx_eq(area, 100.0));

    let forward_trim = forward_trim.unwrap();
    assert_eq!(forward_trim.degree(), cutter.degree());
    assert_eq!(forward_trim.knots(), cutter.knots());
    assert_eq!(forward_trim.domain(), cutter.domain());
    for (actual, expected) in
        forward_trim
            .control_points()
            .iter()
            .zip([[0.0, 2.0], [5.0, 8.0], [10.0, 6.0]])
    {
        assert!(
            document
                .tolerance()
                .approx_eq(actual.point().x(), expected[0])
        );
        assert!(
            document
                .tolerance()
                .approx_eq(actual.point().y(), expected[1])
        );
        assert_eq!(actual.weight(), 1.0);
    }
}

#[test]
fn cutting_object_split_preserves_curved_paths_on_projective_surfaces() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let surface = NurbsSurface::try_new_rational(
        1,
        1,
        2,
        2,
        vec![
            WeightedPoint3::try_new(Point3::try_new(0.0, 0.0, 0.0).unwrap(), 1.0).unwrap(),
            WeightedPoint3::try_new(Point3::try_new(5.0, 0.0, 0.0).unwrap(), 2.0).unwrap(),
            WeightedPoint3::try_new(Point3::try_new(0.0, 20.0 / 3.0, 0.0).unwrap(), 1.5).unwrap(),
            WeightedPoint3::try_new(Point3::try_new(4.0, 4.0, 0.0).unwrap(), 2.5).unwrap(),
        ],
        vec![0.0, 0.0, 1.0, 1.0],
        vec![0.0, 0.0, 1.0, 1.0],
    )
    .unwrap();
    let source_id = document
        .add_geometry(Geometry::NurbsSurface(surface.clone()))
        .unwrap();
    let cutter = NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(Point3::try_new(0.0, 20.0 / 11.0, 0.0).unwrap(), 1.1).unwrap(),
            WeightedPoint3::try_new(Point3::try_new(50.0 / 19.0, 80.0 / 19.0, 0.0).unwrap(), 1.9)
                .unwrap(),
            WeightedPoint3::try_new(
                Point3::try_new(100.0 / 23.0, 60.0 / 23.0, 0.0).unwrap(),
                2.3,
            )
            .unwrap(),
        ],
        vec![0.0, 0.0, 0.0, 2.0, 2.0, 2.0],
    )
    .unwrap();
    let cutter_id = document
        .add_geometry(Geometry::NurbsCurve(cutter.clone()))
        .unwrap();
    document
        .select_objects_direct([source_id, cutter_id], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "Split CuttingObjects=3.3333333333333335,0,0",)
            .unwrap(),
        "Split the selected surface along 1 complete boundary-to-boundary curve into 2 exact B-rep faces"
    );
    assert!(document.object(source_id).is_none());
    assert!(!document.is_selected(cutter_id));
    let outputs = document.selected_objects().collect::<Vec<_>>();
    assert_eq!(outputs.len(), 2);
    let mut forward_cut = None;
    let mut area = 0.0;
    for object in outputs {
        let Geometry::Brep(brep) = object.geometry() else {
            panic!("projective surface cutting Split must create B-rep faces")
        };
        assert_eq!(brep.faces()[0].surface(), &surface);
        assert_eq!(brep.vertices().len(), 4);
        assert_eq!(brep.edges().len(), 4);
        let trim = brep.faces()[0].loops()[0]
            .trims()
            .iter()
            .find(|trim| trim.iso() == viboceros_geometry::SurfaceIso::NotIso)
            .unwrap();
        let edge = brep.edges()[trim.edge().unwrap()].curve();
        assert_eq!(edge.degree(), cutter.degree());
        assert_eq!(edge.knots(), cutter.knots());
        assert_eq!(edge.domain(), cutter.domain());
        assert_eq!(edge.control_points()[0].weight(), 1.0);
        assert_eq!(edge.control_points()[2].weight(), 1.0);
        if !trim.is_reversed_3d() {
            forward_cut = Some((trim.curve().clone(), edge.clone()));
        }
        area += brep.area(document.tolerance()).unwrap();
        brep.tessellate(3, document.tolerance()).unwrap();
    }
    assert!(
        document
            .tolerance()
            .approx_eq(area, surface.area(document.tolerance()).unwrap())
    );

    let (forward_trim, forward_edge) = forward_cut.unwrap();
    assert_eq!(forward_trim.degree(), cutter.degree());
    assert_eq!(forward_trim.knots(), cutter.knots());
    assert_eq!(forward_trim.domain(), cutter.domain());
    for (actual, expected) in
        forward_trim
            .control_points()
            .iter()
            .zip([[0.0, 0.2], [0.5, 0.8], [1.0, 0.6]])
    {
        assert!(
            document
                .tolerance()
                .approx_eq(actual.point().x(), expected[0])
        );
        assert!(
            document
                .tolerance()
                .approx_eq(actual.point().y(), expected[1])
        );
    }
    let weights = forward_trim.control_points();
    assert!(document.tolerance().approx_eq(
        weights[1].weight() / weights[0].weight(),
        (1.1_f64 / 2.3).sqrt()
    ));
    assert!(
        document
            .tolerance()
            .approx_eq(weights[2].weight() / weights[0].weight(), 1.1 / 2.3)
    );
    for sample in 0..=32 {
        let parameter = 2.0 * sample as Real / 32.0;
        let uv = forward_trim.evaluate(parameter).unwrap();
        assert!(surface.evaluate(uv.x(), uv.y()).unwrap().is_near(
            forward_edge.evaluate(parameter).unwrap(),
            document.tolerance()
        ));
    }
}

#[test]
fn cutting_object_split_pulls_curves_back_on_nonaffine_planar_surfaces() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let surface = nonaffine_planar_surface();
    let source_id = document
        .add_geometry(Geometry::NurbsSurface(surface.clone()))
        .unwrap();
    let cutter = NurbsCurve::try_new(
        3,
        vec![
            Point3::try_new(0.0, 2.0, 0.0).unwrap(),
            Point3::try_new(3.2, 20.0 / 3.0, 0.0).unwrap(),
            Point3::try_new(82.0 / 15.0, 8.0, 0.0).unwrap(),
            Point3::try_new(8.8, 6.0, 0.0).unwrap(),
        ],
        vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let cutter_id = document
        .add_geometry(Geometry::NurbsCurve(cutter.clone()))
        .unwrap();
    document
        .select_objects_direct([source_id, cutter_id], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "Split CuttingObjects=5,0,0")
            .unwrap(),
        "Split the selected surface along 1 complete boundary-to-boundary curve into 2 exact B-rep faces"
    );
    assert!(document.object(source_id).is_none());
    assert!(!document.is_selected(cutter_id));
    let outputs = document.selected_objects().collect::<Vec<_>>();
    assert_eq!(outputs.len(), 2);
    let mut forward_cut = None;
    let mut area = 0.0;
    for object in outputs {
        let Geometry::Brep(brep) = object.geometry() else {
            panic!("non-affine planar surface Split must create B-rep faces")
        };
        assert_eq!(brep.faces()[0].surface(), &surface);
        assert_eq!(brep.vertices().len(), 4);
        assert_eq!(brep.edges().len(), 4);
        let trim = brep.faces()[0].loops()[0]
            .trims()
            .iter()
            .find(|trim| trim.iso() == viboceros_geometry::SurfaceIso::NotIso)
            .unwrap();
        let edge = brep.edges()[trim.edge().unwrap()].curve();
        assert_eq!(edge, &cutter);
        if !trim.is_reversed_3d() {
            forward_cut = Some((trim.curve().clone(), edge.clone()));
        }
        area += brep.area(document.tolerance()).unwrap();
        brep.tessellate(3, document.tolerance()).unwrap();
    }
    assert!(
        document
            .tolerance()
            .approx_eq(area, surface.area(document.tolerance()).unwrap())
    );

    let (forward_trim, forward_edge) = forward_cut.unwrap();
    assert_eq!(forward_trim.degree(), 3);
    assert_eq!(forward_trim.domain(), cutter.domain());
    assert_eq!(
        forward_trim.knots(),
        &[0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]
    );
    for (actual, expected) in forward_trim.control_points().iter().zip([
        [0.0, 0.2],
        [1.0 / 3.0, 2.0 / 3.0],
        [2.0 / 3.0, 0.8],
        [1.0, 0.6],
    ]) {
        assert!(
            document
                .tolerance()
                .approx_eq(actual.point().x(), expected[0])
        );
        assert!(
            document
                .tolerance()
                .approx_eq(actual.point().y(), expected[1])
        );
        assert_eq!(actual.weight(), 1.0);
    }
    for sample in 0..=32 {
        let parameter = sample as Real / 32.0;
        let uv = forward_trim.evaluate(parameter).unwrap();
        assert!(surface.evaluate(uv.x(), uv.y()).unwrap().is_near(
            forward_edge.evaluate(parameter).unwrap(),
            document.tolerance()
        ));
    }

    let mut adaptive_document = Document::default();
    let adaptive_source_id = adaptive_document
        .add_geometry(Geometry::NurbsSurface(surface.clone()))
        .unwrap();
    let adaptive_cutter = NurbsCurve::try_new(
        2,
        vec![
            Point3::try_new(0.0, 2.0, 0.0).unwrap(),
            Point3::try_new(5.0, 9.0, 0.0).unwrap(),
            Point3::try_new(8.8, 6.0, 0.0).unwrap(),
        ],
        vec![2.0, 2.0, 2.0, 5.0, 5.0, 5.0],
    )
    .unwrap();
    let adaptive_cutter_id = adaptive_document
        .add_geometry(Geometry::NurbsCurve(adaptive_cutter))
        .unwrap();
    adaptive_document
        .select_objects_direct(
            [adaptive_source_id, adaptive_cutter_id],
            SelectionMode::Replace,
        )
        .unwrap();
    registry
        .execute(&mut adaptive_document, "Split CuttingObjects=5,0,0")
        .unwrap();
    assert!(adaptive_document.object(adaptive_source_id).is_none());
    assert!(!adaptive_document.is_selected(adaptive_cutter_id));
    let adaptive_outputs = adaptive_document.selected_objects().collect::<Vec<_>>();
    assert_eq!(adaptive_outputs.len(), 2);
    for object in adaptive_outputs {
        let Geometry::Brep(brep) = object.geometry() else {
            panic!("adaptive non-affine planar Split must create B-rep faces")
        };
        let trim = brep.faces()[0].loops()[0]
            .trims()
            .iter()
            .find(|trim| trim.iso() == viboceros_geometry::SurfaceIso::NotIso)
            .unwrap();
        assert_eq!(trim.curve().degree(), 3);
        assert!(trim.curve().control_points().len() > 4);
        let edge = brep.edges()[trim.edge().unwrap()].curve();
        for (span_start, span_end) in trim.curve().spans() {
            let parameter = span_start.mul_add(0.5, span_end * 0.5);
            let uv = trim.curve().evaluate(parameter).unwrap();
            let edge_parameter = if trim.is_reversed_3d() {
                -parameter
            } else {
                parameter
            };
            assert!(surface.evaluate(uv.x(), uv.y()).unwrap().is_near(
                edge.evaluate(edge_parameter).unwrap(),
                adaptive_document.tolerance()
            ));
        }
        brep.tessellate(3, adaptive_document.tolerance()).unwrap();
    }
}

#[test]
fn cutting_object_split_clips_curves_on_trimmed_nonaffine_surfaces() {
    let registry = CommandRegistry::with_builtins();
    let exact_cutter = NurbsCurve::try_new(
        3,
        vec![
            Point3::try_new(0.0, 2.0, 0.0).unwrap(),
            Point3::try_new(3.2, 20.0 / 3.0, 0.0).unwrap(),
            Point3::try_new(82.0 / 15.0, 8.0, 0.0).unwrap(),
            Point3::try_new(8.8, 6.0, 0.0).unwrap(),
        ],
        vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let adaptive_cutter = NurbsCurve::try_new(
        2,
        vec![
            Point3::try_new(0.0, 2.0, 0.0).unwrap(),
            Point3::try_new(5.0, 9.0, 0.0).unwrap(),
            Point3::try_new(8.8, 6.0, 0.0).unwrap(),
        ],
        vec![2.0, 2.0, 2.0, 5.0, 5.0, 5.0],
    )
    .unwrap();

    for (cutter, expects_exact_pullback) in [(exact_cutter, true), (adaptive_cutter, false)] {
        let mut document = Document::default();
        let surface = nonaffine_planar_surface();
        let source_id = document
            .add_geometry(Geometry::Brep(
                Brep::try_rectangular_surface_face(
                    surface.clone(),
                    0.0..=0.7,
                    0.0..=0.75,
                    document.tolerance(),
                )
                .unwrap(),
            ))
            .unwrap();
        let original_domain = cutter.domain();
        let cutter_id = document
            .add_geometry(Geometry::NurbsCurve(cutter.clone()))
            .unwrap();
        document
            .select_objects_direct([source_id, cutter_id], SelectionMode::Replace)
            .unwrap();

        assert_eq!(
                registry
                    .execute(&mut document, "Split CuttingObjects=1,1,0")
                    .unwrap_or_else(|error| {
                        panic!(
                            "non-affine clipping failed (exact pullback: {expects_exact_pullback}): {error:?}"
                        )
                    }),
                "Split the selected surface along 1 complete boundary-to-boundary curve into 2 exact B-rep faces"
            );
        assert!(document.object(source_id).is_none());
        assert!(!document.is_selected(cutter_id));
        let outputs = document.selected_objects().collect::<Vec<_>>();
        assert_eq!(outputs.len(), 2);
        let mut area = 0.0;
        let mut cut_domains = Vec::new();
        let mut trim_reversals = Vec::new();
        for object in outputs {
            let Geometry::Brep(brep) = object.geometry() else {
                panic!("trimmed non-affine Split must create B-rep faces")
            };
            assert_eq!(brep.faces().len(), 1);
            assert_eq!(brep.faces()[0].surface(), &surface);
            assert_eq!(brep.vertices().len(), 4);
            assert_eq!(brep.edges().len(), 4);
            let trim = brep.faces()[0].loops()[0]
                .trims()
                .iter()
                .find(|trim| trim.iso() == viboceros_geometry::SurfaceIso::NotIso)
                .unwrap();
            assert_eq!(trim.curve().degree(), 3);
            if expects_exact_pullback {
                assert_eq!(trim.curve().control_points().len(), 4);
            } else {
                assert!(trim.curve().control_points().len() > 4);
            }
            let edge = brep.edges()[trim.edge().unwrap()].curve();
            assert_eq!(edge.degree(), cutter.degree());
            assert!(*edge.domain().start() >= *original_domain.start());
            assert!(*edge.domain().end() < *original_domain.end());
            cut_domains.push(edge.domain());
            trim_reversals.push(trim.is_reversed_3d());

            let trim_domain = trim.curve().domain();
            let mut endpoint_u = [
                trim.curve().evaluate(*trim_domain.start()).unwrap().x(),
                trim.curve().evaluate(*trim_domain.end()).unwrap().x(),
            ];
            endpoint_u.sort_by(Real::total_cmp);
            assert!(document.tolerance().approx_eq(endpoint_u[0], 0.0));
            assert!(document.tolerance().approx_eq(endpoint_u[1], 0.7));
            for (span_start, span_end) in trim.curve().spans() {
                for normalized in [0.0, 0.5, 1.0] {
                    let parameter = (span_end - span_start).mul_add(normalized, span_start);
                    let uv = trim.curve().evaluate(parameter).unwrap();
                    let edge_parameter = if trim.is_reversed_3d() {
                        -parameter
                    } else {
                        parameter
                    };
                    assert!(
                        surface
                            .evaluate(uv.x(), uv.y())
                            .unwrap()
                            .is_near(edge.evaluate(edge_parameter).unwrap(), document.tolerance())
                    );
                }
            }
            area += brep.area(document.tolerance()).unwrap();
            brep.tessellate(3, document.tolerance()).unwrap();
        }
        assert_eq!(cut_domains[0], cut_domains[1]);
        trim_reversals.sort_unstable();
        assert_eq!(trim_reversals, vec![false, true]);
        assert!(document.tolerance().approx_eq(area, 48.5625));
    }
}

#[test]
fn cutting_object_split_supports_parallel_crossing_and_mixed_surface_cutters() {
    let registry = CommandRegistry::with_builtins();
    let run_case = |cutters: &[&str],
                    surface_cutter_x: Option<Real>,
                    expected_pieces: usize,
                    expected_noniso_trims: usize,
                    expected_interior_u_trims: usize| {
        let mut document = Document::default();
        let source_id = document
            .add_geometry(Geometry::NurbsSurface(planar_intersection_surface()))
            .unwrap();
        for cutter in cutters {
            registry.execute(&mut document, cutter).unwrap();
        }
        if let Some(x) = surface_cutter_x {
            document
                .add_geometry(Geometry::NurbsSurface(transverse_surface_at_x(x)))
                .unwrap();
        }
        let ids = document
            .objects()
            .map(|object| object.id())
            .collect::<Vec<_>>();
        document
            .select_objects_direct(ids, SelectionMode::Replace)
            .unwrap();

        assert_eq!(
            registry
                .execute(&mut document, "Split CuttingObjects=1,1,0")
                .unwrap(),
            format!(
                "Split the selected surface along 2 complete boundary-to-boundary curve(s) into {expected_pieces} exact B-rep face(s)"
            )
        );
        assert!(document.object(source_id).is_none());
        let pieces = document.selected_objects().collect::<Vec<_>>();
        assert_eq!(pieces.len(), expected_pieces);
        let mut area = 0.0;
        let mut noniso_trims = 0;
        let mut interior_u_trims = 0;
        for object in pieces {
            let Geometry::Brep(brep) = object.geometry() else {
                panic!("multiple cutting Split must create B-rep faces")
            };
            for trim in brep.faces()[0].loops()[0].trims() {
                noniso_trims += usize::from(trim.iso() == viboceros_geometry::SurfaceIso::NotIso);
                interior_u_trims +=
                    usize::from(trim.iso() == viboceros_geometry::SurfaceIso::InteriorUConstant);
            }
            area += brep.area(document.tolerance()).unwrap();
            brep.tessellate(3, document.tolerance()).unwrap();
        }
        assert_eq!(noniso_trims, expected_noniso_trims);
        assert_eq!(interior_u_trims, expected_interior_u_trims);
        assert!(document.tolerance().approx_eq(area, 100.0));
    };

    run_case(&["Line 0,2,0 10,4,0", "Line 0,6,0 10,8,0"], None, 3, 4, 0);
    run_case(&["Line 0,2,0 10,8,0", "Line 0,8,0 10,2,0"], None, 4, 8, 0);
    run_case(&["Line 0,2,0 10,8,0"], Some(5.0), 4, 4, 4);
}

#[test]
fn cutting_object_split_preserves_single_open_cutter_kinks() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let source_id = document
        .add_geometry(Geometry::NurbsSurface(planar_intersection_surface()))
        .unwrap();
    let cutter = NurbsCurve::try_new(
        1,
        vec![
            Point3::try_new(0.0, 2.0, 0.0).unwrap(),
            Point3::try_new(5.0, 6.0, 0.0).unwrap(),
            Point3::try_new(10.0, 2.0, 0.0).unwrap(),
        ],
        vec![0.0, 0.0, 1.0, 2.0, 2.0],
    )
    .unwrap();
    let cutter_id = document.add_geometry(Geometry::NurbsCurve(cutter)).unwrap();
    document
        .select_objects_direct([source_id, cutter_id], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "Split CuttingObjects=5,1,0")
            .unwrap(),
        "Split the selected surface along 1 complete boundary-to-boundary curve into 2 exact B-rep faces"
    );
    assert!(document.object(source_id).is_none());
    assert!(!document.is_selected(cutter_id));
    let outputs = document.selected_objects().collect::<Vec<_>>();
    assert_eq!(outputs.len(), 2);
    for output in outputs {
        let Geometry::Brep(brep) = output.geometry() else {
            panic!("kinked surface cutting Split must create B-rep faces")
        };
        assert_eq!(brep.vertices().len(), 5);
        assert_eq!(brep.edges().len(), 5);
        let mut cut_domains = brep.faces()[0].loops()[0]
            .trims()
            .iter()
            .filter(|trim| trim.iso() == viboceros_geometry::SurfaceIso::NotIso)
            .map(|trim| brep.edges()[trim.edge().unwrap()].curve().domain())
            .collect::<Vec<_>>();
        cut_domains.sort_by(|left, right| left.start().total_cmp(right.start()));
        assert_eq!(cut_domains, vec![0.0..=1.0, 1.0..=2.0]);
        brep.tessellate(3, document.tolerance()).unwrap();
    }
}

#[test]
fn cutting_object_split_handles_no_hit_and_rejects_invalid_sources_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for command in [
        "Split CuttingObjects",
        "Split CuttingObjects=1,2,3 extra",
        "Split CuttingObjects=1,2,3,4",
    ] {
        assert!(matches!(
            registry.execute(&mut document, command),
            Err(CommandError::Usage(SPLIT_CURVE_USAGE))
        ));
    }
    assert!(matches!(
        registry.execute(&mut document, "Split CuttingObjects=0,0,0"),
        Err(CommandError::SplitRequiresSourceAndCuttingObjects { actual: 0 })
    ));

    registry.execute(&mut document, "Line 0,0 10,0").unwrap();
    let source_id = document.objects().next().unwrap().id();
    document
        .select_object(source_id, SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "Split CuttingObjects=5,0,0"),
        Err(CommandError::SplitRequiresSourceAndCuttingObjects { actual: 1 })
    ));

    registry.execute(&mut document, "Line 0,2 10,2").unwrap();
    let ids = document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    let before = document.objects().cloned().collect::<Vec<_>>();
    document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry
            .execute(&mut document, "Split CuttingObjects=5,0,0")
            .unwrap(),
        "No cutting intersection was available to split the curve"
    );
    assert_eq!(
        document.objects().collect::<Vec<_>>(),
        before.iter().collect::<Vec<_>>()
    );
    assert_eq!(
        document.selected_object_ids().collect::<Vec<_>>(),
        [source_id]
    );

    let point = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let mut surfaces = Document::default();
    for offset in [0.0, 2.0] {
        let surface = NurbsSurface::try_bilinear([
            point(0.0, 0.0, offset),
            point(1.0, 0.0, offset),
            point(1.0, 1.0, offset),
            point(0.0, 1.0, offset),
        ])
        .unwrap();
        surfaces
            .add_geometry(Geometry::NurbsSurface(surface))
            .unwrap();
    }
    let ids = surfaces
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    surfaces
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    let before = surfaces.objects().cloned().collect::<Vec<_>>();
    assert_eq!(
        registry
            .execute(&mut surfaces, "Split CuttingObjects=0,0,0")
            .unwrap(),
        "No complete cutting intersection was available to split the surface"
    );
    assert_eq!(
        surfaces.objects().collect::<Vec<_>>(),
        before.iter().collect::<Vec<_>>()
    );

    let mut solids = Document::default();
    registry.execute(&mut solids, "Box 0,0,0 1,1,0 1").unwrap();
    registry.execute(&mut solids, "Box 3,0,0 4,1,0 1").unwrap();
    registry.execute(&mut solids, "SelAll").unwrap();
    let before = solids.objects().cloned().collect::<Vec<_>>();
    assert!(matches!(
        registry.execute(&mut solids, "Split CuttingObjects=0,0,0"),
        Err(CommandError::SplitRequiresCurveOrRectangularSurfaceSource)
    ));
    assert_eq!(
        solids.objects().collect::<Vec<_>>(),
        before.iter().collect::<Vec<_>>()
    );
}
