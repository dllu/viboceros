#pragma once
#include "opennurbs_public.h"
#include "viboceros_opennurbs.h"

namespace vibo_camera {
// OpenNURBS exposes cached axes to subclasses. Replaying SetCameraDirection/Up
// would rebuild them using lock priority and lose a serialized camera frame.
class CachedViewport final : public ON_Viewport {
public:
  explicit CachedViewport(const ON_Viewport& source) : ON_Viewport(source) {}
  bool RestoreAxes(const double (&axes)[3][3]) {
    const ON_3dVector x(axes[0]), y(axes[1]), z(axes[2]);
    if (!ON_IsRightHandFrame(x, y, z)) return false;
    m_CamX = x;
    m_CamY = y;
    m_CamZ = z;
    m_bValidCameraFrame = true;
    // The public location setter invalidates OpenNURBS's projection hash
    // without recalculating the frame or changing the raw camera vectors.
    return SetCameraLocation(CameraLocation());
  }
};

inline bool read_view_camera(const ON_3dmView& source_view, ViboNamedView& record) {
  if (!source_view.m_vp.IsValid() || !source_view.m_cplane.m_plane.IsValid()) {
    return false;
  }
  record.projection = source_view.m_vp.IsTwoPointPerspectiveProjection()
                          ? 3
                          : static_cast<uint8_t>(source_view.m_vp.Projection());
  if (record.projection < 1 || record.projection > 3) {
    return false;
  }
  const auto fill3 = [](double (&destination)[3], const auto& value) {
    destination[0] = value.x;
    destination[1] = value.y;
    destination[2] = value.z;
  };
  fill3(record.camera_location, source_view.m_vp.CameraLocation());
  fill3(record.camera_direction, source_view.m_vp.CameraDirection());
  fill3(record.camera_up, source_view.m_vp.CameraUp());
  record.has_camera_axes = 1;
  fill3(record.camera_axes[0], source_view.m_vp.CameraX());
  fill3(record.camera_axes[1], source_view.m_vp.CameraY());
  fill3(record.camera_axes[2], source_view.m_vp.CameraZ());
  const ON_3dPoint target = source_view.TargetPoint();
  record.has_target = static_cast<uint8_t>(target.IsValid());
  if (record.has_target) {
    fill3(record.target, target);
  }
  const ON_Plane& plane = source_view.m_cplane.m_plane;
  fill3(record.cplane_origin, plane.origin);
  fill3(record.cplane_x, plane.xaxis);
  fill3(record.cplane_y, plane.yaxis);
  return source_view.m_vp.GetFrustum(&record.frustum[0], &record.frustum[1],
                                    &record.frustum[2], &record.frustum[3],
                                    &record.frustum[4], &record.frustum[5]) &&
         source_view.m_vp.GetScreenPort(&record.screen_port[0], &record.screen_port[1],
                                        &record.screen_port[2], &record.screen_port[3]);
}

inline bool write_view_camera(ON_3dmView& view, const ViboNamedView& source) {
  if (source.name == nullptr || source.name[0] == '\0' ||
      (source.projection < 1 || source.projection > 3)) {
    return false;
  }
  view.m_name = ON_wString(source.name);
  ON_UUID viewport_id;
  ON_CreateUuid(viewport_id);
  if (!view.m_vp.SetViewportId(viewport_id) ||
      !view.m_vp.SetProjection(source.projection == 1 ? ON::parallel_view : ON::perspective_view) ||
      !view.m_vp.SetCameraLocation(ON_3dPoint(source.camera_location))) {
    return false;
  }
  // A valid final frame can pass through an invalid intermediate frame when
  // changing from the default camera orientation (for example, Front view).
  view.m_vp.SetCameraDirection(ON_3dVector(source.camera_direction));
  view.m_vp.SetCameraUp(ON_3dVector(source.camera_up));
  if (source.projection == 3) {
    view.m_vp.SetCameraUpLock(true);
    view.m_vp.SetFrustumLeftRightSymmetry(true);
    view.m_vp.SetFrustumTopBottomSymmetry(false);
  }
  if (source.has_camera_axes) {
    if (!view.m_vp.IsValidCamera()) return false;
    CachedViewport cached(view.m_vp);
    if (!cached.RestoreAxes(source.camera_axes)) return false;
    view.m_vp = cached;
  }
  if (!view.m_vp.IsValidCamera() ||
      !view.m_vp.SetScreenPort(source.screen_port[0], source.screen_port[1],
                               source.screen_port[2], source.screen_port[3]) ||
      !view.m_vp.SetFrustum(source.frustum[0], source.frustum[1],
                            source.frustum[2], source.frustum[3],
                            source.frustum[4], source.frustum[5]) ||
      (source.has_target && !view.SetTargetPoint(ON_3dPoint(source.target)))) {
    return false;
  }
  view.m_cplane.m_plane = ON_Plane(ON_3dPoint(source.cplane_origin),
                                  ON_3dVector(source.cplane_x),
                                  ON_3dVector(source.cplane_y));
  return view.m_cplane.m_plane.IsValid();
}

} // namespace vibo_camera
