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
extern "C" int vibo_subd_edge_loop(const uint8_t* bytes,size_t count,size_t seed,
    uint8_t chain_type,ViboSubDChains** out,char* error,size_t capacity) {
  return guard(out,error,capacity,[&] {
    require(chain_type>=1 && chain_type<=5,"Invalid SubD chain continuation type");
    auto subd=vibo_subd::decode(bytes,count);
    std::vector<const ON_SubDEdge*> edges;
    auto it=subd->EdgeIterator();
    for(auto* e=it.FirstEdge();e;e=it.NextEdge()) edges.push_back(e);
    require(seed<edges.size(),"SubD loop seed outside source table");
    std::sort(edges.begin(),edges.end(),[](auto* a,auto* b){return a->m_id<b->m_id;});
    std::unordered_map<const ON_SubDEdge*,size_t> ordinals;
    for(size_t i=0;i<edges.size();++i) ordinals.emplace(edges[i],i);
    const auto first=ON_SubDEdgePtr::Create(edges[seed],0);
    const auto type=static_cast<ON_SubD::ChainType>(chain_type);
    std::unordered_set<const ON_SubDEdge*> seen{edges[seed]};
    auto extend=[&](ON_ChainDirection direction) {
      std::vector<ON_SubDEdgePtr> result;
      auto current=first;
      while(result.size()<edges.size()) {
        const auto next=ON_SubDEdgeChain::EdgeChainNeighbor(current,direction,type);
        if(next.IsNull() || !seen.insert(next.Edge()).second) break;
        require(ordinals.count(next.Edge()),"SubD loop neighbor outside source");
        result.push_back(next);current=next;
      }
      return result;
    };
    auto after=extend(ON_ChainDirection::Next),before=extend(ON_ChainDirection::Previous);
    std::reverse(before.begin(),before.end());before.push_back(first);
    before.insert(before.end(),after.begin(),after.end());
    auto result=std::make_unique<ViboSubDChains>();
    const ON_SubDVertex* end=nullptr;
    for(const auto ptr:before) {
      require(ptr.RelativeVertex(0) && ptr.RelativeVertex(1) &&
        (!end || end==ptr.RelativeVertex(0)),"Discontinuous SubD loop");
      result->edges.push_back(2*ordinals.at(ptr.Edge())+ptr.EdgeDirection());
      end=ptr.RelativeVertex(1);
    }
    result->offsets.push_back(result->edges.size());return result;
  });
}
extern "C" void vibo_subd_edge_chains_free(ViboSubDChains* value) {delete value;}
#endif
