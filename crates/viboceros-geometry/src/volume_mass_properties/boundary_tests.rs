use super::*;
use crate::{Brep, Frame3, MeshFace, NurbsSurface, Tolerance, TriangleMesh};

fn p(a: [Real; 3]) -> Point3 {
    Point3::try_from(a).unwrap()
}

fn frame() -> Frame3 {
    Frame3::try_from_points(
        p([0.; 3]),
        p([1., 0., 0.]),
        p([0., 1., 0.]),
        Tolerance::DEFAULT,
    )
    .unwrap()
}

fn mesh(vertices: &[[Real; 3]], faces: Vec<MeshFace>) -> TriangleMesh {
    TriangleMesh::try_new_faces(
        vertices.iter().copied().map(p).collect(),
        faces,
        Tolerance::try_new(1e-250, 1e-12, 1e-10).unwrap(),
    )
    .unwrap()
}

fn determinant(a: &[Rational; 3], b: &[Rational; 3], c: &[Rational; 3]) -> Rational {
    &a[0] * (&b[1] * &c[2] - &b[2] * &c[1]) - &a[1] * (&b[0] * &c[2] - &b[2] * &c[0])
        + &a[2] * (&b[0] * &c[1] - &b[1] * &c[0])
}

#[test]
fn open_mesh_cones_match_independent_exact_rationals_without_rounded_subtraction() {
    for (offset, scale) in [
        (0., 1.),
        (0., 1e-200),
        (0., 1e200),
        (1e12, 1.),
        (1e100, 1e85),
        (1e308, 1e292),
    ] {
        let vertices = [
            [offset, 0., 0.],
            [offset + 3. * scale, 0., 0.],
            [offset, 4. * scale, 0.],
            [offset, 0., 5. * scale],
        ];
        let faces = [[0, 1, 3], [0, 3, 2], [1, 2, 3]];
        let m = mesh(&vertices, faces.map(MeshFace::Triangle).to_vec());
        for base in [[1.5, 2., 2.5], [-1e308, 1e308, -1e308], [0.; 3]] {
            let o = base.map(rational);
            let mut volume = Rational::zero();
            let mut first = std::array::from_fn::<_, 3, _>(|_| Rational::zero());
            for f in faces {
                let q = f.map(|i| vertices[i as usize].map(rational));
                let local = q
                    .clone()
                    .map(|a| std::array::from_fn(|axis| &a[axis] - &o[axis]));
                let v = determinant(&local[0], &local[1], &local[2]) / rational(6.);
                for i in 0..3 {
                    first[i] += &v * (&o[i] + &q[0][i] + &q[1][i] + &q[2][i]) / rational(4.);
                }
                volume += v;
            }
            let actual = m.volume_flux(p(base)).unwrap();
            assert_eq!(actual.volume, volume);
            assert_eq!(actual.first, first);
        }
    }
}

#[test]
fn reference_changes_open_piece_results_but_not_the_enclosing_mesh_collection() {
    let vertices = [
        [0., 0., 0.],
        [3., 0., 0.],
        [0., 4., 0.],
        [0., 0., 5.],
        [1e308, 1e308, 1e308],
    ];
    let faces = [[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]];
    let pieces = faces.map(|f| mesh(&vertices, vec![MeshFace::Triangle(f)]));
    let boundaries = pieces.each_ref().map(VolumeBoundary::Mesh);
    assert!(
        boundaries
            .iter()
            .all(|b| !b.is_closed(Tolerance::DEFAULT).unwrap())
    );
    let expected = VolumeMassProperties::from_boundaries(&boundaries, Tolerance::DEFAULT).unwrap();
    assert_eq!(expected.signed_volume().unwrap(), 10.);
    assert_eq!(expected.centroid().unwrap().to_array(), [0.75, 1., 1.25]);
    for base in [[0.; 3], [1.5, 2., 2.5], [1e100, -1e100, 1e100]] {
        let mut total = VolumeMassProperties::default();
        for piece in &pieces {
            total.add(&piece.volume_flux(p(base)).unwrap());
        }
        assert_eq!(total.volume, expected.volume);
        assert_eq!(total.first, expected.first);
    }
    let open = mesh(
        &vertices,
        faces[1..].iter().copied().map(MeshFace::Triangle).collect(),
    );
    let centered =
        VolumeMassProperties::from_boundaries(&[VolumeBoundary::Mesh(&open)], Tolerance::DEFAULT)
            .unwrap();
    assert_eq!(centered.signed_volume().unwrap(), 5.);
    assert_eq!(centered.centroid().unwrap().to_array(), [0.375, 0.5, 1.875]);
    assert_eq!(
        open.volume_flux(p([0.; 3]))
            .unwrap()
            .signed_volume()
            .unwrap(),
        10.
    );
    assert!(open.volume_mass_properties().is_err());
}

#[test]
fn unjoined_planar_surface_faces_share_a_reference_and_enclose_the_box() {
    let vertices = [
        [0., 0., 0.],
        [4., 0., 0.],
        [4., 3., 0.],
        [0., 3., 0.],
        [0., 0., 2.],
        [4., 0., 2.],
        [4., 3., 2.],
        [0., 3., 2.],
    ];
    let faces = [
        [0, 3, 2, 1],
        [4, 5, 6, 7],
        [0, 1, 5, 4],
        [1, 2, 6, 5],
        [2, 3, 7, 6],
        [3, 0, 4, 7],
    ];
    let surfaces = faces.map(|f| {
        NurbsSurface::try_new(
            1,
            1,
            2,
            2,
            [f[0], f[1], f[3], f[2]].map(|i| p(vertices[i])).to_vec(),
            vec![0., 0., 1., 1.],
            vec![0., 0., 1., 1.],
        )
        .unwrap()
    });
    let boundaries = surfaces.each_ref().map(VolumeBoundary::Surface);
    for base in [[2., 1.5, 1.], [-1., -2., -3.], [10., 11., 12.]] {
        let mut total = VolumeMassProperties::default();
        for b in boundaries {
            total.add(&b.volume_flux(p(base), Tolerance::DEFAULT).unwrap());
        }
        assert!((total.signed_volume().unwrap() - 24.).abs() < 1e-10);
        assert!(
            total
                .centroid()
                .unwrap()
                .distance_to(p([2., 1.5, 1.]))
                .unwrap()
                < 1e-10
        );
    }
    let mass = VolumeMassProperties::from_boundaries(&boundaries, Tolerance::DEFAULT).unwrap();
    assert!((mass.signed_volume().unwrap() - 24.).abs() < 1e-10);
    assert!(
        mass.centroid()
            .unwrap()
            .distance_to(p([2., 1.5, 1.]))
            .unwrap()
            < 1e-10
    );
    let bottom = mesh(&vertices, vec![MeshFace::Quad(faces[0].map(|i| i as u32))]);
    let mixed = std::iter::once(VolumeBoundary::Mesh(&bottom))
        .chain(surfaces[1..].iter().map(VolumeBoundary::Surface))
        .collect::<Vec<_>>();
    let mixed_mass = VolumeMassProperties::from_boundaries(&mixed, Tolerance::DEFAULT).unwrap();
    assert!((mixed_mass.signed_volume().unwrap() - 24.).abs() < 1e-10);
    assert!(
        mixed_mass
            .centroid()
            .unwrap()
            .distance_to(p([2., 1.5, 1.]))
            .unwrap()
            < 1e-10
    );
    let solid = Brep::try_box(frame(), [[0., 4.], [0., 3.], [0., 2.]], Tolerance::DEFAULT).unwrap();
    assert!(
        VolumeBoundary::Brep(&solid)
            .is_closed(Tolerance::DEFAULT)
            .unwrap()
    );
}

#[test]
fn empty_collection_is_not_a_solid_and_bilinear_cone_flux_has_analytic_moments() {
    assert!(VolumeMassProperties::from_boundaries(&[], Tolerance::DEFAULT).is_err());
    // p(u,v)=(4u,3v,2uv), n=(-6v,-8u,12), common base=(2,1.5,1).
    // Exact polynomial integrals give V=-2, M=(-4,-3,-4/3).
    // This explicitly tests our defined cone flux, NOT Rhino's different
    // centroid convention for an isolated non-enclosing open surface.
    let surface = NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        [[0., 0., 0.], [4., 0., 0.], [0., 3., 0.], [4., 3., 2.]]
            .map(p)
            .to_vec(),
        vec![0., 0., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let mass = surface
        .volume_flux(p([2., 1.5, 1.]), Tolerance::DEFAULT)
        .unwrap();
    assert!((mass.signed_volume().unwrap() + 2.).abs() < 1e-12);
    assert!(
        mass.centroid()
            .unwrap()
            .distance_to(p([2., 1.5, 2. / 3.]))
            .unwrap()
            < 1e-12
    );
    assert!(surface.volume_mass_properties(Tolerance::DEFAULT).is_err());
}
