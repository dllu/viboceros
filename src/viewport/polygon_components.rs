//! Shared polygon face regions: one cached boundary per control face or n-gon.
use super::display_cache::DisplayGeometry;
use super::*;
use std::collections::BTreeMap;
use viboceros_command::ComponentSelectionKind;

#[derive(Default)]
pub(super) struct FaceRegions {
    outlines: Vec<Vec<[Point3; 2]>>,
    display_sources: Vec<usize>,
    pick_sources: Vec<usize>,
    representatives: Vec<usize>,
}
impl DisplayGeometry {
    fn polygon_face_regions(&self) -> &FaceRegions {
        self.component_regions.get_or_init(|| {
            let Some(mesh) = self.mesh() else {
                return FaceRegions::default();
            };
            let (display_sources, pick_sources, count) = match &*self.geometry {
                Geometry::Mesh(mesh) => {
                    let mut owners = (0..mesh.face_count()).collect::<Vec<_>>();
                    for (index, ngon) in mesh.ngons().iter().enumerate() {
                        for &face in ngon.faces() {
                            owners[face as usize] = mesh.face_count() + index;
                        }
                    }
                    (
                        owners.clone(),
                        owners,
                        mesh.face_count() + mesh.ngons().len(),
                    )
                }
                Geometry::SubD(subd) => {
                    let Some(sources) = self.face_sources() else {
                        return FaceRegions::default();
                    };
                    (
                        sources.to_vec(),
                        (0..subd.faces().len()).collect(),
                        subd.faces().len(),
                    )
                }
                _ => return FaceRegions::default(),
            };
            if display_sources.len() != mesh.face_count() {
                return FaceRegions::default();
            }
            let mut edges = BTreeMap::new();
            for (face, &source) in mesh.faces().iter().zip(&display_sources) {
                let v = face.indices();
                for i in 0..v.len() {
                    let a = v[i];
                    let b = v[(i + 1) % v.len()];
                    *edges.entry((source, a.min(b), a.max(b))).or_insert(0_usize) += 1;
                }
            }
            let mut outlines = vec![Vec::new(); count];
            for ((source, a, b), uses) in edges {
                if uses == 1
                    && let Some(out) = outlines.get_mut(source)
                {
                    out.push([mesh.vertices()[a as usize], mesh.vertices()[b as usize]]);
                }
            }
            let mut representatives = vec![usize::MAX; count];
            for (face, &group) in pick_sources.iter().enumerate() {
                representatives[group] = representatives[group].min(face);
            }
            FaceRegions {
                representatives,
                outlines,
                display_sources,
                pick_sources,
            }
        })
    }
    pub(super) fn component_face_representative(&self, face: usize) -> Option<usize> {
        let regions = self.polygon_face_regions();
        let group = *regions.pick_sources.get(face)?;
        regions
            .representatives
            .get(group)
            .copied()
            .filter(|&i| i != usize::MAX)
    }
    pub(super) fn component_face_region(&self, face: usize) -> Option<usize> {
        self.polygon_face_regions().pick_sources.get(face).copied()
    }
    pub(super) fn component_face_outline(&self, face: usize) -> &[[Point3; 2]] {
        let regions = self.polygon_face_regions();
        regions
            .pick_sources
            .get(face)
            .and_then(|&source| regions.outlines.get(source))
            .map_or(&[], Vec::as_slice)
    }
}
impl Viewport {
    pub(super) fn polygon_components_in_rectangle(
        &self,
        viewport: Rect,
        selection: Rect,
        document: &Document,
        object: &viboceros_document::Object,
        crossing: bool,
        inverted: bool,
    ) -> Vec<ComponentPick> {
        let kind = match object.geometry() {
            Geometry::Mesh(_) => ComponentSelectionKind::MeshFace,
            Geometry::SubD(_) => ComponentSelectionKind::SubDFace,
            _ => return Vec::new(),
        };
        let display = self
            .display_cache
            .borrow_mut()
            .get(object, document.tolerance());
        let regions = display.polygon_face_regions();
        let mut groups = (0..regions.outlines.len())
            .map(|_| ProjectedPrimitives::default())
            .collect::<Vec<_>>();
        for (primitives, segments) in groups.iter_mut().zip(&regions.outlines) {
            for &[a, b] in segments {
                self.add_selection_segment(primitives, a, b, viewport);
            }
        }
        if self.display_mode != DisplayMode::Wireframe
            && let Some(mesh) = display.mesh()
        {
            let mut triangle = 0;
            for (face, &source) in mesh.faces().iter().zip(&regions.display_sources) {
                for _ in 0..if face.is_triangle() { 1 } else { 2 } {
                    if let Some(points) = mesh.triangle_points(triangle)
                        && let Some(primitives) = groups.get_mut(source)
                    {
                        for p in points {
                            self.add_selection_point(primitives, p, viewport);
                        }
                        for clipped in self.clip_selection_triangle(points).into_iter().flatten() {
                            primitives.add_triangle(clipped.map(|p| self.project(p, viewport)));
                        }
                    }
                    triangle += 1;
                }
            }
        }
        let selected = groups
            .iter()
            .map(|p| match (crossing, inverted) {
                (false, false) => p.is_windowed_by(selection),
                (true, false) => p.is_crossed_by(selection),
                (false, true) => !p.is_crossed_by(selection),
                (true, true) => !p.is_windowed_by(selection),
            })
            .collect::<Vec<_>>();
        let mut emitted = std::collections::BTreeSet::new();
        regions
            .pick_sources
            .iter()
            .enumerate()
            .filter_map(|(index, &source)| {
                if !emitted.insert(source) {
                    return None;
                }
                selected
                    .get(source)
                    .copied()
                    .unwrap_or(false)
                    .then_some(ComponentPick {
                        object: object.id(),
                        kind,
                        index,
                    })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
