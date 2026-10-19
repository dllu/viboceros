//! Structural validation of exact native equivalents without writing files.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeometryCheckStatus {
    Valid,
    Invalid,
    /// Transfer or admission failed before native validity could be determined.
    NotChecked,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeometryCheck {
    pub status: GeometryCheckStatus,
    pub diagnostics: String,
}
impl GeometryCheck {
    pub fn not_checked(reason: impl Into<String>) -> Self {
        Self {
            status: GeometryCheckStatus::NotChecked,
            diagnostics: reason.into(),
        }
    }
}

/// Uses the same constructors and exact representation policy as 3DM transfer.
/// Invalid geometry is distinct from an unsupported/unrepresentable native copy.
/// Instance references need their actual definition UUID; use the separate entry.
pub fn check_3dm_geometry(geometry: &ThreeDmGeometry) -> GeometryCheck {
    if matches!(geometry, ThreeDmGeometry::InstanceReference { .. }) {
        return GeometryCheck::not_checked("Instance checks require the definition UUID");
    }
    check(geometry, None)
}

/// Validate only the reference's UUID and placement, as native IsValid does.
/// The document separately owns definition lookup and catalog graph integrity.
pub fn check_3dm_instance_reference(
    definition_uuid: &str,
    transform: viboceros_geometry::AffineTransform3,
) -> GeometryCheck {
    let Ok(id) = definition_uuid.parse::<uuid::Uuid>() else {
        return GeometryCheck::not_checked("Malformed instance definition UUID");
    };
    let id = CString::new(id.to_string()).expect("UUID has no embedded NUL");
    check(
        &ThreeDmGeometry::InstanceReference {
            definition_index: 0,
            transform,
        },
        Some(&id),
    )
}

fn check(geometry: &ThreeDmGeometry, definition_uuid: Option<&CStr>) -> GeometryCheck {
    let payload = match ObjectPayload::from_geometry(geometry) {
        Ok(payload) => payload,
        Err(error) => return GeometryCheck::not_checked(error.to_string()),
    };
    // Bound native transfer work without mistaking a resource limit for bad data.
    let total = payload
        .coordinates
        .len()
        .checked_add(payload.knots_u.len())
        .and_then(|n| n.checked_add(payload.knots_v.len()))
        .and_then(|n| n.checked_mul(8))
        .and_then(|n| {
            payload
                .indices
                .len()
                .checked_mul(4)
                .and_then(|m| n.checked_add(m))
        })
        .and_then(|n| n.checked_add(payload.geometry_data.len()));
    if total.is_none_or(|n| n > 256 * 1024 * 1024) {
        return GeometryCheck::not_checked("Native geometry check payload exceeds 256 MiB");
    }
    let view = ffi::ViboWriteObject {
        object_type: payload.object_type,
        rational_curve: u8::from(
            matches!(geometry, ThreeDmGeometry::NurbsCurve(c) if c.has_rational_representation()),
        ),
        degree_u: payload.degree_u,
        degree_v: payload.degree_v,
        control_point_count_u: payload.control_point_count_u,
        control_point_count_v: payload.control_point_count_v,
        coordinates: pointer_or_null(&payload.coordinates),
        coordinate_count: payload.coordinates.len(),
        knots_u: pointer_or_null(&payload.knots_u),
        knot_u_count: payload.knots_u.len(),
        knots_v: pointer_or_null(&payload.knots_v),
        knot_v_count: payload.knots_v.len(),
        indices: pointer_or_null(&payload.indices),
        index_count: payload.indices.len(),
        geometry_data: pointer_or_null(&payload.geometry_data),
        geometry_data_count: payload.geometry_data.len(),
        ..Default::default()
    };
    let mut valid = -1;
    let mut details = vec![0u8; 16384];
    // The payload and UUID own every referenced array through this call. The
    // native routine borrows them, owns its temporary copy and always NUL-terminates.
    let result = unsafe {
        ffi::vibo_geometry_check(
            &view,
            definition_uuid.map_or(std::ptr::null(), CStr::as_ptr),
            &mut valid,
            details.as_mut_ptr().cast(),
            details.len(),
        )
    };
    let diagnostics = unsafe { CStr::from_ptr(details.as_ptr().cast()) }
        .to_string_lossy()
        .trim()
        .to_owned();
    let status = match (result, valid) {
        (1, 1) => GeometryCheckStatus::Valid,
        (1, 0) => GeometryCheckStatus::Invalid,
        _ => GeometryCheckStatus::NotChecked,
    };
    GeometryCheck {
        status,
        diagnostics,
    }
}

#[cfg(test)]
mod tests;
