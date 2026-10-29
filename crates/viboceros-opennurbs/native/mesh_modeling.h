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
  size_t texture_count, normal_count;
  const double *texture_coordinates, *vertex_normals;
} ViboMeshView;
typedef struct ViboMeshResult ViboMeshResult;
// kind: 0 raw vertex, 1 edge identified by raw endpoints, 2 stored face.
typedef struct { uint32_t kind, first, second; } ViboMeshComponent;
int vibo_mesh_merge_components(const ViboMeshView*, const ViboMeshComponent*, size_t, ViboMeshResult**, char*, size_t);
int vibo_mesh_edit(const ViboMeshView*,uint32_t,double,double,ViboMeshResult**,char*,size_t);
int vibo_mesh_merge_selected(const ViboMeshView*, const size_t*, size_t, int, double, double, ViboMeshResult**, char*, size_t);
int vibo_mesh_dissolve_vertices(const ViboMeshView*, const size_t*, size_t, ViboMeshResult**, char*, size_t);
int vibo_mesh_result_view(const ViboMeshResult*,ViboMeshView*,size_t*);
int vibo_mesh_cleanup_counts(const ViboMeshResult*,size_t*,size_t*,size_t*);
int vibo_quad_planarity(const double*, double*, double*, char*, size_t);
void vibo_mesh_result_free(ViboMeshResult*);
#ifdef __cplusplus
}
class ON_Mesh;
// Shared owning result encoder for public mesh-producing SDK operations.
ViboMeshResult* vibo_mesh_result_from_mesh(const ON_Mesh&);
#endif
#endif
