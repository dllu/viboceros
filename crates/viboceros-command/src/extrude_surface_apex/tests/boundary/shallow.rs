use super::*;

#[test]
fn surface_apex_very_shallow_warped_cuts_keep_the_native_rim_and_cutter() {
    let warped = [
        [2.8, -4., -4.],
        [4.8, 7., -3.],
        [3.3, 7., 5.],
        [4.5, -4., 3.],
    ];
    let other = [
        [3.1, -4., -4.],
        [4.6, 7., -3.],
        [3.9, 7., 5.],
        [3.8, -4., 3.],
    ];
    // Four rim vertices from Rhino 8.32, in source edge order.
    for (corners, height, tilted, rim) in [
        (
            warped,
            1e-8,
            false,
            [
                [3.8138891846603933, 0.6802084442476477, 4.534723112605809e-9],
                [3.8241250903721764, 0.9560312725930439, 6.373541422784327e-9],
                [3.885023721511602, 2.2931161044331487, 4.712559267756654e-9],
                [3.871689396391529, 2.032077650902118, 6.452815415869985e-9],
            ],
        ),
        (
            warped,
            -1e-8,
            false,
            [
                [
                    3.8138891842413694,
                    0.6802084440905136,
                    -4.5347232231043324e-9,
                ],
                [3.824125089926044, 0.9560312724815112, -6.373541755479458e-9],
                [3.885023721671181, 2.2931161043733073, -4.712559490000732e-9],
                [
                    3.8716893964840478,
                    2.0320776508789877,
                    -6.452815415947085e-9,
                ],
            ],
        ),
        (
            other,
            1e-7,
            false,
            [
                [3.787718574442131, 0.6703944654157994, 4.4692964400741263e-8],
                [3.8085141983929454, 0.9521285495982363, 6.347523685319751e-8],
                [3.9116519242106618, 2.2831305284210015, 4.779129806769318e-8],
                [3.891203240206023, 2.027199189948494, 6.485338733192886e-8],
            ],
        ),
        (
            other,
            -1e-7,
            false,
            [
                [
                    3.7877185731351015,
                    0.6703944649256628,
                    -4.469296427338109e-8,
                ],
                [3.8085141971822303, 0.9521285492955576, -6.34752365100413e-8],
                [
                    3.9116519254321056,
                    2.2831305284821883,
                    -4.779129799898699e-8,
                ],
                [3.891203241336642, 2.027199189665839, -6.485338724512177e-8],
            ],
        ),
        (
            warped,
            1e-7,
            true,
            [
                [1027.813889186546, -31.59187496930493, 256.5441667831721],
                [1027.8241250923797, -31.42638128713138, 256.7648250567172],
                [1027.8850237207935, -30.62413037487905, 257.8344929120372],
                [1027.8716893959752, -30.78075346101881, 257.62566215952177],
            ],
        ),
    ] {
        let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
        let mut doc = Document::new(tolerance);
        let place = |[x, y, z]: [f64; 3]| {
            if tilted {
                p(
                    1024. + x,
                    -32. + 0.6 * y - 0.8 * z,
                    256. + 0.8 * y + 0.6 * z,
                )
            } else {
                p(x, y, z)
            }
        };
        let source = NurbsSurface::try_bilinear(
            [[0., 0., 0.], [2., 0., 0.], [2., 3., 0.], [0., 3., 0.]].map(place),
        )
        .unwrap();
        let cutter = corners.map(place);
        let source = doc.add_geometry(Geometry::NurbsSurface(source)).unwrap();
        let boundary = doc
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_bilinear(cutter).unwrap(),
            ))
            .unwrap();
        doc.clear_history().unwrap();
        let [x, y, z] = place([6., 1.5, height]).to_array();
        let registry = CommandRegistry::with_builtins();
        let command = format!(
            "ExtrudeSrfToPoint {x},{y},{z} ToBoundary={boundary} FaceRefs={source}:0 DeleteInput=Yes"
        );
        registry
            .execute(&mut doc, &command)
            .unwrap_or_else(|error| {
                panic!("height={height}, tilted={tilted}, cutter={corners:?}: {error}")
            });
        let Geometry::Brep(cut) = doc.selected_objects().next().unwrap().geometry() else {
            panic!("expected a truncated sweep");
        };
        assert_eq!(cut.faces().len(), 6);
        assert!(
            cut.faces().iter().any(|face| {
                let controls = face.surface().control_points();
                controls.len() == 4
                    && cutter.iter().all(|point| {
                        controls
                            .iter()
                            .any(|control| control.point().is_near(*point, tolerance))
                    })
            }),
            "the cut retains the warped cutter's carrier"
        );
        let rim_tolerance = 0.03 * height.abs();
        for [x, y, z] in rim {
            let expected = p(x, y, z);
            assert!(
                cut.vertices()
                    .iter()
                    .any(|v| v.point().distance_to(expected).unwrap() < rim_tolerance),
                "missing rim vertex {expected:?} at height {height}, tilted={tilted}"
            );
        }
        assert!(doc.object(source).is_none());
        assert!(doc.object(boundary).is_some());
        registry.execute(&mut doc, "Undo").unwrap();
        assert!(doc.object(source).is_some());
    }
}

#[test]
fn surface_apex_very_shallow_cut_with_competing_sections_keeps_sources_and_history() {
    for height in [1e-6, 1e-7, -1e-7] {
        let (mut doc, source, boundary) = document_with_folded_cutter(2.4, 4.8, 1.5, false);
        let before = format!("{doc:?}");
        let command = format!(
            "ExtrudeSrfToPoint 6,1.5,{height} ToBoundary={boundary} FaceRefs={source}:0 DeleteInput=Yes"
        );
        assert!(
            CommandRegistry::with_builtins()
                .execute(&mut doc, &command)
                .is_err(),
            "height={height}"
        );
        assert_eq!(format!("{doc:?}"), before);
    }
}

#[test]
fn surface_apex_shallow_transverse_section_retains_enclosed_cutter_holes() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    let normal = UnitVector3::try_new(1., 0., 0., tolerance).unwrap();
    let registry = CommandRegistry::with_builtins();
    // Rhino 8.32 reported six faces and 13 edges for the enclosed hole.
    // The same cutter outside the section gives 12 edges. At the rim it fails.
    for (height, fraction, radius, expected_edges) in [
        (3e-5, 7. / 12., 1.5e-6, Some(13)),
        (-3e-5, 7. / 12., 1.5e-6, Some(13)),
        (1e-4, 7. / 12., 4e-6, Some(13)),
        (3e-5, 0.3, 1.5e-6, Some(12)),
        (3e-5, 0.5, 1.5e-6, None),
    ] {
        let mut doc = Document::new(tolerance);
        let source = doc.add_geometry(planar_rectangle(0., 0.)).unwrap();
        let center = p(4., 1.5, fraction * height);
        let corners = [-4., 4., 4., -4., -4.];
        let outer = Polyline3::try_new(
            corners
                .into_iter()
                .zip([-4., -4., 4., 4., -4.])
                .map(|(y, z)| p(4., y, z))
                .collect(),
            tolerance,
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        let hole = Circle3::try_new(center, radius, normal, tolerance)
            .unwrap()
            .to_nurbs()
            .unwrap();
        let cutter = Brep::try_planar_face_with_holes(&outer, &[hole], tolerance).unwrap();
        let boundary = doc.add_geometry(Geometry::Brep(cutter)).unwrap();
        doc.clear_history().unwrap();
        let before = format!("{doc:?}");
        let command = format!(
            "ExtrudeSrfToPoint 6,1.5,{height} ToBoundary={boundary} FaceRefs={source}:0 DeleteInput=Yes"
        );
        let outcome = registry.execute(&mut doc, &command);
        let Some(edges) = expected_edges else {
            assert!(outcome.is_err());
            assert_eq!(format!("{doc:?}"), before);
            continue;
        };
        outcome
            .unwrap_or_else(|error| panic!("height {height}, hole position {fraction}: {error}"));
        let Geometry::Brep(cut) = doc.selected_objects().next().unwrap().geometry() else {
            panic!("expected a truncated sweep");
        };
        assert_eq!([cut.faces().len(), cut.edges().len()], [6, edges]);
        assert_eq!(cut.is_solid(), edges == 12);
        for vertex in [
            p(4., 0.75, 0.5 * height),
            p(4., 1., height * 2. / 3.),
            p(4., 2.25, 0.5 * height),
            p(4., 2., height * 2. / 3.),
        ] {
            assert!(
                cut.vertices()
                    .iter()
                    .any(|v| v.point().is_near(vertex, tolerance))
            );
        }
        if edges == 13 {
            let cap = cut
                .faces()
                .iter()
                .find(|face| face.loops().len() == 2)
                .unwrap();
            let hole = &cap.loops()[1].trims()[0];
            let curve = cut.edges()[hole.edge().unwrap()].curve();
            for fraction in [0., 0.13, 0.48, 0.72, 1.] {
                let point = curve
                    .evaluate(curve.parameter_at(fraction).unwrap())
                    .unwrap();
                assert!((point.distance_to(center).unwrap() - radius).abs() < 1e-11);
            }
        }
        assert!(doc.object(source).is_none());
        assert!(doc.object(boundary).is_some());
        registry.execute(&mut doc, "Undo").unwrap();
        assert!(doc.object(source).is_some());
    }
}
