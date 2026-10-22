//! Shared structural transfer lives with geometry, alongside native consumers.
pub(crate) use viboceros_geometry::interop::{
    GeometryCodecError, decode_arc, decode_brep, decode_polycurve, encode_arc, encode_brep,
    encode_polycurve,
};
