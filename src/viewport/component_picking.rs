//! Surface/B-rep components for transient preselection and command picking.
use super::*;
use viboceros_command::ComponentSelectionKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ComponentPick {
    pub object: ObjectId,
    pub kind: ComponentSelectionKind,
    pub index: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComponentPickFilter {
    Any,
    Edges,
    Faces,
    MeshAndSubDFaces,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComponentClick {
    pub picks: Vec<ComponentPick>,
    pub preselection: bool,
    pub modifiers: egui::Modifiers,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComponentWindow {
    pub picks: Vec<ComponentPick>,
    pub preselection: bool,
    pub modifiers: egui::Modifiers,
    pub crossing: bool,
    pub inverted: bool,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ComponentDrag {
    pub start: Pos2,
    pub filter: ComponentPickFilter,
    pub preselection: bool,
    pub modifiers: egui::Modifiers,
}

impl From<EdgePick> for ComponentPick {
    fn from(pick: EdgePick) -> Self {
        Self {
            object: pick.object,
            kind: ComponentSelectionKind::BrepEdge,
            index: pick.edge,
        }
    }
}

impl Viewport {
    pub(super) fn pick_components(
        &self,
        pointer: Pos2,
        rect: Rect,
        document: &Document,
        filter: ComponentPickFilter,
    ) -> Vec<ComponentPick> {
        if !pointer.is_finite() || !rect.contains(pointer) {
            return Vec::new();
        }
        if matches!(
            filter,
            ComponentPickFilter::Any | ComponentPickFilter::Edges
        ) {
            let edges = self.pick_edges(pointer, rect, document);
            if !edges.is_empty() || filter == ComponentPickFilter::Edges {
                return edges.into_iter().map(ComponentPick::from).collect();
            }
        }
        let allowed = (filter == ComponentPickFilter::MeshAndSubDFaces).then(|| {
            document
                .selectable_objects()
                .filter(|o| matches!(o.geometry(), Geometry::Mesh(_) | Geometry::SubD(_)))
                .map(|o| o.id())
                .collect::<std::collections::BTreeSet<_>>()
        });
        self.pick_selected_face_with_point_restricted(
            pointer,
            rect,
            document,
            if filter == ComponentPickFilter::Faces {
                FacePickMode::SurfaceAndBrepAny
            } else {
                FacePickMode::Any
            },
            None,
            allowed.as_ref(),
        )
        .map_or_else(Vec::new, |(object, face, _)| {
            let source = document.object(object).unwrap();
            let face = if matches!(source.geometry(), Geometry::Mesh(_)) {
                self.display_cache
                    .borrow_mut()
                    .get(source, document.tolerance())
                    .component_face_representative(face)
                    .unwrap_or(face)
            } else {
                face
            };
            vec![ComponentPick {
                object,
                kind: match document.object(object).unwrap().geometry() {
                    Geometry::Mesh(_) => ComponentSelectionKind::MeshFace,
                    Geometry::SubD(_) => ComponentSelectionKind::SubDFace,
                    _ => ComponentSelectionKind::BrepFace,
                },
                index: face,
            }]
        })
    }

    pub(super) fn components_in_rectangle(
        &self,
        viewport: Rect,
        selection: Rect,
        document: &Document,
        filter: ComponentPickFilter,
        crossing: bool,
        inverted: bool,
    ) -> Vec<ComponentPick> {
        if !viewport.is_finite()
            || !selection.is_finite()
            || selection.width() < 2.
            || selection.height() < 2.
        {
            return Vec::new();
        }
        let mut result = Vec::new();
        for object in document.selectable_objects() {
            if matches!(object.geometry(), Geometry::Mesh(_) | Geometry::SubD(_)) {
                if matches!(
                    filter,
                    ComponentPickFilter::Any | ComponentPickFilter::MeshAndSubDFaces
                ) {
                    result.extend(self.polygon_components_in_rectangle(
                        viewport, selection, document, object, crossing, inverted,
                    ));
                }
                continue;
            }
            if filter == ComponentPickFilter::MeshAndSubDFaces {
                continue;
            }
            if !matches!(
                object.geometry(),
                Geometry::NurbsSurface(_) | Geometry::Brep(_)
            ) {
                continue;
            }
            let display = self
                .display_cache
                .borrow_mut()
                .get(object, document.tolerance());
            let mut test = |primitives: &ProjectedPrimitives, kind, index| {
                let selected = match (crossing, inverted) {
                    (false, false) => primitives.is_windowed_by(selection),
                    (true, false) => primitives.is_crossed_by(selection),
                    (false, true) => !primitives.is_crossed_by(selection),
                    (true, true) => !primitives.is_windowed_by(selection),
                };
                if selected {
                    result.push(ComponentPick {
                        object: object.id(),
                        kind,
                        index,
                    });
                }
            };
            if matches!(
                filter,
                ComponentPickFilter::Any | ComponentPickFilter::Edges
            ) {
                for (index, segments) in display.edges().iter().enumerate() {
                    let mut primitives = ProjectedPrimitives::default();
                    for &[a, b] in segments {
                        self.add_selection_segment(&mut primitives, a, b, viewport);
                    }
                    test(&primitives, ComponentSelectionKind::BrepEdge, index);
                }
            }
            if filter == ComponentPickFilter::Edges {
                continue;
            }
            let count = match object.geometry() {
                Geometry::Brep(brep) => brep.faces().len(),
                _ => 1,
            };
            let mut faces = (0..count)
                .map(|_| ProjectedPrimitives::default())
                .collect::<Vec<_>>();
            for (index, primitives) in faces.iter_mut().enumerate() {
                for edge in face_edges(object.geometry(), index) {
                    if let Some(segments) = display.edges().get(edge) {
                        for &[a, b] in segments {
                            self.add_selection_segment(primitives, a, b, viewport);
                        }
                    }
                }
            }
            if self.display_mode != DisplayMode::Wireframe
                && let Some(mesh) = display.mesh()
            {
                let sources = display.face_sources();
                let mut triangle = 0;
                for (mesh_face, face) in mesh.faces().iter().enumerate() {
                    let index = sources.and_then(|s| s.get(mesh_face)).copied().unwrap_or(0);
                    for _ in 0..if face.is_triangle() { 1 } else { 2 } {
                        if let Some(points) = mesh.triangle_points(triangle)
                            && let Some(primitives) = faces.get_mut(index)
                        {
                            // Raw points prevent a depth-clipped face from
                            // appearing completely enclosed by its visible part.
                            for point in points {
                                self.add_selection_point(primitives, point, viewport);
                            }
                            for clipped in
                                self.clip_selection_triangle(points).into_iter().flatten()
                            {
                                primitives.add_triangle(
                                    clipped.map(|point| self.project(point, viewport)),
                                );
                            }
                        }
                        triangle += 1;
                    }
                }
            }
            for (index, primitives) in faces.iter().enumerate() {
                test(primitives, ComponentSelectionKind::BrepFace, index);
            }
        }
        result
    }

    pub(super) fn paint_component_highlights(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        document: &Document,
        picks: &[ComponentPick],
    ) {
        let mut edges = Vec::new();
        let mut polygon_regions = std::collections::BTreeSet::new();
        for pick in picks {
            let Some(object) = document
                .object(pick.object)
                .filter(|_| document.is_object_selectable(pick.object))
            else {
                continue;
            };
            match pick.kind {
                ComponentSelectionKind::BrepEdge => edges.push(EdgePick {
                    object: pick.object,
                    edge: pick.index,
                }),
                ComponentSelectionKind::SubDFace | ComponentSelectionKind::MeshFace => {
                    if !matches!(
                        (pick.kind, object.geometry()),
                        (ComponentSelectionKind::SubDFace, Geometry::SubD(_))
                            | (ComponentSelectionKind::MeshFace, Geometry::Mesh(_))
                    ) {
                        continue;
                    }
                    let display = self
                        .display_cache
                        .borrow_mut()
                        .get(object, document.tolerance());
                    let Some(region) = display.component_face_region(pick.index) else {
                        continue;
                    };
                    if !polygon_regions.insert((pick.object, region)) {
                        continue;
                    }
                    for &[a, b] in display.component_face_outline(pick.index) {
                        if let Some([a, b]) = self.project_selection_segment(a, b, rect) {
                            painter.line_segment(
                                [a, b],
                                Stroke::new(3., Color32::from_rgb(245, 160, 20)),
                            );
                        }
                    }
                }
                ComponentSelectionKind::BrepFace => {
                    edges.extend(face_edges(object.geometry(), pick.index).into_iter().map(
                        |edge| EdgePick {
                            object: pick.object,
                            edge,
                        },
                    ))
                }
            }
        }
        edges.sort_by_key(|edge| (edge.object, edge.edge));
        edges.dedup();
        self.paint_edge_highlights(painter, rect, document, &edges);
    }
}

/// Boundary segments of one logical mesh face, including every hole loop.
pub(crate) fn mesh_face_outline(
    mesh: &viboceros_geometry::TriangleMesh,
    index: usize,
) -> Vec<[Point3; 2]> {
    let Ok(index) = u32::try_from(index) else {
        return vec![];
    };
    let single = [index];
    let faces = mesh
        .ngons()
        .iter()
        .find(|g| g.faces().contains(&index))
        .map_or(single.as_slice(), |g| g.faces());
    let mut boundary = std::collections::BTreeMap::<(u32, u32), usize>::new();
    for &face in faces {
        if let Some(face) = mesh.faces().get(face as usize) {
            let v = face.indices();
            for i in 0..v.len() {
                let a = v[i];
                let b = v[(i + 1) % v.len()];
                *boundary.entry((a.min(b), a.max(b))).or_default() += 1;
            }
        }
    }
    boundary
        .into_iter()
        .filter(|(_, n)| *n == 1)
        .map(|((a, b), _)| [mesh.vertices()[a as usize], mesh.vertices()[b as usize]])
        .collect()
}

pub(super) fn face_edges(geometry: &Geometry, face: usize) -> Vec<usize> {
    match geometry {
        Geometry::NurbsSurface(_) if face == 0 => (0..4).collect(),
        Geometry::Brep(brep) => brep.faces().get(face).map_or_else(Vec::new, |face| {
            face.loops()
                .iter()
                .flat_map(|boundary| boundary.trims())
                .filter_map(|trim| trim.edge())
                .collect()
        }),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests;
