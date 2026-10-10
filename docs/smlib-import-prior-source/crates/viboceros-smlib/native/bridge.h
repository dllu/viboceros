#ifndef VIBOCEROS_SMLIB_H
#define VIBOCEROS_SMLIB_H
#include "brep_data.h"
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
typedef struct VbSolid VbSolid;
typedef struct VbCurve VbCurve;
typedef struct VbMesh VbMesh;
int vb_solid_primitive(int kind, const double origin[3], const double size[3], VbSolid **out,
                       char *error, size_t capacity);
int vb_solid_boolean(const VbSolid *a, const VbSolid *b, int operation, VbSolid **out, char *error,
                     size_t capacity);
int vb_solid_properties(const VbSolid *solid, double accuracy, double *volume, double bounds[6],
                        int *manifold, char *error, size_t capacity);
void vb_solid_free(VbSolid *solid);
int vb_solid_brep(const VbSolid *solid, VbBrep **out, char *error, size_t capacity);
int vb_solid_mesh(const VbSolid *solid, const double quality[3], VbMesh **out, char *error,
                  size_t capacity);
int vb_mesh_sizes(const VbMesh *mesh, size_t *vertices, size_t *triangles);
int vb_mesh_copy(const VbMesh *mesh, double *vertices, size_t vertex_count, uint32_t *triangles,
                 size_t triangle_count);
void vb_mesh_free(VbMesh *mesh);
int vb_curve_create(size_t degree, const double *points_xyzw, size_t point_count,
                    const double *knots, size_t knot_count, VbCurve **out, char *error,
                    size_t capacity);
int vb_curve_evaluate(const VbCurve *curve, double parameter, double point[3], char *error,
                      size_t capacity);
int vb_curve_sizes(const VbCurve *curve, size_t *degree, size_t *points, size_t *knots);
int vb_curve_copy(const VbCurve *curve, double *points_xyzw, size_t point_count, double *knots,
                  size_t knot_count);
void vb_curve_free(VbCurve *curve);
#ifdef __cplusplus
}
#endif
#endif
