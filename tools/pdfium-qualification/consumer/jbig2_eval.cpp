#include <algorithm>
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <iterator>
#include <string>
#include <vector>

#include "core/fxcodec/jbig2/jbig2_decoder.h"
#include "core/fxcodec/jbig2/jbig2_document_context.h"
#include "public/fpdfview.h"

namespace {

struct Result {
  std::string status;
  uint32_t width = 0;
  uint32_t height = 0;
  uint32_t channels = 1;
  std::vector<uint8_t> pixels;
};

std::vector<uint8_t> Read(const char* path) {
  std::ifstream file(path, std::ios::binary);
  if (!file) {
    std::perror(path);
    std::exit(2);
  }
  return {std::istreambuf_iterator<char>(file), {}};
}

Result Image(const std::vector<uint8_t>& input,
             const std::vector<uint8_t>& globals,
             uint32_t width,
             uint32_t height) {
  if (!width || !height || uint64_t{width} * height > 32000000) {
    return {"output_limit"};
  }
  uint32_t row = (width + 7) / 8;
  uint32_t pitch = (row + 3) & ~uint32_t{3};
  std::vector<uint8_t> padded(size_t{pitch} * height);
  fxcodec::Jbig2Context context;
  JBig2_DocumentContext document;
  auto status = fxcodec::Jbig2Decoder::StartDecode(
      &context, &document, width, height, input, 1, globals, 2, padded,
      pitch, nullptr, false);
  for (int i = 0; status == FXCODEC_STATUS::kDecodeToBeContinued && i < 10000; ++i) {
    status = fxcodec::Jbig2Decoder::ContinueDecode(&context, nullptr);
  }
  if (status != FXCODEC_STATUS::kDecodeFinished) {
    return {"decode_error"};
  }
  Result result{"ok", width, height};
  result.pixels.reserve(size_t{row} * height);
  for (uint32_t y = 0; y < height; ++y) {
    auto begin = padded.begin() + size_t{y} * pitch;
    result.pixels.insert(result.pixels.end(), begin, begin + row);
    if (width % 8) {
      result.pixels.back() |= (1 << (8 - width % 8)) - 1;
    }
  }
  return result;
}

Result Pdf(const std::vector<uint8_t>& input) {
  FPDF_DOCUMENT doc = FPDF_LoadMemDocument64(input.data(), input.size(), nullptr);
  if (!doc) {
    return {"document_error"};
  }
  Result result{"ok"};
  result.channels = 4;
  int count = FPDF_GetPageCount(doc);
  for (int i = 0; i < count; ++i) {
    FPDF_PAGE page = FPDF_LoadPage(doc, i);
    if (!page) {
      result.status = "page_error";
      break;
    }
    int w = static_cast<int>(FPDF_GetPageWidthF(page) + 0.5f);
    int h = static_cast<int>(FPDF_GetPageHeightF(page) + 0.5f);
    if (w <= 0 || h <= 0 || uint64_t(w) * h > 32000000 ||
        result.pixels.size() + uint64_t(w) * h * 4 > 512 * 1024 * 1024) {
      FPDF_ClosePage(page);
      result.status = "output_limit";
      break;
    }
    FPDF_BITMAP bitmap = FPDFBitmap_Create(w, h, 1);
    if (!bitmap) {
      FPDF_ClosePage(page);
      result.status = "bitmap_error";
      break;
    }
    FPDFBitmap_FillRect(bitmap, 0, 0, w, h, 0xffffffff);
    FPDF_RenderPageBitmap(bitmap, page, 0, 0, w, h, 0, FPDF_ANNOT);
    auto* data = static_cast<uint8_t*>(FPDFBitmap_GetBuffer(bitmap));
    int stride = FPDFBitmap_GetStride(bitmap);
    for (int y = 0; y < h; ++y) {
      result.pixels.insert(result.pixels.end(), data + y * stride,
                           data + y * stride + w * 4);
    }
    result.width = static_cast<uint32_t>(w);
    result.height += static_cast<uint32_t>(h);
    FPDFBitmap_Destroy(bitmap);
    FPDF_ClosePage(page);
  }
  FPDF_CloseDocument(doc);
  return result;
}

}  // namespace

int main(int argc, char** argv) {
  if (argc != 8) {
    std::fprintf(stderr, "image|pdf input globals|- output width height repetitions\n");
    return 2;
  }
  auto input = Read(argv[2]);
  auto globals = std::string(argv[3]) == "-" ? std::vector<uint8_t>{} : Read(argv[3]);
  bool pdf = std::string(argv[1]) == "pdf";
  if (pdf) {
    FPDF_LIBRARY_CONFIG config{};
    config.version = 2;
    FPDF_InitLibraryWithConfig(&config);
  }
  uint32_t width = std::strtoul(argv[5], nullptr, 10);
  uint32_t height = std::strtoul(argv[6], nullptr, 10);
  int runs = std::atoi(argv[7]);
  auto decode = [&]() { return pdf ? Pdf(input) : Image(input, globals, width, height); };
  Result result = decode();
  std::vector<double> times;
  if (result.status == "ok") {
    std::ofstream output(argv[4], std::ios::binary);
    output.write(reinterpret_cast<const char*>(result.pixels.data()), result.pixels.size());
    if (!output) {
      std::fprintf(stderr, "Cannot write pixels\n");
      return 2;
    }
    for (int i = 0; runs && i < 3; ++i) {
      if (decode().status != "ok") {
        std::fprintf(stderr, "Warmup failed\n");
        return 2;
      }
    }
    for (int i = 0; i < runs; ++i) {
      auto start = std::chrono::steady_clock::now();
      auto repeated = decode();
      double elapsed = std::chrono::duration<double, std::micro>(
                           std::chrono::steady_clock::now() - start).count();
      if (repeated.status != "ok" || repeated.pixels != result.pixels) {
        std::fprintf(stderr, "Non-repeatable pixels\n");
        return 2;
      }
      times.push_back(elapsed);
    }
  }
  std::sort(times.begin(), times.end());
  std::printf("%s\t%u\t%u\t%u\t%.3f\t%.3f\t%zu\n",
              result.status.c_str(), result.width, result.height, result.channels,
              times.empty() ? 0 : times[times.size() / 2],
              times.empty() ? 0 : times[times.size() * 9 / 10], times.size());
  if (pdf) {
    FPDF_DestroyLibrary();
  }
  return 0;
}
