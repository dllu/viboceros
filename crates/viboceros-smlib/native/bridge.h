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
typedef struct VbParts VbParts;
int vb_nurbs_insert_knot(const VbNurbsData *source, int axis, double parameter, size_t target,
                        VbSurfaceData **out, char *error, size_t capacity);
int vb_nurbs_change_degree(const VbNurbsData *source, int axis, size_t degree, int deformable,
                          const double *knots, size_t knot_count, VbSurfaceData **out, char *error, size_t capacity);
int vb_nurbs_fit_basis(const VbNurbsData *source, int axis, const double *knots, size_t knot_count,
                       VbSurfaceData **out, char *error, size_t capacity);
int vb_nurbs_bezier(const VbNurbsData *source,int direction,VbSurfaceData **out,size_t count,int *control_error,
                    char *error,size_t capacity);
int vb_curves_edge_surface(const VbCurve *const *curves, size_t count, double tolerance,
                          VbSurfaceData **out, char *error, size_t capacity);
int vb_shape_edge_surface(const VbCurve *const *curves, size_t count, double tolerance,
                         VbSolid **out, char *error, size_t capacity);
int vb_solid_parts(const VbSolid *solid, VbParts **out, char *error, size_t capacity);
size_t vb_parts_count(const VbParts *parts);
VbSolid *vb_parts_take(VbParts *parts, size_t index);
void vb_parts_free(VbParts *parts);
int vb_solid_primitive(int kind, const double origin[3], const double size[3], VbSolid **out,
                       char *error, size_t capacity);
int vb_solid_boolean(const VbSolid *a, const VbSolid *b, int operation, int options, VbSolid **out, char *error,
                     size_t capacity);
int vb_solid_properties(const VbSolid *solid, double accuracy, double *volume, double bounds[6],
                        int *manifold, char *error, size_t capacity);
int vb_solid_volume(const VbSolid *solid, double accuracy, double *volume, char *error,
                    size_t capacity);
void vb_solid_free(VbSolid *solid);
int vb_solid_copy(const VbSolid *solid, VbSolid **out, char *error, size_t capacity);
int vb_solid_empty(const VbSolid *solid, int *empty, char *error, size_t capacity);
int vb_solid_boundary_contact(const VbSolid *a, const VbSolid *b, double tolerance, int *contact,
                              char *error, size_t capacity);
int vb_solid_census(const VbSolid *solid, size_t counts[2], char *error, size_t capacity);
int vb_solid_brep(const VbSolid *solid, VbBrep **out, char *error, size_t capacity);
int vb_sheet_from_brep(const VbBrepView *view, double tolerance, const size_t *components,
                       size_t component_count, VbSolid **out, char *error, size_t capacity);
int vb_shape_offset(const VbSolid *shape, double distance, int wall, double accuracy,
                     VbSolid **out, char *error, size_t capacity);
int vb_shape_brep(const VbSolid *shape, double approximation, VbBrep **out,
                  char *error, size_t capacity);
int vb_shape_stitch(const VbSolid *shape, double distance, size_t *face_origins,
                    size_t face_capacity, size_t *face_count, size_t *stitched_edges,
                    double gaps[2], VbSolid **out, char *error, size_t capacity);
int vb_shape_face_count(const VbSolid *shape, size_t *count, char *error, size_t capacity);
int vb_shape_bounds(const VbSolid *shape, double bounds[6], char *error, size_t capacity);
int vb_solid_from_brep(const VbBrepView *view, double tolerance, const size_t *components,
                       size_t component_count, const int *inward, VbSolid **out, char *error,
                       size_t capacity);
int vb_solid_from_brep_plan(const VbBrepView *view, double tolerance, const size_t *components,
                            size_t component_count, const int *inward, size_t *parents,
                            size_t parent_capacity, VbSolid **out, char *error, size_t capacity);
int vb_solid_mesh(const VbSolid *solid, const double quality[3], VbMesh **out, char *error,
                  size_t capacity);
int vb_imported_solid_fillet_edges(VbSolid *solid, const size_t *indices, size_t count,
                                   double radius, double accuracy, char *error, size_t capacity);
int vb_solid_validate_boundary(const VbSolid *solid, double tolerance, char *error, size_t capacity);
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
int vb_curve_extrude(const VbCurve *curve, const double displacement[3], VbSurfaceData **out,
                     char *error, size_t capacity);
int vb_curves_plane_normal(const VbCurve *const *curves, size_t count, double tolerance, int *has_plane, double normal[3],
                           char *error, size_t capacity);
int vb_curve_translational_sweep(const VbCurve *profile, const VbCurve *path, VbSurfaceData **out,
                                 char *error, size_t capacity);
int vb_shape_translational_sweep(const VbCurve *profile, const VbCurve *path, double tolerance,
                                 VbSolid **out, char *error, size_t capacity);
int vb_curve_apex_surface(const VbCurve *curve, const double apex[3], VbSurfaceData **out,
                          char *error, size_t capacity);
int vb_shape_apex_solid(const VbCurve *curve, const double apex[3], double tolerance,
                        VbSolid **out, char *error, size_t capacity);
int vb_curve_revolve(const VbCurve *curve, const double origin[3], const double axis[3],
                     double angle_degrees, double parameter_length, VbSurfaceData **out,
                     char *error, size_t capacity);
int vb_shape_planar_faces(const VbCurve *const *curves, size_t count, double tolerance,
                          VbSolid **out, char *error, size_t capacity);
int vb_curves_coplanar(const VbCurve *const *curves, size_t count, double tolerance,
                       int *coplanar, char *error, size_t capacity);
int vb_shape_linear_sweep(const VbCurve *const *curves, size_t count,
                          const double displacement[3], int cap_ends, double tolerance,
                          VbSolid **out, char *error, size_t capacity);
#ifdef __cplusplus
}
#endif
#endif
