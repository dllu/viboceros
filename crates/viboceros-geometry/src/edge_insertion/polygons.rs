//! Split original polygon regions without searching unrelated faces per chord.
use super::*;
pub(crate) struct Region {
    pub source: usize,
    pub vertices: Vec<u32>,
}
impl Plan {
    pub fn polygons(&self, faces: &[Vec<u32>]) -> Result<Vec<Region>, GeometryError> {
        let original_count = self.points.len() - self.splits.len();
        let lookup = self
            .edges
            .iter()
            .enumerate()
            .map(|(i, &e)| (key(e), i))
            .collect::<BTreeMap<_, _>>();
        let mut cuts = vec![Vec::<(Real, u32)>::new(); self.edges.len()];
        let mut owners = vec![Vec::new(); self.points.len()];
        for (source, face) in faces.iter().enumerate() {
            for &v in face {
                owners[v as usize].push(source);
            }
        }
        for (i, split) in self.splits.iter().enumerate() {
            let vertex = original_count + i;
            cuts[split.edge].push((split.parameter, vertex as u32));
            owners[vertex] = self.edge_faces[split.edge].clone();
        }
        for row in &mut cuts {
            row.sort_by(|a, b| a.0.total_cmp(&b.0));
        }
        let mut regions = Vec::with_capacity(faces.len() + self.chords.len());
        let mut children = Vec::with_capacity(faces.len());
        for (source, boundary) in faces.iter().enumerate() {
            let mut vertices = Vec::new();
            for (i, &a) in boundary.iter().enumerate() {
                vertices.push(a);
                let b = boundary[(i + 1) % boundary.len()];
                let edge = lookup[&key([a, b])];
                if self.edges[edge][0] == a {
                    vertices.extend(cuts[edge].iter().map(|c| c.1));
                } else {
                    vertices.extend(cuts[edge].iter().rev().map(|c| c.1));
                }
            }
            children.push(vec![regions.len()]);
            regions.push(Region { source, vertices });
        }
        for &[a, b] in &self.chords {
            let mut location = None;
            let mut adjacent = false;
            for &source in &owners[a as usize] {
                if !owners[b as usize].contains(&source) {
                    continue;
                }
                for &index in &children[source] {
                    let polygon = &regions[index].vertices;
                    let (Some(i), Some(j)) = (
                        polygon.iter().position(|&v| v == a),
                        polygon.iter().position(|&v| v == b),
                    ) else {
                        continue;
                    };
                    if (i + 1) % polygon.len() == j || (j + 1) % polygon.len() == i {
                        adjacent = true;
                        break;
                    }
                    location = Some((source, index, i, j));
                    break;
                }
                if location.is_some() || adjacent {
                    break;
                }
            }
            if adjacent {
                continue;
            }
            let (source, index, i, j) =
                location.ok_or_else(|| invalid("crossing inserted polygon chords"))?;
            let polygon = &regions[index].vertices;
            let between = |start, end| {
                (0..polygon.len())
                    .map(move |k| (start + k) % polygon.len())
                    .take_while(move |&k| k != end)
                    .map(|k| polygon[k])
            };
            let left = std::iter::once(a).chain(between(j, i)).collect();
            let right = std::iter::once(b).chain(between(i, j)).collect();
            regions[index].vertices = left;
            children[source].push(regions.len());
            regions.push(Region {
                source,
                vertices: right,
            });
        }
        Ok(regions)
    }
}
