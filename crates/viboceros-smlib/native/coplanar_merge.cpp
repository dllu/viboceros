#include "brep_data.h"
#include <SmBrep.h>
#include <SmFace.h>
#include <SmEdge.h>
#include <SmVertex.h>
#include <SmBSplineSurface.h>
#include <cmath>
#include <SmGeomUtility.h>
#include <stdexcept>
#include <unordered_map>
#include <unordered_set>

namespace {
void require(bool ok, const char *message) { if (!ok) throw std::runtime_error(message); }
struct Plane { SmPoint3d origin; SmVector3d normal; };
}

size_t vb_merge_coplanar_faces(SmBrep &brep, double tolerance, double angle_tolerance,
                               const std::vector<SmFace*> *selected, bool region) {
    SmTArray<SmFace*> faces;
    brep.GetFaces(faces);
    require(faces.GetSize()>0 && faces.GetSize()<=362,"Coplanar merge face resource limit");
    std::unordered_set<const SmFace*> requested;
    if (selected) {
        require(selected->size()==(region?1U:2U),"Select two faces or one region seed");
        SmTArray<SmFace*> ordered;
        for (auto *face:*selected) {
            ULONG index=0;
            require(face && faces.FindElement(face,index) && requested.insert(face).second,"Invalid selected face identities");
            ordered.Add(face);
        }
        for (ULONG i=0;i<faces.GetSize();++i)if(!requested.count(faces[i]))ordered.Add(faces[i]);
        faces=ordered; // The selected seed owns the fixed admission plane.
    }
    std::vector<Plane> anchors;
    std::unordered_map<const SmFace*,size_t> groups;
    size_t controls=0;
    for (ULONG i=0;i<faces.GetSize();++i) {
        auto *surface=faces[i]->GetSurface();
        if (!surface->IsKindOf(SmBSplineSurface_TYPE)) continue;
        SmTArray<SmPoint3d> points;
        SmTArray<double> weights;
        ULONG u=0,v=0;
        require(static_cast<SmBSplineSurface*>(surface)->GetControlPointNet(u,v,points,weights)==SM_SUCCESS,
                "Read planar surface controls");
        require(weights.GetSize()==0 || weights.GetSize()==points.GetSize(),"Invalid planar surface weights");
        for (ULONG w=0;w<weights.GetSize();++w)
            require(std::isfinite(weights[w]) && weights[w]>0,"Coplanar merge requires positive finite surface weights");
        controls+=points.GetSize();
        require(controls<=2000000,"Coplanar merge control resource limit");
        Plane plane;
        SmPointSetType set_type;
        double deviation=0,radius=0;
        if (smgu_BestFitPlaneToPoints(points,tolerance/8,set_type,plane.origin,plane.normal,
                                      deviation,radius)!=SM_SUCCESS || set_type!=SM_PST_PLANAR)
            continue;
        if (plane.normal.Unitize()!=SM_SUCCESS) continue;
        // The upstream fit's reported gap is one-sided. Check every control
        // explicitly; positive rational weights then bound the full surface.
        bool planar=true;
        for (ULONG p=0;p<points.GetSize();++p) {
            const double distance=std::abs(plane.normal.Dot(points[p]-plane.origin));
            if (!std::isfinite(distance) || distance>tolerance/8) {planar=false;break;}
        }
        if (!planar) continue;
        size_t group=anchors.size();
        for (size_t g=0;g<anchors.size();++g) {
            const auto &a=anchors[g];
            const auto cross=a.normal*plane.normal;
            const double angle=std::atan2(cross.Length(),std::abs(a.normal.Dot(plane.normal)));
            if (angle>angle_tolerance) continue;
            bool contained=true;
            for (ULONG p=0;p<points.GetSize();++p) {
                const double distance=std::abs(a.normal.Dot(points[p]-a.origin));
                if (!std::isfinite(distance) || distance>tolerance) {contained=false;break;}
            }
            if (contained) {group=g;break;}
        }
        if (group==anchors.size()) anchors.push_back(plane);
        groups[faces[i]]=group;
    }
    SmTArray<SmEdge*> edges,keep;
    brep.GetEdges(edges);
    std::unordered_set<const SmFace*> connected;
    if (selected && region) {
        const auto seed=groups.find((*selected)[0]);
        if(seed==groups.end())return 0;
        std::vector<const SmFace*> queue{(*selected)[0]};connected.insert(queue[0]);
        std::unordered_map<const SmFace*,std::vector<const SmFace*>> neighbors;
        for(ULONG e=0;e<edges.GetSize();++e){
            SmTArray<SmFace*> adjacent;edges[e]->GetFaces(adjacent);
            if(adjacent.GetSize()!=2 || adjacent[0]==adjacent[1])continue;
            const auto a=groups.find(adjacent[0]),b=groups.find(adjacent[1]);
            if(a!=groups.end() && b!=groups.end() && a->second==seed->second && b->second==seed->second){
                neighbors[adjacent[0]].push_back(adjacent[1]);neighbors[adjacent[1]].push_back(adjacent[0]);
            }
        }
        for(size_t q=0;q<queue.size();++q)for(auto *face:neighbors[queue[q]])if(connected.insert(face).second)queue.push_back(face);
    }
    size_t eligible=0;
    for (ULONG i=0;i<edges.GetSize();++i) {
        SmTArray<SmFace*> adjacent;
        edges[i]->GetFaces(adjacent);
        bool merge=false;
        if (adjacent.GetSize()==2 && adjacent[0]!=adjacent[1]) {
            const auto a=groups.find(adjacent[0]),b=groups.find(adjacent[1]);
            merge=a!=groups.end() && b!=groups.end() && a->second==b->second;
            if (selected) merge=merge && (region
                ? connected.count(adjacent[0]) && connected.count(adjacent[1])
                : requested.count(adjacent[0]) && requested.count(adjacent[1]));
        }
        if (!merge) keep.Add(edges[i]); else ++eligible;
    }
    if (!eligible) return 0;
    vb_validate_boundary_intersections(brep,tolerance,true);
    SmTArray<SmVertex*> keep_vertices;
    SmTArray<SmVertex*> vertices;
    brep.GetVertices(vertices);
    for (ULONG i=0;i<vertices.GetSize();++i) {
        SmTArray<SmEdge*> incident;
        vertices[i]->GetEdges(incident);
        bool protected_vertex=vertices[i]->IsLoopVertex();
        for (ULONG e=0;e<incident.GetSize()&&!protected_vertex;++e) {
            ULONG index=0;
            protected_vertex=keep.FindElement(incident[e],index);
        }
        if (protected_vertex) keep_vertices.Add(vertices[i]);
    }
    require(brep.RemoveTopologicalEdgesAndVertices(&keep,&keep_vertices)==SM_SUCCESS,
            "Merge coplanar native faces");
    SmTArray<SmFace*> result;
    brep.GetFaces(result);
    require(result.GetSize()>0 && result.GetSize()<=faces.GetSize(),"Invalid coplanar merge face count");
    vb_validate_boundary_intersections(brep,tolerance,true);
    return faces.GetSize()-result.GetSize();
}
