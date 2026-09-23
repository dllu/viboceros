//! Versioned n-gon payload for the OpenNURBS bridge's mesh records.
use viboceros_geometry::MeshNgon;

use super::ThreeDmError;

const MAGIC: &[u8; 8] = b"VIBONGON";
const VERSION: u32 = 1;

pub(super) fn encode(ngons: &[MeshNgon]) -> Result<Vec<u8>, ThreeDmError> {
    if ngons.is_empty() {
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
    Ok(bytes)
}

pub(super) fn decode(bytes: &[u8]) -> Result<Vec<MeshNgon>, ThreeDmError> {
    if bytes.is_empty() {
        return Ok(Vec::new());
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
    if reader.remaining() != 0 {
        return Err(invalid());
    }
    Ok(ngons)
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
        let bytes = encode(&ngons).unwrap();
        assert_eq!(decode(&bytes).unwrap(), ngons);
        for end in 1..bytes.len() {
            assert!(decode(&bytes[..end]).is_err());
        }
        let mut trailing = bytes;
        trailing.push(0);
        assert!(decode(&trailing).is_err());
        assert!(decode(&[]).unwrap().is_empty());
    }
}
