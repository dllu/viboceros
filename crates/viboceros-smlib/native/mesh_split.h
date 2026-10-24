// Open-surface mesh partitions on independently owned polygon topology.
#include <SmGeomUtility.h>
struct VbMeshSplit {
    std::vector<std::unique_ptr<VbMesh>> parts;
    std::vector<std::vector<uint32_t>> source_faces;
};

namespace vb_mesh_split_detail {
using Edges = std::unordered_set<SmPolyEdge *>;

// Keep the constraints as geometry: later imprints may replace their original
// edge pointers, and common coplanar triangle edges are not all region borders.
struct CutSegment : SmObject {
    SmPoint3d a, b;
    CutSegment(const double *p) : a(p[0],p[1],p[2]), b(p[3],p[4],p[5]) {}
};
Edges barriers(SmPolyBrep &mesh, const double *lines, size_t count, double tolerance) {
    require(lines && count>0 && count<=limit, "Invalid mesh split constraints");
    std::vector<std::unique_ptr<CutSegment>> segments;
    SmExtent3d bounds;
    check(mesh.CalculateBoundingBox(bounds), "bound split constraints");
    bounds.ExpandAbsolute(tolerance);
    auto tree = std::unique_ptr<SmTree>(new (*SmApiGetOrCreateContext()) SmTree(bounds));
    for (size_t i=0; i<count; ++i) {
        require(finite3(lines+6*i) && finite3(lines+6*i+3), "Nonfinite mesh split constraint");
        auto segment = std::unique_ptr<CutSegment>(new (*SmApiGetOrCreateContext()) CutSegment(lines+6*i));
        const auto &a=segment->a, &b=segment->b;
        SmExtent3d box(SmPoint3d(std::min(a.x,b.x),std::min(a.y,b.y),std::min(a.z,b.z)),
            SmPoint3d(std::max(a.x,b.x),std::max(a.y,b.y),std::max(a.z,b.z)));
        check(tree->AddToSpatialTree(box,segment.get()), "index split constraints");
        segments.push_back(std::move(segment));
    }
    SmTArray<SmPolyEdge *> edges;
    mesh.GetPolyEdges(edges);
    Edges blocked;
    size_t work=0;
    for (ULONG i=0; i<edges.GetSize(); ++i) {
        const auto a=edges[i]->GetStartPoint(), b=edges[i]->GetEndPoint();
        const SmVector3d direction=b-a;
        const double length=direction.Length();
        require(std::isfinite(length) && length>0., "Invalid imprinted split edge");
        SmExtent3d box;
        check(edges[i]->CalculateBoundingBox(box), "bound imprinted split edge");
        box.ExpandAbsolute(tolerance);
        SmTArray<SmObjectList *> candidates;
        check(tree->GetObjectListInBox(box,candidates), "query split constraints");
        std::vector<std::pair<double,double>> intervals;
        for (ULONG j=0; j<candidates.GetSize(); ++j) {
            require(++work<=10*limit, "Mesh split constraint work limit");
            auto *segment=static_cast<CutSegment *>(candidates[j]->m_pObject);
            double first=0.,last=0.;
            check(smgu_LineClosestPoint(a,direction,segment->a,first), "project first split constraint");
            check(smgu_LineClosestPoint(a,direction,segment->b,last), "project last split constraint");
            require(std::isfinite(first) && std::isfinite(last), "Nonfinite split constraint parameter");
            const double da=(segment->a-(a+direction*first)).Length();
            const double db=(segment->b-(a+direction*last)).Length();
            require(std::isfinite(da) && std::isfinite(db), "Nonfinite split constraint distance");
            if (da>tolerance || db>tolerance) continue;
            const double lo=std::max(0.,std::min(first,last)), hi=std::min(1.,std::max(first,last));
            if (lo<hi) intervals.emplace_back(lo,hi);
        }
        std::sort(intervals.begin(),intervals.end());
        double end=0.;
        for (const auto &interval:intervals) {
            if (interval.first>end+tolerance/length) break;
            end=std::max(end,interval.second);
            if (end>=1.-tolerance/length) { blocked.insert(edges[i]); break; }
        }
    }
    return blocked;
}

void partitions(SmPolyBrep &mesh, const Edges &blocked, double tolerance, VbMeshSplit &out) {
    SmTArray<SmPolyFace *> faces;
    mesh.GetPolyFaces(faces);
    require(faces.GetSize() <= limit, "Mesh split face limit");
    std::unordered_map<SmPolyFace *, size_t> index;
    for (ULONG i = 0; i < faces.GetSize(); ++i) index.emplace(faces[i], i);
    std::vector<bool> visited(faces.GetSize(), false);
    size_t work = 0, points = 0, triangles = 0;
    for (ULONG start = 0; start < faces.GetSize(); ++start) {
        if (visited[start]) continue;
        std::vector<size_t> queue{start};
        SmTArray<SmPolyFace *> component;
        visited[start] = true;
        for (size_t head = 0; head < queue.size(); ++head) {
            auto *face = faces[queue[head]];
            component.Add(face);
            SmTArray<SmPolyEdge *> edges;
            face->GetPolyEdges(edges);
            for (ULONG j = 0; j < edges.GetSize(); ++j) {
                if (blocked.count(edges[j])) continue;
                SmTArray<SmPolyEdge *> radial;
                edges[j]->GetAllRadials(radial);
                for (ULONG k = 0; k < radial.GetSize(); ++k) {
                    require(++work <= 10*limit, "Mesh split adjacency work limit");
                    auto *next = radial[k]->GetPolyFace();
                    require(next && index.count(next), "Missing mesh split adjacency");
                    const auto slot = index.at(next);
                    if (!visited[slot]) { visited[slot] = true; queue.push_back(slot); }
                }
            }
        }
        auto copy = std::unique_ptr<SmPolyBrep>(new (*SmApiGetOrCreateContext()) SmPolyBrep(tolerance));
        check(mesh.CopyFaces(component, copy.get()), "copy mesh split component");
        out.source_faces.emplace_back();
        auto part = vb_mesh_boolean_detail::export_mesh(*copy, &out.source_faces.back());
        require(part != nullptr, "Missing mesh split component");
        points += part->points.size()/3;
        triangles += part->triangles.size()/3;
        require(points <= limit && triangles <= limit && out.parts.size() < limit,
            "Mesh split output limit");
        out.parts.push_back(std::move(part));
    }
}
}

extern "C" int vb_mesh_split(const double *first, size_t first_count,
    const uint32_t *first_faces, size_t first_face_count, const VbMeshSplitCutter *cutters,
    size_t cutter_count, const double *lines, size_t line_count,
    double tolerance, VbMeshSplit **out, char *error, size_t capacity) {
    if (out) *out = nullptr;
    return guarded(error, capacity, [&] {
        require(out && cutters && cutter_count>0 && cutter_count<=limit &&
            std::isfinite(tolerance) && tolerance>0., "Invalid mesh split options");
        std::vector<SmPolyFace *> source_faces;
        auto a = vb_mesh_boolean_detail::import_mesh(first, first_count, first_faces, first_face_count,
            tolerance, &source_faces);
        for (size_t i=0; i<source_faces.size(); ++i)
            source_faces[i]->AddAttribute(new (*SmApiGetOrCreateContext())
                SmLongAttribute(vb_mesh_boolean_detail::face_source_attribute, static_cast<long>(i)));
        for (size_t i=0; i<cutter_count; ++i) {
            const auto &c=cutters[i];
            auto b=vb_mesh_boolean_detail::import_mesh(c.points,c.point_count,c.faces,c.face_count,tolerance);
            SmPolyIntersector query(*SmApiGetOrCreateContext(),a.get(),b.get(),tolerance,1e-10);
            check(query.IntersectInsertRelate(), "imprint mesh split curves");
        }
        auto result=std::make_unique<VbMeshSplit>();
        const auto blocked=vb_mesh_split_detail::barriers(*a,lines,line_count,tolerance);
        vb_mesh_split_detail::partitions(*a,blocked,tolerance,*result);
        *out=result.release();
    });
}
extern "C" size_t vb_mesh_split_count(const VbMeshSplit *p) { return p ? p->parts.size() : 0; }
extern "C" int vb_mesh_split_take(VbMeshSplit *p, size_t index, VbMesh **out) {
    if (!p || !out || index>=p->parts.size() || !p->parts[index]) return -1;
    *out = p->parts[index].release(); return 0;
}
extern "C" int vb_mesh_split_face_sources(const VbMeshSplit *p, size_t index, uint32_t *out, size_t count) {
    if (!p || !out || index>=p->source_faces.size() || count!=p->source_faces[index].size()) return -1;
    std::copy(p->source_faces[index].begin(), p->source_faces[index].end(), out);
    return 0;
}
extern "C" void vb_mesh_split_free(VbMeshSplit *p) { delete p; }
