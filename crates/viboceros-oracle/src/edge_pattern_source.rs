//! Independently constructed polygon recipes for loop/ring comparisons.
use super::*;
use viboceros_geometry::{MeshNgon, SubD};
pub(super) fn geometry(
    kind: &str,
    scene: &str,
    tolerance: Tolerance,
) -> Result<(Geometry, Vec<Point3>), ProbeError> {
    let (mut points, mut corners, mut creases) = (Vec::new(), Vec::new(), Vec::new());
    let mut faces: Vec<Vec<u32>>;
    let mut ngons = Vec::new();
    if scene.starts_with("grid")
        || matches!(
            scene,
            "triangles" | "nonmanifold" | "quad_triangle" | "collapsed" | "unwelded"
        )
    {
        points = (0..4)
            .flat_map(|y| (0..5).map(move |x| [3. * x as f64, 3. * y as f64, 0.]))
            .collect();
        faces = (0..3)
            .flat_map(|y| {
                (0..4).map(move |x| {
                    let a = 5 * y + x;
                    vec![a, a + 1, a + 6, a + 5]
                })
            })
            .collect();
        corners = vec![0, 4, 15, 19];
        match scene {
            "grid" => {}
            "grid_smooth" => corners.clear(),
            "grid_corner" => corners.push(7),
            "grid_crease" => creases = (5..9).map(|i| [i, i + 1]).collect(),
            "grid_crease_end" => creases = vec![[6, 7]],
            "triangles" => {
                faces = faces
                    .into_iter()
                    .flat_map(|f| [vec![f[0], f[1], f[2]], vec![f[0], f[2], f[3]]])
                    .collect()
            }
            "nonmanifold" => {
                points.extend([[4.5, 4., 3.], [4.5, 2., 3.]]);
                faces.push(vec![6, 7, 20, 21]);
            }
            "quad_triangle" => {
                faces.splice(5..6, [vec![6, 7, 12], vec![6, 12, 11]]);
            }
            "collapsed" => points[7] = points[6],
            "unwelded" => {
                for f in &mut faces {
                    let start = points.len() as u32;
                    let added = f.iter().map(|&i| points[i as usize]).collect::<Vec<_>>();
                    points.extend(added);
                    *f = (start..start + 4).collect();
                }
            }
            _ => return Err(ProbeError::FixtureInvariant("unknown edge selection grid")),
        }
    } else if matches!(scene, "fan3" | "fan5" | "fan5crease") {
        let n = if scene == "fan3" { 3 } else { 5 };
        points.push([0., 0., 0.]);
        for (r, offset) in [(3., 0.), (6., 0.5)] {
            for i in 0..n {
                let a = std::f64::consts::TAU * (i as f64 + offset) / n as f64;
                points.push([r * a.cos(), r * a.sin(), 0.]);
            }
        }
        faces = (0..n)
            .map(|i| vec![0, i + 1, n + i + 1, (i + 1) % n + 1])
            .collect();
        if scene == "fan5crease" {
            creases = vec![[0, 1], [0, 3]];
        }
    } else if scene == "annulus" {
        for r in [3., 6., 9.] {
            for i in 0..8 {
                let a = std::f64::consts::TAU * i as f64 / 8.;
                points.push([r * a.cos(), r * a.sin(), 0.]);
            }
        }
        faces = (0..2)
            .flat_map(|j| {
                (0..8).map(move |i| {
                    vec![
                        j * 8 + i,
                        (j + 1) * 8 + i,
                        (j + 1) * 8 + (i + 1) % 8,
                        j * 8 + (i + 1) % 8,
                    ]
                })
            })
            .collect();
    } else if scene.starts_with("ngon_") {
        match scene {
            "ngon_pentagon" => {
                points = (0..5)
                    .map(|i| {
                        let a = std::f64::consts::TAU * i as f64 / 5.;
                        [4. * a.cos(), 4. * a.sin(), 0.]
                    })
                    .collect();
                faces = vec![vec![0, 1, 2], vec![0, 2, 3], vec![0, 3, 4]];
                ngons.push(MeshNgon::from_parts((0..5).collect(), vec![0, 1, 2]));
            }
            "ngon_quad" => {
                points = vec![[0., 0., 0.], [3., 0., 0.], [3., 3., 0.], [0., 3., 0.]];
                faces = vec![vec![0, 1, 2], vec![0, 2, 3]];
                ngons.push(MeshNgon::from_parts(vec![0, 1, 2, 3], vec![0, 1]));
            }
            "ngon_strip" | "ngon_pair" => {
                points = vec![
                    [0., 0., 0.],
                    [3., 0., 0.],
                    [6., 0., 0.],
                    [0., 3., 0.],
                    [3., 3., 0.],
                    [6., 3., 0.],
                ];
                faces = vec![vec![0, 1, 4, 3], vec![1, 2, 5, 4]];
                ngons.push(MeshNgon::from_parts(vec![0, 1, 2, 5, 4, 3], vec![0, 1]));
                if scene == "ngon_pair" {
                    points.extend([[9., 0., 0.], [9., 3., 0.]]);
                    faces.push(vec![2, 6, 7, 5]);
                }
            }
            _ => return Err(ProbeError::FixtureInvariant("unknown edge selection n-gon")),
        }
    } else {
        return Err(ProbeError::FixtureInvariant("unknown edge selection scene"));
    }
    let points = points
        .into_iter()
        .map(Point3::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    let faces = faces
        .into_iter()
        .map(|f| match f.as_slice() {
            [a, b, c] => MeshFace::Triangle([*a, *b, *c]),
            [a, b, c, d] => MeshFace::Quad([*a, *b, *c, *d]),
            _ => unreachable!(),
        })
        .collect();
    let source_points = points.clone();
    let mesh = TriangleMesh::try_from_face_records(points, faces)?.try_with_ngons(ngons)?;
    let geometry = match kind {
        "mesh" => Ok(Geometry::Mesh(mesh)),
        "subd" => {
            let s = SubD::try_from_mesh(&mesh, Default::default(), tolerance)?
                .try_with_vertex_tags(&corners.into_iter().map(|i| (i, 3)).collect::<Vec<_>>())?;
            let tags = s
                .edges()
                .iter()
                .enumerate()
                .filter(|(_, e)| creases.iter().any(|c| **e == *c || **e == [c[1], c[0]]))
                .map(|(i, _)| (i, 2))
                .collect::<Vec<_>>();
            Ok(Geometry::SubD(s.try_with_edge_tags(&tags)?))
        }
        _ => Err(ProbeError::FixtureInvariant("mesh or SubD required")),
    }?;
    Ok((geometry, source_points))
}
