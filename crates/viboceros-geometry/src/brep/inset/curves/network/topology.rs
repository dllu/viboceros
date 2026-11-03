//! Shared spatial edges and per-surface trim reconstruction for graph cells.
use super::*;

type EdgeBinding = (usize, bool, BrepTrimType);

pub(super) struct Topology<'a> {
    graph: &'a SurfaceCutGraph,
    vertices: Vec<Option<usize>>,
    edges: Vec<Option<EdgeBinding>>,
    pub produced: Vec<usize>,
}

impl<'a> Topology<'a> {
    pub fn build(
        brep: &mut Brep,
        graph: &'a SurfaceCutGraph,
        sources: &[Option<input::Source>],
        active: &[bool],
        tolerance: Tolerance,
        work: &mut Work,
    ) -> Result<Self, GeometryError> {
        let mut bindings = bindings::Bindings::build(brep, graph, sources, tolerance, work)?;
        for (_, edge) in graph.edges.iter().enumerate().filter(|(i, _)| active[*i]) {
            for node in edge.nodes {
                if bindings.vertices[node].is_none() {
                    bindings.vertices[node] = Some(brep.vertices.len());
                    brep.vertices
                        .push(BrepVertex::try_new(graph.nodes[node].point, 0.)?);
                }
            }
        }
        let mut edges = vec![None; graph.edges.len()];
        let mut produced = Vec::new();
        for (i, edge) in graph.edges.iter().enumerate().filter(|(i, _)| active[*i]) {
            let vertices = edge.nodes.map(|n| bindings.vertices[n].unwrap());
            if let Some((c, source)) = surface_cut_arrangement_edge_contributors(edge)
                .find_map(|c| sources[c.source].map(|s| (c, s)))
            {
                let old = bindings.fragment(c.source, c.segment, source.reversed)?;
                edges[i] = Some((old, source.reversed ^ !c.same_direction, source.trim_type));
            } else {
                if matches!(edge.kind, SurfaceCutArrangementEdgeKind::Boundary(_)) {
                    return Err(invalid("inset material reaches the auxiliary rectangle"));
                }
                let id = brep.edges.len();
                work.charge_curve(&edge.spatial)?;
                brep.edges
                    .push(BrepEdge::try_new(vertices, edge.spatial.clone(), 0.)?);
                edges[i] = Some((id, false, BrepTrimType::Mated));
                produced.push(id);
            }
        }
        Ok(Self {
            graph,
            vertices: bindings.vertices,
            edges,
            produced,
        })
    }

    pub fn cycle(
        &self,
        halves: &[usize],
        surface: &NurbsSurface,
        flip: bool,
        mut pullback: Option<&mut Pullback<'_>>,
    ) -> Result<Vec<BrepTrim>, GeometryError> {
        let halves = if flip {
            halves.iter().rev().map(|h| h ^ 1).collect::<Vec<_>>()
        } else {
            halves.to_vec()
        };
        halves
            .into_iter()
            .map(|h| {
                let edge = &self.graph.edges[h / 2];
                let (id, reversed, kind) = self.edges[h / 2].unwrap();
                let backward = !h.is_multiple_of(2);
                let mut vertices = edge.nodes.map(|n| self.vertices[n].unwrap());
                let curve = if let Some(cache) = &mut pullback {
                    cache.curve(h / 2, &edge.spatial)?
                } else {
                    edge.parameter.clone()
                };
                let curve = if backward {
                    vertices.swap(0, 1);
                    curve.reversed()?
                } else {
                    curve
                };
                let iso = trim_iso::classify(&curve, surface);
                BrepTrim::try_new(
                    vertices,
                    Some(id),
                    reversed ^ backward,
                    curve,
                    kind,
                    iso,
                    [0.; 2],
                )
            })
            .collect()
    }

    pub fn face(
        &self,
        region: usize,
        source: &BrepFace,
        flip: bool,
        mut pullback: Option<&mut Pullback<'_>>,
    ) -> Result<BrepFace, GeometryError> {
        let region = &self.graph.regions[region];
        let mut loops = vec![BrepLoop::try_new(
            BrepLoopType::Outer,
            self.cycle(
                &region.outer,
                &source.surface,
                flip,
                pullback.as_deref_mut(),
            )?,
        )?];
        for hole in &region.holes {
            loops.push(BrepLoop::try_new(
                BrepLoopType::Inner,
                self.cycle(hole, &source.surface, flip, pullback.as_deref_mut())?,
            )?);
        }
        BrepFace::try_new(source.surface.clone(), source.reversed, loops)
    }
}

pub(super) struct Pullback<'a> {
    source: &'a NurbsSurface,
    affine: Option<NurbsSurface>,
    curves: Vec<Option<NurbsCurve2>>,
    tolerance: Tolerance,
}
impl<'a> Pullback<'a> {
    pub fn new(
        source: &'a NurbsSurface,
        count: usize,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        // Certify the large control net once, then use its four-corner map for
        // every affine trim. Non-affine charts retain the certified pullback.
        let affine = source
            .try_affine_patch_corners(tolerance)
            .ok()
            .map(|corners| {
                NurbsSurface::try_bilinear(corners)?
                    .try_reparameterized(source.domain_u(), source.domain_v())
            })
            .transpose()?;
        Ok(Self {
            source,
            affine,
            curves: vec![None; count],
            tolerance,
        })
    }
    fn curve(&mut self, index: usize, spatial: &NurbsCurve) -> Result<NurbsCurve2, GeometryError> {
        if self.curves[index].is_none() {
            let uv = if let Some(affine) = &self.affine {
                affine.try_pullback_affine_curve(spatial, self.tolerance)?
            } else {
                let a = spatial.evaluate(*spatial.domain().start())?;
                let b = spatial.evaluate(*spatial.domain().end())?;
                let (u, v) = self.source.closest_parameters(a, self.tolerance)?;
                let start = Point2::try_new(u, v)?;
                let (u, v) = self.source.closest_parameters(b, self.tolerance)?;
                let end = Point2::try_new(u, v)?;
                surface_split_parameter_curve(self.source, spatial, start, end, self.tolerance)?
            };
            self.curves[index] = Some(uv);
        }
        Ok(self.curves[index].as_ref().unwrap().clone())
    }
}
