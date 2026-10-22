#ifndef VIBOCEROS_SUBD_CONTROL_MESH_H
#define VIBOCEROS_SUBD_CONTROL_MESH_H
// Public conversion retains crease seams or texture corners, including n-gons.
extern "C" int vibo_subd_control_mesh(const uint8_t* bytes,size_t count,uint32_t priority,
    ViboSubDResult** out,char* error,size_t capacity) {
  return guard(out,error,capacity,[&]{
    require(priority<=1,"Invalid SubD control mesh priority");
    auto subd=vibo_subd::decode(bytes,count);ON_Mesh mesh;
    require(subd->GetControlNetMesh(&mesh,static_cast<ON_SubDGetControlNetMeshPriority>(priority)),
      "Cannot get SubD control mesh");
    require(mesh.IsValid() && mesh.VertexCount()<=4000000 && mesh.FaceCount()<=1000000,
      "Invalid or excessive SubD control mesh");
    auto result=std::make_unique<ViboSubDResult>();
    for(int i=0;i<mesh.VertexCount();++i) {
      point(result->points,mesh.Vertex(i));point(result->surface_points,mesh.Vertex(i));
      result->vertex_tags.push_back(0);
      if(mesh.HasTextureCoordinates()) {
        const auto t=mesh.m_T[i];
        require(std::isfinite(t.x) && std::isfinite(t.y),"Invalid control mesh texture coordinate");
        result->mesh_texture_coordinates.insert(result->mesh_texture_coordinates.end(),{t.x,t.y});
      }
      if(mesh.HasVertexNormals()) {
        const auto n=mesh.m_N[i];
        require(std::isfinite(n.x) && std::isfinite(n.y) && std::isfinite(n.z),"Invalid control mesh normal");
        result->mesh_vertex_normals.insert(result->mesh_vertex_normals.end(),{n.x,n.y,n.z});
      }
    }
    for(int i=0;i<mesh.FaceCount();++i) {
      const auto& face=mesh.m_F[i];const unsigned n=face.IsTriangle()?3:4;result->face_sizes.push_back(n);
      for(unsigned j=0;j<n;++j) result->face_vertices.push_back(static_cast<uint32_t>(face.vi[j]));
    }
    for(unsigned i=0;i<mesh.NgonCount();++i) {
      const auto* ngon=mesh.Ngon(i);if(!ngon)continue;
      result->mesh_ngons.push_back({result->mesh_ngon_vertices.size(),ngon->m_Vcount,
        result->mesh_ngon_faces.size(),ngon->m_Fcount});
      result->mesh_ngon_vertices.insert(result->mesh_ngon_vertices.end(),ngon->m_vi,ngon->m_vi+ngon->m_Vcount);
      result->mesh_ngon_faces.insert(result->mesh_ngon_faces.end(),ngon->m_fi,ngon->m_fi+ngon->m_Fcount);
    }
    return result;
  });
}
#endif
