#ifndef VIBOCEROS_OPENNURBS_H
#define VIBOCEROS_OPENNURBS_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

enum ViboObjectType {
  VIBO_OBJECT_POINT = 1,
  VIBO_OBJECT_LINE = 2,
  VIBO_OBJECT_NURBS_CURVE = 3,
  VIBO_OBJECT_TRIANGLE_MESH = 4,
  VIBO_OBJECT_NURBS_SURFACE = 5,
  VIBO_OBJECT_POINT_CLOUD = 6,
  VIBO_OBJECT_BREP = 7,
  VIBO_OBJECT_POLYCURVE = 8,
  VIBO_OBJECT_POLYLINE = 9,
  VIBO_OBJECT_ARC = 10,
};

typedef struct ViboThreeDmModel ViboThreeDmModel;

typedef struct ViboObjectInfo {
  int32_t object_type;
  int32_t source_layer_index;
  const char* name;
  uint8_t visible;
  uint8_t locked;
  uint8_t color_source;
  uint8_t color_red;
  uint8_t color_green;
  uint8_t color_blue;
  int32_t wire_density;
  uint32_t degree_u;
  uint32_t degree_v;
  size_t control_point_count_u;
  size_t control_point_count_v;
  size_t coordinate_count;
  size_t knot_u_count;
  size_t knot_v_count;
  size_t index_count;
  size_t geometry_data_count;
  size_t group_index_count;
} ViboObjectInfo;

typedef struct ViboWriteLayer {
  const char* name;
  uint8_t red;
  uint8_t green;
  uint8_t blue;
  uint8_t visible;
  uint8_t locked;
} ViboWriteLayer;

typedef struct ViboWriteGroup {
  const char* name;
} ViboWriteGroup;

typedef struct ViboNamedView {
  const char* name;
  uint8_t projection; // 1 parallel, 2 perspective
  uint8_t has_target;
  double camera_location[3];
  double camera_direction[3];
  double camera_up[3];
  double target[3];
  double cplane_origin[3];
  double cplane_x[3];
  double cplane_y[3];
  double frustum[6]; // left, right, bottom, top, near, far
  int32_t screen_port[4]; // left, right, bottom, top
} ViboNamedView;

typedef struct ViboUserText {
  const char* key;
  const char* value;
} ViboUserText;

typedef struct ViboWriteObject {
  int32_t object_type;
  size_t layer_index;
  const char* name;
  uint8_t visible;
  uint8_t locked;
  uint8_t color_source;
  uint8_t color_red;
  uint8_t color_green;
  uint8_t color_blue;
  int32_t wire_density;
  uint32_t degree_u;
  uint32_t degree_v;
  size_t control_point_count_u;
  size_t control_point_count_v;
  const double* coordinates;
  size_t coordinate_count;
  const double* knots_u;
  size_t knot_u_count;
  const double* knots_v;
  size_t knot_v_count;
  const uint32_t* indices;
  size_t index_count;
  const uint8_t* geometry_data;
  size_t geometry_data_count;
  const size_t* group_indices;
  size_t group_index_count;
  const ViboUserText* user_text;
  size_t user_text_count;
  const ViboUserText* geometry_user_text;
  size_t geometry_user_text_count;
} ViboWriteObject;

int32_t vibo_3dm_read(const char* path, ViboThreeDmModel** output,
                      char* error, size_t error_capacity);
void vibo_3dm_free(ViboThreeDmModel* model);
int32_t vibo_3dm_units(const ViboThreeDmModel* model, uint32_t* unit_system,
                      double* meters_per_unit, const char** name);

size_t vibo_3dm_layer_count(const ViboThreeDmModel* model);
int32_t vibo_3dm_layer(const ViboThreeDmModel* model, size_t index,
                       int32_t* source_index, const char** name, uint8_t* red,
                       uint8_t* green, uint8_t* blue, uint8_t* visible,
                       uint8_t* locked);

size_t vibo_3dm_group_count(const ViboThreeDmModel* model);
int32_t vibo_3dm_group(const ViboThreeDmModel* model, size_t index,
                       int32_t* source_index, const char** name);
size_t vibo_3dm_named_view_count(const ViboThreeDmModel* model);
int32_t vibo_3dm_named_view(const ViboThreeDmModel* model, size_t index,
                            ViboNamedView* view);

size_t vibo_3dm_object_count(const ViboThreeDmModel* model);
size_t vibo_3dm_unsupported_object_count(const ViboThreeDmModel* model);
int32_t vibo_3dm_object(const ViboThreeDmModel* model, size_t index,
                        ViboObjectInfo* info, const double** coordinates,
                        const double** knots_u, const double** knots_v,
                        const uint32_t** indices,
                        const uint8_t** geometry_data,
                        const int32_t** group_indices);
size_t vibo_3dm_object_user_text_count(const ViboThreeDmModel* model,
                                       size_t index);
int32_t vibo_3dm_object_user_text(const ViboThreeDmModel* model,
                                  size_t index, size_t text_index,
                                  const char** key, const char** value);
size_t vibo_3dm_object_geometry_user_text_count(const ViboThreeDmModel* model,
                                                size_t index);
int32_t vibo_3dm_object_geometry_user_text(const ViboThreeDmModel* model,
                                           size_t index, size_t text_index,
                                           const char** key, const char** value);

int32_t vibo_3dm_write(const char* path, uint32_t unit_system,
                       double meters_per_unit, const char* unit_name,
                       double absolute_tolerance, double relative_tolerance,
                       double angle_tolerance,
                       const ViboWriteLayer* layers,
                       size_t layer_count, const ViboWriteGroup* groups,
                       size_t group_count, const ViboNamedView* named_views,
                       size_t named_view_count, const ViboWriteObject* objects,
                       size_t object_count, char* error,
                       size_t error_capacity);

int32_t vibo_3dm_tolerances(const ViboThreeDmModel* model,
                          double* absolute, double* relative, double* angle);

#ifdef __cplusplus
}
#endif

#endif
