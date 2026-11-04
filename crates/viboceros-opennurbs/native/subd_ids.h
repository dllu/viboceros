#ifndef VIBOCEROS_SUBD_IDS_H
#define VIBOCEROS_SUBD_IDS_H
struct ViboSubDIdTable { std::vector<uint32_t> vertices,edges,faces; };
extern "C" int vibo_subd_component_ids(const uint8_t* bytes,size_t count,
    ViboSubDIdTable** out,char* error,size_t capacity) {
  return guard(out,error,capacity,[&] {
    auto subd=vibo_subd::decode(bytes,count);
    auto result=std::make_unique<ViboSubDIdTable>();
    auto vi=subd->VertexIterator();
    for(auto* v=vi.FirstVertex();v;v=vi.NextVertex()) result->vertices.push_back(v->m_id);
    auto ei=subd->EdgeIterator();
    for(auto* e=ei.FirstEdge();e;e=ei.NextEdge()) result->edges.push_back(e->m_id);
    auto fi=subd->FaceIterator();
    for(auto* f=fi.FirstFace();f;f=fi.NextFace()) result->faces.push_back(f->m_id);
    for(auto* ids:{&result->vertices,&result->edges,&result->faces}) std::sort(ids->begin(),ids->end());
    return result;
  });
}
extern "C" int vibo_subd_component_ids_view(const ViboSubDIdTable* value,ViboSubDIdView* out) {
  if(!value || !out) return 0;
  *out={value->vertices.size(),value->edges.size(),value->faces.size(),
    value->vertices.data(),value->edges.data(),value->faces.data()};return 1;
}
extern "C" void vibo_subd_component_ids_free(ViboSubDIdTable* value) {delete value;}
#endif
