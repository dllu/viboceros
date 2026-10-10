#include <SmApiBrep.h>
#include <SmApiGeneral.h>
#include <SmApiPrimitives.h>
#include <SmApiPolygons.h>
#include <SmApiQueries.h>
#include <SmBrep.h>
#include <SmPoly.h>
#include <SmVector3d.h>

#include <array>
#include <chrono>
#include <cmath>
#include <iomanip>
#include <iostream>
#include <memory>
#include <stdexcept>
#include <string>

namespace {
using Owner = std::unique_ptr<SmBrep>;
using Clock = std::chrono::steady_clock;
using Boolean = SmApiStatus (*)(SmBrep*, SmBrep*, SmBrep*&);

void check(SmApiStatus status, const char* operation) {
    if (status != SM_SUCCESS)
        throw std::runtime_error(std::string(operation) + " status " + std::to_string(status));
}

Owner box(double x, double y, double z) {
    SmBrep* result = nullptr;
    check(SmApiCreateBox(SmVector3d(x, y, z), 10, 10, 10, result), "create_box");
    return Owner(result);
}

void qualify(const char* name, const Owner& solid, double expected_volume,
             const std::array<double, 3>& expected_min,
             const std::array<double, 3>& expected_max, double operation_ms) {
    SmBoolean manifold = FALSE;
    check(SmApiBrepIsManifoldSolid(solid.get(), manifold), "is_manifold");
    if (!manifold) throw std::runtime_error(std::string(name) + " is not manifold");
    double volume = 0;
    check(SmApiBrepComputeVolume(solid.get(), 1e-8, volume), "volume");
    if (!std::isfinite(volume) || std::abs(volume - expected_volume) > 1e-6 * expected_volume)
        throw std::runtime_error(std::string(name) + " volume mismatch: " + std::to_string(volume));
    SmPoint3d lo, hi;
    check(SmApiBrepBoundingBox(solid.get(), TRUE, lo, hi), "tight_bounds");
    for (int axis = 0; axis < 3; ++axis) {
        if (!std::isfinite(lo[axis]) || !std::isfinite(hi[axis]) ||
            std::abs(lo[axis] - expected_min[axis]) > 1e-4 ||
            std::abs(hi[axis] - expected_max[axis]) > 1e-4)
            throw std::runtime_error(std::string(name) + " bounds mismatch");
    }
    SmTessellationParams params;
    params.dChordHeightTol = 0.005;
    params.dCurveAngleTolDeg = 15;
    params.dSurfaceAngleTolDeg = 15;
    SmPolyBrep* raw_mesh = nullptr;
    auto tess_start = Clock::now();
    check(SmApiTessellate(solid.get(), raw_mesh, params), "tessellate");
    std::unique_ptr<SmPolyBrep> mesh(raw_mesh);
    auto tess_ms = std::chrono::duration<double, std::milli>(Clock::now() - tess_start).count();
    SmBoolean mesh_manifold = FALSE;
    check(SmApiPolyBrepIsManifoldSolid(mesh.get(), mesh_manifold), "mesh_is_manifold");
    if (!mesh_manifold) throw std::runtime_error(std::string(name) + " mesh is not manifold");
    double mesh_volume = 0;
    check(SmApiPolyBrepComputeVolume(mesh.get(), mesh_volume), "mesh_volume");
    if (!std::isfinite(mesh_volume) || std::abs(mesh_volume - expected_volume) > 0.005 * expected_volume)
        throw std::runtime_error(std::string(name) + " mesh volume mismatch");
    SmTArray<SmPolyFace*> mesh_faces;
    SmTArray<SmPolyVertex*> mesh_vertices;
    mesh->GetPolyFaces(mesh_faces);
    mesh->GetPolyVertices(mesh_vertices);
    if (mesh_faces.GetSize() == 0 || mesh_vertices.GetSize() == 0)
        throw std::runtime_error(std::string(name) + " mesh is empty");
    std::cout << "{\"case\":\"" << name << "\",\"manifold\":true,\"volume\":" << volume
              << ",\"expected_volume\":" << expected_volume
              << ",\"operation_ms\":" << operation_ms
              << ",\"tessellation_ms\":" << tess_ms
              << ",\"mesh_manifold\":true,\"mesh_volume\":" << mesh_volume
              << ",\"mesh_faces\":" << mesh_faces.GetSize()
              << ",\"mesh_vertices\":" << mesh_vertices.GetSize() << ",\"bounds\":[["
              << lo[0] << ',' << lo[1] << ',' << lo[2] << "],["
              << hi[0] << ',' << hi[1] << ',' << hi[2] << "]]}\n";
}

void boolean_case(const char* name, Owner a, Owner b, Boolean operation,
                  double expected_volume, const std::array<double, 3>& lo,
                  const std::array<double, 3>& hi) {
    SmBrep* result = nullptr;
    auto start = Clock::now();
    const auto status = operation(a.get(), b.get(), result);
    const auto elapsed = std::chrono::duration<double, std::milli>(Clock::now() - start).count();
    // These three native overloads consume B on success and return modified A.
    if (status == SM_SUCCESS) b.release();
    check(status, name);
    if (result != a.get()) throw std::runtime_error("Unexpected Boolean result ownership");
    qualify(name, a, expected_volume, lo, hi, elapsed);
}
}

int main() {
    try {
        SmApiCreateContext();
        std::cout << std::setprecision(17);
        auto plain = box(0, 0, 0);
        qualify("box", plain, 1000, {0, 0, 0}, {10, 10, 10}, 0);
        boolean_case("box_union", box(0, 0, 0), box(5, 2, 3), SmApiBooleanUnion,
                     1720, {0, 0, 0}, {15, 12, 13});
        boolean_case("box_intersection", box(0, 0, 0), box(5, 2, 3), SmApiBooleanIntersection,
                     280, {5, 2, 3}, {10, 10, 10});
        boolean_case("box_difference", box(0, 0, 0), box(5, 2, 3), SmApiBooleanDifference,
                     720, {0, 0, 0}, {10, 10, 10});
        SmBrep* cylinder = nullptr;
        check(SmApiCreateCylinder(SmVector3d(5, 5, -1), 2, 12, cylinder), "create_cylinder");
        boolean_case("cylinder_through_hole", box(0, 0, 0), Owner(cylinder), SmApiBooleanDifference,
                     1000 - 40 * std::acos(-1.0), {0, 0, 0}, {10, 10, 10});
        SmBrep* sphere = nullptr;
        check(SmApiCreateSphere(SmVector3d(5, 5, 5), 2, sphere), "create_sphere");
        boolean_case("sphere_cavity", box(0, 0, 0), Owner(sphere), SmApiBooleanDifference,
                     1000 - (32.0 / 3) * std::acos(-1.0), {0, 0, 0}, {10, 10, 10});
        sphere = nullptr;
        check(SmApiCreateSphere(SmVector3d(5, 5, 10), 2, sphere), "create_sphere");
        boolean_case("hemisphere_pocket", box(0, 0, 0), Owner(sphere), SmApiBooleanDifference,
                     1000 - (16.0 / 3) * std::acos(-1.0), {0, 0, 0}, {10, 10, 10});
        return 0;
    } catch (const std::exception& error) {
        std::cerr << "usd-brep smoke failed: " << error.what() << '\n';
        return 1;
    }
}
