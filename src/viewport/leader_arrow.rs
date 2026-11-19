//! Resolve user arrowheads as ordinary block geometry in the leader's plane.
use super::*;
use display_cache::DisplayGeometry;
use std::{collections::HashMap, rc::Rc};
use viboceros_document::{
    BlockDefinitionId, BlockEditTreeRevision, BlockMemberDisplay, BlockReference, ColorRgb,
};
use viboceros_geometry::{AffineTransform3, BoundingBox3, Leader};

#[derive(Default)]
pub(super) struct ArrowBlocks {
    revision: Option<BlockEditTreeRevision>,
    blocks: HashMap<BlockDefinitionId, Option<CachedArrow>>,
}

struct CachedArrow {
    geometry: Rc<DisplayGeometry>,
    appearances: HashMap<Color32, Rc<[BlockMemberDisplay]>>,
}

struct ArrowPlacement {
    map: AffineTransform3,
    geometry: Rc<DisplayGeometry>,
    states: Rc<[BlockMemberDisplay]>,
}

#[derive(Clone, Copy)]
pub(super) enum ArrowPiece {
    Point(Point3),
    Segment([Point3; 2]),
    Triangle([Point3; 3]),
}

impl ArrowBlocks {
    fn resolve(
        &mut self,
        document: &Document,
        id: BlockDefinitionId,
        parent: Color32,
    ) -> Option<(Rc<DisplayGeometry>, Rc<[BlockMemberDisplay]>)> {
        let revision = document.block_edit_tree_revision();
        if self.revision != Some(revision) {
            self.blocks.clear();
            self.revision = Some(revision);
        }
        let arrow = self
            .blocks
            .entry(id)
            .or_insert_with(|| {
                let reference = BlockReference::try_new(id, AffineTransform3::identity()).ok()?;
                let geometry = document.block_instance_geometry(reference).ok()?;
                Some(CachedArrow {
                    geometry: Rc::new(DisplayGeometry::new(
                        geometry.into(),
                        0,
                        document.tolerance(),
                    )),
                    appearances: HashMap::new(),
                })
            })
            .as_mut()?;
        let Geometry::BlockInstance(instance) = &*arrow.geometry.geometry else {
            return None;
        };
        let states = arrow.appearances.entry(parent).or_insert_with(|| {
            let [r, g, b, _] = parent.to_srgba_unmultiplied();
            let root = ObjectAttributes::on_layer(document.current_layer_id())
                .with_object_color(ColorRgb::new(r, g, b));
            document
                .block_instance_member_displays(&root, instance)
                .unwrap_or_default()
                .into()
        });
        Some((Rc::clone(&arrow.geometry), Rc::clone(states)))
    }
}

impl Viewport {
    fn arrow_placement(
        &self,
        document: &Document,
        leader: &Leader,
        parent: Color32,
    ) -> Option<ArrowPlacement> {
        let id = leader.style().leader_appearance().arrow_block_id()?;
        let map = leader.arrow_transform()?;
        let id = BlockDefinitionId::from_bytes(id);
        let (geometry, states) = self
            .display_cache
            .borrow_mut()
            .arrow_blocks
            .resolve(document, id, parent)?;
        Some(ArrowPlacement {
            map,
            geometry,
            states,
        })
    }

    pub(super) fn bounds_with_leader_arrows(
        &self,
        document: &Document,
        geometry: &Geometry,
        mut bounds: Option<BoundingBox3>,
    ) -> Option<BoundingBox3> {
        let mut include = |leader: &Leader| {
            if let Some(placement) = self.arrow_placement(document, leader, Color32::WHITE) {
                let points = placement
                    .geometry
                    .block_members()
                    .iter()
                    .zip(placement.states.iter())
                    .filter(|(_, state)| state.visible)
                    .filter_map(|(member, _)| member.bounds())
                    .flat_map(|bounds| {
                        let min = bounds.min().to_array();
                        let max = bounds.max().to_array();
                        (0..8).filter_map(move |corner| {
                            let xyz = std::array::from_fn(|axis| {
                                if corner & (1 << axis) == 0 {
                                    min[axis]
                                } else {
                                    max[axis]
                                }
                            });
                            placement
                                .map
                                .transform_point(Point3::try_from(xyz).ok()?)
                                .ok()
                        })
                    });
                if let Ok(arrow) = BoundingBox3::from_points(points) {
                    bounds = Some(
                        bounds
                            .and_then(|body| body.union(arrow).ok())
                            .unwrap_or(arrow),
                    );
                }
            }
        };
        match geometry {
            Geometry::Leader(leader) => include(leader),
            Geometry::BlockInstance(block) => {
                for member in block.members() {
                    if let Geometry::Leader(leader) = &*member.geometry {
                        include(leader);
                    }
                }
            }
            _ => {}
        }
        bounds
    }

    pub(super) fn project_custom_arrow_face(
        &self,
        face: [Point3; 3],
        rect: Rect,
    ) -> impl Iterator<Item = [Pos2; 3]> {
        self.clip_selection_triangle(face)
            .into_iter()
            .flatten()
            .filter_map(move |[a, b, c]| {
                Some([
                    self.project(a, rect)?,
                    self.project(b, rect)?,
                    self.project(c, rect)?,
                ])
            })
    }

    pub(super) fn visit_custom_leader_arrow(
        &self,
        document: &Document,
        leader: &Leader,
        parent: Color32,
        mut visit: impl FnMut(ArrowPiece, Color32),
    ) {
        let Some(ArrowPlacement {
            map,
            geometry: arrow,
            states,
        }) = self.arrow_placement(document, leader, parent)
        else {
            return;
        };
        let transform = |point| map.transform_point(point).ok();
        for (member, state) in arrow.block_members().iter().zip(states.iter()) {
            if !state.visible {
                continue;
            }
            let color = Color32::from_rgb(state.color.red, state.color.green, state.color.blue);
            if self.display_mode != DisplayMode::Wireframe
                && let Some(mesh) = member.mesh()
            {
                let alpha = if self.display_mode == DisplayMode::Ghosted {
                    110
                } else {
                    (255.0 * state.opacity).round() as u8
                };
                let face = Color32::from_rgba_unmultiplied(
                    state.color.red,
                    state.color.green,
                    state.color.blue,
                    alpha,
                );
                for &indices in mesh.triangles() {
                    if let [Some(a), Some(b), Some(c)] =
                        indices.map(|i| transform(mesh.vertices()[i as usize]))
                    {
                        visit(ArrowPiece::Triangle([a, b, c]), face);
                    }
                }
            }
            for &[a, b] in member.wires() {
                if let (Some(a), Some(b)) = (transform(a), transform(b)) {
                    visit(ArrowPiece::Segment([a, b]), color);
                }
            }
            let mut point = |p| {
                if let Some(p) = transform(p) {
                    visit(ArrowPiece::Point(p), color);
                }
            };
            match &*member.geometry {
                Geometry::Point(p) => point(*p),
                Geometry::PointCloud(cloud) => {
                    for (index, &p) in cloud.points().iter().enumerate() {
                        if !cloud.is_hidden(index) {
                            point(p);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}
