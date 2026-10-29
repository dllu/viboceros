//! Shared owned transfer payload for native checks and technical reports.
use super::*;

pub(super) struct DiagnosticPayload {
    payload: ObjectPayload,
    rational_curve: u8,
}

impl DiagnosticPayload {
    pub fn new(geometry: &ThreeDmGeometry) -> Result<Self, ThreeDmError> {
        let payload = ObjectPayload::from_geometry(geometry)?;
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
            return Err(ThreeDmError::Native(
                "Native geometry diagnostic payload exceeds 256 MiB".into(),
            ));
        }
        Ok(Self {
            payload,
            rational_curve: u8::from(
                matches!(geometry, ThreeDmGeometry::NurbsCurve(c) if c.has_rational_representation()),
            ),
        })
    }

    /// Borrowed pointers remain valid while this payload is alive and unchanged.
    pub fn view(&self) -> ffi::ViboWriteObject {
        let p = &self.payload;
        ffi::ViboWriteObject {
            object_type: p.object_type,
            rational_curve: self.rational_curve,
            degree_u: p.degree_u,
            degree_v: p.degree_v,
            control_point_count_u: p.control_point_count_u,
            control_point_count_v: p.control_point_count_v,
            coordinates: pointer_or_null(&p.coordinates),
            coordinate_count: p.coordinates.len(),
            knots_u: pointer_or_null(&p.knots_u),
            knot_u_count: p.knots_u.len(),
            knots_v: pointer_or_null(&p.knots_v),
            knot_v_count: p.knots_v.len(),
            indices: pointer_or_null(&p.indices),
            index_count: p.indices.len(),
            geometry_data: pointer_or_null(&p.geometry_data),
            geometry_data_count: p.geometry_data.len(),
            ..Default::default()
        }
    }
}
