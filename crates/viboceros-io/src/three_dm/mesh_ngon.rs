//! Versioned mesh attributes for the OpenNURBS bridge's mesh records.
use viboceros_geometry::{MeshNgon, Vector3};

use super::ThreeDmError;

const MAGIC: &[u8; 8] = b"VIBONGON";
const VERSION: u32 = 2;

#[derive(Debug, PartialEq)]
pub(super) struct MeshPayload {
    pub(super) ngons: Vec<MeshNgon>,
    pub(super) vertex_colors: Option<Vec<[u8; 4]>>,
    pub(super) texture_coordinates: Option<Vec<[f64; 2]>>,
    pub(super) vertex_normals: Option<Vec<Vector3>>,
}

pub(super) fn encode(
    ngons: &[MeshNgon],
    colors: Option<&[[u8; 4]]>,
    texture_coordinates: Option<&[[f64; 2]]>,
    vertex_normals: Option<&[Vector3]>,
) -> Result<Vec<u8>, ThreeDmError> {
    if ngons.is_empty()
        && colors.is_none()
        && texture_coordinates.is_none()
        && vertex_normals.is_none()
    {
        return Ok(Vec::new());
    }
    let mut bytes = Vec::new();
    bytes.extend_from_slice(MAGIC);
    let version = if texture_coordinates.is_some() || vertex_normals.is_some() {
        3_u32
    } else {
        VERSION
    };
    bytes.extend_from_slice(&version.to_le_bytes());
    write_count(&mut bytes, ngons.len())?;
    for ngon in ngons {
        write_count(&mut bytes, ngon.vertices().len())?;
        write_count(&mut bytes, ngon.faces().len())?;
        for &index in ngon.vertices().iter().chain(ngon.faces()) {
            bytes.extend_from_slice(&index.to_le_bytes());
        }
    }
    write_count(&mut bytes, colors.map_or(0, |colors| colors.len()))?;
    if let Some(colors) = colors {
        bytes.extend(colors.iter().flat_map(|color| *color));
    }
    if version >= 3 {
        write_count(&mut bytes, texture_coordinates.map_or(0, <[_]>::len))?;
        if let Some(values) = texture_coordinates {
            for value in values.iter().flatten() {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        write_count(&mut bytes, vertex_normals.map_or(0, <[_]>::len))?;
        if let Some(values) = vertex_normals {
            for value in values.iter().flat_map(|v| v.to_array()) {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
    }
    Ok(bytes)
}

pub(super) fn decode(bytes: &[u8], vertex_count: usize) -> Result<MeshPayload, ThreeDmError> {
    if bytes.is_empty() {
        return Ok(MeshPayload {
            ngons: Vec::new(),
            vertex_colors: None,
            texture_coordinates: None,
            vertex_normals: None,
        });
    }
    let mut reader = Reader { bytes, cursor: 0 };
    if reader.take(8)? != MAGIC {
        return Err(invalid());
    }
    let version = reader.u32()?;
    if version != VERSION && version != 3 {
        return Err(invalid());
    }
    let count = reader.u32()? as usize;
    if count > reader.remaining() / 8 {
        return Err(invalid());
    }
    let mut ngons = Vec::new();
    ngons.try_reserve(count).map_err(|_| invalid())?;
    for _ in 0..count {
        let vertex_count = reader.u32()? as usize;
        let face_count = reader.u32()? as usize;
        let index_count = vertex_count.checked_add(face_count).ok_or_else(invalid)?;
        if vertex_count < 3 || face_count == 0 || index_count > reader.remaining() / 4 {
            return Err(invalid());
        }
        let mut vertices = Vec::new();
        vertices.try_reserve(vertex_count).map_err(|_| invalid())?;
        for _ in 0..vertex_count {
            vertices.push(reader.u32()?);
        }
        let mut faces = Vec::new();
        faces.try_reserve(face_count).map_err(|_| invalid())?;
        for _ in 0..face_count {
            faces.push(reader.u32()?);
        }
        ngons.push(MeshNgon::from_parts(vertices, faces));
    }
    let color_count = reader.u32()? as usize;
    if color_count != 0 && color_count != vertex_count {
        return Err(invalid());
    }
    let colors = if color_count == 0 {
        None
    } else {
        let color_bytes = reader.take(color_count.checked_mul(4).ok_or_else(invalid)?)?;
        Some(
            color_bytes
                .chunks_exact(4)
                .map(|rgba| [rgba[0], rgba[1], rgba[2], rgba[3]])
                .collect(),
        )
    };
    let texture_coordinates = if version >= 3 {
        reader.channel::<2>(vertex_count)?
    } else {
        None
    };
    let vertex_normals = if version >= 3 {
        reader
            .channel::<3>(vertex_count)?
            .map(|n| {
                n.into_iter()
                    .map(Vector3::try_from)
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()
            .map_err(|_| invalid())?
    } else {
        None
    };
    if reader.remaining() != 0 {
        return Err(invalid());
    }
    Ok(MeshPayload {
        ngons,
        vertex_colors: colors,
        texture_coordinates,
        vertex_normals,
    })
}

fn write_count(bytes: &mut Vec<u8>, count: usize) -> Result<(), ThreeDmError> {
    let count = u32::try_from(count).map_err(|_| invalid())?;
    bytes.extend_from_slice(&count.to_le_bytes());
    Ok(())
}

fn invalid() -> ThreeDmError {
    ThreeDmError::InvalidModel("invalid mesh n-gon payload".to_owned())
}

struct Reader<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> Reader<'a> {
    fn remaining(&self) -> usize {
        self.bytes.len() - self.cursor
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], ThreeDmError> {
        let end = self.cursor.checked_add(count).ok_or_else(invalid)?;
        let data = self.bytes.get(self.cursor..end).ok_or_else(invalid)?;
        self.cursor = end;
        Ok(data)
    }

    fn channel<const N: usize>(
        &mut self,
        vertex_count: usize,
    ) -> Result<Option<Vec<[f64; N]>>, ThreeDmError> {
        let count = self.u32()? as usize;
        if count == 0 {
            return Ok(None);
        }
        if count != vertex_count || count > self.remaining() / (8 * N) {
            return Err(invalid());
        }
        let mut values = Vec::with_capacity(count);
        for _ in 0..count {
            let mut value = [0.; N];
            for coordinate in &mut value {
                *coordinate = f64::from_le_bytes(self.take(8)?.try_into().map_err(|_| invalid())?);
                if !(*coordinate as f32).is_finite() {
                    return Err(invalid());
                }
            }
            values.push(value);
        }
        Ok(Some(values))
    }

    fn u32(&mut self) -> Result<u32, ThreeDmError> {
        let data: [u8; 4] = self.take(4)?.try_into().map_err(|_| invalid())?;
        Ok(u32::from_le_bytes(data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertex_channel_payload_rejects_bad_counts_nonfinite_and_truncated_values() {
        let uv = [[0.25, 0.5]; 3];
        let normals = [Vector3::try_new(0., 0., 0.).unwrap(); 3];
        let bytes = encode(&[], None, Some(&uv), Some(&normals)).unwrap();
        let result = decode(&bytes, 3).unwrap();
        assert_eq!(result.texture_coordinates.as_deref(), Some(uv.as_slice()));
        assert_eq!(result.vertex_normals.as_deref(), Some(normals.as_slice()));
        for end in 1..bytes.len() {
            assert!(decode(&bytes[..end], 3).is_err());
        }
        assert!(decode(&bytes, 4).is_err());
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(decode(&trailing, 3).is_err());
        for value in [f64::NAN, f64::INFINITY, f64::MAX] {
            let mut bad = bytes.clone();
            // Header, n-gon count, color count, UV count, first UV component.
            bad[24..32].copy_from_slice(&value.to_le_bytes());
            assert!(decode(&bad, 3).is_err());
        }
    }

    #[test]
    fn codec_round_trips_and_rejects_truncation_or_trailing_bytes() {
        let ngons = vec![MeshNgon::from_parts(vec![0, 1, 2, 3], vec![5, 6])];
        let colors = [[1, 2, 3, 0], [4, 5, 6, 128]];
        let bytes = encode(&ngons, Some(&colors), None, None).unwrap();
        assert_eq!(
            decode(&bytes, colors.len()).unwrap(),
            MeshPayload {
                ngons,
                vertex_colors: Some(colors.to_vec()),
                texture_coordinates: None,
                vertex_normals: None,
            }
        );
        for end in 1..bytes.len() {
            assert!(decode(&bytes[..end], colors.len()).is_err());
        }
        let mut trailing = bytes;
        trailing.push(0);
        assert!(decode(&trailing, colors.len()).is_err());
        assert_eq!(
            decode(&[], 0).unwrap(),
            MeshPayload {
                ngons: Vec::new(),
                vertex_colors: None,
                texture_coordinates: None,
                vertex_normals: None,
            }
        );
        assert!(
            decode(
                &encode(&[], Some(&colors), None, None).unwrap(),
                colors.len() + 1
            )
            .is_err()
        );
        let ngons = vec![MeshNgon::from_parts(vec![0, 1, 2], vec![0])];
        assert_eq!(
            decode(&encode(&ngons, None, None, None).unwrap(), 3).unwrap(),
            MeshPayload {
                ngons,
                vertex_colors: None,
                texture_coordinates: None,
                vertex_normals: None,
            }
        );
    }
}
