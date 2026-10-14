//! Read-only infinite plane definitions, independent of saved surface objects.
use super::*;
use viboceros_geometry::Frame3;

pub const USAGE: &str = "InfinitePlane=WorldXY|WorldYZ|WorldZX|CPlaneXY|CPlaneYZ|CPlaneZX|x,y,z,nx,ny,nz|Object:id[:face]|AroundCurve:id:parameter";

#[derive(Clone, Copy, Debug)]
pub struct InfinitePlane {
    pub plane: Plane,
    pub object: Option<ObjectId>,
}

pub fn axis_plane(value: &str, cplane: Frame3) -> Option<Plane> {
    let value = value.trim_start_matches('_').to_ascii_lowercase();
    let (frame, axis) = if let Some(axis) = value.strip_prefix("world") {
        (construction_plane::WorldPlane::Top.frame(), axis)
    } else {
        (cplane, value.strip_prefix("cplane")?)
    };
    let normal = match axis {
        "xy" => frame.z_axis(),
        "yz" => frame.x_axis(),
        "zx" => frame.y_axis(),
        _ => return None,
    };
    Some(Plane::new(frame.origin(), normal))
}

pub fn object_plane(
    document: &Document,
    id: ObjectId,
    face: Option<usize>,
) -> Result<InfinitePlane, CommandError> {
    if !document.is_object_selectable(id) {
        return Err(CommandError::Usage(USAGE));
    }
    let geometry = document
        .object(id)
        .ok_or(DocumentError::ObjectNotFound(id))?
        .geometry();
    let tolerance = document.tolerance();
    let plane = match geometry {
        Geometry::NurbsSurface(s) if face.is_none_or(|i| i == 0) => {
            s.plane(tolerance)?.ok_or(CommandError::Usage(USAGE))?
        }
        Geometry::Brep(b) => {
            let index = face
                .or_else(|| (b.faces().len() == 1).then_some(0))
                .ok_or(CommandError::Usage(USAGE))?;
            b.faces()
                .get(index)
                .ok_or(CommandError::Usage(USAGE))?
                .surface()
                .plane(tolerance)?
                .ok_or(CommandError::Usage(USAGE))?
        }
        Geometry::Mesh(m) => {
            let index = face
                .or_else(|| (m.faces().len() == 1).then_some(0))
                .ok_or(CommandError::Usage(USAGE))?;
            let frame = construction_plane::frame_from_mesh_face(m, index, tolerance)
                .map_err(|_| CommandError::Usage(USAGE))?;
            let plane = Plane::new(frame.origin(), frame.z_axis());
            for &vertex in m
                .faces()
                .get(index)
                .ok_or(CommandError::Usage(USAGE))?
                .indices()
            {
                if plane
                    .signed_distance_to(m.vertices()[vertex as usize])?
                    .abs()
                    > tolerance.absolute()
                {
                    return Err(CommandError::Usage(USAGE));
                }
            }
            plane
        }
        _ if face.is_none()
            && geometry
                .curve_ref()
                .is_some_and(|c| c.is_planar(tolerance).unwrap_or(false)) =>
        {
            let frame = construction_plane::frame_from_object(geometry, tolerance)
                .map_err(|_| CommandError::Usage(USAGE))?;
            Plane::new(frame.origin(), frame.z_axis())
        }
        _ => return Err(CommandError::Usage(USAGE)),
    };
    Ok(InfinitePlane {
        plane,
        object: Some(id),
    })
}

pub fn around_curve(
    document: &Document,
    id: ObjectId,
    parameter: Real,
) -> Result<InfinitePlane, CommandError> {
    if !document.is_object_selectable(id) {
        return Err(CommandError::Usage(USAGE));
    }
    let curve = document
        .object(id)
        .and_then(|o| o.geometry().curve_ref())
        .ok_or(CommandError::Usage(USAGE))?;
    if !curve.domain().contains(&parameter) {
        return Err(CommandError::Usage(USAGE));
    }
    let sample = curve.evaluate_with_tangent(parameter)?;
    Ok(InfinitePlane {
        plane: Plane::new(sample.point(), sample.tangent()),
        object: Some(id),
    })
}

pub fn resolve(
    document: &Document,
    value: &str,
    cplane: Frame3,
) -> Result<InfinitePlane, CommandError> {
    if let Some(plane) = axis_plane(value, cplane) {
        return Ok(InfinitePlane {
            plane,
            object: None,
        });
    }
    let parts = value.trim_start_matches('_').split(':').collect::<Vec<_>>();
    if parts
        .first()
        .is_some_and(|p| p.eq_ignore_ascii_case("Object"))
    {
        let (id, face) = match parts.as_slice() {
            [_, id] => (*id, None),
            [_, id, face] => (
                *id,
                Some(
                    face.parse::<usize>()
                        .map_err(|_| CommandError::Usage(USAGE))?,
                ),
            ),
            _ => return Err(CommandError::Usage(USAGE)),
        };
        return object_plane(
            document,
            id.parse().map_err(|_| CommandError::Usage(USAGE))?,
            face,
        );
    }
    if let [kind, id, parameter] = parts.as_slice()
        && kind.eq_ignore_ascii_case("AroundCurve")
    {
        return around_curve(
            document,
            id.parse().map_err(|_| CommandError::Usage(USAGE))?,
            parse_finite_real(parameter)?,
        );
    }
    let coordinates = value
        .split(',')
        .map(parse_finite_real)
        .collect::<Result<Vec<_>, _>>()?;
    let [x, y, z, nx, ny, nz] = coordinates.as_slice() else {
        return Err(CommandError::Usage(USAGE));
    };
    let normal = Vector3::try_new(*nx, *ny, *nz)?.normalized_nonzero()?;
    Ok(InfinitePlane {
        plane: Plane::new(Point3::try_new(*x, *y, *z)?, normal),
        object: None,
    })
}
