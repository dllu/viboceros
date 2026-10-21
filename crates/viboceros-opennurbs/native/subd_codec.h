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
inline std::vector<uint8_t> encode(const ON_SubD& subd) {
  check(subd);
  constexpr int version=80;
  const unsigned library=ON::Version();
  ON_Write3dmBufferArchive archive(0, archive_limit-16, version, library);
  require(subd.Write(archive), "Cannot archive SubD");
  std::vector<uint8_t> bytes{'V','I','B','O','S','U','B','D'};
  u32(bytes,version); u32(bytes,library);
  const auto* p=static_cast<const uint8_t*>(archive.Buffer());
  bytes.insert(bytes.end(),p,p+archive.SizeOfArchive());
  return bytes;
}
inline std::unique_ptr<ON_SubD> decode(const uint8_t* bytes, size_t count) {
  require(bytes && count>16 && count<=archive_limit && std::memcmp(bytes,"VIBOSUBD",8)==0,
          "Invalid SubD archive envelope");
  require(u32(bytes+8)==80, "Unsupported SubD archive version");
  ON_Read3dmBufferArchive archive(count-16,bytes+16,false,80,u32(bytes+12));
  auto subd=std::make_unique<ON_SubD>();
  require(subd->Read(archive) && archive.CurrentPosition()==count-16, "Invalid SubD archive");
  check(*subd); return subd;
}
}
#endif
