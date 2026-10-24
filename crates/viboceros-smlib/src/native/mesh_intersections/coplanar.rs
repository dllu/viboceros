//! Coverage of original coplanar triangles, using iOverlay's planar arrangement.
use super::*;
use i_overlay::{
    core::{fill_rule::FillRule, overlay_rule::OverlayRule},
    float::clip::FloatClip,
    float::overlay::{FloatOverlay, OverlayOptions},
    string::clip::ClipRule,
};
use std::collections::VecDeque;
use viboceros_geometry::{Frame3, Polyline3};
mod contacts;
mod self_intersection;
pub(super) use self_intersection::append as append_self;

type Path = Vec<[f64; 2]>;

fn intersection(a: &[Path], b: &[Path]) -> Vec<Vec<Path>> {
    let mut options = OverlayOptions::<f64, i64>::default();
    options.preserve_input_collinear = true;
    options.preserve_output_collinear = true;
    FloatOverlay::<[f64; 2], i64>::from_subj_and_clip_custom(a, b, options, Default::default())
        .overlay(OverlayRule::Intersect, FillRule::NonZero)
}

fn triangle(vertices: &[Point3], faces: &[u32], index: usize) -> Result<[Point3; 3], Error> {
    let face = faces
        .get(3 * index..3 * index + 3)
        .ok_or(Error::InvalidOutput)?;
    let point = |i: usize| {
        vertices
            .get(face[i] as usize)
            .copied()
            .ok_or(Error::InvalidOutput)
    };
    Ok([point(0)?, point(1)?, point(2)?])
}

fn project(points: [Point3; 3], frame: Frame3) -> Result<Path, Error> {
    let mut path = points
        .into_iter()
        .map(|p| frame.projected_coordinates_of(p))
        .collect::<Result<Path, _>>()?;
    if path
        .iter()
        .flatten()
        .any(|x| !x.is_finite() || x.abs() > 2_f64.powi(500))
    {
        return Err(GeometryError::MeshOutlineCoordinateRange.into());
    }
    // Coverage is independent of the input mesh's face winding.
    let normal = points[0]
        .vector_to(points[1])?
        .cross(points[0].vector_to(points[2])?)?;
    if normal.dot(frame.z_axis().as_vector())? < 0. {
        path.swap(1, 2);
    }
    Ok(path)
}

pub(super) fn append(
    av: &[Point3],
    af: &[u32],
    bv: &[Point3],
    bf: &[u32],
    pairs: &[u32],
    tolerance: Tolerance,
    result: &mut MeshIntersections,
) -> Result<(), Error> {
    if pairs.is_empty() {
        return Ok(());
    }
    let offset = af.len() / 3;
    let mut graph = vec![Vec::new(); offset + bf.len() / 3];
    let mut partial = vec![Vec::new(); offset];
    let mut contact_count = result.transverse.len();
    for pair in pairs.chunks_exact(4) {
        let (a, b) = (pair[0] as usize, pair[1] as usize);
        if a >= offset || b >= bf.len() / 3 {
            return Err(Error::InvalidOutput);
        }
        if pair[2] > 7 || pair[3] > 7 {
            return Err(Error::InvalidOutput);
        }
        graph[a].push(offset + b);
        graph[offset + b].push(a);
        let index = a;
        let (a, b) = (triangle(av, af, a)?, triangle(bv, bf, b)?);
        let normal = a[0]
            .vector_to(a[1])?
            .cross(a[0].vector_to(a[2])?)?
            .normalized_nonzero()?;
        let frame = Frame3::try_from_unit_normal(a[0], normal)?;
        if intersection(&[project(a, frame)?], &[project(b, frame)?]).is_empty() {
            for contact in contacts::collect(a, b, [pair[2], pair[3]], tolerance)? {
                contact_count += 1;
                if contact_count > 1_000_000 {
                    return Err(Error::InvalidOutput);
                }
                if contact.boundary {
                    result.tangential.push(contact.segment);
                } else if contact.whole_edge {
                    result.transverse.push(contact.segment);
                } else {
                    partial[index].push(contact.segment);
                }
            }
        }
    }
    let mut visited = vec![false; graph.len()];
    let mut boundary_count = 0;
    // Connected triangle pairs share one plane. Overlay each component once,
    // preserving holes and removing internal diagonals in area coverage.
    for start in 0..offset {
        if visited[start] || graph[start].is_empty() {
            continue;
        }
        let points = triangle(av, af, start)?;
        let normal = points[0]
            .vector_to(points[1])?
            .cross(points[0].vector_to(points[2])?)?
            .normalized_nonzero()?;
        let frame = Frame3::try_from_unit_normal(points[0], normal)?;
        let (mut a, mut b) = (Vec::new(), Vec::new());
        let mut lines = Vec::new();
        let mut queue = VecDeque::from([start]);
        visited[start] = true;
        while let Some(index) = queue.pop_front() {
            if index < offset {
                a.push(project(triangle(av, af, index)?, frame)?);
                for segment in partial[index].drain(..) {
                    lines.push(
                        segment
                            .into_iter()
                            .map(|p| frame.projected_coordinates_of(p))
                            .collect::<Result<Path, _>>()?,
                    );
                }
            } else {
                b.push(project(triangle(bv, bf, index - offset)?, frame)?);
            }
            for &next in &graph[index] {
                if !visited[next] {
                    visited[next] = true;
                    queue.push_back(next);
                }
            }
        }
        let shapes = intersection(&a, &b);
        // Partial edge overlaps inside shared area are imprint artifacts.
        // Clip them together, keeping isolated line contacts outside coverage.
        // Whole coincident facet seams remain public intersection lines.
        if !lines.is_empty() {
            let rule = ClipRule {
                invert: true,
                boundary_included: true,
            };
            for path in lines.clip_by_as::<i64>(&shapes, FillRule::NonZero, rule) {
                for segment in path.windows(2) {
                    if result.transverse.len() >= 1_000_000 {
                        return Err(Error::InvalidOutput);
                    }
                    result.transverse.push([
                        frame.point_at([segment[0][0], segment[0][1], 0.])?,
                        frame.point_at([segment[1][0], segment[1][1], 0.])?,
                    ]);
                }
            }
        }
        for path in shapes.into_iter().flatten() {
            if path.len() < 3 {
                continue;
            }
            let mut points = path
                .into_iter()
                .map(|[x, y]| frame.point_at([x, y, 0.]))
                .collect::<Result<Vec<_>, _>>()?;
            boundary_count += points.len();
            if boundary_count > 1_000_000 || result.coplanar.len() >= 1_000_000 {
                return Err(Error::InvalidOutput);
            }
            points.push(points[0]);
            result
                .coplanar
                .push(Polyline3::try_new(points, Tolerance::MESH_VALIDATION)?);
        }
    }
    Ok(())
}
