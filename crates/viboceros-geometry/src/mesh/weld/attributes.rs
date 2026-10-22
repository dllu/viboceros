//! Stored seam attributes share one reduction policy across welding tools.
use super::*;

pub(in crate::mesh) struct WeldAttributes {
    pub channels: VertexChannels,
    pub colors: Option<Vec<[u8; 4]>>,
}
impl WeldAttributes {
    pub fn new(source: &TriangleMesh, normals: Option<&[Vector3]>) -> Self {
        Self {
            channels: VertexChannels {
                uv: source.vertex_channels.uv.clone(),
                normals: normals.map(<[_]>::to_vec),
            },
            colors: source.vertex_colors.clone(),
        }
    }
    pub fn blend(&mut self, a: usize, b: usize, allow_zero: bool) -> Result<bool, GeometryError> {
        if let Some(normals) = &mut self.channels.normals {
            let normal = match normalized_sum([normals[a], normals[b]])? {
                Some(normal) => normal,
                None if allow_zero => Vector3::try_from([0.; 3])?,
                None => return Ok(false),
            };
            normals[a] = normal;
            normals[b] = normal;
        }
        if let Some(uv) = &mut self.channels.uv {
            let blended = std::array::from_fn(|i| ((uv[a][i] + uv[b][i]) * 0.5) as f32 as Real);
            uv[a] = blended;
            uv[b] = blended;
        }
        if let Some(colors) = &mut self.colors {
            let blended = std::array::from_fn(|i| {
                // Public ON_Color fractional storage and nearest-byte rounding.
                let factor = 0.003_921_568_627_450_98;
                (((colors[a][i] as Real * factor + colors[b][i] as Real * factor) * 0.5) * 255.)
                    .round() as u8
            });
            colors[a] = blended;
            colors[b] = blended;
        }
        Ok(true)
    }
}

pub(in crate::mesh) fn normalized_sum(
    normals: impl IntoIterator<Item = Vector3>,
) -> Result<Option<Vector3>, GeometryError> {
    let mut sum = [0.; 3];
    for normal in normals {
        for (sum, value) in sum.iter_mut().zip(normal.to_array()) {
            *sum += value;
        }
    }
    let Ok(normal) = Vector3::try_from(sum)?.normalized_nonzero() else {
        return Ok(None);
    };
    Vector3::try_from(normal.as_vector().to_array().map(|n| n as f32 as Real)).map(Some)
}
