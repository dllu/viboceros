#ifndef VIBOCEROS_SUBD_SPIN_H
#define VIBOCEROS_SUBD_SPIN_H
// Public OpenNURBS rewiring retains the native component IDs and face data.
extern "C" int vibo_subd_spin_edge(const uint8_t* bytes,size_t count,size_t index,
    int clockwise,int* changed,ViboSubDResult** out,char* error,size_t capacity) {
  if(changed) *changed=0;
  return guard(out,error,capacity,[&] {
    require(changed && (clockwise==0 || clockwise==1),"Invalid SubD edge spin");
    auto subd=vibo_subd::decode(bytes,count);
    std::vector<ON_SubDEdge*> edges;auto it=subd->EdgeIterator();
    for(auto* e=it.FirstEdge();e;e=it.NextEdge()) edges.push_back(const_cast<ON_SubDEdge*>(e));
    std::sort(edges.begin(),edges.end(),[](auto* a,auto* b){return a->m_id<b->m_id;});
    require(index<edges.size(),"SubD spin edge outside its source table");
    if(subd->SpinEdge(edges[index],clockwise!=0)) {
      *changed=1;
      subd->UpdateAllTagsAndSectorCoefficients(false);subd->ClearEvaluationCache();
    }
    return extract(*subd);
  });
}
#endif
