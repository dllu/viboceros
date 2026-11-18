//! Shared structural transfer lives with geometry, alongside native consumers.
#[cfg(test)]
pub(crate) use viboceros_geometry::interop::decode_brep;
pub(crate) use viboceros_geometry::interop::{
    GeometryCodecError, decode_arc, decode_archive_brep, decode_polycurve, encode_arc, encode_brep,
    encode_polycurve,
};
