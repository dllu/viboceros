#ifndef VIBOCEROS_SUBD_ARCHIVE_SHARPNESS_H
#define VIBOCEROS_SUBD_ARCHIVE_SHARPNESS_H
#include <cmath>
#include <limits>
#include <unordered_set>
namespace vibo_subd {
struct SharpnessRecord { uint32_t id; double ends[2]; };

// Native sharpness is stored as floats. A valid input just outside the native
// snapping interval can round inside that interval when stored. Read() applies
// the snapping rule again and can change the surface. Recover an input that
// the public factory stores as exactly the original float; never access private
// sharpness storage or change the public kernel's snapping tolerance.
inline double stored_sharpness_input(double value) {
  require(std::isfinite(value) && value>=0. && value<=ON_SubDEdgeSharpness::MaximumValue,
    "Invalid stored edge sharpness");
  if(ON_SubDEdgeSharpness::FromConstant(value).EndSharpness(0)==value) return value;
  require(static_cast<double>(static_cast<float>(value))==value,
    "Stored edge sharpness is not a native float");
  const double nearest=std::floor(value+0.5);
  const double direction=value<nearest ? -1. : 1.;
  const double boundary=nearest+direction*ON_SubDEdgeSharpness::Tolerance;
  const double input=std::nextafter(boundary,direction*std::numeric_limits<double>::infinity());
  require(ON_SubDEdgeSharpness::FromConstant(input).EndSharpness(0)==value,
    "Cannot preserve stored edge sharpness");
  return input;
}
inline std::vector<SharpnessRecord> sharpness_records(const ON_SubD& subd) {
  std::vector<SharpnessRecord> records;
  auto it=subd.EdgeIterator();
  for(auto* e=it.FirstEdge();e;e=it.NextEdge()) {
    const auto s=e->Sharpness(false);
    if(!s.IsSharp()) continue;
    if(ON_SubDEdgeSharpness::FromInterval(s.EndSharpness(0),s.EndSharpness(1))!=s)
      records.push_back({e->m_id,{s.EndSharpness(0),s.EndSharpness(1)}});
  }
  return records;
}
inline void sharpness_double(std::vector<uint8_t>& bytes,double value) {
  static_assert(sizeof(double)==sizeof(uint64_t) && std::numeric_limits<double>::is_iec559);
  uint64_t bits;std::memcpy(&bits,&value,sizeof(bits));
  for(unsigned i=0;i<8;++i) bytes.push_back(static_cast<uint8_t>(bits>>(8*i)));
}
inline double sharpness_double(const uint8_t* bytes) {
  uint64_t bits=0;for(unsigned i=0;i<8;++i) bits|=uint64_t(bytes[i])<<(8*i);
  double value;std::memcpy(&value,&bits,sizeof(value));return value;
}
inline void restore_sharpness(ON_SubD& subd,const uint8_t* bytes,size_t count) {
  require(count<=subd.EdgeCount(),"Excessive stored sharpness records");
  std::unordered_set<unsigned> seen;
  for(size_t i=0;i<count;++i,bytes+=20) {
    const auto id=u32(bytes);
    require(seen.insert(id).second,"Repeated stored sharpness edge");
    auto* edge=const_cast<ON_SubDEdge*>(subd.EdgeFromId(id));
    require(edge && edge->IsSmooth(),"Stored sharpness edge is missing or creased");
    const double a=sharpness_double(bytes+4),b=sharpness_double(bytes+12);
    const auto value=ON_SubDEdgeSharpness::FromInterval(stored_sharpness_input(a),stored_sharpness_input(b));
    require(value.EndSharpness(0)==a && value.EndSharpness(1)==b,
      "Stored sharpness restoration failed");
    edge->SetSharpnessForExperts(value);
  }
  if(count) subd.ClearEvaluationCache();
}
}
#endif
