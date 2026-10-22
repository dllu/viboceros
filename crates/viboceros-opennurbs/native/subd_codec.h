#ifndef VIBOCEROS_SUBD_CODEC_H
#define VIBOCEROS_SUBD_CODEC_H
// OpenNURBS owns the subdivision topology, tags, sharpness and archive schema.
// A small versioned envelope records the archive settings needed by Read().
#include "opennurbs_public.h"
#include <cstring>
#include <memory>
#include <stdexcept>
#include <vector>
namespace vibo_subd {
constexpr size_t archive_limit = 64U * 1024U * 1024U;
constexpr size_t component_limit = 100000;
inline void require(bool ok, const char* message) {
  if (!ok) throw std::runtime_error(message);
}
inline void check(const ON_SubD& subd) {
  require(subd.VertexCount() >= 3 && subd.VertexCount() <= component_limit &&
          subd.EdgeCount() <= component_limit * 4 && subd.FaceCount() > 0 &&
          subd.FaceCount() <= component_limit, "SubD component limit exceeded");
  require(subd.IsValid(), "Invalid SubD topology or geometry");
}
inline void u32(std::vector<uint8_t>& bytes, uint32_t n) {
  for (unsigned i=0; i<4; ++i) bytes.push_back(static_cast<uint8_t>(n >> (8*i)));
}
inline uint32_t u32(const uint8_t* p) {
  uint32_t n=0; for (unsigned i=0; i<4; ++i) n |= uint32_t(p[i]) << (8*i); return n;
}
}
#include "subd_archive_sharpness.h"
namespace vibo_subd {
inline std::vector<uint8_t> encode(const ON_SubD& subd) {
  check(subd);
  constexpr int version=80;
  const unsigned library=ON::Version();
  ON_Write3dmBufferArchive archive(0, archive_limit-16, version, library);
  require(subd.Write(archive), "Cannot archive SubD");
  const auto records=sharpness_records(subd);
  const size_t header=records.empty()?16:24;
  require(header+archive.SizeOfArchive()+20*records.size()<=archive_limit,"SubD archive limit exceeded");
  std::vector<uint8_t> bytes{'V','I','B','O','S','U','B',static_cast<uint8_t>(records.empty()?'D':'2')};
  u32(bytes,version); u32(bytes,library);
  if(!records.empty()) {u32(bytes,static_cast<uint32_t>(archive.SizeOfArchive()));u32(bytes,static_cast<uint32_t>(records.size()));}
  const auto* p=static_cast<const uint8_t*>(archive.Buffer());
  bytes.insert(bytes.end(),p,p+archive.SizeOfArchive());
  for(const auto& record:records) {u32(bytes,record.id);sharpness_double(bytes,record.ends[0]);sharpness_double(bytes,record.ends[1]);}
  return bytes;
}
inline std::unique_ptr<ON_SubD> decode(const uint8_t* bytes, size_t count) {
  require(bytes && count>16 && count<=archive_limit &&
          (std::memcmp(bytes,"VIBOSUBD",8)==0 || std::memcmp(bytes,"VIBOSUB2",8)==0),
          "Invalid SubD archive envelope");
  require(u32(bytes+8)==80, "Unsupported SubD archive version");
  const bool exact=bytes[7]=='2';
  const size_t header=exact?24:16;
  require(count>header,"Truncated SubD archive envelope");
  const size_t payload=exact?u32(bytes+16):count-header;
  const size_t records=exact?u32(bytes+20):0;
  require(payload>0 && payload<=count-header && records<=4*component_limit &&
    records*20==count-header-payload,"Invalid SubD archive sharpness envelope");
  ON_Read3dmBufferArchive archive(payload,bytes+header,false,80,u32(bytes+12));
  auto subd=std::make_unique<ON_SubD>();
  require(subd->Read(archive) && archive.CurrentPosition()==payload, "Invalid SubD archive");
  restore_sharpness(*subd,bytes+header+payload,records);
  check(*subd); return subd;
}
}
#endif
