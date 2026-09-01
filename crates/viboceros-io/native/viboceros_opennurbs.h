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
};

typedef struct ViboThreeDmModel ViboThreeDmModel;

typedef struct ViboObjectInfo {
  int32_t object_type;
  int32_t source_layer_index;
  const char* name;
  uint8_t visible;
  uint8_t locked;
  uint32_t degree_u;
  uint32_t degree_v;
  size_t control_point_count_u;
  size_t control_point_count_v;
  size_t coordinate_count;
  size_t knot_u_count;
  size_t knot_v_count;
  size_t index_count;
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

typedef struct ViboWriteObject {
  int32_t object_type;
  size_t layer_index;
  const char* name;
  uint8_t visible;
  uint8_t locked;
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
  const size_t* group_indices;
  size_t group_index_count;
} ViboWriteObject;

int32_t vibo_3dm_read(const char* path, ViboThreeDmModel** output,
                      char* error, size_t error_capacity);
void vibo_3dm_free(ViboThreeDmModel* model);

size_t vibo_3dm_layer_count(const ViboThreeDmModel* model);
int32_t vibo_3dm_layer(const ViboThreeDmModel* model, size_t index,
                       int32_t* source_index, const char** name, uint8_t* red,
                       uint8_t* green, uint8_t* blue, uint8_t* visible,
                       uint8_t* locked);

size_t vibo_3dm_group_count(const ViboThreeDmModel* model);
int32_t vibo_3dm_group(const ViboThreeDmModel* model, size_t index,
                       int32_t* source_index, const char** name);

size_t vibo_3dm_object_count(const ViboThreeDmModel* model);
size_t vibo_3dm_unsupported_object_count(const ViboThreeDmModel* model);
int32_t vibo_3dm_object(const ViboThreeDmModel* model, size_t index,
                        ViboObjectInfo* info, const double** coordinates,
                        const double** knots_u, const double** knots_v,
                        const uint32_t** indices,
                        const int32_t** group_indices);

int32_t vibo_3dm_write(const char* path, const ViboWriteLayer* layers,
                       size_t layer_count, const ViboWriteGroup* groups,
                       size_t group_count, const ViboWriteObject* objects,
                       size_t object_count, char* error,
                       size_t error_capacity);

#ifdef __cplusplus
}
#endif

#endif
