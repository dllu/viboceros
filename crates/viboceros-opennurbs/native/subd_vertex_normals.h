#ifndef VIBOCEROS_SUBD_VERTEX_NORMALS_H
#define VIBOCEROS_SUBD_VERTEX_NORMALS_H
struct ViboSubDVertexNormals {std::vector<double> normals;};
extern "C" int vibo_subd_smooth_vertex_normals(const uint8_t* bytes,size_t count,
    const size_t* indices,size_t index_count,ViboSubDVertexNormals** out,char* error,size_t capacity) {
  return guard(out,error,capacity,[&]{
    require(index_count<=vibo_subd::component_limit && (index_count==0 || indices),
      "Invalid SubD normal selection");
    auto subd=vibo_subd::decode(bytes,count);
    std::vector<const ON_SubDVertex*> vertices;
    auto it=subd->VertexIterator();
    for(auto* vertex=it.FirstVertex();vertex;vertex=it.NextVertex()) vertices.push_back(vertex);
    std::sort(vertices.begin(),vertices.end(),[](auto* a,auto* b){return a->m_id<b->m_id;});
    for(size_t i=0;i<index_count;++i)
      require(indices[i]<vertices.size() && vertices[indices[i]]->IsSmooth(),
        "SubD limit normal requires a smooth source vertex");
    auto result=std::make_unique<ViboSubDVertexNormals>();result->normals.reserve(3*index_count);
    // A smooth vertex has one sector. Repeated requests reuse its native cache
    // without searching high-valence face fans for each incident corner.
    for(size_t i=0;i<index_count;++i) {
      auto normal=vertices[indices[i]]->SurfaceNormal(nullptr,true);
      require(normal.IsValid() && normal.Unitize(),"Undefined smooth SubD limit normal");
      result->normals.insert(result->normals.end(),{normal.x,normal.y,normal.z});
    }
    return result;
  });
}
extern "C" int vibo_subd_vertex_normals_view(const ViboSubDVertexNormals* r,ViboSubDVertexNormalsView* v) {
  if(!r || !v) return 0;*v={r->normals.size()/3,r->normals.data()};return 1;
}
extern "C" void vibo_subd_vertex_normals_free(ViboSubDVertexNormals* r) {delete r;}
#endif
