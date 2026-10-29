//! Technical OpenNURBS dumps of owned, representation-preserving native copies.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeometryDump {
    pub text: String,
    /// The native output exceeded the 128 KiB report buffer.
    pub truncated: bool,
}

/// Dump the exact native equivalent used by 3DM transfer, without writing files.
/// An instance reference requires its real definition UUID; use the separate entry.
pub fn dump_3dm_geometry(geometry: &ThreeDmGeometry) -> Result<GeometryDump, ThreeDmError> {
    if matches!(geometry, ThreeDmGeometry::InstanceReference { .. }) {
        return Err(ThreeDmError::Native(
            "Instance dumps require the definition UUID".into(),
        ));
    }
    dump(geometry, None)
}

pub fn dump_3dm_instance_reference(
    definition_uuid: &str,
    transform: viboceros_geometry::AffineTransform3,
) -> Result<GeometryDump, ThreeDmError> {
    let id = definition_uuid
        .parse::<uuid::Uuid>()
        .map_err(|_| ThreeDmError::Native("Malformed instance definition UUID".into()))?;
    let id = CString::new(id.to_string()).expect("UUID has no embedded NUL");
    dump(
        &ThreeDmGeometry::InstanceReference {
            definition_index: 0,
            transform,
        },
        Some(&id),
    )
}

fn dump(geometry: &ThreeDmGeometry, uuid: Option<&CStr>) -> Result<GeometryDump, ThreeDmError> {
    let payload = diagnostics::DiagnosticPayload::new(geometry)?;
    let view = payload.view();
    let mut bytes = vec![0u8; 128 * 1024];
    let mut truncated = 0;
    // Payload, UUID and output remain alive through the call. Native code owns
    // its temporary geometry and always NUL-terminates the output buffer.
    let result = unsafe {
        ffi::vibo_geometry_dump(
            &view,
            uuid.map_or(std::ptr::null(), CStr::as_ptr),
            &mut truncated,
            bytes.as_mut_ptr().cast(),
            bytes.len(),
        )
    };
    let text = unsafe { CStr::from_ptr(bytes.as_ptr().cast()) }
        .to_string_lossy()
        .trim()
        .to_owned();
    if result != 1 {
        return Err(ThreeDmError::Native(text));
    }
    Ok(GeometryDump {
        text,
        truncated: truncated != 0,
    })
}

#[cfg(test)]
mod tests;
