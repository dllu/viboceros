//! Local tangent-plane processing at original smooth extraordinary vertices.
use super::*;

pub(super) fn apply(
    source: &SubD,
    patches: &mut [SubDNurbsPatch],
    mode: SubDNurbsContinuity,
) -> Result<(), GeometryError> {
    if mode == SubDNurbsContinuity::G0 {
        return Ok(());
    }
    let mut valence = vec![0usize; source.control_points().len()];
    for &[a, b] in source.edges() {
        valence[a as usize] += 1;
        valence[b as usize] += 1;
    }
    let selected = source
        .vertex_tags()
        .iter()
        .enumerate()
        .filter_map(|(i, &tag)| (tag == 1 && valence[i] != 4).then_some(i))
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Ok(());
    }
    let normals = viboceros_opennurbs::subd_smooth_vertex_normals(source.archive(), &selected)?;
    let mut by_vertex = vec![None; valence.len()];
    for (vertex, normal) in selected.into_iter().zip(normals) {
        by_vertex[vertex] = Some(normal);
    }
    for patch in patches {
        let face = &source.faces()[patch.face];
        let corners = if let Some(corner) = patch.corner {
            [by_vertex[face[corner] as usize], None, None, None]
        } else {
            std::array::from_fn(|i| by_vertex[face[i] as usize])
        };
        if corners.iter().all(Option::is_none) {
            continue;
        }
        patch.surface = project(
            &patch.surface,
            corners,
            if mode == SubDNurbsContinuity::G2 {
                3
            } else {
                2
            },
        )?;
    }
    Ok(())
}

fn project(
    surface: &NurbsSurface,
    normals: [Option<[f64; 3]>; 4],
    ring: usize,
) -> Result<NurbsSurface, GeometryError> {
    let (nu, nv) = (
        surface.control_point_count_u(),
        surface.control_point_count_v(),
    );
    let mut controls = surface
        .control_points()
        .iter()
        .map(|p| p.point())
        .collect::<Vec<_>>();
    for (corner, normal) in normals.into_iter().enumerate() {
        let Some(normal) = normal else { continue };
        let index = |i: usize, j: usize| {
            let u = if corner == 1 || corner == 2 {
                nu - 1 - i
            } else {
                i
            };
            let v = if corner >= 2 { nv - 1 - j } else { j };
            v * nu + u
        };
        let origin = controls[index(0, 0)].to_array();
        for j in 0..ring {
            for i in 0..ring {
                if i == 0 && j == 0 {
                    continue;
                }
                let k = index(i, j);
                let point = controls[k].to_array();
                let offset = (0..3)
                    .map(|a| (point[a] - origin[a]) * normal[a])
                    .sum::<f64>();
                controls[k] =
                    Point3::try_from(std::array::from_fn(|a| point[a] - offset * normal[a]))?;
            }
        }
    }
    NurbsSurface::try_new(
        3,
        3,
        nu,
        nv,
        controls,
        surface.knots_u().to_vec(),
        surface.knots_v().to_vec(),
    )
}
