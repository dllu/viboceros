#include "viboceros_opennurbs.h"

#include "opennurbs_public.h"

#include <algorithm>
#include <cmath>
#include <cstring>
#include <exception>
#include <limits>
#include <memory>
#include <mutex>
#include <new>
#include <string>
#include <utility>
#include <vector>

namespace {

struct BridgeLayer {
  int32_t source_index = 0;
  std::string name;
  uint8_t red = 0;
  uint8_t green = 0;
  uint8_t blue = 0;
  uint8_t visible = 1;
  uint8_t locked = 0;
};

struct BridgeGroup {
  int32_t source_index = 0;
  std::string name;
};

struct BridgeObject {
  int32_t object_type = 0;
  int32_t source_layer_index = 0;
  std::string name;
  uint8_t visible = 1;
  uint8_t locked = 0;
  uint32_t degree_u = 0;
  uint32_t degree_v = 0;
  size_t control_point_count_u = 0;
  size_t control_point_count_v = 0;
  std::vector<double> coordinates;
  std::vector<double> knots_u;
  std::vector<double> knots_v;
  std::vector<uint32_t> indices;
  std::vector<int32_t> group_indices;
};

std::once_flag g_open_nurbs_once;

void begin_open_nurbs() {
  std::call_once(g_open_nurbs_once, [] { ON::Begin(); });
}

std::string utf8(const ON_wString& value) {
  const ON_String converted(value);
  const char* text = static_cast<const char*>(converted);
  return text == nullptr ? std::string() : std::string(text);
}

void set_error(char* destination, size_t capacity, const std::string& message) {
  if (destination == nullptr || capacity == 0) {
    return;
  }
  const size_t count = std::min(capacity - 1, message.size());
  std::memcpy(destination, message.data(), count);
  destination[count] = '\0';
}

bool finite_coordinates(const double* values, size_t count) {
  if (count != 0 && values == nullptr) {
    return false;
  }
  for (size_t index = 0; index < count; ++index) {
    if (!std::isfinite(values[index])) {
      return false;
    }
  }
  return true;
}

bool append_point(const ON_Point& point, BridgeObject& output) {
  const ON_3dPoint value = point.point;
  if (!value.IsValid()) {
    return false;
  }
  output.object_type = VIBO_OBJECT_POINT;
  output.coordinates = {value.x, value.y, value.z};
  return true;
}

bool append_point_cloud(const ON_PointCloud& cloud, BridgeObject& output) {
  if (!cloud.IsValid() || cloud.PointCount() <= 0) {
    return false;
  }
  output.object_type = VIBO_OBJECT_POINT_CLOUD;
  output.coordinates.reserve(static_cast<size_t>(cloud.PointCount()) * 3);
  for (int index = 0; index < cloud.PointCount(); ++index) {
    const ON_3dPoint point = cloud[index];
    if (!point.IsValid()) {
      return false;
    }
    output.coordinates.insert(output.coordinates.end(),
                              {point.x, point.y, point.z});
  }
  return true;
}

bool append_line(const ON_LineCurve& line, BridgeObject& output) {
  const ON_3dPoint start = line.PointAtStart();
  const ON_3dPoint end = line.PointAtEnd();
  if (!start.IsValid() || !end.IsValid() || start == end) {
    return false;
  }
  output.object_type = VIBO_OBJECT_LINE;
  output.coordinates = {start.x, start.y, start.z, end.x, end.y, end.z};
  return true;
}

bool append_nurbs(const ON_Curve& source, BridgeObject& output) {
  ON_NurbsCurve curve;
  if (source.GetNurbForm(curve) <= 0 || !curve.IsValid() || curve.Order() < 2 ||
      curve.CVCount() < curve.Order()) {
    return false;
  }

  output.object_type = VIBO_OBJECT_NURBS_CURVE;
  output.degree_u = static_cast<uint32_t>(curve.Order() - 1);
  output.control_point_count_u = static_cast<size_t>(curve.CVCount());
  output.coordinates.reserve(output.control_point_count_u * 4);
  for (int index = 0; index < curve.CVCount(); ++index) {
    ON_3dPoint point;
    const double weight = curve.Weight(index);
    if (!curve.GetCV(index, point) || !point.IsValid() ||
        !std::isfinite(weight) || weight <= 0.0) {
      return false;
    }
    output.coordinates.insert(output.coordinates.end(),
                              {point.x, point.y, point.z, weight});
  }

  output.knots_u.reserve(static_cast<size_t>(curve.KnotCount()) + 2);
  output.knots_u.push_back(curve.SuperfluousKnot(0));
  for (int index = 0; index < curve.KnotCount(); ++index) {
    output.knots_u.push_back(curve.Knot(index));
  }
  output.knots_u.push_back(curve.SuperfluousKnot(1));
  return std::all_of(output.knots_u.begin(), output.knots_u.end(),
                     [](double knot) { return std::isfinite(knot); });
}

bool append_nurbs_surface(const ON_Surface& source, BridgeObject& output) {
  ON_NurbsSurface surface;
  if (source.GetNurbForm(surface) <= 0 || !surface.IsValid() ||
      surface.Order(0) < 2 || surface.Order(1) < 2 ||
      surface.CVCount(0) < surface.Order(0) ||
      surface.CVCount(1) < surface.Order(1)) {
    return false;
  }

  output.object_type = VIBO_OBJECT_NURBS_SURFACE;
  output.degree_u = static_cast<uint32_t>(surface.Order(0) - 1);
  output.degree_v = static_cast<uint32_t>(surface.Order(1) - 1);
  output.control_point_count_u = static_cast<size_t>(surface.CVCount(0));
  output.control_point_count_v = static_cast<size_t>(surface.CVCount(1));
  if (output.control_point_count_u >
      std::numeric_limits<size_t>::max() / output.control_point_count_v / 4) {
    return false;
  }
  output.coordinates.reserve(output.control_point_count_u *
                             output.control_point_count_v * 4);
  for (int v = 0; v < surface.CVCount(1); ++v) {
    for (int u = 0; u < surface.CVCount(0); ++u) {
      ON_3dPoint point;
      const double weight = surface.Weight(u, v);
      if (!surface.GetCV(u, v, point) || !point.IsValid() ||
          !std::isfinite(weight) || weight <= 0.0) {
        return false;
      }
      output.coordinates.insert(output.coordinates.end(),
                                {point.x, point.y, point.z, weight});
    }
  }

  for (int direction = 0; direction < 2; ++direction) {
    std::vector<double>& knots =
        direction == 0 ? output.knots_u : output.knots_v;
    knots.reserve(static_cast<size_t>(surface.KnotCount(direction)) + 2);
    knots.push_back(surface.SuperfluousKnot(direction, 0));
    for (int index = 0; index < surface.KnotCount(direction); ++index) {
      knots.push_back(surface.Knot(direction, index));
    }
    knots.push_back(surface.SuperfluousKnot(direction, 1));
    if (!std::all_of(knots.begin(), knots.end(),
                     [](double knot) { return std::isfinite(knot); })) {
      return false;
    }
  }
  return true;
}

bool append_mesh(const ON_Mesh& mesh, BridgeObject& output) {
  if (!mesh.IsValid() || mesh.VertexCount() <= 0 || mesh.FaceCount() <= 0) {
    return false;
  }
  if (static_cast<uint64_t>(mesh.VertexCount()) >
      std::numeric_limits<uint32_t>::max()) {
    return false;
  }

  output.object_type = VIBO_OBJECT_TRIANGLE_MESH;
  output.coordinates.reserve(static_cast<size_t>(mesh.VertexCount()) * 3);
  for (int index = 0; index < mesh.VertexCount(); ++index) {
    const ON_3dPoint point = mesh.Vertex(index);
    if (!point.IsValid()) {
      return false;
    }
    output.coordinates.insert(output.coordinates.end(),
                              {point.x, point.y, point.z});
  }

  output.indices.reserve(static_cast<size_t>(mesh.FaceCount()) * 6);
  for (int index = 0; index < mesh.FaceCount(); ++index) {
    const ON_MeshFace& face = mesh.m_F[index];
    const int vertex_count = face.IsTriangle() ? 3 : 4;
    for (int vertex = 0; vertex < vertex_count; ++vertex) {
      if (face.vi[vertex] < 0 || face.vi[vertex] >= mesh.VertexCount()) {
        return false;
      }
    }
    output.indices.insert(output.indices.end(),
                          {static_cast<uint32_t>(face.vi[0]),
                           static_cast<uint32_t>(face.vi[1]),
                           static_cast<uint32_t>(face.vi[2])});
    if (face.IsQuad()) {
      output.indices.insert(output.indices.end(),
                            {static_cast<uint32_t>(face.vi[0]),
                             static_cast<uint32_t>(face.vi[2]),
                             static_cast<uint32_t>(face.vi[3])});
    }
  }
  return true;
}

ON_3dmObjectAttributes* attributes_for(const ViboWriteObject& source,
                                       int layer_index) {
  auto* attributes = new ON_3dmObjectAttributes();
  attributes->m_layer_index = layer_index;
  if (source.name != nullptr && source.name[0] != '\0') {
    const ON_wString name(source.name);
    if (!attributes->SetName(name, true)) {
      delete attributes;
      return nullptr;
    }
  }
  if (source.visible == 0) {
    attributes->SetMode(ON::hidden_object);
  } else if (source.locked != 0) {
    attributes->SetMode(ON::locked_object);
  } else {
    attributes->SetMode(ON::normal_object);
  }
  return attributes;
}

ON_Object* geometry_for(const ViboWriteObject& source, std::string& error) {
  if (!finite_coordinates(source.coordinates, source.coordinate_count) ||
      !finite_coordinates(source.knots_u, source.knot_u_count) ||
      !finite_coordinates(source.knots_v, source.knot_v_count)) {
    error = "geometry contains a null array or non-finite number";
    return nullptr;
  }

  switch (source.object_type) {
    case VIBO_OBJECT_POINT: {
      if (source.coordinate_count != 3) {
        error = "a point must contain three coordinates";
        return nullptr;
      }
      return new ON_Point(source.coordinates[0], source.coordinates[1],
                          source.coordinates[2]);
    }
    case VIBO_OBJECT_LINE: {
      if (source.coordinate_count != 6) {
        error = "a line must contain six coordinates";
        return nullptr;
      }
      const ON_3dPoint start(source.coordinates[0], source.coordinates[1],
                            source.coordinates[2]);
      const ON_3dPoint end(source.coordinates[3], source.coordinates[4],
                          source.coordinates[5]);
      auto* line = new ON_LineCurve(start, end);
      if (!line->IsValid()) {
        delete line;
        error = "line endpoints are degenerate";
        return nullptr;
      }
      return line;
    }
    case VIBO_OBJECT_POINT_CLOUD: {
      if (source.coordinate_count == 0 || source.coordinate_count % 3 != 0 ||
          source.coordinate_count / 3 >
              static_cast<size_t>(std::numeric_limits<int>::max())) {
        error = "point cloud dimensions are inconsistent";
        return nullptr;
      }
      const size_t point_count = source.coordinate_count / 3;
      auto* cloud = new ON_PointCloud(static_cast<int>(point_count));
      for (size_t index = 0; index < point_count; ++index) {
        const double* point = source.coordinates + index * 3;
        cloud->AppendPoint(ON_3dPoint(point[0], point[1], point[2]));
      }
      if (!cloud->IsValid()) {
        delete cloud;
        error = "point cloud is not valid in OpenNURBS";
        return nullptr;
      }
      return cloud;
    }
    case VIBO_OBJECT_NURBS_CURVE: {
      if (source.degree_u == 0 ||
          source.degree_u >= source.control_point_count_u ||
          source.degree_v != 0 || source.control_point_count_v != 0 ||
          source.knot_v_count != 0 || source.control_point_count_u >
              static_cast<size_t>(std::numeric_limits<int>::max()) ||
          source.coordinate_count != source.control_point_count_u * 4 ||
          source.knot_u_count != source.control_point_count_u +
                                     static_cast<size_t>(source.degree_u) + 1) {
        error = "NURBS curve dimensions are inconsistent";
        return nullptr;
      }
      auto* curve = new ON_NurbsCurve(
          3, true, static_cast<int>(source.degree_u) + 1,
          static_cast<int>(source.control_point_count_u));
      for (size_t index = 0; index < source.control_point_count_u; ++index) {
        const double* point = source.coordinates + index * 4;
        const double weight = point[3];
        if (weight <= 0.0 ||
            !curve->SetCV(static_cast<int>(index),
                          ON_4dPoint(point[0] * weight, point[1] * weight,
                                     point[2] * weight, weight))) {
          delete curve;
          error = "NURBS curve has an invalid control point weight";
          return nullptr;
        }
      }
      for (int index = 0; index < curve->KnotCount(); ++index) {
        if (!curve->SetKnot(
                index, source.knots_u[static_cast<size_t>(index) + 1])) {
          delete curve;
          error = "NURBS curve has an invalid knot vector";
          return nullptr;
        }
      }
      if (!curve->IsValid()) {
        delete curve;
        error = "NURBS curve is not valid in OpenNURBS";
        return nullptr;
      }
      return curve;
    }
    case VIBO_OBJECT_NURBS_SURFACE: {
      if (source.degree_u == 0 || source.degree_v == 0 ||
          source.degree_u >= source.control_point_count_u ||
          source.degree_v >= source.control_point_count_v ||
          source.control_point_count_u >
              static_cast<size_t>(std::numeric_limits<int>::max()) ||
          source.control_point_count_v >
              static_cast<size_t>(std::numeric_limits<int>::max()) ||
          source.control_point_count_u >
              std::numeric_limits<size_t>::max() /
                  source.control_point_count_v / 4 ||
          source.coordinate_count != source.control_point_count_u *
                                         source.control_point_count_v * 4 ||
          source.knot_u_count != source.control_point_count_u +
                                     static_cast<size_t>(source.degree_u) + 1 ||
          source.knot_v_count != source.control_point_count_v +
                                     static_cast<size_t>(source.degree_v) + 1) {
        error = "NURBS surface dimensions are inconsistent";
        return nullptr;
      }
      auto* surface = new ON_NurbsSurface(
          3, true, static_cast<int>(source.degree_u) + 1,
          static_cast<int>(source.degree_v) + 1,
          static_cast<int>(source.control_point_count_u),
          static_cast<int>(source.control_point_count_v));
      for (size_t v = 0; v < source.control_point_count_v; ++v) {
        for (size_t u = 0; u < source.control_point_count_u; ++u) {
          const size_t index = v * source.control_point_count_u + u;
          const double* point = source.coordinates + index * 4;
          const double weight = point[3];
          if (weight <= 0.0 ||
              !surface->SetCV(
                  static_cast<int>(u), static_cast<int>(v),
                  ON_4dPoint(point[0] * weight, point[1] * weight,
                             point[2] * weight, weight))) {
            delete surface;
            error = "NURBS surface has an invalid control point weight";
            return nullptr;
          }
        }
      }
      for (int direction = 0; direction < 2; ++direction) {
        const double* knots =
            direction == 0 ? source.knots_u : source.knots_v;
        for (int index = 0; index < surface->KnotCount(direction); ++index) {
          if (!surface->SetKnot(
                  direction, index, knots[static_cast<size_t>(index) + 1])) {
            delete surface;
            error = "NURBS surface has an invalid knot vector";
            return nullptr;
          }
        }
      }
      if (!surface->IsValid()) {
        delete surface;
        error = "NURBS surface is not valid in OpenNURBS";
        return nullptr;
      }
      return surface;
    }
    case VIBO_OBJECT_TRIANGLE_MESH: {
      if (source.coordinate_count == 0 || source.coordinate_count % 3 != 0 ||
          source.index_count == 0 || source.index_count % 3 != 0 ||
          source.indices == nullptr ||
          source.coordinate_count / 3 >
              static_cast<size_t>(std::numeric_limits<int>::max()) ||
          source.index_count / 3 >
              static_cast<size_t>(std::numeric_limits<int>::max())) {
        error = "triangle mesh dimensions are inconsistent";
        return nullptr;
      }
      const size_t vertex_count = source.coordinate_count / 3;
      const size_t face_count = source.index_count / 3;
      auto* mesh = new ON_Mesh(static_cast<int>(face_count),
                               static_cast<int>(vertex_count), false, false);
      for (size_t index = 0; index < vertex_count; ++index) {
        const double* point = source.coordinates + index * 3;
        if (!mesh->SetVertex(static_cast<int>(index),
                             ON_3dPoint(point[0], point[1], point[2]))) {
          delete mesh;
          error = "triangle mesh has an invalid vertex";
          return nullptr;
        }
      }
      for (size_t index = 0; index < face_count; ++index) {
        const uint32_t* face = source.indices + index * 3;
        if (face[0] >= vertex_count || face[1] >= vertex_count ||
            face[2] >= vertex_count ||
            !mesh->SetTriangle(static_cast<int>(index),
                               static_cast<int>(face[0]),
                               static_cast<int>(face[1]),
                               static_cast<int>(face[2]))) {
          delete mesh;
          error = "triangle mesh has an invalid face";
          return nullptr;
        }
      }
      if (!mesh->IsValid()) {
        delete mesh;
        error = "triangle mesh is not valid in OpenNURBS";
        return nullptr;
      }
      return mesh;
    }
    default:
      error = "object type is not supported";
      return nullptr;
  }
}

}  // namespace

struct ViboThreeDmModel {
  std::vector<BridgeLayer> layers;
  std::vector<BridgeGroup> groups;
  std::vector<BridgeObject> objects;
  size_t unsupported_object_count = 0;
};

extern "C" int32_t vibo_3dm_read(const char* path,
                                  ViboThreeDmModel** output, char* error,
                                  size_t error_capacity) {
  if (output != nullptr) {
    *output = nullptr;
  }
  if (path == nullptr || path[0] == '\0' || output == nullptr) {
    set_error(error, error_capacity, "path and output pointer are required");
    return 0;
  }

  try {
    begin_open_nurbs();
    ON_wString diagnostics;
    ON_TextLog log(diagnostics);
    ONX_Model source;
    if (!source.Read(path, &log)) {
      std::string message = "OpenNURBS could not read the 3DM file";
      const std::string details = utf8(diagnostics);
      if (!details.empty()) {
        message += ": " + details;
      }
      set_error(error, error_capacity, message);
      return 0;
    }

    auto decoded = std::make_unique<ViboThreeDmModel>();
    ONX_ModelComponentIterator layer_iterator(
        source, ON_ModelComponent::Type::Layer);
    for (const ON_Layer* layer =
             ON_Layer::Cast(layer_iterator.FirstComponent());
         layer != nullptr;
         layer = ON_Layer::Cast(layer_iterator.NextComponent())) {
      const ON_Color color = layer->Color();
      decoded->layers.push_back(
          {layer->Index(), utf8(layer->Name()),
           static_cast<uint8_t>(color.Red()),
           static_cast<uint8_t>(color.Green()),
           static_cast<uint8_t>(color.Blue()),
           static_cast<uint8_t>(layer->IsVisible()),
           static_cast<uint8_t>(layer->IsLocked())});
    }

    ONX_ModelComponentIterator group_iterator(
        source, ON_ModelComponent::Type::Group);
    for (const ON_Group* group =
             ON_Group::Cast(group_iterator.FirstComponent());
         group != nullptr;
         group = ON_Group::Cast(group_iterator.NextComponent())) {
      decoded->groups.push_back({group->Index(), utf8(group->Name())});
    }

    ONX_ModelComponentIterator object_iterator(
        source, ON_ModelComponent::Type::ModelGeometry);
    for (const ON_ModelComponent* component = object_iterator.FirstComponent();
         component != nullptr; component = object_iterator.NextComponent()) {
      const ON_ModelGeometryComponent* model_geometry =
          ON_ModelGeometryComponent::Cast(component);
      if (model_geometry == nullptr) {
        ++decoded->unsupported_object_count;
        continue;
      }
      const ON_Geometry* geometry = model_geometry->Geometry(nullptr);
      if (geometry == nullptr) {
        ++decoded->unsupported_object_count;
        continue;
      }

      BridgeObject object;
      if (const ON_3dmObjectAttributes* attributes =
              model_geometry->Attributes(nullptr)) {
        object.source_layer_index = attributes->m_layer_index;
        object.name = utf8(attributes->Name());
        object.visible = static_cast<uint8_t>(attributes->IsVisible());
        object.locked = static_cast<uint8_t>(attributes->Mode() ==
                                             ON::locked_object);
        const int group_count = attributes->GroupCount();
        const int* group_list = attributes->GroupList();
        if (group_count > 0 && group_list == nullptr) {
          ++decoded->unsupported_object_count;
          continue;
        }
        object.group_indices.reserve(static_cast<size_t>(group_count));
        for (int group_position = 0; group_position < group_count;
             ++group_position) {
          object.group_indices.push_back(
              static_cast<int32_t>(group_list[group_position]));
        }
      }

      bool supported = false;
      if (const ON_Point* point = ON_Point::Cast(geometry)) {
        supported = append_point(*point, object);
      } else if (const ON_PointCloud* cloud = ON_PointCloud::Cast(geometry)) {
        supported = append_point_cloud(*cloud, object);
      } else if (const ON_LineCurve* line = ON_LineCurve::Cast(geometry)) {
        supported = append_line(*line, object);
      } else if (const ON_Mesh* mesh = ON_Mesh::Cast(geometry)) {
        supported = append_mesh(*mesh, object);
      } else if (const ON_Curve* curve = ON_Curve::Cast(geometry)) {
        supported = append_nurbs(*curve, object);
      } else if (const ON_Surface* surface = ON_Surface::Cast(geometry)) {
        supported = append_nurbs_surface(*surface, object);
      }

      if (supported) {
        decoded->objects.push_back(std::move(object));
      } else {
        ++decoded->unsupported_object_count;
      }
    }

    *output = decoded.release();
    set_error(error, error_capacity, "");
    return 1;
  } catch (const std::exception& exception) {
    set_error(error, error_capacity,
              std::string("OpenNURBS exception: ") + exception.what());
    return 0;
  } catch (...) {
    set_error(error, error_capacity, "unknown OpenNURBS exception");
    return 0;
  }
}

extern "C" void vibo_3dm_free(ViboThreeDmModel* model) { delete model; }

extern "C" size_t vibo_3dm_layer_count(const ViboThreeDmModel* model) {
  return model == nullptr ? 0 : model->layers.size();
}

extern "C" int32_t vibo_3dm_layer(
    const ViboThreeDmModel* model, size_t index, int32_t* source_index,
    const char** name, uint8_t* red, uint8_t* green, uint8_t* blue,
    uint8_t* visible, uint8_t* locked) {
  if (model == nullptr || index >= model->layers.size() ||
      source_index == nullptr || name == nullptr || red == nullptr ||
      green == nullptr || blue == nullptr || visible == nullptr ||
      locked == nullptr) {
    return 0;
  }
  const BridgeLayer& layer = model->layers[index];
  *source_index = layer.source_index;
  *name = layer.name.c_str();
  *red = layer.red;
  *green = layer.green;
  *blue = layer.blue;
  *visible = layer.visible;
  *locked = layer.locked;
  return 1;
}

extern "C" size_t vibo_3dm_group_count(const ViboThreeDmModel* model) {
  return model == nullptr ? 0 : model->groups.size();
}

extern "C" int32_t vibo_3dm_group(const ViboThreeDmModel* model,
                                    size_t index, int32_t* source_index,
                                    const char** name) {
  if (model == nullptr || index >= model->groups.size() ||
      source_index == nullptr || name == nullptr) {
    return 0;
  }
  const BridgeGroup& group = model->groups[index];
  *source_index = group.source_index;
  *name = group.name.c_str();
  return 1;
}

extern "C" size_t vibo_3dm_object_count(const ViboThreeDmModel* model) {
  return model == nullptr ? 0 : model->objects.size();
}

extern "C" size_t vibo_3dm_unsupported_object_count(
    const ViboThreeDmModel* model) {
  return model == nullptr ? 0 : model->unsupported_object_count;
}

extern "C" int32_t vibo_3dm_object(
    const ViboThreeDmModel* model, size_t index, ViboObjectInfo* info,
    const double** coordinates, const double** knots_u, const double** knots_v,
    const uint32_t** indices, const int32_t** group_indices) {
  if (model == nullptr || index >= model->objects.size() || info == nullptr ||
      coordinates == nullptr || knots_u == nullptr || knots_v == nullptr ||
      indices == nullptr || group_indices == nullptr) {
    return 0;
  }
  const BridgeObject& object = model->objects[index];
  *info = {object.object_type,
           object.source_layer_index,
           object.name.c_str(),
           object.visible,
           object.locked,
           object.degree_u,
           object.degree_v,
           object.control_point_count_u,
           object.control_point_count_v,
           object.coordinates.size(),
           object.knots_u.size(),
           object.knots_v.size(),
           object.indices.size(),
           object.group_indices.size()};
  *coordinates = object.coordinates.empty() ? nullptr : object.coordinates.data();
  *knots_u = object.knots_u.empty() ? nullptr : object.knots_u.data();
  *knots_v = object.knots_v.empty() ? nullptr : object.knots_v.data();
  *indices = object.indices.empty() ? nullptr : object.indices.data();
  *group_indices =
      object.group_indices.empty() ? nullptr : object.group_indices.data();
  return 1;
}

extern "C" int32_t vibo_3dm_write(
    const char* path, const ViboWriteLayer* layers, size_t layer_count,
    const ViboWriteGroup* groups, size_t group_count,
    const ViboWriteObject* objects, size_t object_count, char* error,
    size_t error_capacity) {
  if (path == nullptr || path[0] == '\0' ||
      (layer_count != 0 && layers == nullptr) ||
      (group_count != 0 && groups == nullptr) ||
      (object_count != 0 && objects == nullptr)) {
    set_error(error, error_capacity, "path and input arrays are required");
    return 0;
  }

  try {
    begin_open_nurbs();
    ONX_Model model;
    model.m_sStartSectionComments =
        "Created by Viboceros using the OpenNURBS toolkit.";
    model.m_properties.m_Application.m_application_name = L"Viboceros";
    model.m_properties.m_Application.m_application_URL =
        L"https://github.com/dllu/viboceros";
    model.m_properties.m_RevisionHistory.NewRevision();

    std::vector<int> layer_indices;
    layer_indices.reserve(std::max<size_t>(layer_count, 1));
    if (layer_count == 0) {
      const int index = model.AddDefaultLayer(L"Default", ON_Color::Black);
      if (index < 0) {
        set_error(error, error_capacity,
                  "OpenNURBS could not create a default layer");
        return 0;
      }
      layer_indices.push_back(index);
    } else {
      for (size_t index = 0; index < layer_count; ++index) {
        const ViboWriteLayer& source = layers[index];
        if (source.name == nullptr || source.name[0] == '\0') {
          set_error(error, error_capacity, "3DM layer names cannot be empty");
          return 0;
        }
        ON_Layer layer;
        const ON_wString name(source.name);
        if (!layer.SetName(name)) {
          set_error(error, error_capacity, "3DM layer name is invalid");
          return 0;
        }
        layer.SetColor(ON_Color(source.red, source.green, source.blue));
        layer.SetVisible(source.visible != 0);
        layer.SetLocked(source.locked != 0);
        const ON_ModelComponentReference reference =
            model.AddModelComponent(layer, true);
        const ON_Layer* added = ON_Layer::FromModelComponentRef(reference, nullptr);
        if (added == nullptr || added->Index() < 0) {
          set_error(error, error_capacity,
                    "OpenNURBS could not add a layer to the model");
          return 0;
        }
        layer_indices.push_back(added->Index());
      }
    }

    std::vector<int> group_indices;
    group_indices.reserve(group_count);
    for (size_t index = 0; index < group_count; ++index) {
      const ViboWriteGroup& source = groups[index];
      if (source.name == nullptr || source.name[0] == '\0') {
        set_error(error, error_capacity, "3DM group names cannot be empty");
        return 0;
      }
      ON_Group group;
      if (!group.SetName(ON_wString(source.name))) {
        set_error(error, error_capacity, "3DM group name is invalid");
        return 0;
      }
      const ON_ModelComponentReference reference =
          model.AddModelComponent(group, true);
      const ON_Group* added = ON_Group::FromModelComponentRef(reference, nullptr);
      if (added == nullptr || added->Index() < 0) {
        set_error(error, error_capacity,
                  "OpenNURBS could not add a group to the model");
        return 0;
      }
      group_indices.push_back(added->Index());
    }

    for (size_t index = 0; index < object_count; ++index) {
      const ViboWriteObject& source = objects[index];
      if (source.layer_index >= layer_indices.size()) {
        set_error(error, error_capacity,
                  "3DM object references a missing layer");
        return 0;
      }
      if (source.group_index_count != 0 && source.group_indices == nullptr) {
        set_error(error, error_capacity,
                  "3DM object has a null group index array");
        return 0;
      }
      bool valid_groups = true;
      for (size_t group_position = 0;
           group_position < source.group_index_count; ++group_position) {
        if (source.group_indices[group_position] >= group_indices.size()) {
          valid_groups = false;
          break;
        }
      }
      if (!valid_groups) {
        set_error(error, error_capacity,
                  "3DM object references a missing group");
        return 0;
      }
      std::string geometry_error;
      ON_Object* geometry = geometry_for(source, geometry_error);
      if (geometry == nullptr) {
        set_error(error, error_capacity,
                  "object " + std::to_string(index) + ": " + geometry_error);
        return 0;
      }
      ON_3dmObjectAttributes* attributes =
          attributes_for(source, layer_indices[source.layer_index]);
      if (attributes == nullptr) {
        delete geometry;
        set_error(error, error_capacity,
                  "object " + std::to_string(index) +
                      ": object name is invalid");
        return 0;
      }
      for (size_t group_position = 0;
           group_position < source.group_index_count; ++group_position) {
        attributes->AddToGroup(
            group_indices[source.group_indices[group_position]]);
      }
      const ON_ModelComponentReference reference =
          model.AddManagedModelGeometryComponent(geometry, attributes, true);
      if (reference.IsEmpty()) {
        set_error(error, error_capacity,
                  "OpenNURBS could not add object " + std::to_string(index));
        return 0;
      }
    }

    ON_wString diagnostics;
    ON_TextLog log(diagnostics);
    if (!model.Write(path, 0, &log)) {
      std::string message = "OpenNURBS could not write the 3DM file";
      const std::string details = utf8(diagnostics);
      if (!details.empty()) {
        message += ": " + details;
      }
      set_error(error, error_capacity, message);
      return 0;
    }
    set_error(error, error_capacity, "");
    return 1;
  } catch (const std::exception& exception) {
    set_error(error, error_capacity,
              std::string("OpenNURBS exception: ") + exception.what());
    return 0;
  } catch (...) {
    set_error(error, error_capacity, "unknown OpenNURBS exception");
    return 0;
  }
}
