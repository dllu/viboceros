//! Exact restoration of picked trim runs without fitting or extending surfaces.
use super::*;

/// Restored geometry and source boundaries/walls supplying retained trim objects.
/// Partial exterior edits retain the entire original loop when trims are kept.
#[derive(Clone, Debug, PartialEq)]
pub struct BrepBoundaryRestoration {
    brep: Brep,
    removed_faces: Vec<usize>,
    removed_boundaries: Vec<(usize, usize)>,
}
impl BrepBoundaryRestoration {
    pub fn brep(&self) -> &Brep {
        &self.brep
    }
    pub fn into_brep(self) -> Brep {
        self.brep
    }
    pub fn removed_faces(&self) -> &[usize] {
        &self.removed_faces
    }
    pub fn removed_boundaries(&self) -> &[(usize, usize)] {
        &self.removed_boundaries
    }
}

impl Brep {
    /// Restores a hole or a connected exterior trim run. Natural exterior
    /// picks and `all_similar` restore the complete outer boundary. An exterior
    /// run also restores the complete boundary when no other trimmed run remains.
    /// Holes preserve exact geometry and traverse joined walls.
    ///
    /// Rectangular partial edits with four initial side edges and forward spatial
    /// proxies reproduce native edge allocation. Other partial loops retain
    /// source order and append natural fragments. Complete restoration reuses
    /// the first removed spatial edge slot, then appends natural edges. All
    /// exterior picks on multi-face B-reps are ignored. Partial runs across
    /// seams/singularities return an error. Sources are unchanged on failure;
    /// complete results are validated before return.
    pub fn try_untrim_boundary(
        &self,
        face: usize,
        boundary: usize,
        trim: usize,
        all_similar: bool,
        tolerance: Tolerance,
    ) -> Result<Option<BrepBoundaryRestoration>, GeometryError> {
        let selected = self.untrim_boundary_trims(face, boundary, trim, all_similar)?;
        let source_face = &self.faces[face];
        if source_face.loops[boundary].loop_type == BrepLoopType::Inner {
            let holes = selected
                .iter()
                .map(|&(boundary, _)| (face, boundary))
                .collect::<BTreeSet<_>>();
            return Ok(self
                .try_remove_holes_with_topology(&holes.into_iter().collect::<Vec<_>>(), tolerance)?
                .map(|mut result| {
                    // The edge command renews affected surviving faces in
                    // descending source order. Hole removal alone keeps source order.
                    let renewed = result
                        .removed_openings
                        .iter()
                        .map(|&(face, _)| face)
                        .collect::<BTreeSet<_>>();
                    let survivors = (0..self.faces.len())
                        .filter(|face| !result.removed_faces.contains(face))
                        .collect::<Vec<_>>();
                    let mut faces = survivors
                        .iter()
                        .enumerate()
                        .filter(|(_, face)| !renewed.contains(face))
                        .map(|(index, _)| result.brep.faces[index].clone())
                        .collect::<Vec<_>>();
                    faces.extend(
                        survivors
                            .iter()
                            .enumerate()
                            .rev()
                            .filter(|(_, face)| renewed.contains(face))
                            .map(|(index, _)| result.brep.faces[index].clone()),
                    );
                    result.brep.faces = faces;
                    BrepBoundaryRestoration {
                        brep: result.brep,
                        removed_faces: result.removed_faces,
                        removed_boundaries: result.removed_openings,
                    }
                }));
        }
        // Rhino ignores exterior picks on multi-face B-reps, including
        // disconnected components. Inner picks above remain eligible.
        if self.faces.len() != 1 {
            return Ok(None);
        }
        let outer = &source_face.loops[0];
        let mut marked = vec![false; outer.trims.len()];
        for (_, index) in selected {
            marked[index] = true;
        }
        let remaining_trimmed = outer.trims.iter().zip(&marked).any(|(trim, &removed)| {
            !removed && !natural_iso(trim_iso::classify(&trim.curve, &source_face.surface))
        });
        let brep = if !remaining_trimmed {
            self.restore_complete_outer(tolerance)?
        } else {
            self.restore_outer_run(&marked, tolerance)?
        };
        Ok(Some(BrepBoundaryRestoration {
            brep,
            removed_faces: vec![],
            removed_boundaries: vec![(face, boundary)],
        }))
    }

    fn restore_complete_outer(&self, tolerance: Tolerance) -> Result<Self, GeometryError> {
        let face = &self.faces[0];
        let mut natural = natural_untrimmed_face(face.surface.clone(), face.reversed, tolerance)?;
        let mut work = self.clone();
        let vertex_offset = work.vertices.len();
        work.vertices.append(&mut natural.vertices);
        let removed = face.loops[0]
            .trims
            .iter()
            .filter_map(|trim| trim.edge)
            .collect::<BTreeSet<_>>();
        let retained = face.loops[1..]
            .iter()
            .flat_map(|ring| &ring.trims)
            .filter_map(|trim| trim.edge)
            .collect::<BTreeSet<_>>();
        let reuse = removed.difference(&retained).next().copied();
        let mut edge_map = Vec::new();
        for (index, mut edge) in natural.edges.into_iter().enumerate() {
            edge.vertices = edge.vertices.map(|v| v + vertex_offset);
            if index == 0
                && let Some(slot) = reuse
            {
                work.edges[slot] = edge;
                edge_map.push(slot);
            } else {
                edge_map.push(work.edges.len());
                work.edges.push(edge);
            }
        }
        for trim in &mut natural.faces[0].loops[0].trims {
            trim.vertices = trim.vertices.map(|v| v + vertex_offset);
            trim.edge = trim.edge.map(|e| edge_map[e]);
        }
        work.faces[0].loops[0] = natural.faces.remove(0).loops.remove(0);
        compact_retained_faces(&work, work.faces.clone(), tolerance)
    }

    fn restore_outer_run(
        &self,
        marked: &[bool],
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let face = &self.faces[0];
        let outer = &face.loops[0];
        let n = marked.len();
        let starts = (0..n)
            .filter(|&i| marked[i] && !marked[(i + n - 1) % n])
            .collect::<Vec<_>>();
        let [first] = starts.as_slice() else {
            return Err(GeometryError::InvalidBrepTopology {
                context: "partial Untrim needs one connected exterior run",
            });
        };
        let first = *first;
        let mut end = (first + 1) % n;
        while marked[end] {
            end = (end + 1) % n;
        }
        let last = (end + n - 1) % n;
        let surface = &face.surface;
        let natural = natural_untrimmed_face(surface.clone(), face.reversed, tolerance)?;
        if natural.vertices.len() != 4
            || natural.edges.len() != 4
            || natural.faces[0].loops[0]
                .trims
                .iter()
                .any(|trim| trim.trim_type != BrepTrimType::Boundary)
        {
            return Err(GeometryError::InvalidBrepTopology {
                context: "partial Untrim across seams or singularities is unsupported",
            });
        }
        let u = surface.domain_u();
        let v = surface.domain_v();
        let corners = [
            Point2::try_new(*u.start(), *v.start())?,
            Point2::try_new(*u.end(), *v.start())?,
            Point2::try_new(*u.end(), *v.end())?,
            Point2::try_new(*u.start(), *v.end())?,
        ];
        let start = outer.trims[first].curve.start_point()?;
        let finish = outer.trims[last].curve.end_point()?;
        let (side, position) = perimeter(start, &corners)?;
        let (end_side, end_position) = perimeter(finish, &corners)?;
        let mut path = vec![(side, start)];
        let mut current_side = side;
        let mut current_position = position;
        loop {
            if current_side == end_side && end_position > current_position {
                path.push((current_side, finish));
                break;
            }
            let corner = (current_side + 1) % 4;
            path.push((current_side, corners[corner]));
            current_side = corner;
            current_position = perimeter_coordinate(current_side, corners[corner]);
            if corners[corner] == finish {
                break;
            }
            if path.len() > 5 {
                return Err(GeometryError::InvalidBrepTopology {
                    context: "partial Untrim natural path does not close",
                });
            }
        }
        let mut work = self.clone();
        let mut ids = BTreeMap::new();
        ids.insert(0, outer.trims[first].vertices[0]);
        ids.insert(path.len() - 1, outer.trims[last].vertices[1]);
        for (corner, &uv) in corners.iter().enumerate() {
            for (i, &(_, point)) in path.iter().enumerate().skip(1).take(path.len() - 2) {
                if point == uv {
                    ids.insert(i, work.vertices.len());
                    work.vertices.push(natural.vertices[corner]);
                }
            }
        }
        let mut replacements = Vec::new();
        for i in 0..path.len() - 1 {
            let a = path[i].1;
            let b = path[i + 1].1;
            let side = path[i + 1].0;
            let axis = side % 2;
            let values = if axis == 0 {
                [a.x(), b.x()]
            } else {
                [a.y(), b.y()]
            };
            let interval = values[0].min(values[1])..=values[0].max(values[1]);
            let curve = if axis == 0 {
                surface.isocurve_u(a.y())?
            } else {
                surface.isocurve_v(a.x())?
            }
            .try_trimmed(interval)?;
            let curve = if side >= 2 { curve.reversed()? } else { curve };
            let vertices = [ids[&i], ids[&(i + 1)]];
            let domain = if axis == 0 { &u } else { &v };
            let trim_domain = if side < 2 {
                values
            } else {
                [
                    domain.start() + (domain.end() - values[0]),
                    domain.start() + (domain.end() - values[1]),
                ]
            };
            let trim_curve = NurbsCurve2::try_new(
                1,
                vec![a, b],
                vec![
                    trim_domain[0],
                    trim_domain[0],
                    trim_domain[1],
                    trim_domain[1],
                ],
            )?;
            // Spatial references are installed after allocation.
            let trim = BrepTrim {
                vertices,
                edge: None,
                reversed_3d: false,
                curve: trim_curve,
                trim_type: BrepTrimType::Boundary,
                iso: [
                    SurfaceIso::South,
                    SurfaceIso::East,
                    SurfaceIso::North,
                    SurfaceIso::West,
                ][side],
                tolerance: [0., 0.],
            };
            replacements.push((side, BrepEdge::try_new(vertices, curve, 0.)?, trim));
        }
        let mut ordering = (0..replacements.len()).collect::<Vec<_>>();
        ordering.sort_by_key(|&i| replacements[i].0);
        for i in ordering {
            replacements[i].2.edge = Some(work.edges.len());
            work.edges.push(replacements[i].1.clone());
        }
        let mut new_edges = [None; 4];
        for (side, _, trim) in &replacements {
            new_edges[*side] = trim.edge;
        }
        let mut trims = replacements
            .into_iter()
            .map(|(_, _, trim)| trim)
            .collect::<Vec<_>>();
        let mut index = end;
        while index != first {
            trims.push(outer.trims[index].clone());
            index = (index + 1) % n;
        }
        work.faces[0].loops[0] = BrepLoop::try_new(BrepLoopType::Outer, trims)?;
        super::ordering::apply(self, marked, new_edges, &mut work);
        compact_retained_faces(&work, work.faces.clone(), tolerance)
    }
}

fn natural_iso(iso: SurfaceIso) -> bool {
    matches!(
        iso,
        SurfaceIso::South | SurfaceIso::East | SurfaceIso::North | SurfaceIso::West
    )
}
fn perimeter(point: Point2, corners: &[Point2; 4]) -> Result<(usize, Real), GeometryError> {
    for (i, &corner) in corners.iter().enumerate() {
        if point == corner {
            return Ok((i, perimeter_coordinate(i, point)));
        }
    }
    let u = [corners[0].x(), corners[1].x()];
    let v = [corners[0].y(), corners[2].y()];
    let value = if point.y() == v[0] && point.x() > u[0] && point.x() < u[1] {
        (0, point.x())
    } else if point.x() == u[1] && point.y() > v[0] && point.y() < v[1] {
        (1, point.y())
    } else if point.y() == v[1] && point.x() > u[0] && point.x() < u[1] {
        (2, -point.x())
    } else if point.x() == u[0] && point.y() > v[0] && point.y() < v[1] {
        (3, -point.y())
    } else {
        return Err(GeometryError::InvalidBrepTopology {
            context: "Untrim run endpoint must lie on a natural UV boundary",
        });
    };
    Ok(value)
}

// Native coordinates preserve ordering between adjacent binary64 values; a
// normalized perimeter fraction could round distinct endpoints together.
fn perimeter_coordinate(side: usize, point: Point2) -> Real {
    match side {
        0 => point.x(),
        1 => point.y(),
        2 => -point.x(),
        3 => -point.y(),
        _ => unreachable!(),
    }
}
