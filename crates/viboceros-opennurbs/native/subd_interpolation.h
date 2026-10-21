#ifndef VIBOCEROS_SUBD_INTERPOLATION_H
#define VIBOCEROS_SUBD_INTERPOLATION_H
#include "subd_codec.h"
#include <map>
#include <set>
#include <unordered_map>
struct ViboSubDWeights {
  std::vector<size_t> offsets;
  std::vector<uint32_t> columns;
  std::vector<double> values;
};
namespace vibo_subd {
inline std::unique_ptr<ViboSubDWeights> interpolation_weights(ON_SubD& subd) {
  auto result=std::make_unique<ViboSubDWeights>();result->offsets.push_back(0);
  std::vector<ON_SubDVertex*> vertices;
  std::unordered_map<const ON_SubDVertex*,uint32_t> indices;
  auto it=subd.VertexIterator();
  for(auto* v=it.FirstVertex();v;v=it.NextVertex()) {
    indices.emplace(v,static_cast<uint32_t>(vertices.size()));vertices.push_back(const_cast<ON_SubDVertex*>(v));
  }
  // Limit position is linear in control coordinates for fixed component tags
  // and sector coefficients. Evaluate local basis triples at the native limit,
  // without subtracting large model coordinates or approximating derivatives.
  for(auto* v:vertices) v->SetControlNetPoint(ON_3dPoint::Origin,false);
  subd.ClearEvaluationCache();
  uint64_t work=0;
  for(size_t row=0;row<vertices.size();++row) {
    auto* v=vertices[row];std::map<uint32_t,double> coefficients;
    if(v->m_vertex_tag==ON_SubDVertexTag::Corner) coefficients[static_cast<uint32_t>(row)]=1.;
    else if(v->m_vertex_tag==ON_SubDVertexTag::Crease && !v->IsSharp(false)) {
      // The cubic boundary/crease limit mask is independent of face sizes,
      // valence and normals. Avoid evaluating a huge adjacent n-gon per row.
      coefficients[static_cast<uint32_t>(row)]=4./6.;unsigned count=0;
      for(unsigned i=0;i<v->EdgeCount();++i) {
        const auto* e=v->Edge(i);
        if(e->m_edge_tag==ON_SubDEdgeTag::Crease) {
          const auto* other=e->m_vertex[e->m_vertex[0]==v?1:0];
          coefficients[indices.at(other)]+=1./6.;++count;
        }
      }
      require(count==2,"Crease interpolation requires two crease neighbors");
    } else {
      require(v->m_vertex_tag==ON_SubDVertexTag::Smooth || v->m_vertex_tag==ON_SubDVertexTag::Dart || v->m_vertex_tag==ON_SubDVertexTag::Crease,
        "Unsupported interpolation vertex tag");
      std::set<uint32_t> support{static_cast<uint32_t>(row)};
      for(unsigned i=0;i<v->FaceCount();++i) {
        const auto* face=v->Face(i);
        for(unsigned j=0;j<face->EdgeCount();++j) support.insert(indices.at(face->Vertex(j)));
      }
      work+=uint64_t(support.size())*uint64_t(support.size());
      require(work<=32000000,"SubD interpolation stencil work budget exceeded");
      std::vector<uint32_t> columns(support.begin(),support.end());
      for(size_t first=0;first<columns.size();first+=3) {
        const size_t end=std::min(first+3,columns.size());
        for(size_t j=first;j<end;++j) {
          ON_3dPoint basis=ON_3dPoint::Origin;basis[static_cast<int>(j-first)]=1.;
          vertices[columns[j]]->SetControlNetPoint(basis,true);
        }
        double point[3];require(v->GetSurfacePoint(point),"Cannot evaluate native interpolation stencil");
        for(size_t j=first;j<end;++j) {
          const double weight=point[j-first];
          require(std::isfinite(weight) && weight>=-1e-12,"Invalid native interpolation weight");
          if(weight!=0.) coefficients[columns[j]]=weight;
        }
        for(size_t j=first;j<end;++j) vertices[columns[j]]->SetControlNetPoint(ON_3dPoint::Origin,true);
      }
    }
    double sum=0.;
    for(const auto& item:coefficients) {
      result->columns.push_back(item.first);result->values.push_back(item.second);sum+=item.second;
    }
    require(std::abs(sum-1.)<=1e-10,"Interpolation stencil does not preserve translations");
    require(result->values.size()<=4000000,"SubD interpolation coefficient budget exceeded");
    result->offsets.push_back(result->values.size());
  }
  return result;
}
}
#endif
