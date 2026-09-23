//! Versioned n-gon and vertex-color payload for the OpenNURBS bridge's mesh records.
use viboceros_geometry::MeshNgon;

use super::ThreeDmError;

const MAGIC: &[u8; 8] = b"VIBONGON";
const VERSION: u32 = 2;

#[derive(Debug, PartialEq)]
pub(super) struct MeshPayload {
    pub(super) ngons: Vec<MeshNgon>,
    pub(super) vertex_colors: Option<Vec<[u8; 4]>>,
}

pub(super) fn encode(
    ngons: &[MeshNgon],
    colors: Option<&[[u8; 4]]>,
) -> Result<Vec<u8>, ThreeDmError> {
    if ngons.is_empty() && colors.is_none() {
        return Ok(Vec::new());
    }
    let mut bytes = Vec::new();
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&VERSION.to_le_bytes());
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
    Ok(bytes)
}

pub(super) fn decode(bytes: &[u8], vertex_count: usize) -> Result<MeshPayload, ThreeDmError> {
    if bytes.is_empty() {
        return Ok(MeshPayload {
            ngons: Vec::new(),
            vertex_colors: None,
        });
    }
    let mut reader = Reader { bytes, cursor: 0 };
    if reader.take(8)? != MAGIC || reader.u32()? != VERSION {
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
    if reader.remaining() != 0 {
        return Err(invalid());
    }
    Ok(MeshPayload {
        ngons,
        vertex_colors: colors,
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

    fn u32(&mut self) -> Result<u32, ThreeDmError> {
        let data: [u8; 4] = self.take(4)?.try_into().map_err(|_| invalid())?;
        Ok(u32::from_le_bytes(data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codec_round_trips_and_rejects_truncation_or_trailing_bytes() {
        let ngons = vec![MeshNgon::from_parts(vec![0, 1, 2, 3], vec![5, 6])];
        let colors = [[1, 2, 3, 0], [4, 5, 6, 128]];
        let bytes = encode(&ngons, Some(&colors)).unwrap();
        assert_eq!(
            decode(&bytes, colors.len()).unwrap(),
            MeshPayload {
                ngons,
                vertex_colors: Some(colors.to_vec()),
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
            }
        );
        assert!(decode(&encode(&[], Some(&colors)).unwrap(), colors.len() + 1).is_err());
        let ngons = vec![MeshNgon::from_parts(vec![0, 1, 2], vec![0])];
        assert_eq!(
            decode(&encode(&ngons, None).unwrap(), 3).unwrap(),
            MeshPayload {
                ngons,
                vertex_colors: None,
            }
        );
    }
}
