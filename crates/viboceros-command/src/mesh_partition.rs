//! Mesh, surface, projected-curve and finite representations of infinite cutters.
use crate::*;
use std::borrow::Cow;
use viboceros_geometry::{BoundingBox3, Frame3, MeshFace};

pub(super) fn source_mesh<'a>(
    geometry: &'a Geometry,
    tolerance: Tolerance,
) -> Result<Cow<'a, TriangleMesh>, CommandError> {
    Ok(match geometry {
        Geometry::Mesh(mesh) => Cow::Borrowed(mesh),
        Geometry::Brep(b) => Cow::Owned(b.polygon_mesh(0.5, true, false, tolerance)?),
        Geometry::NurbsSurface(s) => {
            Cow::Owned(s.tessellate(interface::SURFACE_DISPLAY_SAMPLES_PER_SPAN, tolerance)?)
        }
        _ => {
            return Err(CommandError::Usage(
                "meshes, surfaces, polysurfaces or curve cutters",
            ));
        }
    })
}

fn plane_mesh(
    plane: Plane,
    bounds: BoundingBox3,
    tolerance: Tolerance,
) -> Result<TriangleMesh, CommandError> {
    let frame = Frame3::try_from_unit_normal(plane.origin(), plane.normal())?;
    let mut lo = [f64::INFINITY; 2];
    let mut hi = [f64::NEG_INFINITY; 2];
    for point in crate::bounding_box::bounding_box_corners(bounds)? {
        let p = frame.projected_coordinates_of(point)?;
        for i in 0..2 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    let margin = (bounds.min().distance_to(bounds.max())? * 0.01).max(tolerance.absolute() * 4.);
    let vertices = [
        [lo[0] - margin, lo[1] - margin],
        [hi[0] + margin, lo[1] - margin],
        [hi[0] + margin, hi[1] + margin],
        [lo[0] - margin, hi[1] + margin],
    ]
    .into_iter()
    .map(|[x, y]| frame.point_at([x, y, 0.]))
    .collect::<Result<Vec<_>, _>>()?;
    Ok(TriangleMesh::try_from_face_records(
        vertices,
        vec![MeshFace::Quad([0, 1, 2, 3])],
    )?)
}

fn curve_mesh(
    geometry: &Geometry,
    bounds: BoundingBox3,
    cplane: Frame3,
    tolerance: Tolerance,
) -> Result<TriangleMesh, CommandError> {
    let curve = geometry.curve_ref().ok_or(CommandError::Usage(
        "meshes, surfaces, polysurfaces or curve cutters",
    ))?;
    let points = match curve.polyline_vertices()? {
        Some(points) => points,
        None => curve
            .try_polyline_by_angle(0.01, MAX_SPAN_OUTPUT_OBJECTS)?
            .vertices()
            .to_vec(),
    };
    let z = cplane.z_axis().as_vector();
    let mut range = [f64::INFINITY, f64::NEG_INFINITY];
    for p in crate::bounding_box::bounding_box_corners(bounds)?
        .into_iter()
        .chain(points.iter().copied())
    {
        let height = cplane.origin().vector_to(p)?.dot(z)?;
        range[0] = range[0].min(height);
        range[1] = range[1].max(height);
    }
    let margin = (bounds.min().distance_to(bounds.max())? * 0.01).max(tolerance.absolute() * 4.);
    let mut vertices = Vec::with_capacity(2 * points.len());
    for &p in &points {
        let height = cplane.origin().vector_to(p)?.dot(z)?;
        vertices.push(p.translated(z.scaled(range[0] - height - margin)?)?);
        vertices.push(p.translated(z.scaled(range[1] - height + margin)?)?);
    }
    let faces = (0..points.len().saturating_sub(1))
        .map(|i| {
            MeshFace::Quad([
                2 * i as u32,
                2 * i as u32 + 1,
                2 * i as u32 + 3,
                2 * i as u32 + 2,
            ])
        })
        .collect();
    Ok(TriangleMesh::try_from_face_records(vertices, faces)?)
}

pub(super) fn cutters<'a>(
    doc: &'a Document,
    target: ObjectId,
    bounds: BoundingBox3,
    objects: &[ObjectId],
    planes: &[String],
    context: CommandContext,
    tolerance: Tolerance,
) -> Result<Vec<Cow<'a, TriangleMesh>>, CommandError> {
    let mut cutters = Vec::new();
    for &id in objects {
        if id == target {
            continue;
        }
        let geometry = doc
            .object(id)
            .ok_or(DocumentError::ObjectNotFound(id))?
            .geometry();
        cutters.push(if geometry.curve_ref().is_some() {
            Cow::Owned(curve_mesh(
                geometry,
                bounds,
                context.construction_plane,
                tolerance,
            )?)
        } else {
            source_mesh(geometry, doc.tolerance())?
        });
    }
    for definition in planes {
        let plane = infinite_plane::resolve(doc, definition, context.construction_plane)?.plane;
        cutters.push(Cow::Owned(plane_mesh(plane, bounds, tolerance)?));
    }
    Ok(cutters)
}

mod appearance;
pub(super) use appearance::Appearance;
