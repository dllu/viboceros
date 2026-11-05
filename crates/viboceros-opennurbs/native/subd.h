#ifndef VIBOCEROS_SUBD_H
#define VIBOCEROS_SUBD_H
#include <stddef.h>
#include <stdint.h>
#include "mesh_modeling.h"
#ifdef __cplusplus
extern "C" {
#endif
typedef struct ViboSubDResult ViboSubDResult;
typedef struct ViboSubDIdTable ViboSubDIdTable;
typedef struct { size_t vertex_count,edge_count,face_count;const uint32_t *vertices,*edges,*faces; } ViboSubDIdView;
int vibo_subd_component_ids(const uint8_t*,size_t,ViboSubDIdTable**,char*,size_t);
int vibo_subd_component_ids_view(const ViboSubDIdTable*,ViboSubDIdView*);
void vibo_subd_component_ids_free(ViboSubDIdTable*);
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
  size_t mesh_ngon_count, mesh_ngon_vertex_count, mesh_ngon_face_count;
  size_t mesh_texture_count, mesh_normal_count;
  const ViboNgonSpan* mesh_ngons;
  const uint32_t *mesh_ngon_vertices, *mesh_ngon_faces;
  const double *mesh_texture_coordinates, *mesh_vertex_normals;
  size_t surface_jet_count, corner_jet_count;
  const double* surface_jets;
  const uint32_t* corner_jet_indices;
  int solid_orientation;
} ViboSubDView;
int vibo_subd_create(const double*, size_t, const uint32_t*, size_t,
  const uint32_t*, size_t, ViboSubDResult**, char*, size_t);
int vibo_subd_append(const uint8_t*, size_t, const double*, size_t, const uint32_t*, size_t,
  const uint32_t*, size_t, ViboSubDResult**, char*, size_t);
int vibo_subd_join_assemble(const uint8_t*, size_t, const size_t*, size_t,
  const uint32_t*, size_t, const uint8_t*, int, ViboSubDResult**, char*, size_t);
int vibo_subd_set_tags(const uint8_t*, size_t, int, const size_t*, const uint8_t*, size_t,
  ViboSubDResult**, char*, size_t);
int vibo_subd_crease(const uint8_t*, size_t, const size_t*, size_t, const size_t*, size_t,
  int, size_t*, ViboSubDResult**, char*, size_t);
int vibo_subd_remap(const uint8_t*, size_t, const size_t*, size_t, const size_t*, size_t,
  const size_t*, size_t, const uint32_t*, size_t, int, size_t*, ViboSubDResult**, char*, size_t);
int vibo_subd_insert_vertex(const uint8_t*, size_t, size_t, const double*, size_t*, size_t*, int*, ViboSubDResult**, char*, size_t);
int vibo_subd_insert_path(const uint8_t*, size_t, const size_t*, const double*, const double*, size_t, const uint32_t*, size_t, ViboSubDResult**, char*, size_t);
int vibo_subd_insert_face_edge(const uint8_t*, size_t, size_t, size_t, size_t, size_t*, int*, ViboSubDResult**, char*, size_t);
int vibo_subd_spin_edge(const uint8_t*, size_t, size_t, int, int*, ViboSubDResult**, char*, size_t);
int vibo_subd_soft_crease(const uint8_t*, size_t, const size_t*, const double*, size_t,
  size_t*, ViboSubDResult**, char*, size_t);
int vibo_subd_reverse_orientation(const uint8_t*, size_t, ViboSubDResult**, char*, size_t);
int vibo_subd_read(const uint8_t*, size_t, ViboSubDResult**, char*, size_t);
int vibo_subd_edit_points(const uint8_t*, size_t, const double*, size_t,
  ViboSubDResult**, char*, size_t);
int vibo_subd_transform(const uint8_t*, size_t, const double*,
  ViboSubDResult**, char*, size_t);
int vibo_subd_control_mesh(const uint8_t*, size_t, uint32_t, ViboSubDResult**, char*, size_t);
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
typedef struct ViboSubDFaceGrids ViboSubDFaceGrids;
typedef struct {
  size_t face, corner, side, offset, crease_corners;
} ViboSubDFaceGridInfo;
typedef struct {
  size_t grid_count, point_count;
  const ViboSubDFaceGridInfo* grids;
  const double* points;
} ViboSubDFaceGridsView;
int vibo_subd_face_grids(const uint8_t*,size_t,unsigned,int,ViboSubDFaceGrids**,char*,size_t);
int vibo_subd_face_grids_view(const ViboSubDFaceGrids*,ViboSubDFaceGridsView*);
void vibo_subd_face_grids_free(ViboSubDFaceGrids*);
typedef struct ViboSubDRegularPatches ViboSubDRegularPatches;
typedef struct {
  size_t patch_count;
  const size_t* faces;
  const double* controls;
} ViboSubDRegularPatchesView;
int vibo_subd_regular_patches(const uint8_t*,size_t,ViboSubDRegularPatches**,char*,size_t);
int vibo_subd_regular_patches_view(const ViboSubDRegularPatches*,ViboSubDRegularPatchesView*);
void vibo_subd_regular_patches_free(ViboSubDRegularPatches*);
typedef struct ViboSubDVertexNormals ViboSubDVertexNormals;
typedef struct ViboSubDFacePacks ViboSubDFacePacks;
typedef struct {uint32_t id,rotation,set;double rectangle[4];} ViboSubDFacePack;
typedef struct {size_t count;const ViboSubDFacePack* records;} ViboSubDFacePacksView;
int vibo_subd_set_face_packs(const uint8_t*,size_t,const ViboSubDFacePack*,size_t,ViboSubDResult**,char*,size_t);
int vibo_subd_face_packs(const uint8_t*,size_t,ViboSubDFacePacks**,char*,size_t);
int vibo_subd_face_packs_view(const ViboSubDFacePacks*,ViboSubDFacePacksView*);
void vibo_subd_face_packs_free(ViboSubDFacePacks*);
typedef struct {size_t count;const double* normals;} ViboSubDVertexNormalsView;
int vibo_subd_smooth_vertex_normals(const uint8_t*,size_t,const size_t*,size_t,ViboSubDVertexNormals**,char*,size_t);
int vibo_subd_vertex_normals_view(const ViboSubDVertexNormals*,ViboSubDVertexNormalsView*);
void vibo_subd_vertex_normals_free(ViboSubDVertexNormals*);
typedef struct ViboSubDChains ViboSubDChains;
typedef struct {
  size_t chain_count,edge_count;
  const size_t* offsets;
  const size_t* directed_edges;
} ViboSubDChainsView;
int vibo_subd_edge_chains(const uint8_t*,size_t,const size_t*,size_t,ViboSubDChains**,char*,size_t);
int vibo_subd_edge_loop(const uint8_t*,size_t,size_t,uint8_t,ViboSubDChains**,char*,size_t);
int vibo_subd_edge_chains_view(const ViboSubDChains*,ViboSubDChainsView*);
void vibo_subd_edge_chains_free(ViboSubDChains*);
int vibo_subd_sharpness_from_percent(double,double*,char*,size_t);
int vibo_subd_chain_sharpness(const double*,size_t,double*,char*,size_t);
// Per interval: two endpoint subdivision counts, then k/32 break bits (1<=k<32).
int vibo_subd_sharpness_refinement(const double*,size_t,uint32_t*,char*,size_t);
int vibo_subd_stored_sharpness(const uint8_t*,size_t,const size_t*,const double*,size_t,
  size_t*,ViboSubDResult**,char*,size_t);
int vibo_subd_offset(const uint8_t*,size_t,double,int,ViboSubDResult**,char*,size_t);
int vibo_subd_retain_faces(const uint8_t*,size_t,const size_t*,size_t,const size_t*,size_t,ViboSubDResult**,char*,size_t);
#ifdef __cplusplus
}
#endif
#endif
