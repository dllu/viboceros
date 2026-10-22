#ifndef VIBOCEROS_SUBD_H
#define VIBOCEROS_SUBD_H
#include <stddef.h>
#include <stdint.h>
#include "mesh_modeling.h"
#ifdef __cplusplus
extern "C" {
#endif
typedef struct ViboSubDResult ViboSubDResult;
int vibo_subd_from_mesh(const ViboMeshView*, int, int, ViboSubDResult**, char*, size_t);
int vibo_subd_box(const double*, const uint32_t*, ViboSubDResult**, char*, size_t);
int vibo_subd_cylinder(const double*, double, double, const uint32_t*, uint32_t, ViboSubDResult**, char*, size_t);
typedef struct ViboSubDWeights ViboSubDWeights;
typedef struct {
  size_t vertex_count, coefficient_count;
  const size_t* row_offsets;
  const uint32_t* columns;
  const double* values;
} ViboSubDWeightsView;
int vibo_subd_weights(const uint8_t*, size_t, ViboSubDWeights**, char*, size_t);
int vibo_subd_weights_view(const ViboSubDWeights*, ViboSubDWeightsView*);
void vibo_subd_weights_free(ViboSubDWeights*);
typedef struct {
  size_t archive_count, point_count, edge_count, face_count, corner_count, normal_count;
  const uint8_t *archive;
  const double *points, *surface_points;
  const uint8_t *vertex_tags;
  const uint32_t *edges;
  const uint8_t *edge_tags;
  const uint32_t *face_sizes, *face_vertices;
  const double *corner_normals;
  size_t boundary_edge_count, face_source_count;
  const uint32_t* face_sources;
  size_t selected_face_count;
  const uint32_t* selected_faces;
  size_t control_edge_count;
  const double* edge_sharpness;
  const uint32_t* edge_face_counts;
} ViboSubDView;
int vibo_subd_create(const double*, size_t, const uint32_t*, size_t,
  const uint32_t*, size_t, ViboSubDResult**, char*, size_t);
int vibo_subd_append(const uint8_t*, size_t, const double*, size_t, const uint32_t*, size_t,
  const uint32_t*, size_t, ViboSubDResult**, char*, size_t);
int vibo_subd_set_tags(const uint8_t*, size_t, int, const size_t*, const uint8_t*, size_t,
  ViboSubDResult**, char*, size_t);
int vibo_subd_crease(const uint8_t*, size_t, const size_t*, size_t, const size_t*, size_t,
  int, size_t*, ViboSubDResult**, char*, size_t);
int vibo_subd_soft_crease(const uint8_t*, size_t, const size_t*, const double*, size_t,
  size_t*, ViboSubDResult**, char*, size_t);
int vibo_subd_read(const uint8_t*, size_t, ViboSubDResult**, char*, size_t);
int vibo_subd_edit_points(const uint8_t*, size_t, const double*, size_t,
  ViboSubDResult**, char*, size_t);
int vibo_subd_transform(const uint8_t*, size_t, const double*,
  ViboSubDResult**, char*, size_t);
int vibo_subd_control_mesh(const uint8_t*, size_t, ViboSubDResult**, char*, size_t);
int vibo_subd_tessellate(const uint8_t*, size_t, unsigned, int,
  ViboSubDResult**, char*, size_t);
int vibo_subd_subdivide(const uint8_t*, size_t, unsigned, const size_t*, size_t, int,
  ViboSubDResult**, char*, size_t);
int vibo_subd_view(const ViboSubDResult*, ViboSubDView*);
void vibo_subd_free(ViboSubDResult*);
typedef struct ViboSubDEdgeSamples ViboSubDEdgeSamples;
typedef struct {
  size_t edge_count, point_count;
  const size_t* offsets;
  const double* points;
} ViboSubDEdgeSamplesView;
int vibo_subd_edge_samples(const uint8_t*, size_t, unsigned, ViboSubDEdgeSamples**, char*, size_t);
int vibo_subd_edge_samples_view(const ViboSubDEdgeSamples*, ViboSubDEdgeSamplesView*);
void vibo_subd_edge_samples_free(ViboSubDEdgeSamples*);
#ifdef __cplusplus
}
#endif
#endif
