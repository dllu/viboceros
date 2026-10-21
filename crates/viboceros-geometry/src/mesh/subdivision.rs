//! Linear face refinement with conforming neighbors and retained raw seams.
use super::*;
mod neighbors;
mod ngons;

impl TriangleMesh {
    /// Split every stored triangle/quad into corner quads. Original positions
    /// stay fixed. Triangle face centers retain the native float reciprocal;
    /// colors interpolate RGB and transparency. Source n-gon overlays are replaced by the refined faces.
    pub fn subdivided(&self) -> Result<Self, GeometryError> {
        self.subdivide_faces(None)
    }
    /// Split selected stored faces and refine adjacent polygons along the
    /// shared midpoints. Distinct raw seam vertices remain distinct.
    pub fn subdivided_faces(&self, faces: &[usize]) -> Result<Self, GeometryError> {
        self.subdivide_faces(Some(faces))
    }
    fn subdivide_faces(&self, selection: Option<&[usize]>) -> Result<Self, GeometryError> {
        if self.vertices.len() > 4_000_000 || self.faces.len() > 1_000_000 {
            return Err(GeometryError::MeshSubdivisionLimit);
        }
        let mut selected = vec![selection.is_none(); self.faces.len()];
        if let Some(indices) = selection {
            if indices.is_empty() {
                return Err(GeometryError::EmptyMesh);
            }
            for &face in indices {
                let total = selected.len();
                *selected
                    .get_mut(face)
                    .ok_or(GeometryError::MeshFaceIndexOutOfRange {
                        face,
                        face_count: total,
                    })? = true;
            }
        }
        let topology = self.topology_data();
        let pair = |a: u32, b: u32| if a < b { (a, b) } else { (b, a) };
        let logical_pair = |a: u32, b: u32| {
            let a = topology.topological_vertices[a as usize] as u32;
            let b = topology.topological_vertices[b as usize] as u32;
            pair(a, b)
        };
        let mut touched = BTreeSet::new();
        for (face, &split) in self.faces.iter().zip(&selected) {
            if split {
                let f = face.indices();
                for i in 0..f.len() {
                    touched.insert(logical_pair(f[i], f[(i + 1) % f.len()]));
                }
            }
        }
        let mut midpoints = BTreeMap::new();
        for face in &self.faces {
            let f = face.indices();
            for i in 0..f.len() {
                let (a, b) = (f[i], f[(i + 1) % f.len()]);
                if touched.contains(&logical_pair(a, b)) {
                    midpoints.entry(pair(a, b)).or_insert(0_u32);
                }
            }
        }
        drop(topology);
        drop(touched);
        let mut complete = Vec::with_capacity(self.faces.len());
        let mut center_count = 0;
        let mut faces_count = 0;
        for (face, &selected) in self.faces.iter().zip(&selected) {
            let f = face.indices();
            let m = std::array::from_fn::<_, 4, _>(|i| {
                if i < f.len() {
                    midpoints.get(&pair(f[i], f[(i + 1) % f.len()])).copied()
                } else {
                    None
                }
            });
            let center = selected || m[..f.len()].iter().all(Option::is_some);
            center_count += usize::from(center);
            complete.push(center);
            faces_count += neighbors::count(f.len(), &m, center);
        }
        let vertices_count = self.vertices.len() + midpoints.len() + center_count;
        if vertices_count > 4_000_000 || faces_count > 1_000_000 {
            return Err(GeometryError::MeshSubdivisionLimit);
        }
        let mut vertices = self.vertices.clone();
        vertices.reserve(vertices_count - vertices.len());
        let mut colors = self.vertex_colors.clone();
        for (&(a, b), index) in &mut midpoints {
            *index = vertices.len() as u32;
            let a = self.vertices[a as usize].to_array();
            let b = self.vertices[b as usize].to_array();
            vertices.push(Point3::try_from(std::array::from_fn(|i| {
                (a[i] + b[i]) * 0.5
            }))?);
            if let Some(out) = &mut colors {
                out.push([0; 4]);
            }
        }
        // Color seams use raw endpoints, independently of coincident topology.
        if let Some(out) = &mut colors {
            let source = self.vertex_colors.as_ref().unwrap();
            for (&(a, b), &i) in &midpoints {
                out[i as usize] = average_color(source, &[a, b], 0.5);
            }
        }
        let mut centers = vec![None; self.faces.len()];
        for (i, face) in self.faces.iter().enumerate() {
            if complete[i] {
                let f = face.indices();
                let reciprocal = f64::from(1.0_f32 / f.len() as f32);
                let mut sum = [0.; 3];
                for &v in f {
                    let p = self.vertices[v as usize].to_array();
                    for c in 0..3 {
                        sum[c] += p[c];
                    }
                }
                centers[i] = Some(vertices.len() as u32);
                vertices.push(Point3::try_from(sum.map(|v| v * reciprocal))?);
                if let Some(out) = &mut colors {
                    let source = self.vertex_colors.as_ref().unwrap();
                    out.push(average_color(source, f, reciprocal));
                }
            }
        }
        let remainders = ngons::Remainders::new(self, &selected);
        let mut ranges =
            (!remainders.groups.is_empty()).then(|| Vec::with_capacity(self.faces.len()));
        let mut faces = Vec::with_capacity(faces_count);
        let mut groups = Vec::new();
        for (index, face) in self.faces.iter().enumerate() {
            let f = face.indices();
            let m = std::array::from_fn::<_, 4, _>(|i| {
                if i < f.len() {
                    midpoints.get(&pair(f[i], f[(i + 1) % f.len()])).copied()
                } else {
                    None
                }
            });
            let start = faces.len() as u32;
            faces.extend(neighbors::pieces(*face, &m, centers[index]));
            let range = start..faces.len() as u32;
            if let Some(ranges) = &mut ranges {
                ranges.push(range.clone());
            }
            if !selected[index]
                && m.iter().any(Option::is_some)
                && !remainders.covered.get(index).copied().unwrap_or(false)
            {
                groups.push(range.collect::<Vec<_>>());
            }
        }
        if let Some(ranges) = ranges {
            for component in remainders.groups {
                groups.push(
                    component
                        .into_iter()
                        .flat_map(|face| ranges[face].clone())
                        .collect(),
                );
            }
        }

        debug_assert_eq!(faces.len(), faces_count);
        let output =
            Self::try_from_face_records(vertices, faces)?.try_with_vertex_colors(colors)?;
        let ngons = groups
            .into_iter()
            .enumerate()
            .map(|(index, faces)| {
                output
                    .ngon_from_faces(faces)
                    .ok_or(GeometryError::InvalidMeshNgon { ngon: index })
            })
            .collect::<Result<Vec<_>, _>>()?;
        output.try_with_ngons(ngons)
    }
}

// ON_Color rounds RGB and stored transparency, rather than alpha. This makes
// half-integer alpha averages round down while RGB averages round up.
fn average_color(colors: &[[u8; 4]], vertices: &[u32], reciprocal: f64) -> [u8; 4] {
    std::array::from_fn(|channel| {
        let sum = vertices
            .iter()
            .map(|&i| {
                let value = colors[i as usize][channel];
                u32::from(if channel == 3 { 255 - value } else { value })
            })
            .sum::<u32>();
        let value = (f64::from(sum) * reciprocal + 0.5).floor().clamp(0., 255.) as u8;
        if channel == 3 { 255 - value } else { value }
    })
}
