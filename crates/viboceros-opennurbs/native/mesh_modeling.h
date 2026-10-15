#ifndef VIBOCEROS_OPENNURBS_MESH_MODELING_H
#define VIBOCEROS_OPENNURBS_MESH_MODELING_H
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
typedef struct { size_t first_vertex, vertex_count, first_face, face_count; } ViboNgonSpan;
typedef struct {
  size_t vertex_count, face_count, color_count, ngon_count, ngon_vertex_count, ngon_face_count;
  const double *vertices;
  const uint32_t *faces;
  const uint8_t *colors;
  const ViboNgonSpan *ngons;
  const uint32_t *ngon_vertices, *ngon_faces;
} ViboMeshView;
typedef struct ViboMeshResult ViboMeshResult;
int vibo_mesh_edit(const ViboMeshView*,uint32_t,double,double,ViboMeshResult**,char*,size_t);
int vibo_mesh_merge_selected(const ViboMeshView*, const size_t*, size_t, int, double, double, ViboMeshResult**, char*, size_t);
int vibo_mesh_result_view(const ViboMeshResult*,ViboMeshView*,size_t*);
void vibo_mesh_result_free(ViboMeshResult*);
#ifdef __cplusplus
}
#endif
#endif
