use super::*;
use viboceros_geometry::{MeshFace, MeshNgon, TriangleMesh};
#[test]
fn inset_filter_picks_and_windows_surface_faces_in_all_display_modes() {
    use viboceros_geometry::{Brep, NurbsSurface, Tolerance};
    let surface =
        NurbsSurface::try_bilinear([point(0., 0.), point(8., 0.), point(8., 6.), point(0., 6.)])
            .unwrap();
    for geometry in [
        Geometry::NurbsSurface(surface.clone()),
        Geometry::Brep(Brep::try_surface_face(surface, Tolerance::DEFAULT).unwrap()),
    ] {
        let mut doc = Document::default();
        let id = doc.add_geometry(geometry).unwrap();
        let expected = [ComponentPick {
            object: id,
            index: 0,
            kind: ComponentSelectionKind::BrepFace,
        }];
        for mode in [
            DisplayMode::Wireframe,
            DisplayMode::Shaded,
            DisplayMode::Ghosted,
        ] {
            let view = Viewport {
                display_mode: mode,
                ..Viewport::new(ViewKind::Top)
            };
            let pointer = view.project(point(2., 2.), rect()).unwrap();
            assert_eq!(
                view.pick_components(pointer, rect(), &doc, ComponentPickFilter::InsetFaces),
                expected
            );
            for crossing in [false, true] {
                for inverted in [false, true] {
                    let window = if inverted {
                        selection(&view, (-5., -5.), (-3., -3.))
                    } else {
                        selection(&view, (-0.1, -0.1), (8.1, 6.1))
                    };
                    assert_eq!(
                        view.components_in_rectangle(
                            rect(),
                            window,
                            &doc,
                            ComponentPickFilter::InsetFaces,
                            crossing,
                            inverted
                        ),
                        expected
                    );
                }
            }
        }
    }
}
#[test]
fn inset_filter_picks_and_windows_stored_mesh_polygons_inside_ngons() {
    let mesh = TriangleMesh::try_from_face_records(
        vec![
            point(-1., 2.),
            point(0., 0.),
            point(2., 4.),
            point(4., 0.),
            point(5., 2.),
        ],
        vec![
            MeshFace::Triangle([2, 1, 3]),
            MeshFace::Triangle([1, 2, 0]),
            MeshFace::Triangle([4, 2, 3]),
        ],
    )
    .unwrap()
    .try_with_ngons(vec![MeshNgon::from_parts(
        vec![1, 3, 4, 2, 0],
        vec![0, 1, 2],
    )])
    .unwrap();
    let mut doc = Document::default();
    let id = doc.add_geometry(Geometry::Mesh(mesh)).unwrap();
    for mode in [
        DisplayMode::Wireframe,
        DisplayMode::Shaded,
        DisplayMode::Ghosted,
    ] {
        let view = Viewport {
            display_mode: mode,
            ..Viewport::new(ViewKind::Top)
        };
        let pointer = view.project(point(1. / 3., 2.), rect()).unwrap();
        assert_eq!(
            view.pick_components(pointer, rect(), &doc, ComponentPickFilter::InsetFaces),
            [ComponentPick {
                object: id,
                index: 1,
                kind: ComponentSelectionKind::MeshFace
            }]
        );
        assert_eq!(
            view.pick_components(pointer, rect(), &doc, ComponentPickFilter::MeshAndSubDFaces),
            [ComponentPick {
                object: id,
                index: 0,
                kind: ComponentSelectionKind::MeshFace
            }]
        );
        let window = selection(&view, (-1.01, -0.01), (2.01, 4.01));
        assert_eq!(
            view.components_in_rectangle(
                rect(),
                window,
                &doc,
                ComponentPickFilter::InsetFaces,
                false,
                false
            ),
            [ComponentPick {
                object: id,
                index: 1,
                kind: ComponentSelectionKind::MeshFace
            }]
        );
        assert!(
            view.components_in_rectangle(
                rect(),
                window,
                &doc,
                ComponentPickFilter::MeshAndSubDFaces,
                false,
                false
            )
            .is_empty()
        );
    }
}
