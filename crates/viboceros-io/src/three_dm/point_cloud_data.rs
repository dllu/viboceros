//! Versioned per-point payload shared with the OpenNURBS bridge.
use viboceros_geometry::{Point3, PointCloud3, PointCloudChannels, PointCloudPlane, Vector3};

use super::ThreeDmError;

const MAGIC: &[u8; 8] = b"VIBOPCLD";
const VERSION: u32 = 2;
const COLORS: u32 = 1;
const NORMALS: u32 = 2;
const VALUES: u32 = 4;
const ORDERED: u32 = 8;
const PLANE: u32 = 16;

pub(super) fn encode(cloud: &PointCloud3) -> Vec<u8> {
    let channels = cloud.channels();
    let mut flags = 0;
    flags |= u32::from(channels.colors.is_some()) * COLORS;
    flags |= u32::from(channels.normals.is_some()) * NORMALS;
    flags |= u32::from(channels.values.is_some()) * VALUES;
    flags |= u32::from(channels.ordered) * ORDERED;
    flags |= u32::from(channels.plane.is_some()) * PLANE;
    if flags == 0 {
        return Vec::new();
    }
    let mut bytes = Vec::new();
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&VERSION.to_le_bytes());
    bytes.extend_from_slice(&flags.to_le_bytes());
    if let Some(colors) = &channels.colors {
        bytes.extend(colors.iter().flat_map(|rgba| *rgba));
    }
    if let Some(normals) = &channels.normals {
        for normal in normals {
            for coordinate in normal.to_array() {
                bytes.extend_from_slice(&coordinate.to_le_bytes());
            }
        }
    }
    if let Some(values) = &channels.values {
        for value in values {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    if let Some(plane) = channels.plane {
        for coordinate in plane.origin().to_array() {
            bytes.extend_from_slice(&coordinate.to_le_bytes());
        }
        for axis in plane.axes() {
            for coordinate in axis.to_array() {
                bytes.extend_from_slice(&coordinate.to_le_bytes());
            }
        }
    }
    bytes
}

pub(super) fn decode(bytes: &[u8], count: usize) -> Result<PointCloudChannels, ThreeDmError> {
    if bytes.is_empty() {
        return Ok(PointCloudChannels::default());
    }
    let mut reader = Reader { bytes, offset: 0 };
    if reader.take(8)? != MAGIC || reader.u32()? != VERSION {
        return Err(invalid());
    }
    let flags = reader.u32()?;
    if flags == 0 || flags & !(COLORS | NORMALS | VALUES | ORDERED | PLANE) != 0 {
        return Err(invalid());
    }
    let colors = if flags & COLORS != 0 {
        Some(
            reader
                .take(count.checked_mul(4).ok_or_else(invalid)?)?
                .chunks_exact(4)
                .map(|rgba| [rgba[0], rgba[1], rgba[2], rgba[3]])
                .collect(),
        )
    } else {
        None
    };
    let normals = if flags & NORMALS != 0 {
        let raw = reader.take(count.checked_mul(24).ok_or_else(invalid)?)?;
        Some(
            raw.chunks_exact(24)
                .map(|chunk| {
                    let values = std::array::from_fn(|index| {
                        let start = index * 8;
                        f64::from_le_bytes(chunk[start..start + 8].try_into().unwrap())
                    });
                    Vector3::try_from(values).map_err(ThreeDmError::from)
                })
                .collect::<Result<Vec<_>, _>>()?,
        )
    } else {
        None
    };
    let values = if flags & VALUES != 0 {
        let raw = reader.take(count.checked_mul(8).ok_or_else(invalid)?)?;
        Some(
            raw.chunks_exact(8)
                .map(|chunk| f64::from_le_bytes(chunk.try_into().unwrap()))
                .collect(),
        )
    } else {
        None
    };
    let plane = if flags & PLANE != 0 {
        let origin = Point3::try_from(reader.triple()?)?;
        let axes = [
            Vector3::try_from(reader.triple()?)?,
            Vector3::try_from(reader.triple()?)?,
            Vector3::try_from(reader.triple()?)?,
        ];
        Some(PointCloudPlane::try_new(origin, axes)?)
    } else {
        None
    };
    if !reader.finished() {
        return Err(invalid());
    }
    Ok(PointCloudChannels {
        colors,
        normals,
        values,
        ordered: flags & ORDERED != 0,
        plane,
        hidden: None,
    })
}

fn invalid() -> ThreeDmError {
    ThreeDmError::InvalidModel("invalid point-cloud channel payload".into())
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], ThreeDmError> {
        let end = self.offset.checked_add(count).ok_or_else(invalid)?;
        let chunk = self.bytes.get(self.offset..end).ok_or_else(invalid)?;
        self.offset = end;
        Ok(chunk)
    }

    fn u32(&mut self) -> Result<u32, ThreeDmError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn triple(&mut self) -> Result<[f64; 3], ThreeDmError> {
        let bytes = self.take(24)?;
        Ok(std::array::from_fn(|index| {
            f64::from_le_bytes(bytes[index * 8..index * 8 + 8].try_into().unwrap())
        }))
    }

    fn finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_geometry::Point3;

    #[test]
    fn channel_payload_round_trips_and_rejects_damage() {
        let cloud = PointCloud3::try_with_channels(
            vec![Point3::try_new(1.0, 2.0, 3.0).unwrap()],
            PointCloudChannels {
                colors: Some(vec![[12, 34, 56, 78]]),
                normals: Some(vec![Vector3::try_new(0.0, 1.0, 0.0).unwrap()]),
                values: Some(vec![-12.5]),
                ordered: true,
                plane: Some(
                    PointCloudPlane::try_new(
                        Point3::try_new(0.0, 0.0, 5.0).unwrap(),
                        [
                            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
                            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
                            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
                        ],
                    )
                    .unwrap(),
                ),
                hidden: None,
            },
        )
        .unwrap();
        let bytes = encode(&cloud);
        assert_eq!(decode(&bytes, 1).unwrap(), *cloud.channels());
        let hidden_cloud = cloud.with_hidden(vec![true]).unwrap();
        assert_eq!(encode(&hidden_cloud), bytes);
        assert_eq!(decode(&encode(&hidden_cloud), 1).unwrap().hidden, None);
        for end in 1..bytes.len() {
            assert!(decode(&bytes[..end], 1).is_err());
        }
        let mut trailing = bytes;
        trailing.push(0);
        assert!(decode(&trailing, 1).is_err());
        assert_eq!(decode(&[], 1).unwrap(), PointCloudChannels::default());
    }
}
