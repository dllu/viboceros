use super::*;
use crate::{Brep, Frame3, NurbsMeshDensity, Point3, Vector3};

fn frame() -> Frame3 {
    Frame3::try_from_x_and_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}
fn wave() -> NurbsSurface {
    NurbsSurface::try_new(
        3,
        1,
        4,
        2,
        (0..2)
            .flat_map(|v| {
                [0., 1., -1., 0.]
                    .into_iter()
                    .enumerate()
                    .map(move |(u, z)| Point3::try_new(u as Real / 3., v as Real, z).unwrap())
            })
            .collect(),
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap()
}

#[test]
fn analysis_nurbs_density_preserves_native_plane_floor_and_curved_direction_rows() {
    let plane = NurbsSurface::try_bilinear(
        [[0., 0., 0.], [3., 0., 0.], [3., 2., 0.], [0., 2., 0.]]
            .map(|p| Point3::try_from(p).unwrap()),
    )
    .unwrap();
    let cylinder = NurbsSurface::try_cylinder(frame(), 2., 0., 3.).unwrap();
    let brep = Brep::try_surface_face(cylinder.clone(), Tolerance::DEFAULT).unwrap();
    let mut previous = 0;
    for density in [0., 0.25, 0.5, 0.8, 1.] {
        let density = NurbsMeshDensity::new(density).unwrap();
        let flat = plane
            .tessellate_with_density(density, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(flat.mesh.faces().len(), 16);
        assert_eq!(flat.mesh.vertices().len(), 25);
        assert!(flat.mesh.faces().iter().all(|f| f.is_quad()));
        let surface = cylinder
            .tessellate_with_density(density, Tolerance::DEFAULT)
            .unwrap();
        let shell = brep
            .tessellate_with_density(density, Tolerance::DEFAULT)
            .unwrap();
        assert!(surface.mesh.faces().len() >= previous);
        previous = surface.mesh.faces().len();
        for mesh in [&surface.mesh, &shell.mesh] {
            assert!(
                mesh.vertices()
                    .iter()
                    .all(|p| p.z().abs() < 1e-12 || (p.z() - 3.).abs() < 1e-12),
                "density {}: unexpected height {:?}",
                density.value(),
                mesh.vertices()
                    .iter()
                    .find(|p| p.z().abs() >= 1e-12 && (p.z() - 3.).abs() >= 1e-12)
            );
            assert!(mesh.topology().is_manifold());
        }
        assert_eq!(
            shell.mesh.faces().len(),
            surface.mesh.faces().len(),
            "density {} surface grid {:?}, shell grid {:?}",
            density.value(),
            cylinder.analysis_mesh_resolution(density, false, Tolerance::DEFAULT),
            brep.faces()[0]
                .surface()
                .analysis_mesh_resolution(density, false, Tolerance::DEFAULT)
        );
        assert_eq!(shell.corners.len(), shell.mesh.triangles().len());
        assert!(
            shell
                .corners
                .iter()
                .flatten()
                .all(|p| p.face == Some(0) && p.normal.is_some())
        );
    }
    for invalid in [Real::NAN, Real::INFINITY, -0.1, 1.1] {
        assert!(NurbsMeshDensity::new(invalid).is_none());
    }
}

#[test]
fn analysis_nurbs_density_conforming_fallback_keeps_shared_edges_and_face_normals() {
    let curve = Brep::try_surface_face(wave(), Tolerance::DEFAULT).unwrap();
    let plane = NurbsSurface::try_bilinear(
        [[1., 0., 0.], [2., 0., 0.], [2., 1., 0.], [1., 1., 0.]]
            .map(|p| Point3::try_from(p).unwrap()),
    )
    .unwrap();
    let flat = Brep::try_surface_face(plane, Tolerance::DEFAULT).unwrap();
    let joined = crate::join_breps(&[&curve, &flat], 1e-9, Tolerance::DEFAULT).unwrap();
    assert_eq!(joined.len(), 1);
    let before = joined[0].brep.clone();
    let output = before
        .tessellate_with_density(NurbsMeshDensity::default(), Tolerance::DEFAULT)
        .unwrap();
    assert!(output.mesh.topology().is_manifold());
    let borders = output
        .mesh
        .filtered_edge_polylines(crate::MeshEdgeFilter::Naked, Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(borders.len(), 1);
    assert!(borders[0].is_closed());
    let faces = output
        .corners
        .iter()
        .flatten()
        .filter_map(|c| c.face)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(faces, std::collections::BTreeSet::from([0, 1]));
    assert_eq!(before, joined[0].brep);
}

#[test]
fn analysis_profile_simple_planes_keeps_curved_bilinear_faces_and_analytic_channels() {
    let settings = crate::AnalysisMeshSettings {
        simple_planes: true,
        ..Default::default()
    };
    for raised in [false, true] {
        let surface = NurbsSurface::try_bilinear(
            [
                [0., 0., 0.],
                [3., 0., 0.],
                [3., 2., if raised { 1. } else { 0. }],
                [0., 2., 0.],
            ]
            .map(|p| Point3::try_from(p).unwrap()),
        )
        .unwrap();
        let brep = Brep::try_surface_face(surface.clone(), Tolerance::DEFAULT).unwrap();
        for mesh in [
            surface
                .curvature_mesh_with_analysis_settings(settings, Tolerance::DEFAULT)
                .unwrap(),
            brep.curvature_mesh_with_analysis_settings(settings, Tolerance::DEFAULT)
                .unwrap(),
        ] {
            assert_eq!(mesh.mesh.faces().len(), if raised { 16 } else { 1 });
            assert!(mesh.mesh.faces().iter().all(|face| face.is_quad()));
            assert_eq!(mesh.normals.len(), mesh.mesh.triangles().len());
            assert!(mesh.normals.iter().flatten().all(Option::is_some));
            assert!(
                mesh.principal_curvatures(None)
                    .iter()
                    .flatten()
                    .all(Option::is_some)
            );
        }
    }
}
