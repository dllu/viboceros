use super::*;
use crate::{Frame3, Point3, SubD, Tolerance, Vector3};
fn frame() -> Frame3 {
    Frame3::try_from_x_and_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}
#[test]
fn smooth_sphere_pole_limits_and_affine_curvature_follow_the_surface() {
    let sphere = NurbsSurface::try_sphere(frame(), 2.).unwrap();
    let mesh = sphere.curvature_mesh(4, Tolerance::DEFAULT).unwrap();
    let scale = AffineTransform3::try_new(
        [[3., 0., 0.], [0., 3., 0.], [0., 0., 3.]],
        Vector3::try_new(0., 0., 0.).unwrap(),
    )
    .unwrap();
    let base = mesh.principal_curvatures(None);
    let scaled = mesh.principal_curvatures(Some(scale));
    let mut count = 0;
    for (a, b) in base.into_iter().flatten().zip(scaled.into_iter().flatten()) {
        let a = a.expect("a smooth sphere pole has a sampled curvature limit");
        let b = b.unwrap();
        for (a, b) in a.into_iter().zip(b) {
            assert!((a.abs() - 0.5).abs() < 1e-7);
            assert!((b.abs() - 1. / 6.).abs() < 1e-7);
        }
        count += 1;
    }
    assert!(count > 500);
}

#[test]
fn subd_regular_patch_jets_preserve_limit_points_and_flat_curvature() {
    let plane = SubD::try_plane_grid(frame(), [4., 4.], [2, 2]).unwrap();
    let mesh = plane.curvature_mesh().unwrap();
    let values = mesh.principal_curvatures(None);
    let mut count = 0;
    for (triangle, indices) in mesh.mesh.triangles().iter().zip(&mesh.samples.indices) {
        for (index, sample) in triangle.iter().zip(indices) {
            if let Some(sample) = sample {
                let p = mesh.mesh.vertices()[*index as usize];
                assert!(p.distance_to(mesh.samples.jets[*sample].point).unwrap() < 1e-10);
            }
        }
    }
    for k in values.into_iter().flatten().flatten() {
        assert!(k.into_iter().all(|k| k.abs() < 1e-10));
        count += 1;
    }
    assert!(count > 100);
    let samples = mesh.principal_samples().collect::<Vec<_>>();
    // Boundary samples without a regular patch remain unavailable.
    assert!(samples.len() <= mesh.mesh.vertices().len());
    assert!(samples.len() > mesh.mesh.vertices().len() / 2);
    assert!(samples.len() < count / 2);
    assert!(samples.iter().flatten().all(|k| k.abs() < 1e-10));
}

#[test]
fn sheared_curvature_jets_match_evaluating_the_transformed_surface() {
    let p = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let s = NurbsSurface::try_bilinear([
        p(-2., -2., 0.),
        p(2., -2., 1.),
        p(2., 2., -1.),
        p(-2., 2., 1.),
    ])
    .unwrap();
    let map = AffineTransform3::try_new(
        [[2., 0.2, 0.], [0., 1., 0.1], [0., 0., 0.5]],
        Vector3::try_new(0., 0., 0.).unwrap(),
    )
    .unwrap();
    let source = s.curvature_mesh(4, Tolerance::DEFAULT).unwrap();
    let transformed = s
        .transformed(map)
        .unwrap()
        .curvature_mesh(4, Tolerance::DEFAULT)
        .unwrap();
    for (a, b) in source
        .principal_curvatures(Some(map))
        .into_iter()
        .flatten()
        .zip(transformed.principal_curvatures(None).into_iter().flatten())
    {
        for (a, b) in a.unwrap().into_iter().zip(b.unwrap()) {
            assert!((a - b).abs() < 1e-10);
        }
    }
}

#[test]
fn normal_mesh_defers_and_reuses_curvature_samples() {
    let sphere = NurbsSurface::try_sphere(frame(), 2.).unwrap();
    let mesh = sphere.curvature_mesh(2, Tolerance::DEFAULT).unwrap();
    assert!(std::sync::LazyLock::get(&mesh.samples).is_none());
    let normals = mesh.normals.clone();
    assert_eq!(normals.len(), mesh.mesh.triangles().len());
    assert!(std::sync::LazyLock::get(&mesh.samples).is_none());
    let first = mesh.principal_curvatures(None);
    let samples = std::sync::LazyLock::get(&mesh.samples).unwrap();
    assert_eq!(first, mesh.principal_curvatures(None));
    assert!(std::ptr::eq(
        samples,
        std::sync::LazyLock::get(&mesh.samples).unwrap()
    ));
    assert_eq!(normals, mesh.normals);
}
