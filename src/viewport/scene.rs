//! Per-frame GPU scene staging, object display dispatch, and depth encoding.

use super::display_cache::DisplayGeometry;
use super::*;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;
use viboceros_document::ColorRgb;

const SMOOTH_SHADING_COSINE: Real = std::f64::consts::FRAC_1_SQRT_2;

fn vector_to_gpu(vector: NaVector3<Real>) -> [f32; 3] {
    [vector.x as f32, vector.y as f32, vector.z as f32]
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

pub(super) fn smooth_corner_normals(mesh: &TriangleMesh) -> Vec<[NaVector3<Real>; 3]> {
    let fallback = NaVector3::new(0.0, 0.0, 1.0);
    let face_normals = (0..mesh.triangles().len())
        .map(|index| {
            mesh.face_normal(index)
                .map(|normal| NaVector3::new(normal.x(), normal.y(), normal.z()))
                .unwrap_or(fallback)
        })
        .collect::<Vec<_>>();

    // Surface tessellation intentionally duplicates vertices at knot-span
    // boundaries. Group exact coincident samples so continuous spans shade as
    // one surface, then use the crease angle below to keep analytic caps and
    // other genuinely sharp joins hard.
    let mut incident_faces: HashMap<[u64; 3], Vec<usize>> = HashMap::new();
    for (face_index, triangle) in mesh.triangles().iter().enumerate() {
        for &vertex_index in triangle {
            let point = mesh.vertices()[vertex_index as usize];
            incident_faces
                .entry(point_position_key(point))
                .or_default()
                .push(face_index);
        }
    }

    mesh.triangles()
        .iter()
        .enumerate()
        .map(|(face_index, triangle)| {
            let reference = face_normals[face_index];
            triangle.map(|vertex_index| {
                let point = mesh.vertices()[vertex_index as usize];
                let mut sum = NaVector3::zeros();
                for &incident in &incident_faces[&point_position_key(point)] {
                    let candidate = face_normals[incident];
                    if reference.dot(&candidate) >= SMOOTH_SHADING_COSINE {
                        sum += candidate;
                    }
                }
                sum.try_normalize(Real::EPSILON).unwrap_or(reference)
            })
        })
        .collect()
}

fn point_position_key(point: Point3) -> [u64; 3] {
    [point.x(), point.y(), point.z()].map(|value| {
        if value == 0.0 {
            0.0_f64.to_bits()
        } else {
            value.to_bits()
        }
    })
}

// Source snapshots are shared with the geometry cache; pointer equality here is
// safe because DisplayCache validates immutable snapshot identity before reuse.
struct DisplayObject {
    geometry: Rc<DisplayGeometry>,
    color: Color32,
    highlighted: bool,
    member_colors_enabled: bool,
    width: f32,
    point_radius: f32,
}

impl PartialEq for DisplayObject {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.geometry, &other.geometry)
            && self.color == other.color
            && self.highlighted == other.highlighted
            && self.member_colors_enabled == other.member_colors_enabled
            && self.width == other.width
            && self.point_radius == other.point_radius
    }
}

#[derive(PartialEq)]
struct SceneKey {
    kind: ViewKind,
    mode: DisplayMode,
    rect: Rect,
    pixels_per_unit: f32,
    pan: Vec2,
    orbit: [Real; 2],
    distance: Real,
    target: NaVector3<Real>,
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
    pub(super) min_depth: Real,
    max_depth: Real,
}

impl GpuSceneBuilder {
    pub(super) fn new() -> Self {
        Self {
            triangles: Vec::new(),
            lines: Vec::new(),
            points: Vec::new(),
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
            self.triangles
                .sort_by(|left, right| right.depth.total_cmp(&left.depth));
        }
        GpuViewportScene {
            uniform,
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
    pub(super) fn paint_objects(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        document: &Document,
        viewport_index: usize,
        preview: Option<ObjectSelectionFilter>,
        preview_ids: &[ObjectId],
    ) {
        crate::viewport_gpu::paint(
            painter,
            rect,
            viewport_index,
            self.object_scene_with_preview(rect, document, preview, preview_ids),
        );
    }

    #[cfg(test)]
    pub(super) fn object_scene(&self, rect: Rect, document: &Document) -> Arc<GpuViewportScene> {
        self.object_scene_with_preview(rect, document, None, &[])
    }

    pub(super) fn object_scene_with_preview(
        &self,
        rect: Rect,
        document: &Document,
        preview: Option<ObjectSelectionFilter>,
        preview_ids: &[ObjectId],
    ) -> Arc<GpuViewportScene> {
        let mut objects = Vec::new();
        let mut visible = HashSet::new();
        let preview_ids = preview_ids.iter().copied().collect::<HashSet<_>>();
        let mut cache = self.display_cache.borrow_mut();
        for object in document.objects() {
            let attributes = object.attributes();
            let Some(layer) = document.layer(attributes.layer_id()) else {
                continue;
            };
            if !layer.is_visible() || (preview.is_some() && layer.is_locked()) {
                continue;
            }
            if let Some(filter) = preview {
                if !filter.accepts_object(object) {
                    continue;
                }
            } else if !attributes.is_visible() {
                continue;
            }
            visible.insert(object.id());
            let mut color = if preview.is_none() && (attributes.is_locked() || layer.is_locked()) {
                LOCKED_COLOR
            } else {
                resolved_display_color(attributes, layer.color())
            };
            if self.display_mode == DisplayMode::Ghosted {
                color = color_with_alpha(color, 110);
            }
            let highlighted = preview_ids.contains(&object.id());
            let selected = highlighted || (preview.is_none() && document.is_selected(object.id()));
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
            objects.push(DisplayObject {
                geometry: cache.get(object, document.tolerance()),
                color,
                highlighted,
                member_colors_enabled: !selected
                    && (preview.is_some() || (!attributes.is_locked() && !layer.is_locked())),
                width,
                point_radius: if selected { 3.5 } else { 2.5 },
            });
        }
        objects.sort_by_key(|object| object.highlighted);
        cache.retain_visible(&visible);
        drop(cache);
        let key = SceneKey {
            kind: self.kind,
            mode: self.display_mode,
            rect,
            pixels_per_unit: self.pixels_per_unit,
            pan: self.pan,
            orbit: [self.orbit_yaw, self.orbit_pitch],
            distance: self.perspective_camera_distance,
            target: self.target,
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
            let display = &object.geometry;
            match &*display.geometry {
                Geometry::Point(point) => {
                    self.add_gpu_point(&mut scene, rect, *point, 4.5, object.color)
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
                        self.add_gpu_point(&mut scene, rect, *point, object.point_radius, color);
                    }
                }
                _ => {
                    if self.display_mode != DisplayMode::Wireframe
                        && let Some(mesh) = display.mesh()
                    {
                        self.add_gpu_mesh_faces_with_normals(
                            &mut scene,
                            mesh,
                            display.normals(),
                            object.color,
                            object.member_colors_enabled,
                        );
                    }
                    for &[a, b] in display.wires() {
                        self.add_gpu_line(&mut scene, rect, a, b, object.width, object.color);
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

    #[cfg(test)]
    pub(super) fn add_gpu_mesh_faces(
        &self,
        scene: &mut GpuSceneBuilder,
        mesh: &TriangleMesh,
        color: Color32,
    ) {
        if self.display_mode == DisplayMode::Wireframe {
            return;
        }

        self.add_gpu_mesh_faces_with_normals(
            scene,
            mesh,
            &smooth_corner_normals(mesh),
            color,
            true,
        );
    }

    fn add_gpu_mesh_faces_with_normals(
        &self,
        scene: &mut GpuSceneBuilder,
        mesh: &TriangleMesh,
        corner_normals: &[[NaVector3<Real>; 3]],
        color: Color32,
        member_colors_enabled: bool,
    ) {
        let face_color = if self.display_mode == DisplayMode::Ghosted {
            color_with_alpha(color, 35)
        } else {
            color
        };
        let gpu_color = color_to_gpu(face_color);
        for (triangle_index, normals) in corner_normals.iter().enumerate() {
            let Some(points) = mesh.triangle_points(triangle_index) else {
                continue;
            };
            let clipped = self.clip_triangle(points);
            if clipped[0].is_none() {
                continue;
            }
            // Submit the original triangle so hardware clipping interpolates
            // smooth normals correctly. Only visible geometry sets the depth
            // range; behind-camera vertices must not move the near plane.
            let [Some(first), Some(second), Some(third)] =
                points.map(|point| self.gpu_position(point))
            else {
                continue;
            };
            let normals = normals.map(vector_to_gpu);
            let vertex_colors = if member_colors_enabled {
                mesh.vertex_colors().map(|colors| {
                    mesh.triangles()[triangle_index].map(|index| {
                        let [red, green, blue, transparency] = colors[index as usize];
                        let alpha = if self.display_mode == DisplayMode::Ghosted {
                            35
                        } else {
                            255 - transparency
                        };
                        color_to_gpu(Color32::from_rgba_unmultiplied(red, green, blue, alpha))
                    })
                })
            } else {
                None
            }
            .unwrap_or([gpu_color; 3]);
            let vertex_depths = points.map(|point| self.view_depth(point));
            let depth_scale = vertex_depths.iter().map(|d| d.abs()).fold(0.0, Real::max);
            let mut depth = 0.0;
            let count = if clipped[1].is_some() { 4.0 } else { 3.0 };
            for point in clipped[0]
                .into_iter()
                .flatten()
                .chain(clipped[1].map(|triangle| triangle[2]))
            {
                let point_depth = self.view_depth(point);
                scene.include_depth(point_depth);
                if depth_scale > 0.0 {
                    depth += point_depth / depth_scale;
                }
            }
            let depth = (depth / count) * depth_scale;
            scene.triangles.push(DepthTriangle {
                depth,
                vertex_depths,
                vertices: [
                    GpuTriangleVertex {
                        position: first,
                        normal: normals[0],
                        color: vertex_colors[0],
                    },
                    GpuTriangleVertex {
                        position: second,
                        normal: normals[1],
                        color: vertex_colors[1],
                    },
                    GpuTriangleVertex {
                        position: third,
                        normal: normals[2],
                        color: vertex_colors[2],
                    },
                ],
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    #[test]
    fn gpu_curve_lines_match_after_exact_knot_translation() {
        let view = Viewport::new(ViewKind::Top);
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
        let make = |origin| {
            NurbsCurve::try_new(
                2,
                vec![point(0., 0., 0.), point(1., 2., 0.), point(2., 0., 0.)],
                vec![
                    origin,
                    origin,
                    origin,
                    origin + 2.,
                    origin + 2.,
                    origin + 2.,
                ],
            )
            .unwrap()
        };
        let mut expected = GpuSceneBuilder::new();
        let mut actual = GpuSceneBuilder::new();
        view.add_gpu_nurbs_curve(&mut expected, rect, &make(0.), 1., Color32::WHITE);
        view.add_gpu_nurbs_curve(
            &mut actual,
            rect,
            &make(2.0_f64.powi(52)),
            1.,
            Color32::WHITE,
        );
        assert_eq!(actual.lines.len(), CURVE_SAMPLES_PER_SPAN);
        for (a, b) in actual.lines.iter().zip(&expected.lines) {
            assert_eq!(a.instance.start_width, b.instance.start_width);
            assert_eq!(a.instance.end_padding, b.instance.end_padding);
            assert_eq!(a.depths, b.depths);
        }
    }

    #[test]
    fn smooth_shading_keeps_ninety_degree_mesh_edges_hard() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(0.0, 0.0, 1.0),
            ],
            vec![[0, 1, 2], [0, 1, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let normals = smooth_corner_normals(&mesh);
        assert!(Tolerance::DEFAULT.approx_eq(normals[0][0].z, 1.0));
        assert!(Tolerance::DEFAULT.approx_eq(normals[0][0].y, 0.0));
        assert!(Tolerance::DEFAULT.approx_eq(normals[1][0].y, -1.0));
        assert!(Tolerance::DEFAULT.approx_eq(normals[1][0].z, 0.0));
    }

    #[test]
    fn object_color_overrides_layer_color_only_for_object_source() {
        let object = ColorRgb::new(12, 34, 56);
        let layer = ColorRgb::new(78, 90, 123);
        let document = Document::default();
        let base =
            ObjectAttributes::on_layer(document.current_layer_id()).with_object_color(object);
        assert_eq!(
            resolved_display_color(&base, layer),
            Color32::from_rgb(12, 34, 56)
        );
        for source in [
            viboceros_document::ObjectColorSource::Layer,
            viboceros_document::ObjectColorSource::Material,
            viboceros_document::ObjectColorSource::Parent,
        ] {
            assert_eq!(
                resolved_display_color(&base.clone().with_color_source(source), layer),
                Color32::from_rgb(78, 90, 123)
            );
        }
    }

    #[test]
    fn special_preview_draws_only_candidates_and_highlights_pending_picks() {
        let mut document = Document::default();
        let visible = document
            .add_geometry(Geometry::Point(point(-2., 0., 0.)))
            .unwrap();
        let hidden = document
            .add_geometry(Geometry::Point(point(0., 0., 0.)))
            .unwrap();
        let locked = document
            .add_geometry(Geometry::Point(point(2., 0., 0.)))
            .unwrap();
        document.set_objects_visibility([hidden], false).unwrap();
        document.set_objects_locked([locked], true).unwrap();
        let view = Viewport::new(ViewKind::Top);
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
        let normal = view.object_scene(rect, &document);
        assert_eq!(normal.points.len(), 2);
        let shown = view.object_scene_with_preview(
            rect,
            &document,
            Some(ObjectSelectionFilter::HiddenObjects),
            &[hidden],
        );
        assert_eq!(shown.points.len(), 1);
        assert_eq!(shown.points[0].color, color_to_gpu(SELECTED_COLOR));
        let unlocked = view.object_scene_with_preview(
            rect,
            &document,
            Some(ObjectSelectionFilter::LockedObjects),
            &[],
        );
        assert_eq!(unlocked.points.len(), 1);
        assert_ne!(unlocked.points[0].color, color_to_gpu(LOCKED_COLOR));
        assert!(document.object(visible).unwrap().attributes().is_visible());
    }

    #[test]
    fn choice_highlight_is_drawn_after_coincident_unselected_points() {
        let mut document = Document::default();
        let point = point(0.0, 0.0, 0.0);
        let first = document.add_geometry(Geometry::Point(point)).unwrap();
        let second = document.add_geometry(Geometry::Point(point)).unwrap();
        let view = Viewport::new(ViewKind::Top);
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let scene = view.object_scene_with_preview(rect, &document, None, &[first]);
        assert_eq!(scene.points.len(), 2);
        assert_ne!(scene.points[0].color, color_to_gpu(PICK_PREVIEW_COLOR));
        assert_eq!(scene.points[1].color, color_to_gpu(PICK_PREVIEW_COLOR));
        assert_eq!(document.selected_object_count(), 0);
        let scene = view.object_scene_with_preview(rect, &document, None, &[second]);
        assert_eq!(scene.points[1].color, color_to_gpu(PICK_PREVIEW_COLOR));
        document
            .select_objects([first, second], SelectionMode::Replace)
            .unwrap();
        let scene = view.object_scene_with_preview(rect, &document, None, &[first]);
        assert_eq!(scene.points[0].color, color_to_gpu(SELECTED_COLOR));
        assert_eq!(scene.points[1].color, color_to_gpu(PICK_PREVIEW_COLOR));
    }

    #[test]
    fn point_cloud_member_colors_reach_the_gpu_scene() {
        let mut document = Document::default();
        let cloud = PointCloud3::try_with_colors(
            vec![point(-1.0, 0.0, 0.0), point(1.0, 0.0, 0.0)],
            Some(vec![[200, 20, 30, 0], [40, 50, 220, 128]]),
        )
        .unwrap();
        let id = document.add_geometry(Geometry::PointCloud(cloud)).unwrap();
        let view = Viewport::new(ViewKind::Top);
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let scene = view.object_scene(rect, &document);
        assert_eq!(scene.points.len(), 2);
        let expected = [
            [200.0 / 255.0, 20.0 / 255.0, 30.0 / 255.0, 1.0],
            [40.0 / 255.0, 50.0 / 255.0, 220.0 / 255.0, 127.0 / 255.0],
        ];
        for color in expected {
            assert!(
                scene.points.iter().any(|point| {
                    point
                        .color
                        .iter()
                        .zip(color)
                        .all(|(actual, expected)| (actual - expected).abs() <= 1.1 / 255.0)
                }),
                "expected {color:?}, got {:?}",
                scene
                    .points
                    .iter()
                    .map(|point| point.color)
                    .collect::<Vec<_>>()
            );
        }
        document
            .select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        let selected = view.object_scene(rect, &document);
        assert!(
            selected
                .points
                .iter()
                .all(|point| point.color == color_to_gpu(SELECTED_COLOR))
        );
    }

    #[test]
    fn hidden_cloud_members_do_not_reach_gpu_scene() {
        let mut document = Document::default();
        let cloud = PointCloud3::try_new(vec![point(-1.0, 0.0, 0.0), point(1.0, 0.0, 0.0)])
            .unwrap()
            .with_hidden(vec![true, false])
            .unwrap();
        document.add_geometry(Geometry::PointCloud(cloud)).unwrap();
        let view = Viewport::new(ViewKind::Top);
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let scene = view.object_scene(rect, &document);
        assert_eq!(scene.points.len(), 1);
    }

    #[test]
    fn mesh_vertex_colors_reach_the_gpu_scene() {
        let mut document = Document::default();
        let mesh = TriangleMesh::try_new(
            vec![
                point(-1.0, -1.0, 0.0),
                point(1.0, -1.0, 0.0),
                point(0.0, 1.0, 0.0),
            ],
            vec![[0, 1, 2]],
            document.tolerance(),
        )
        .unwrap()
        .try_with_vertex_colors(Some(vec![
            [200, 20, 30, 0],
            [40, 50, 220, 128],
            [10, 240, 80, 0],
        ]))
        .unwrap();
        let id = document.add_geometry(Geometry::Mesh(mesh)).unwrap();
        let mut view = Viewport::new(ViewKind::Top);
        view.display_mode = DisplayMode::Shaded;
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let scene = view.object_scene(rect, &document);
        assert_eq!(scene.triangles.len(), 3);
        let expected = [
            [200.0 / 255.0, 20.0 / 255.0, 30.0 / 255.0, 1.0],
            [40.0 / 255.0, 50.0 / 255.0, 220.0 / 255.0, 127.0 / 255.0],
            [10.0 / 255.0, 240.0 / 255.0, 80.0 / 255.0, 1.0],
        ];
        for (actual, expected) in scene.triangles.iter().zip(expected) {
            assert!(
                actual
                    .color
                    .iter()
                    .zip(expected)
                    .all(|(actual, expected)| (actual - expected).abs() <= 1.1 / 255.0)
            );
        }
        document
            .select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        let selected = view.object_scene(rect, &document);
        assert!(
            selected
                .triangles
                .iter()
                .all(|vertex| vertex.color == color_to_gpu(SELECTED_COLOR))
        );
    }
}
