#ifndef VIBOCEROS_MESH_MERGE_H
#define VIBOCEROS_MESH_MERGE_H
#include "mesh_merge_selection.h"
// Public OpenNURBS face-set merging. Raw endpoint identities avoid exposing
// an unrelated Rust edge order as a native topology-edge index.
extern "C" bool vibo_mesh_set_face_set_holes_allowed(bool);
namespace {
class ScopedMeshSingleBoundary {
  bool previous;
public:
  ScopedMeshSingleBoundary():previous(vibo_mesh_set_face_set_holes_allowed(false)) {}
  ~ScopedMeshSingleBoundary() { vibo_mesh_set_face_set_holes_allowed(previous); }
  ScopedMeshSingleBoundary(const ScopedMeshSingleBoundary&)=delete;
  ScopedMeshSingleBoundary& operator=(const ScopedMeshSingleBoundary&)=delete;
};
}
extern "C" int vibo_mesh_merge_components(
    const ViboMeshView* input, const ViboMeshComponent* selected, size_t count,
    ViboMeshResult** out, char* error, size_t capacity) {
  if(out) *out=nullptr;
  if(error && capacity) error[0]=0;
  try {
    require(input && out && selected && count && count<=vibo_mesh::vertex_limit,
            "Missing or excessive merge components");
    vibo_begin_open_nurbs();
    ON_Mesh mesh;
    vibo_mesh::import_mesh(*input,mesh);
    const auto& topology=mesh.Topology();
    require(topology.TopFaceCount()==mesh.FaceCount()
                && topology.m_topv_map.Count()==mesh.VertexCount()
                && topology.TopVertexCount()>=3,
            "Mesh merge topology is unavailable");
    ON_SimpleArray<ON_COMPONENT_INDEX> components;
    for(size_t i=0;i<count;++i) {
      const auto& component=selected[i];
      if(component.kind==0) {
        require(component.first<mesh.VertexUnsignedCount(),"Merge vertex index is out of range");
        components.Append(ON_COMPONENT_INDEX(ON_COMPONENT_INDEX::mesh_vertex,static_cast<int>(component.first)));
      } else if(component.kind==1) {
        require(component.first<mesh.VertexUnsignedCount() && component.second<mesh.VertexUnsignedCount(),
                "Merge edge endpoint is out of range");
        const int edge=topology.TopEdge(topology.m_topv_map[component.first],topology.m_topv_map[component.second]);
        require(edge>=0,"Merge endpoints do not identify a topology edge");
        components.Append(ON_COMPONENT_INDEX(ON_COMPONENT_INDEX::meshtop_edge,edge));
      } else {
        require(component.kind==2 && component.first<mesh.FaceUnsignedCount(),"Merge face index is out of range");
        components.Append(ON_COMPONENT_INDEX(ON_COMPONENT_INDEX::mesh_face,static_cast<int>(component.first)));
      }
    }
    remove_singleton_merge_components(mesh,topology,components);
    if(components.Count()==0) {
      *out=vibo_mesh_result_from_mesh(mesh);
      return 1;
    }
    const size_t before=logical_count(mesh);
    {
      // Reuse the upstream AddNgon single-boundary path for MergeFaces, while
      // leaving the existing hole-aware coplanar adapter unchanged.
      const ScopedMeshSingleBoundary single_boundary;
      mesh.MergeFaceSets(components);
    }
    ON_wString details;
    ON_TextLog log(details);
    if(!mesh.IsValid(&log)) {
      ON_String message(details);
      throw std::runtime_error(std::string("Invalid merged mesh: ")+message.Array());
    }
    const size_t after=logical_count(mesh);
    require(after<=before,"Mesh merging increased the logical face count");
    auto result=std::unique_ptr<ViboMeshResult>(vibo_mesh_result_from_mesh(mesh));
    result->changed=before-after;
    *out=result.release();
    return 1;
  } catch(const std::exception& e) {
    if(error && capacity) { std::strncpy(error,e.what(),capacity-1);error[capacity-1]=0; }
  } catch(...) {
    if(error && capacity) { std::strncpy(error,"Unknown mesh merging failure",capacity-1);error[capacity-1]=0; }
  }
  return 0;
}
#endif
