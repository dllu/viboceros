#ifndef VIBOCEROS_SUBD_H
#define VIBOCEROS_SUBD_H
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
typedef struct ViboSubDResult ViboSubDResult;
typedef struct {
  size_t archive_count, point_count, edge_count, face_count, corner_count;
  const uint8_t *archive;
  const double *points, *surface_points;
  const uint8_t *vertex_tags;
  const uint32_t *edges;
  const uint8_t *edge_tags;
  const uint32_t *face_sizes, *face_vertices;
} ViboSubDView;
int vibo_subd_create(const double*, size_t, const uint32_t*, size_t,
  const uint32_t*, size_t, ViboSubDResult**, char*, size_t);
int vibo_subd_read(const uint8_t*, size_t, ViboSubDResult**, char*, size_t);
int vibo_subd_edit_points(const uint8_t*, size_t, const double*, size_t,
  ViboSubDResult**, char*, size_t);
int vibo_subd_transform(const uint8_t*, size_t, const double*,
  ViboSubDResult**, char*, size_t);
int vibo_subd_tessellate(const uint8_t*, size_t, unsigned,
  ViboSubDResult**, char*, size_t);
int vibo_subd_view(const ViboSubDResult*, ViboSubDView*);
void vibo_subd_free(ViboSubDResult*);
#ifdef __cplusplus
}
#endif
#endif
