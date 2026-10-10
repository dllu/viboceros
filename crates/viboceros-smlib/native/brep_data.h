#ifndef VIBOCEROS_SMLIB_BREP_DATA_H
#define VIBOCEROS_SMLIB_BREP_DATA_H
#include <stddef.h>
#ifdef __cplusplus
extern "C" {
#endif
typedef struct VbBrep VbBrep;
typedef struct {
    size_t degree[2], count[2], knot_count[2];
    const double *knots[2];
    const double *points_xyzw;
} VbNurbsData;
typedef struct {
    double point[3], tolerance;
} VbVertexData;
typedef struct {
    size_t vertices[2];
    double interval[2], tolerance;
    VbNurbsData curve;
} VbEdgeData;
typedef struct {
    size_t vertices[2], edge;
    int reversed;
    VbNurbsData curve;
} VbTrimData;
typedef struct {
    size_t first_trim, trim_count;
    int inner;
} VbLoopData;
typedef struct {
    VbNurbsData surface;
    size_t first_loop, loop_count;
    int reversed;
} VbFaceData;
typedef struct {
    size_t vertex_count, edge_count, trim_count, loop_count, face_count;
    const VbVertexData *vertices;
    const VbEdgeData *edges;
    const VbTrimData *trims;
    const VbLoopData *loops;
    const VbFaceData *faces;
} VbBrepView;
int vb_brep_view(const VbBrep *brep, VbBrepView *view);
void vb_brep_free(VbBrep *brep);
#ifdef __cplusplus
}
class SmBrep;
class SmEdge;
#include <vector>
VbBrep *vb_export_brep(const SmBrep &solid);
SmBrep *vb_import_brep(const VbBrepView &view, double tolerance, const size_t *components,
                       size_t component_count, const int *inward,
                       std::vector<size_t> *parents = nullptr,
                       std::vector<SmEdge *> *source_edges = nullptr);
#endif
#endif
