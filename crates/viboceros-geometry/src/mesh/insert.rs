//! InsertPoint retains polygon regions and interpolates each raw seam independently.
use super::*;
mod ngons;
mod pairing;
mod regions;
mod source;
use crate::{
    PolygonEdgePoint,
    edge_insertion::{Plan, invalid},
};

impl TriangleMesh {
    /// Insert points on original topology edges, connecting consecutive picks
    /// across faces. Source raw seams and vertex channels survive; n-gons are
    /// rebuilt from the resulting polygon regions after a topology change.
    pub fn try_insert_edge_path(
        &self,
        picks: &[PolygonEdgePoint],
        view_normal: Option<UnitVector3>,
        _tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        // Existing mesh facets may contain features below the document's
        // modelling tolerance. Insertion does not collapse those features.
        let tolerance = Tolerance::MESH_VALIDATION;
        source::validate(self, tolerance)?;
        let topology = self.topology_data();
        let faces = self
            .faces
            .iter()
            .map(|f| {
                f.indices()
                    .iter()
                    .map(|&i| topology.topological_vertices[i as usize] as u32)
                    .collect()
            })
            .collect::<Vec<Vec<u32>>>();
        let edges = topology
            .edges
            .keys()
            .map(|&(a, b)| [a as u32, b as u32])
            .collect::<Vec<_>>();
        let plan = Plan::new(
            &topology.topological_points,
            &faces,
            &edges,
            picks,
            false,
            view_normal,
            tolerance,
        )?;
        if plan.splits.is_empty() && plan.chords.is_empty() {
            return Ok(self.clone());
        }
        let mut vertices = self.vertices.clone();
        let mut colors = self.vertex_colors.clone();
        let mut channels = ChannelBuilder::new(&self.vertex_channels);
        for i in 0..self.vertices.len() {
            channels.push(i);
        }
        let incidences = topology.edges.values().collect::<Vec<_>>();
        let mut raw_points = BTreeMap::new();
        for (split, pick) in plan.splits.iter().enumerate() {
            let mut shared = BTreeMap::new();
            for usage in incidences[pick.edge].uses() {
                let raw = if let Some(&raw) = shared.get(&usage.raw_vertices) {
                    raw
                } else {
                    let raw = u32::try_from(vertices.len())
                        .map_err(|_| GeometryError::TooManyMeshVertices)?;
                    vertices.push(plan.points[topology.topological_vertex_count + split]);
                    let [a, b] = usage.raw_vertices;
                    channels.blend(&[
                        (a as usize, 1. - pick.parameter),
                        (b as usize, pick.parameter),
                    ])?;
                    if let Some(out) = &mut colors {
                        out.push(interpolated_vertex_color(
                            self.vertex_colors.as_ref().unwrap(),
                            [a, b],
                            pick.parameter,
                        ));
                    }
                    shared.insert(usage.raw_vertices, raw);
                    raw
                };
                raw_points.insert(
                    (
                        usage.face,
                        (topology.topological_vertex_count + split) as u32,
                    ),
                    raw,
                );
            }
        }
        for (face, polygon) in self.faces.iter().enumerate() {
            for &raw in polygon.indices() {
                raw_points.insert(
                    (face, topology.topological_vertices[raw as usize] as u32),
                    raw,
                );
            }
        }
        let mut records = Vec::new();
        let mut face_sources = Vec::new();
        let mut polygon_groups = Vec::new();
        let edge_lookup = edges
            .iter()
            .enumerate()
            .map(|(i, &e)| (crate::edge_insertion::key(e), i))
            .collect::<BTreeMap<_, _>>();
        let mut edge_splits = vec![Vec::new(); edges.len()];
        for (i, pick) in plan.splits.iter().enumerate() {
            edge_splits[pick.edge].push(i);
        }
        for splits in &mut edge_splits {
            splits.sort_by(|&a, &b| {
                plan.splits[a]
                    .parameter
                    .total_cmp(&plan.splits[b].parameter)
            });
        }
        for (region_index, region) in plan.polygons(&faces)?.into_iter().enumerate() {
            let source = region.source;
            let polygon = region.vertices;
            let source_boundary = self.faces[source].indices();
            let first_record = records.len();
            let raw = polygon
                .iter()
                .map(|&i| {
                    raw_points
                        .get(&(source, i))
                        .copied()
                        .ok_or_else(|| invalid("missing inserted raw mesh vertex"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            // Insertion elsewhere must not remesh an untouched concave or
            // warped quad, or change its stored diagonal and n-gon membership.
            if raw == source_boundary {
                records.push(self.faces[source]);
                face_sources.push(source);
                polygon_groups.push(None);
                continue;
            }
            let mut anchors = Vec::new();
            for i in 0..source_boundary.len() {
                let previous = (i + source_boundary.len() - 1) % source_boundary.len();
                let a = faces[source][previous];
                let edge = edge_lookup[&crate::edge_insertion::key([a, faces[source][i]])];
                let splits = &edge_splits[edge];
                for j in 0..splits.len() {
                    let index = if edges[edge][0] == a {
                        j
                    } else {
                        splits.len() - 1 - j
                    };
                    anchors.push(
                        raw_points[&(
                            source,
                            (topology.topological_vertex_count + splits[index]) as u32,
                        )],
                    );
                }
                anchors.push(source_boundary[i]);
            }
            let points = raw
                .iter()
                .map(|&i| vertices[i as usize])
                .collect::<Vec<_>>();
            records.extend(
                regions::tile(points, tolerance)?
                    .into_iter()
                    .map(|f| match f {
                        MeshFace::Triangle(v) => MeshFace::Triangle(v.map(|i| raw[i as usize])),
                        MeshFace::Quad(v) => MeshFace::Quad(v.map(|i| raw[i as usize])),
                    }),
            );
            face_sources.extend(std::iter::repeat_n(source, records.len() - first_record));
            // A split triangle may have four boundary vertices with one
            // collinear point. Its two triangles remain separate cells;
            // automatic n-gons describe regions with more than four corners.
            polygon_groups.extend(std::iter::repeat_n(
                (raw.len() > 4).then_some(region_index),
                records.len() - first_record,
            ));
            // Include every split in source-boundary order, starting on the
            // closing side. Omitting splits on other sides changes a quad's
            // stored 0-2 diagonal when its earliest original corner is removed.
            for face in &mut records[first_record..] {
                if let MeshFace::Quad(v) = face
                    && let Some(start) = anchors.iter().find_map(|a| v.iter().position(|x| x == a))
                {
                    v.rotate_left(start);
                }
            }
        }
        // Retain unchanged source records, including a valid polygon whose
        // fixed diagonal contains a zero-area triangle. New records remain
        // subject to the ordinary strict geometric admission policy.
        for (index, (&face, &source)) in records.iter().zip(&face_sources).enumerate() {
            if face != self.faces[source] {
                face_records::validate_face(&vertices, face, index, tolerance)?;
            }
        }
        let mut result =
            Self::try_from_face_records(vertices, records)?.try_with_vertex_colors(colors)?;
        result.vertex_channels = channels.finish();
        ngons::rebuild(self, result, &plan, &face_sources, &polygon_groups)
    }
}
