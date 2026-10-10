#include "bridge.h"
#include <SmApiBrep.h>
#include <SmApiCurves.h>
#include <SmApiGeneral.h>
#include <SmApiPrimitives.h>
#include <SmApiQueries.h>
#include <SmBSplineCurve.h>
#include <SmBrep.h>
#include <SmPoly.h>
#include <algorithm>
#include <cmath>
#include <cstdio>
#include <memory>
#include <mutex>
#include <stdexcept>
#include <string>
#include <unordered_map>
#include <vector>

struct VbSolid {
    std::unique_ptr<SmBrep> value;
};
struct VbParts {
    std::vector<std::unique_ptr<VbSolid>> values;
};
struct VbCurve {
    std::unique_ptr<SmBSplineCurve> value;
    size_t degree;
    std::vector<double> points, knots;
};
struct VbMesh {
    std::vector<double> points;
    std::vector<uint32_t> triangles;
};

namespace {
std::mutex kernel_mutex;
constexpr size_t limit = 1'000'000;
void require(bool condition, const char *message) {
    if (!condition)
        throw std::runtime_error(message);
}
void check(SmApiStatus status, const char *operation) {
    if (status != SM_SUCCESS)
        throw std::runtime_error(std::string(operation) + " status " + std::to_string(status));
}
template <class Function> int guarded(char *error, size_t capacity, Function function) noexcept {
    try {
        std::lock_guard<std::mutex> lock(kernel_mutex);
        static const bool initialized = [] {
            SmApiCreateContext();
            return true;
        }();
        (void)initialized;
        function();
        return 0;
    } catch (const std::exception &exception) {
        if (error && capacity)
            std::snprintf(error, capacity, "%s", exception.what());
    } catch (...) {
        if (error && capacity)
            std::snprintf(error, capacity, "Unknown SMLib exception");
    }
    return 1;
}
bool finite3(const double *p) {
    return p && std::isfinite(p[0]) && std::isfinite(p[1]) && std::isfinite(p[2]);
}
} // namespace

extern "C" int vb_solid_primitive(int kind, const double origin[3], const double size[3],
                                  VbSolid **out, char *error, size_t capacity) {
    if (out)
        *out = nullptr;
    return guarded(error, capacity, [&] {
        require(out && finite3(origin) && finite3(size), "Invalid primitive coordinates");
        require(size[0] > 0 && (kind == 1 || size[1] > 0) && (kind != 0 || size[2] > 0),
                "Invalid primitive dimensions");
        auto owner = std::make_unique<VbSolid>();
        SmBrep *result = nullptr;
        const SmVector3d point(origin[0], origin[1], origin[2]);
        SmApiStatus status;
        switch (kind) {
        case 0:
            status = SmApiCreateBox(point, size[0], size[1], size[2], result);
            break;
        case 1:
            status = SmApiCreateSphere(point, size[0], result);
            break;
        case 2:
            status = SmApiCreateCylinder(point, size[0], size[1], result);
            break;
        default:
            throw std::runtime_error("Unknown primitive");
        }
        owner->value.reset(result);
        check(status, "create primitive");
        require(result, "Missing primitive result");
        *out = owner.release();
    });
}

extern "C" int vb_solid_boolean(const VbSolid *a, const VbSolid *b, int operation, VbSolid **out,
                                char *error, size_t capacity) {
    if (out)
        *out = nullptr;
    return guarded(error, capacity, [&] {
        require(a && b && out && operation >= 0 && operation <= 2,
                "Invalid Boolean operands or operation");
        auto owner = std::make_unique<VbSolid>();
        SmBrep *raw_a = nullptr;
        SmBrep *raw_b = nullptr;
        const auto copy_a = SmApiBrepCopy(a->value.get(), raw_a);
        owner->value.reset(raw_a);
        check(copy_a, "copy A");
        const auto copy_b = SmApiBrepCopy(b->value.get(), raw_b);
        std::unique_ptr<SmBrep> cutter(raw_b);
        check(copy_b, "copy B");
        SmBrep *result = nullptr;
        const auto status = operation == 0   ? SmApiBooleanUnion(raw_a, raw_b, result)
                            : operation == 1 ? SmApiBooleanIntersection(raw_a, raw_b, result)
                                             : SmApiBooleanDifference(raw_a, raw_b, result);
        if (status == SM_SUCCESS)
            cutter.release();
        check(status, "Boolean");
        require(result == raw_a, "Unexpected Boolean ownership");
        *out = owner.release();
    });
}

extern "C" int vb_solid_properties(const VbSolid *solid, double accuracy, double *volume,
                                   double bounds[6], int *manifold, char *error, size_t capacity) {
    return guarded(error, capacity, [&] {
        require(solid && volume && bounds && manifold && std::isfinite(accuracy) &&
                    accuracy >= 1e-8 && accuracy <= 0.1,
                "Invalid property query");
        SmBoolean closed = FALSE;
        check(SmApiBrepIsManifoldSolid(solid->value.get(), closed), "manifold");
        SmPoint3d lo, hi;
        check(SmApiBrepBoundingBox(solid->value.get(), TRUE, lo, hi), "bounds");
        double v = 0;
        check(SmApiBrepComputeVolume(solid->value.get(), accuracy, v), "volume");
        require(std::isfinite(v), "Nonfinite volume");
        for (int i = 0; i < 3; ++i) {
            require(std::isfinite(lo[i]) && std::isfinite(hi[i]) && lo[i] <= hi[i],
                    "Invalid bounds");
            bounds[i] = lo[i];
            bounds[i + 3] = hi[i];
        }
        *volume = v;
        *manifold = closed == TRUE;
    });
}

extern "C" void vb_solid_free(VbSolid *solid) {
    std::lock_guard<std::mutex> lock(kernel_mutex);
    delete solid;
}

extern "C" int vb_solid_census(const VbSolid *solid, size_t counts[2], char *error,
                               size_t capacity) {
    return guarded(error, capacity, [&] {
        require(solid && counts, "Invalid material census");
        long material = 0, voids = 0;
        check(SmApiBrepMaterialCensus(solid->value.get(), material, voids), "material census");
        require(material >= 0 && voids >= 0, "Invalid material counts");
        counts[0] = material;
        counts[1] = voids;
    });
}

extern "C" int vb_solid_brep(const VbSolid *solid, VbBrep **out, char *error, size_t capacity) {
    if (out)
        *out = nullptr;
    return guarded(error, capacity, [&] {
        require(solid && out, "Invalid solid export");
        *out = vb_export_brep(*solid->value);
    });
}

extern "C" int vb_solid_from_brep(const VbBrepView *view, double tolerance,
                                  const size_t *components, size_t component_count,
                                  const int *inward, VbSolid **out, char *error, size_t capacity) {
    if (out)
        *out = nullptr;
    return guarded(error, capacity, [&] {
        require(view && out, "Invalid B-rep import");
        auto owner = std::make_unique<VbSolid>();
        owner->value.reset(vb_import_brep(*view, tolerance, components, component_count, inward));
        *out = owner.release();
    });
}

extern "C" int vb_solid_mesh(const VbSolid *solid, const double quality[3], VbMesh **out,
                             char *error, size_t capacity) {
    if (out)
        *out = nullptr;
    return guarded(error, capacity, [&] {
        require(solid && out && finite3(quality) && quality[0] > 0 && quality[1] > 0 &&
                    quality[1] <= 180 && quality[2] > 0 && quality[2] <= 180,
                "Invalid tessellation controls");
        SmTessellationParams params;
        params.dChordHeightTol = quality[0];
        params.dCurveAngleTolDeg = quality[1];
        params.dSurfaceAngleTolDeg = quality[2];
        SmPolyBrep *raw = nullptr;
        const auto status = SmApiTessellate(solid->value.get(), raw, params);
        std::unique_ptr<SmPolyBrep> mesh(raw);
        check(status, "tessellate");
        require(raw, "Missing tessellation result");
        auto result = std::make_unique<VbMesh>();
        SmTArray<SmPolyVertex *> vertices;
        SmTArray<SmPolyFace *> faces;
        mesh->GetPolyVertices(vertices);
        mesh->GetPolyFaces(faces);
        require(vertices.GetSize() <= limit && faces.GetSize() <= limit, "Mesh resource limit");
        std::unordered_map<const SmPolyVertex *, uint32_t> indices;
        for (ULONG i = 0; i < vertices.GetSize(); ++i) {
            indices.emplace(vertices[i], static_cast<uint32_t>(i));
            const auto &p = vertices[i]->GetPoint();
            require(std::isfinite(p.x) && std::isfinite(p.y) && std::isfinite(p.z),
                    "Nonfinite mesh vertex");
            result->points.insert(result->points.end(), {p.x, p.y, p.z});
        }
        for (ULONG i = 0; i < faces.GetSize(); ++i) {
            auto *loop = faces[i]->GetOuterPolyLoop();
            require(loop, "Missing mesh face loop");
            SmTArray<SmPolyEdge *> edges;
            loop->GetPolyEdges(edges);
            require(edges.GetSize() == 3, "Expected triangular tessellation face");
            for (ULONG j = 0; j < 3; ++j)
                result->triangles.push_back(indices.at(edges[j]->GetStartPolyVertex()));
        }
        *out = result.release();
    });
}

extern "C" int vb_mesh_sizes(const VbMesh *mesh, size_t *vertices, size_t *triangles) {
    if (!mesh || !vertices || !triangles)
        return 1;
    *vertices = mesh->points.size() / 3;
    *triangles = mesh->triangles.size() / 3;
    return 0;
}
extern "C" int vb_mesh_copy(const VbMesh *mesh, double *vertices, size_t vertex_count,
                            uint32_t *triangles, size_t triangle_count) {
    if (!mesh || vertex_count != mesh->points.size() / 3 ||
        triangle_count != mesh->triangles.size() / 3 || !vertices || !triangles)
        return 1;
    std::copy(mesh->points.begin(), mesh->points.end(), vertices);
    std::copy(mesh->triangles.begin(), mesh->triangles.end(), triangles);
    return 0;
}
extern "C" void vb_mesh_free(VbMesh *mesh) { delete mesh; }

extern "C" int vb_curve_create(size_t degree, const double *points_xyzw, size_t point_count,
                               const double *knots, size_t knot_count, VbCurve **out, char *error,
                               size_t capacity) {
    if (out)
        *out = nullptr;
    return guarded(error, capacity, [&] {
        require(out && points_xyzw && knots && degree > 0 && degree <= 64 && point_count > degree &&
                    point_count <= limit && knot_count == point_count + degree + 1,
                "Invalid NURBS dimensions");
        SmTArray<SmPoint3d> points;
        SmTArray<double> weights, unique;
        SmTArray<ULONG> multiplicities;
        for (size_t i = 0; i < point_count; ++i) {
            const auto *p = points_xyzw + i * 4;
            require(finite3(p) && std::isfinite(p[3]) && p[3] > 0,
                    "Native NURBS require positive finite weights");
            points.Add(SmPoint3d(p[0], p[1], p[2]));
            weights.Add(p[3]);
        }
        for (size_t i = 0; i < knot_count; ++i) {
            require(std::isfinite(knots[i]) && (i == 0 || knots[i] >= knots[i - 1]),
                    "Invalid knots");
            if (i == 0 || knots[i] != knots[i - 1]) {
                unique.Add(knots[i]);
                multiplicities.Add(1);
            } else
                ++multiplicities[multiplicities.GetSize() - 1];
        }
        require(unique.GetSize() >= 2 && multiplicities[0] == degree + 1 &&
                    multiplicities[multiplicities.GetSize() - 1] == degree + 1,
                "Native bridge currently requires clamped NURBS");
        for (ULONG i = 1; i + 1 < multiplicities.GetSize(); ++i)
            require(multiplicities[i] <= degree,
                    "Native bridge does not support discontinuous interior knots");
        auto owner = std::make_unique<VbCurve>();
        SmBSplineCurve *raw = nullptr;
        const auto status = SmApiCreateCanonicalCurve(points, unique, multiplicities, &weights,
                                                      degree, SM_CF_UNSPECIFIED, raw);
        owner->value.reset(raw);
        check(status, "create canonical curve");
        require(raw, "Missing NURBS result");
        owner->degree = raw->GetDegree();
        SmTArray<SmPoint3d> exported_points;
        SmTArray<double> exported_weights, exported_knots;
        SmTArray<ULONG> exported_multiplicities;
        check(raw->GetControlPolygon(exported_points, exported_weights), "control polygon");
        check(raw->GetKnots(exported_knots, &exported_multiplicities), "knots");
        const bool polynomial = exported_weights.GetSize() == 0 && !raw->IsRational();
        require(polynomial || exported_points.GetSize() == exported_weights.GetSize(),
                "Invalid exported weights");
        require(exported_knots.GetSize() == exported_multiplicities.GetSize(),
                "Invalid exported knot multiplicities");
        for (ULONG i = 0; i < exported_points.GetSize(); ++i) {
            const auto &p = exported_points[i];
            owner->points.insert(owner->points.end(),
                                 {p.x, p.y, p.z, polynomial ? 1.0 : exported_weights[i]});
        }
        for (ULONG i = 0; i < exported_knots.GetSize(); ++i)
            owner->knots.insert(owner->knots.end(), exported_multiplicities[i], exported_knots[i]);
        *out = owner.release();
    });
}
extern "C" int vb_curve_evaluate(const VbCurve *curve, double parameter, double point[3],
                                 char *error, size_t capacity) {
    return guarded(error, capacity, [&] {
        require(curve && point && std::isfinite(parameter) &&
                    parameter >= curve->knots[curve->degree] &&
                    parameter <= curve->knots[curve->knots.size() - curve->degree - 1],
                "Parameter outside NURBS domain");
        SmVector3d result;
        SmVector3d *result_pointer = &result;
        SmVector3d *derivative = nullptr;
        SmVector3d *second_derivative = nullptr;
        check(SmApiEvaluateCurve(curve->value.get(), parameter, result_pointer, derivative,
                                 second_derivative),
              "evaluate curve");
        require(std::isfinite(result.x) && std::isfinite(result.y) && std::isfinite(result.z),
                "Nonfinite curve result");
        point[0] = result.x;
        point[1] = result.y;
        point[2] = result.z;
    });
}
extern "C" int vb_curve_sizes(const VbCurve *curve, size_t *degree, size_t *points, size_t *knots) {
    if (!curve || !degree || !points || !knots)
        return 1;
    *degree = curve->degree;
    *points = curve->points.size() / 4;
    *knots = curve->knots.size();
    return 0;
}
extern "C" int vb_curve_copy(const VbCurve *curve, double *points_xyzw, size_t point_count,
                             double *knots, size_t knot_count) {
    if (!curve || !points_xyzw || !knots || point_count != curve->points.size() / 4 ||
        knot_count != curve->knots.size())
        return 1;
    std::copy(curve->points.begin(), curve->points.end(), points_xyzw);
    std::copy(curve->knots.begin(), curve->knots.end(), knots);
    return 0;
}
extern "C" void vb_curve_free(VbCurve *curve) {
    std::lock_guard<std::mutex> lock(kernel_mutex);
    delete curve;
}

extern "C" int vb_solid_empty(const VbSolid *solid, int *empty, char *error, size_t capacity) {
    return guarded(error, capacity, [&] {
        require(solid && empty, "Invalid empty query");
        SmTArray<SmFace *> faces;
        solid->value->GetFaces(faces);
        *empty = faces.GetSize() == 0;
    });
}

extern "C" int vb_solid_parts(const VbSolid *solid, VbParts **out, char *error, size_t capacity) {
    if (out)
        *out = nullptr;
    return guarded(error, capacity, [&] {
        require(solid && out, "Invalid solid parts query");
        SmBrep *raw = nullptr;
        const auto copied = SmApiBrepCopy(solid->value.get(), raw);
        std::unique_ptr<SmBrep> copy(raw);
        check(copied, "copy for material parts");
        SmTArray<SmBrep *> bodies;
        SmBrep *separate = copy.release();
        struct Cleanup {
            SmBrep *&source;
            SmTArray<SmBrep *> &bodies;
            ~Cleanup() {
                bool listed = false;
                for (ULONG i = 0; i < bodies.GetSize(); ++i) {
                    if (bodies[i] == source)
                        listed = true;
                    delete bodies[i];
                }
                if (!listed)
                    delete source;
            }
        } cleanup{separate, bodies};
        check(SmBrep::CreateOneBrepPerBody(separate, bodies, FALSE), "separate material bodies");
        require(bodies.GetSize() <= 128, "Material part resource limit");
        auto parts = std::make_unique<VbParts>();
        parts->values.reserve(bodies.GetSize());
        for (ULONG i = 0; i < bodies.GetSize(); ++i) {
            auto item = std::make_unique<VbSolid>();
            item->value.reset(bodies[i]);
            if (separate == bodies[i])
                separate = nullptr;
            bodies[i] = nullptr;
            parts->values.push_back(std::move(item));
        }
        separate = nullptr;
        *out = parts.release();
    });
}
extern "C" size_t vb_parts_count(const VbParts *parts) { return parts ? parts->values.size() : 0; }
extern "C" VbSolid *vb_parts_take(VbParts *parts, size_t index) {
    return parts && index < parts->values.size() ? parts->values[index].release() : nullptr;
}
extern "C" void vb_parts_free(VbParts *parts) {
    std::lock_guard<std::mutex> lock(kernel_mutex);
    delete parts;
}
