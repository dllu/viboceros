#ifndef VIBOCEROS_SUBD_CHAINS_H
#define VIBOCEROS_SUBD_CHAINS_H
struct ViboSubDChains { std::vector<size_t> offsets{0},edges; };
extern "C" int vibo_subd_edge_chains(const uint8_t* bytes,size_t count,
    const size_t* indices,size_t edge_count,ViboSubDChains** out,char* error,size_t capacity) {
  return guard(out,error,capacity,[&] {
    require(edge_count<=4*vibo_subd::component_limit && (edge_count==0 || indices),
      "Invalid or excessive SubD chain selection");
    auto subd=vibo_subd::decode(bytes,count);
    std::vector<const ON_SubDEdge*> edges;
    auto it=subd->EdgeIterator();
    for(auto* e=it.FirstEdge();e;e=it.NextEdge()) edges.push_back(e);
    std::sort(edges.begin(),edges.end(),[](auto* a,auto* b){return a->m_id<b->m_id;});
    std::vector<size_t> ordered;
    for(size_t i=0;i<edge_count;++i) {
      require(indices[i]<edges.size(),"SubD chain edge outside source table");
      ordered.push_back(indices[i]);
    }
    std::sort(ordered.begin(),ordered.end());
    require(std::adjacent_find(ordered.begin(),ordered.end())==ordered.end(),"Repeated SubD chain edge");
    ON_SimpleArray<ON_SubDEdgePtr> selected,sorted;
    std::unordered_map<const ON_SubDEdge*,size_t> ordinals;
    for(const auto i:ordered) {
      selected.Append(ON_SubDEdgePtr::Create(edges[i],0));
      ordinals.emplace(edges[i],i);
    }
    auto result=std::make_unique<ViboSubDChains>();
    if(edge_count==0) return result;
    // The public sorter temporarily marks components; it only sees our owned
    // decoded copy. Use the edge-pointer overload, without filtering components.
    const unsigned chains=ON_SubDEdgeChain::SortEdgesIntoEdgeChains(selected,sorted);
    std::unordered_set<size_t> seen;
    const ON_SubDVertex* end=nullptr;
    for(int i=0;i<sorted.Count();++i) {
      const auto ptr=sorted[i];
      if(ptr.IsNull()) {
        if(result->offsets.back()!=result->edges.size()) result->offsets.push_back(result->edges.size());
        end=nullptr;continue;
      }
      const auto found=ordinals.find(ptr.Edge());
      require(found!=ordinals.end() && seen.insert(found->second).second,"Invalid native SubD chain partition");
      require(ptr.RelativeVertex(0) && ptr.RelativeVertex(1) &&
        (!end || end==ptr.RelativeVertex(0)),"Discontinuous native SubD chain");
      result->edges.push_back(2*found->second+ptr.EdgeDirection());
      end=ptr.RelativeVertex(1);
    }
    if(result->offsets.back()!=result->edges.size()) result->offsets.push_back(result->edges.size());
    require(seen.size()==edge_count && result->offsets.size()==size_t(chains)+1,
      "Incomplete native SubD chain partition");
    return result;
  });
}
extern "C" int vibo_subd_edge_chains_view(const ViboSubDChains* value,ViboSubDChainsView* out) {
  if(!value || !out) return 0;
  *out={value->offsets.size()-1,value->edges.size(),value->offsets.data(),value->edges.data()};
  return 1;
}
extern "C" void vibo_subd_edge_chains_free(ViboSubDChains* value) {delete value;}
#endif
