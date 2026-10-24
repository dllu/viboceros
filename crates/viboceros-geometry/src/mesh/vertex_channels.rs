//! Raw-vertex channels follow explicit source indices, never position matching.
use super::*;
use crate::{AffineNormalTransform3, Vector3};

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct VertexChannels {
    pub uv: Option<Vec<[Real; 2]>>,
    pub normals: Option<Vec<Vector3>>,
}

fn stored(value: Real, context: &'static str) -> Result<Real, GeometryError> {
    let value = value as f32;
    if value.is_finite() {
        Ok(value as Real)
    } else {
        Err(GeometryError::Degenerate { context })
    }
}

impl TriangleMesh {
    pub(super) fn stored_or_computed_normals(
        &self,
    ) -> Result<Option<std::borrow::Cow<'_, [Vector3]>>, GeometryError> {
        if let Some(normals) = self.vertex_normals() {
            return Ok(Some(std::borrow::Cow::Borrowed(normals)));
        }
        viboceros_opennurbs::recompute_mesh_normals(&self.open_nurbs_data())?
            .vertex_normals
            .map(|normals| {
                normals
                    .into_iter()
                    .map(Vector3::try_from)
                    .collect::<Result<Vec<_>, _>>()
                    .map(std::borrow::Cow::Owned)
            })
            .transpose()
    }
    /// Rebuild an existing normal channel after changing face geometry. Meshes
    /// without stored normals keep their implicit normal representation.
    pub(super) fn recompute_stored_normals(&mut self) -> Result<(), GeometryError> {
        self.recompute_stored_normals_from(0)
    }

    /// Radial unwelding appends replacements after retained raw vertices.
    /// Those retained normals remain verbatim, even with ModifyNormals enabled.
    pub(super) fn recompute_stored_normals_from(
        &mut self,
        first_vertex: usize,
    ) -> Result<(), GeometryError> {
        if self.vertex_channels.normals.is_some() {
            let result = viboceros_opennurbs::recompute_mesh_normals(&self.open_nurbs_data())?;
            match result.vertex_normals {
                Some(normals) => {
                    let stored = self.vertex_channels.normals.as_mut().unwrap();
                    for (target, normal) in stored.iter_mut().zip(normals).skip(first_vertex) {
                        *target = Vector3::try_from(normal)?;
                    }
                }
                None => self.vertex_channels.normals = None,
            }
        }
        Ok(())
    }
    /// Texture coordinates use the native mesh's float storage, indexed by raw
    /// vertex. Coincident seam vertices may have different coordinates.
    pub fn try_with_texture_coordinates(
        mut self,
        uv: Option<Vec<[Real; 2]>>,
    ) -> Result<Self, GeometryError> {
        self.vertex_channels.uv = uv
            .map(|values| {
                if values.len() != self.vertices.len() {
                    return Err(GeometryError::Degenerate {
                        context: "mesh texture coordinate count",
                    });
                }
                values
                    .into_iter()
                    .map(|[u, v]| {
                        Ok([
                            stored(u, "mesh texture coordinate")?,
                            stored(v, "mesh texture coordinate")?,
                        ])
                    })
                    .collect()
            })
            .transpose()?;
        Ok(self)
    }
    pub fn texture_coordinates(&self) -> Option<&[[Real; 2]]> {
        self.vertex_channels.uv.as_deref()
    }

    /// Retain native stored normals verbatim, including zero or nonunit values.
    pub fn try_with_vertex_normals(
        mut self,
        normals: Option<Vec<Vector3>>,
    ) -> Result<Self, GeometryError> {
        self.vertex_channels.normals = normals
            .map(|values| {
                if values.len() != self.vertices.len() {
                    return Err(GeometryError::Degenerate {
                        context: "mesh vertex normal count",
                    });
                }
                values
                    .into_iter()
                    .map(|n| {
                        let [x, y, z] = n.to_array();
                        Vector3::try_new(
                            stored(x, "mesh vertex normal")?,
                            stored(y, "mesh vertex normal")?,
                            stored(z, "mesh vertex normal")?,
                        )
                    })
                    .collect()
            })
            .transpose()?;
        Ok(self)
    }
    pub fn vertex_normals(&self) -> Option<&[Vector3]> {
        self.vertex_channels.normals.as_deref()
    }
}

impl VertexChannels {
    pub fn transformed(&self, transform: AffineTransform3) -> Result<Self, GeometryError> {
        let map = AffineNormalTransform3::new(transform);
        Ok(Self {
            uv: self.uv.clone(),
            normals: self
                .normals
                .as_ref()
                .map(|normals| {
                    normals
                        .iter()
                        .map(|&n| {
                            if n.to_array() == [0.; 3] {
                                return Ok(n);
                            }
                            let n = map.transform_normal(n)?.as_vector();
                            Vector3::try_from(n.to_array().map(|x| x as f32 as Real))
                        })
                        .collect()
                })
                .transpose()?,
        })
    }
    pub fn pop(&mut self) {
        if let Some(uv) = &mut self.uv {
            uv.pop();
        }
        if let Some(normals) = &mut self.normals {
            normals.pop();
        }
    }
    pub fn remapped(&self, indices: impl IntoIterator<Item = usize>) -> Self {
        let mut output = Self {
            uv: self.uv.as_ref().map(|_| Vec::new()),
            normals: self.normals.as_ref().map(|_| Vec::new()),
        };
        for index in indices {
            if let Some(uv) = &mut output.uv {
                uv.push(self.uv.as_ref().unwrap()[index]);
            }
            if let Some(normals) = &mut output.normals {
                normals.push(self.normals.as_ref().unwrap()[index]);
            }
        }
        output
    }
    pub fn appended(meshes: &[&TriangleMesh]) -> Self {
        let uv = meshes
            .iter()
            .all(|m| m.texture_coordinates().is_some())
            .then(|| {
                meshes
                    .iter()
                    .flat_map(|m| m.texture_coordinates().unwrap().iter().copied())
                    .collect()
            });
        let normals = meshes
            .iter()
            .all(|m| m.vertex_normals().is_some())
            .then(|| {
                meshes
                    .iter()
                    .flat_map(|m| m.vertex_normals().unwrap().iter().copied())
                    .collect()
            });
        Self { uv, normals }
    }
    pub fn reversed(&self) -> Self {
        Self {
            uv: self.uv.clone(),
            normals: self.normals.as_ref().map(|n| {
                n.iter()
                    .map(|n| {
                        Vector3::try_from(n.to_array().map(|x| -x)).expect("finite stored normal")
                    })
                    .collect()
            }),
        }
    }
}

pub(super) struct ChannelBuilder<'a> {
    source: &'a VertexChannels,
    normal_source: Option<&'a [Vector3]>,
    output: VertexChannels,
}
impl<'a> ChannelBuilder<'a> {
    pub(super) fn pop(&mut self) {
        self.output.pop();
    }
    pub(super) fn last_key(&self) -> [u64; 5] {
        let mut key = [0; 5];
        if let Some(uv) = &self.output.uv {
            key[..2].copy_from_slice(&uv.last().unwrap().map(canonical_coordinate_bits));
        }
        if let Some(normals) = &self.output.normals {
            key[2..].copy_from_slice(
                &normals
                    .last()
                    .unwrap()
                    .to_array()
                    .map(canonical_coordinate_bits),
            );
        }
        key
    }
    pub fn new(source: &'a VertexChannels) -> Self {
        Self::with_normals(source, source.normals.as_deref())
    }
    pub fn with_normals(source: &'a VertexChannels, normals: Option<&'a [Vector3]>) -> Self {
        Self {
            source,
            normal_source: normals,
            output: VertexChannels {
                uv: source.uv.as_ref().map(|_| Vec::new()),
                normals: normals.map(|_| Vec::new()),
            },
        }
    }
    pub fn push(&mut self, index: usize) {
        if let Some(uv) = &mut self.output.uv {
            uv.push(self.source.uv.as_ref().unwrap()[index]);
        }
        if let Some(normals) = &mut self.output.normals {
            normals.push(self.normal_source.unwrap()[index]);
        }
    }
    pub fn blend(&mut self, sources: &[(usize, Real)]) -> Result<(), GeometryError> {
        if let Some(uv) = &mut self.output.uv {
            let source = self.source.uv.as_ref().unwrap();
            let mut sum = [0.; 2];
            for &(index, weight) in sources {
                for (i, s) in sum.iter_mut().enumerate() {
                    *s += source[index][i] * weight;
                }
            }
            uv.push([
                stored(sum[0], "interpolated texture coordinate")?,
                stored(sum[1], "interpolated texture coordinate")?,
            ]);
        }
        if let Some(normals) = &mut self.output.normals {
            let source = self.normal_source.unwrap();
            let mut sum = [0.; 3];
            for &(index, weight) in sources {
                for (i, s) in sum.iter_mut().enumerate() {
                    *s += source[index].to_array()[i] * weight;
                }
            }
            let n = Vector3::try_from(sum)?;
            let n = n
                .normalized_nonzero()
                .map(UnitVector3::as_vector)
                .unwrap_or(n);
            normals.push(Vector3::try_from(n.to_array().map(|x| x as f32 as Real))?);
        }
        Ok(())
    }
    pub fn finish(self) -> VertexChannels {
        self.output
    }
}
