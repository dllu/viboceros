use super::*;
use crate::viewport::*;
use viboceros_command::CommandRegistry;

fn point(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

fn near(actual: [f32; 3], expected: NaVector3<Real>) {
    assert!(
        actual
            .into_iter()
            .zip(expected.iter())
            .all(|(a, b)| (Real::from(a) - b).abs() < 1e-6),
        "{actual:?} != {expected:?}"
    );
}

#[test]
fn unwelded_implicit_normals_reach_both_display_modes_and_history() {
    let commands = CommandRegistry::with_builtins();
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
    // Exercise bends below and above the old automatic 45-degree crease.
    for height in [0.25, 2.] {
        let mesh = TriangleMesh::try_new(
            vec![
                point(0., 0., 0.),
                point(1., 0., 0.),
                point(0., 1., 0.),
                point(0., -1., height),
            ],
            vec![[0, 1, 2], [1, 0, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let first = NaVector3::new(0., 0., 1.);
        let second = NaVector3::new(0., height, 1.).normalize();
        let shared = (first + second).normalize();
        for mode in [DisplayMode::Shaded, DisplayMode::Ghosted] {
            let mut document = Document::default();
            let id = document.add_geometry(Geometry::Mesh(mesh.clone())).unwrap();
            document
                .select_objects_direct([id], SelectionMode::Replace)
                .unwrap();
            let mut view = Viewport::new(ViewKind::Top);
            view.display_mode = mode;
            let before = view.object_scene(rect, &document);
            assert_eq!(before.triangles.len(), 6);
            for (vertex, expected) in before
                .triangles
                .iter()
                .zip([shared, shared, first, shared, shared, second])
            {
                near(vertex.normal, expected);
            }
            commands
                .execute(&mut document, "Unweld Angle=0 ModifyNormals=No")
                .unwrap();
            let Geometry::Mesh(result) = document.object(id).unwrap().geometry() else {
                panic!()
            };
            assert!(result.vertex_normals().is_none());
            assert_eq!(result.vertices().len(), 6);
            let after = view.object_scene(rect, &document);
            assert_eq!(after.triangles.len(), 6);
            for (face, normal) in after.triangles.chunks_exact(3).zip([first, second]) {
                for vertex in face {
                    near(vertex.normal, normal);
                }
            }
            document.undo().unwrap().unwrap();
            let restored = view.object_scene(rect, &document);
            for (a, b) in restored.triangles.iter().zip(&before.triangles) {
                assert_eq!(a.normal, b.normal);
            }
            document.redo().unwrap().unwrap();
            let restored = view.object_scene(rect, &document);
            for (a, b) in restored.triangles.iter().zip(&after.triangles) {
                assert_eq!(a.normal, b.normal);
            }
        }
    }
}

#[test]
fn coincident_surface_samples_still_smooth_across_span_boundaries() {
    let mesh = TriangleMesh::try_new(
        vec![
            point(0., 0., 0.),
            point(1., 0., 0.),
            point(0., 1., 0.),
            point(-0., 0., -0.),
            point(1., 0., 0.),
            point(0., -1., 0.25),
        ],
        vec![[0, 1, 2], [4, 3, 5]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let first = NaVector3::new(0., 0., 1.);
    let second = NaVector3::new(0., 0.25, 1.).normalize();
    let shared = (first + second).normalize();
    let tessellation = tessellation_corner_normals(&mesh);
    for (face, normals) in tessellation.iter().enumerate() {
        for normal in &normals[..2] {
            assert!((normal - shared).norm() < 1e-12);
        }
        assert!((normals[2] - [first, second][face]).norm() < 1e-12);
    }
    // The same raw mesh deliberately retains its two separate seams.
    for (face, normals) in mesh_corner_normals(&mesh).iter().enumerate() {
        for normal in normals {
            assert!((normal - [first, second][face]).norm() < 1e-7);
        }
    }
}
