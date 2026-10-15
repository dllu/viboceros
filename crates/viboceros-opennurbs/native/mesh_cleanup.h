// Use the same executive as CullDegenerateFaces, including native repair of
// collapsed quads. Its face map distinguishes repair from actual deletion.
void cull_degenerate_faces(ON_Mesh &mesh,ViboMeshResult &result){
  const size_t face_count=mesh.FaceUnsignedCount(),vertex_count=mesh.VertexUnsignedCount();
  std::vector<bool> quads(face_count);
  std::vector<unsigned> face_map(face_count,ON_UNSET_UINT_INDEX);
  for(size_t f=0;f<face_count;++f)quads[f]=mesh.m_F[static_cast<int>(f)].IsQuad();
  require(mesh.DeleteComponents(nullptr,0,true,true,true,true,face_map.data()),"Native degenerate face cleanup failed");
  for(size_t f=0;f<face_count;++f){
    const auto index=face_map[f];
    if(index==ON_UNSET_UINT_INDEX){++result.removed_faces;continue;}
    require(index<mesh.FaceUnsignedCount(),"Invalid cleanup face map");
    if(quads[f] && mesh.m_F[static_cast<int>(index)].IsTriangle())++result.repaired_faces;
  }
  require(mesh.FaceUnsignedCount()<=face_count && result.removed_faces==face_count-mesh.FaceUnsignedCount() && mesh.VertexUnsignedCount()<=vertex_count,"Invalid cleanup counts");
  result.removed_vertices=vertex_count-mesh.VertexUnsignedCount();
  result.changed=result.removed_faces+result.repaired_faces+result.removed_vertices;
  // DeleteComponents can repair a quad without refreshing its n-gon's outer
  // loop. Rebuild only invalid overlays using the native boundary classifier.
  // Missing/disconnected boundaries dissolve the overlay, retaining the faces.
  const auto ngon_count=mesh.NgonUnsignedCount();
  ON_SimpleArray<unsigned> workspace;
  for(unsigned g=0;g<ngon_count;++g){
    const auto *ngon=mesh.Ngon(g);
    if(!ngon || ON_MeshNgon::IsValid(ngon,g,nullptr,mesh.VertexUnsignedCount(),mesh.FaceUnsignedCount(),mesh.m_F.Array(),workspace))continue;
    ON_SimpleArray<unsigned> faces;
    faces.Append(ngon->m_Fcount,ngon->m_fi);
    mesh.RemoveNgon(g);
    add_face_region_ngon(mesh,faces);
  }
  mesh.RemoveEmptyNgons();
  require(mesh.FaceCount()?mesh.IsValid():mesh.VertexCount()==0 && mesh.NgonCount()==0,"Invalid native cleanup output mesh");
}
