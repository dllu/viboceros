//! Per-frame GPU scene staging, object display dispatch, and depth encoding.

mod curvature;
mod faces;
use super::display_cache::DisplayGeometry;
use super::object_preview::ObjectPreview;
#[cfg(test)]
use super::object_preview::TransformedObjects;
use super::*;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;
use viboceros_document::ColorRgb;
#[cfg(test)]
use viboceros_geometry::MeshFace;
use viboceros_geometry::{AffineNormalTransform3, AffineTransform3};

#[derive(Default)]
pub(super) struct SurfaceAnalyses<'a> {
    pub draft: Option<&'a viboceros_command::draft_angle_analysis::View>,
    pub curvature: Option<&'a viboceros_command::curvature_analysis::View>,
}
#[derive(Clone)]
struct CurvatureFaces {
    options: viboceros_command::curvature_analysis::Options,
    mesh: Option<Arc<viboceros_geometry::CurvatureMesh>>,
}
impl PartialEq for CurvatureFaces {
    fn eq(&self, other: &Self) -> bool {
        self.options == other.options
            && match (&self.mesh, &other.mesh) {
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            }
    }
}

#[derive(Clone, Copy)]
struct AffineMeshTransform {
    transform: AffineTransform3,
    normals: AffineNormalTransform3,
    reverse_normals: bool,
}

#[derive(Clone)]
struct FaceAppearance {
    color: Color32,
    member_colors: bool,
    draft: Option<viboceros_geometry::DraftAngleMapping>,
    analysis_colors: Option<super::display_cache::DraftColors>,
}

fn color_to_gpu(color: Color32) -> [f32; 4] {
    color
        .to_srgba_unmultiplied()
        .map(|component| f32::from(component) / 255.0)
}

fn color_with_alpha(color: Color32, alpha: u8) -> Color32 {
    let [red, green, blue, _] = color.to_srgba_unmultiplied();
    Color32::from_rgba_unmultiplied(red, green, blue, alpha)
}

fn resolved_display_color(attributes: &ObjectAttributes, layer_color: ColorRgb) -> Color32 {
    let color = attributes.display_color(layer_color);
    Color32::from_rgb(color.red, color.green, color.blue)
}

// Source snapshots are shared with the geometry cache; pointer equality here is
// safe because DisplayCache validates immutable snapshot identity before reuse.
#[derive(Clone)]
struct DisplayObject {
    geometry: Rc<DisplayGeometry>,
    color: Color32,
    face_color: Color32,
    draw_faces: bool,
    highlighted: bool,
    member_colors_enabled: bool,
    face_member_colors_enabled: bool,
    draft: Option<viboceros_command::draft_angle_analysis::Options>,
    curvature: Option<CurvatureFaces>,
    width: f32,
    point_radius: f32,
    transform: Option<AffineTransform3>,
    reversing: bool,
    overlay: bool,
}

impl PartialEq for DisplayObject {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.geometry, &other.geometry)
            && self.color == other.color
            && self.face_color == other.face_color
            && self.draw_faces == other.draw_faces
            && self.highlighted == other.highlighted
            && self.member_colors_enabled == other.member_colors_enabled
            && self.face_member_colors_enabled == other.face_member_colors_enabled
            && self.draft == other.draft
            && self.curvature == other.curvature
            && self.width == other.width
            && self.point_radius == other.point_radius
            && self.transform == other.transform
            && self.reversing == other.reversing
            && self.overlay == other.overlay
    }
}

impl DisplayObject {
    fn display_point(&self, point: Point3) -> Option<Point3> {
        self.transform
            .map_or(Some(point), |map| map.transform_point(point).ok())
    }
}

fn push_display_object(
    objects: &mut Vec<DisplayObject>,
    display: DisplayObject,
    owner: &viboceros_document::Object,
    document: &Document,
    mode: DisplayMode,
) {
    let Geometry::BlockInstance(instance) = &*display.geometry.geometry else {
        objects.push(display);
        return;
    };
    let Ok(states) = document.block_instance_member_displays(owner.attributes(), instance) else {
        return;
    };
    for (state, geometry) in states.into_iter().zip(display.geometry.block_members()) {
        if !state.visible {
            continue;
        }
        let mut leaf = display.clone();
        leaf.geometry = Rc::clone(geometry);
        let base = if state.locked {
            LOCKED_COLOR
        } else {
            Color32::from_rgb(state.color.red, state.color.green, state.color.blue)
        };
        let color = if mode == DisplayMode::Ghosted {
            color_with_alpha(base, 110)
        } else {
            base
        };
        if display.member_colors_enabled {
            leaf.color = color;
        }
        if display.face_member_colors_enabled {
            leaf.face_color = color;
        }
        leaf.member_colors_enabled &= !state.locked;
        leaf.face_member_colors_enabled &= !state.locked;
        objects.push(leaf);
    }
}

#[derive(PartialEq)]
struct SceneKey {
    camera: CameraSnapshot,
    mode: DisplayMode,
    rect: Rect,
    objects: Vec<DisplayObject>,
}

pub(super) struct CachedScene {
    key: SceneKey,
    scene: Arc<GpuViewportScene>,
}

#[derive(Clone, Copy)]
pub(super) struct DepthTriangle {
    pub(super) depth: Real,
    vertex_depths: [Real; 3],
    pub(super) vertices: [GpuTriangleVertex; 3],
}

pub(super) struct DepthPrimitive<T, const N: usize> {
    depths: [Real; N],
    instance: T,
}

pub(super) struct GpuSceneBuilder {
    pub(super) triangles: Vec<DepthTriangle>,
    pub(super) lines: Vec<DepthPrimitive<GpuLineInstance, 2>>,
    pub(super) points: Vec<DepthPrimitive<GpuPointInstance, 1>>,
    overlay_lines: Vec<DepthPrimitive<GpuLineInstance, 2>>,
    overlay_points: Vec<DepthPrimitive<GpuPointInstance, 1>>,
    pub(super) min_depth: Real,
    max_depth: Real,
}

impl GpuSceneBuilder {
    pub(super) fn new() -> Self {
        Self {
            triangles: Vec::new(),
            lines: Vec::new(),
            points: Vec::new(),
            overlay_lines: Vec::new(),
            overlay_points: Vec::new(),
            min_depth: Real::INFINITY,
            max_depth: Real::NEG_INFINITY,
        }
    }

    fn include_depth(&mut self, depth: Real) {
        if depth.is_finite() {
            self.min_depth = self.min_depth.min(depth);
            self.max_depth = self.max_depth.max(depth);
        }
    }

    pub(super) fn depth_range(&self) -> Option<(Real, Real)> {
        (self.min_depth.is_finite() && self.max_depth.is_finite())
            .then_some((self.min_depth, self.max_depth))
    }

    pub(super) fn finish(
        mut self,
        viewport: &Viewport,
        rect: Rect,
        transparent: bool,
    ) -> GpuViewportScene {
        let range = self.depth_range();
        let uniform = viewport.gpu_view_uniform(rect, range);
        if transparent {
            self.triangles.sort_by(|left, right| {
                let translucent = |t: &DepthTriangle| t.vertices.iter().any(|v| v.color[3] < 1.);
                translucent(left)
                    .cmp(&translucent(right))
                    .then_with(|| right.depth.total_cmp(&left.depth))
            });
        }
        let overlay_line_start = self.lines.len();
        let overlay_point_start = self.points.len();
        self.lines.append(&mut self.overlay_lines);
        self.points.append(&mut self.overlay_points);
        GpuViewportScene {
            uniform,
            overlay_line_start,
            overlay_point_start,
            triangles: self
                .triangles
                .into_iter()
                .flat_map(|mut triangle| {
                    for (vertex, depth) in triangle.vertices.iter_mut().zip(triangle.vertex_depths)
                    {
                        viewport.encode_gpu_depth(&mut vertex.position, depth, range);
                    }
                    triangle.vertices
                })
                .collect(),
            lines: self
                .lines
                .into_iter()
                .map(|mut line| {
                    viewport.encode_gpu_depth(
                        &mut line.instance.start_width,
                        line.depths[0],
                        range,
                    );
                    viewport.encode_gpu_depth(
                        &mut line.instance.end_padding,
                        line.depths[1],
                        range,
                    );
                    line.instance
                })
                .collect(),
            points: self
                .points
                .into_iter()
                .map(|mut point| {
                    viewport.encode_gpu_depth(
                        &mut point.instance.position_size,
                        point.depths[0],
                        range,
                    );
                    point.instance
                })
                .collect(),
            transparent,
        }
    }
}

impl Viewport {
    #[cfg(test)]
    pub(super) fn object_scene(&self, rect: Rect, document: &Document) -> Arc<GpuViewportScene> {
        self.object_scene_with_preview(rect, document, None, &[])
    }

    #[cfg(test)]
    pub(super) fn object_scene_with_preview(
        &self,
        rect: Rect,
        document: &Document,
        preview: Option<ObjectSelectionFilter>,
        preview_ids: &[ObjectId],
    ) -> Arc<GpuViewportScene> {
        self.object_scene_with_transform(rect, document, preview, preview_ids, None)
    }

    #[cfg(test)]
    fn object_scene_with_transform(
        &self,
        rect: Rect,
        document: &Document,
        preview: Option<ObjectSelectionFilter>,
        preview_ids: &[ObjectId],
        transform: Option<TransformedObjects<'_>>,
    ) -> Arc<GpuViewportScene> {
        self.object_scene_with_object_preview(
            rect,
            document,
            preview,
            preview_ids,
            transform.map(ObjectPreview::Affine),
        )
    }

    #[cfg(test)]
    pub(super) fn object_scene_with_object_preview(
        &self,
        rect: Rect,
        document: &Document,
        preview: Option<ObjectSelectionFilter>,
        preview_ids: &[ObjectId],
        preview_transform: Option<ObjectPreview<'_>>,
    ) -> Arc<GpuViewportScene> {
        self.object_scene_with_analysis(
            rect,
            document,
            preview,
            preview_ids,
            preview_transform,
            None,
        )
    }

    #[cfg(test)]
    pub(super) fn object_scene_with_analysis(
        &self,
        rect: Rect,
        document: &Document,
        preview: Option<ObjectSelectionFilter>,
        preview_ids: &[ObjectId],
        preview_transform: Option<ObjectPreview<'_>>,
        analysis: Option<&viboceros_command::draft_angle_analysis::View>,
    ) -> Arc<GpuViewportScene> {
        self.object_scene_with_analyses(
            rect,
            document,
            preview,
            preview_ids,
            preview_transform,
            SurfaceAnalyses {
                draft: analysis,
                curvature: None,
            },
        )
    }

    pub(super) fn object_scene_with_analyses(
        &self,
        rect: Rect,
        document: &Document,
        preview: Option<ObjectSelectionFilter>,
        preview_ids: &[ObjectId],
        preview_transform: Option<ObjectPreview<'_>>,
        analyses: SurfaceAnalyses<'_>,
    ) -> Arc<GpuViewportScene> {
        let (transform, deformed, instances) = match preview_transform {
            Some(ObjectPreview::Affine(map)) => (Some(map), None, None),
            Some(ObjectPreview::Deformed(objects)) => (None, Some(objects), None),
            Some(ObjectPreview::Instances(instances)) => (None, None, Some(instances)),
            None => (None, None, None),
        };
        let mut objects = Vec::new();
        let mut visible = HashSet::new();
        let instance_sources = instances.map(|p| p.sources.iter().copied().collect::<HashSet<_>>());
        let preview_ids = preview_ids.iter().copied().collect::<HashSet<_>>();
        let block_sources = preview == Some(ObjectSelectionFilter::BlockEditSources);
        let sources = transform.map(|preview| {
            preview
                .sources
                .iter()
                .copied()
                .chain(preview.grips.iter().map(|id| id.object))
                .collect::<HashSet<_>>()
        });
        let mut cache = self.display_cache.borrow_mut();
        for object in document.objects() {
            let attributes = object.attributes();
            let Some(layer) = document.layer(attributes.layer_id()) else {
                continue;
            };
            if !layer.is_visible() || (preview.is_some() && !block_sources && layer.is_locked()) {
                continue;
            }
            if block_sources {
                if !attributes.is_visible() {
                    continue;
                }
            } else if let Some(filter) = preview {
                if !filter.accepts_object(object) {
                    continue;
                }
            } else if !attributes.is_visible() {
                continue;
            }
            visible.insert(object.id());
            let mut color = if (preview.is_none() || block_sources)
                && (attributes.is_locked() || layer.is_locked())
            {
                LOCKED_COLOR
            } else {
                resolved_display_color(attributes, layer.color())
            };
            let base_color = color;
            if self.display_mode == DisplayMode::Ghosted {
                color = color_with_alpha(color, 110);
            }
            let deformed = deformed.and_then(|objects| objects.get(&object.id()));
            let highlighted = preview_ids.contains(&object.id());
            let transformed = sources
                .as_ref()
                .is_some_and(|ids| ids.contains(&object.id()));
            let selected = highlighted
                || (!transformed
                    && (preview.is_none() || block_sources)
                    && (document.is_selected(object.id())
                        || transform
                            .is_some_and(|preview| preview.reference == Some(object.id()))));
            if highlighted && preview.is_none() {
                color = PICK_PREVIEW_COLOR;
            } else if selected {
                color = SELECTED_COLOR;
            }
            let width = match self.display_mode {
                DisplayMode::Wireframe => 1.5,
                DisplayMode::Shaded => 2.25,
                DisplayMode::Ghosted => 1.25,
            } + if selected { 1.5 } else { 0.0 };
            let mut display = DisplayObject {
                geometry: cache.get(object, document.tolerance(), document.subd_control_net()),
                color,
                face_color: if self.display_mode == DisplayMode::Ghosted {
                    color_with_alpha(base_color, 110)
                } else {
                    base_color
                },
                draw_faces: true,
                highlighted,
                member_colors_enabled: !selected
                    && (preview.is_some() || (!attributes.is_locked() && !layer.is_locked())),
                face_member_colors_enabled: preview.is_some()
                    || (!attributes.is_locked() && !layer.is_locked()),
                draft: analyses
                    .draft
                    .filter(|v| v.objects.contains(&object.id()))
                    .map(|v| v.options),
                curvature: analyses
                    .curvature
                    .filter(|v| v.objects.contains(&object.id()))
                    .map(|v| CurvatureFaces {
                        options: v.options,
                        mesh: v.meshes.get(&object.id()).cloned(),
                    }),
                width,
                point_radius: if selected { 3.5 } else { 2.5 },
                transform: None,
                reversing: false,
                overlay: false,
            };
            if transformed {
                let transform = transform.unwrap();
                let partial = transform.grips.iter().any(|id| id.object == object.id());
                let posed = if partial {
                    let Some(geometry) = self.grip_preview_geometry(object, transform, document)
                    else {
                        push_display_object(
                            &mut objects,
                            display,
                            object,
                            document,
                            self.display_mode,
                        );
                        continue;
                    };
                    geometry
                } else if let Some(geometry) = transform.source_geometry {
                    cache.temporary_source(
                        object,
                        geometry,
                        document.tolerance(),
                        document.subd_control_net(),
                    )
                } else {
                    Rc::clone(&display.geometry)
                };
                push_display_object(
                    &mut objects,
                    DisplayObject {
                        curvature: display.curvature.as_ref().map(|c| CurvatureFaces {
                            options: c.options,
                            mesh: if partial {
                                posed.curvature_mesh()
                            } else {
                                c.mesh.clone()
                            },
                        }),
                        geometry: posed,
                        color: SELECTED_COLOR,
                        face_color: display.face_color,
                        draw_faces: true,
                        highlighted: true,
                        member_colors_enabled: false,
                        face_member_colors_enabled: display.face_member_colors_enabled,
                        draft: display.draft,
                        width: width + 1.5,
                        point_radius: 3.5,
                        transform: if partial {
                            None
                        } else {
                            transform.object_transform(object.id()).ok()
                        },
                        reversing: !partial && transform.reversing,
                        overlay: false,
                    },
                    object,
                    document,
                    self.display_mode,
                );
                display.draw_faces = transform.draw_source_faces;
                if transform.reference_sources {
                    display.color = LOCKED_COLOR;
                    display.member_colors_enabled = false;
                }
                push_display_object(&mut objects, display, object, document, self.display_mode);
            } else if let Some(deformed) = deformed {
                // Native deformation commands keep selected originals, including their
                // faces, while the temporary deformation is drawn as colored wires.
                display.face_member_colors_enabled = !attributes.is_locked() && !layer.is_locked();
                push_display_object(&mut objects, display, object, document, self.display_mode);
                push_display_object(
                    &mut objects,
                    DisplayObject {
                        geometry: Rc::clone(&deformed.geometry),
                        color: base_color,
                        face_color: base_color,
                        draw_faces: false,
                        highlighted: true,
                        member_colors_enabled: true,
                        face_member_colors_enabled: true,
                        draft: None,
                        curvature: None,
                        width: match self.display_mode {
                            DisplayMode::Wireframe => 1.5,
                            DisplayMode::Shaded => 2.25,
                            DisplayMode::Ghosted => 1.25,
                        },
                        point_radius: 2.5,
                        transform: deformed.transform,
                        reversing: false,
                        overlay: true,
                    },
                    object,
                    document,
                    self.display_mode,
                );
            } else if let Some(instances) = instances.filter(|_| {
                instance_sources
                    .as_ref()
                    .is_some_and(|ids| ids.contains(&object.id()))
            }) {
                push_display_object(
                    &mut objects,
                    display.clone(),
                    object,
                    document,
                    self.display_mode,
                );
                for map in instances.transforms {
                    let mut posed = display.clone();
                    posed.transform = Some(*map);
                    posed.color = base_color;
                    posed.highlighted = true;
                    posed.member_colors_enabled = true;
                    posed.width = match self.display_mode {
                        DisplayMode::Wireframe => 1.5,
                        DisplayMode::Shaded => 2.25,
                        DisplayMode::Ghosted => 1.25,
                    };
                    posed.point_radius = 2.5;
                    posed.reversing = map.orientation_reversing().unwrap_or(false);
                    push_display_object(&mut objects, posed, object, document, self.display_mode);
                }
            } else {
                push_display_object(&mut objects, display, object, document, self.display_mode);
            }
        }
        objects.sort_by_key(|object| object.highlighted);
        cache.retain_visible(&visible);
        self.grip_preview_cache
            .borrow_mut()
            .retain_visible(&visible);
        drop(cache);
        let key = SceneKey {
            camera: self.camera_snapshot(),
            mode: self.display_mode,
            rect,
            objects,
        };
        let mut cached = self.cached_scene.borrow_mut();
        if let Some(previous) = cached.as_ref()
            && previous.key == key
        {
            return Arc::clone(&previous.scene);
        }
        let mut scene = GpuSceneBuilder::new();
        for object in &key.objects {
            let normal_transform = object.transform.map(AffineNormalTransform3::new);
            let display = &object.geometry;
            match &*display.geometry {
                Geometry::Point(point) => {
                    if let Some(point) = object.display_point(*point) {
                        let count = scene.points.len();
                        self.add_gpu_point(&mut scene, rect, point, 4.5, object.color);
                        if object.overlay && scene.points.len() > count {
                            let point = scene.points.pop().unwrap();
                            scene.overlay_points.push(point);
                        }
                    }
                }
                Geometry::PointCloud(cloud) => {
                    for (index, point) in cloud.points().iter().enumerate() {
                        if cloud.is_hidden(index) {
                            continue;
                        }
                        let color = if object.member_colors_enabled {
                            cloud.colors().map_or(object.color, |colors| {
                                let [red, green, blue, transparency] = colors[index];
                                let color = Color32::from_rgba_unmultiplied(
                                    red,
                                    green,
                                    blue,
                                    255 - transparency,
                                );
                                if self.display_mode == DisplayMode::Ghosted {
                                    color_with_alpha(color, 110)
                                } else {
                                    color
                                }
                            })
                        } else {
                            object.color
                        };
                        if let Some(point) = object.display_point(*point) {
                            let count = scene.points.len();
                            self.add_gpu_point(&mut scene, rect, point, object.point_radius, color);
                            if object.overlay && scene.points.len() > count {
                                let point = scene.points.pop().unwrap();
                                scene.overlay_points.push(point);
                            }
                        }
                    }
                }
                _ => {
                    if (self.display_mode != DisplayMode::Wireframe
                        || object.draft.is_some()
                        || object.curvature.is_some())
                        && object.draw_faces
                    {
                        if let Some(curvature) = &object.curvature {
                            self.add_curvature_faces(
                                &mut scene,
                                object,
                                display,
                                curvature,
                                normal_transform,
                            );
                        } else if let Some(mesh) = display.mesh() {
                            let draft = object.draft.and_then(|s| s.mapping().ok());
                            self.add_gpu_mesh_faces_with_normals(
                                &mut scene,
                                mesh,
                                display.normals().iter().copied(),
                                FaceAppearance {
                                    color: object.face_color,
                                    member_colors: object.face_member_colors_enabled,
                                    draft,
                                    analysis_colors: draft
                                        .filter(|_| object.transform.is_none())
                                        .map(|m| display.draft_colors(m)),
                                },
                                object.transform.map(|transform| AffineMeshTransform {
                                    transform,
                                    normals: normal_transform.unwrap(),
                                    // Raw meshes/surfaces keep face/parameter
                                    // order. B-reps reverse face orientation.
                                    reverse_normals: object.reversing
                                        && matches!(&*display.geometry, Geometry::Brep(_)),
                                }),
                            );
                        }
                    }
                    if object.draft.is_some_and(|s| !s.show_isocurves) {
                        continue;
                    }
                    if object
                        .curvature
                        .as_ref()
                        .is_some_and(|c| !c.options.show_isocurves)
                    {
                        continue;
                    }
                    for &[a, b] in display.wires() {
                        if let (Some(a), Some(b)) =
                            (object.display_point(a), object.display_point(b))
                        {
                            let count = scene.lines.len();
                            self.add_gpu_line(&mut scene, rect, a, b, object.width, object.color);
                            if object.overlay && scene.lines.len() > count {
                                let line = scene.lines.pop().unwrap();
                                scene.overlay_lines.push(line);
                            }
                        }
                    }
                }
            }
        }
        let scene = Arc::new(scene.finish(self, rect, self.display_mode == DisplayMode::Ghosted));
        *cached = Some(CachedScene {
            key,
            scene: Arc::clone(&scene),
        });
        scene
    }

    pub(super) fn add_gpu_point(
        &self,
        scene: &mut GpuSceneBuilder,
        rect: Rect,
        point: Point3,
        radius: f32,
        color: Color32,
    ) {
        let Some(projected) = self.project(point, rect) else {
            return;
        };
        if !rect.expand(radius).contains(projected) {
            return;
        }
        let Some(position) = self.gpu_position(point) else {
            return;
        };
        let depth = self.view_depth(point);
        scene.include_depth(depth);
        scene.points.push(DepthPrimitive {
            depths: [depth],
            instance: GpuPointInstance {
                position_size: [position[0], position[1], position[2], radius],
                color: color_to_gpu(color),
            },
        });
    }

    pub(super) fn add_gpu_line(
        &self,
        scene: &mut GpuSceneBuilder,
        rect: Rect,
        start: Point3,
        end: Point3,
        width: f32,
        color: Color32,
    ) {
        let Some([start, end]) = self.clip_segment(start, end) else {
            return;
        };
        if self.project(start, rect).is_none() || self.project(end, rect).is_none() {
            return;
        }
        let (Some(start_position), Some(end_position)) =
            (self.gpu_position(start), self.gpu_position(end))
        else {
            return;
        };
        let depths = [self.view_depth(start), self.view_depth(end)];
        for depth in depths {
            scene.include_depth(depth);
        }
        scene.lines.push(DepthPrimitive {
            depths,
            instance: GpuLineInstance {
                start_width: [
                    start_position[0],
                    start_position[1],
                    start_position[2],
                    width,
                ],
                end_padding: [end_position[0], end_position[1], end_position[2], 0.0],
                color: color_to_gpu(color),
            },
        });
    }

    #[cfg(test)]
    fn add_gpu_nurbs_curve(
        &self,
        scene: &mut GpuSceneBuilder,
        rect: Rect,
        curve: &impl ViewportCurve,
        width: f32,
        color: Color32,
    ) {
        curve.visit_segments(|start, end| {
            self.add_gpu_line(scene, rect, start, end, width, color);
        });
    }
}

#[cfg(test)]
mod assembly_tests;

#[cfg(test)]
mod mirror_tests;

#[cfg(test)]
mod translation_tests;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod affine_tests;

#[cfg(test)]
mod grip_tests;

#[cfg(test)]
mod block_tests;

#[cfg(test)]
mod subd_normal_tests;
