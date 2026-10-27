#include "USalignImpl.hpp"
#include <cstring>
#include <exception>

extern "C" int cuemol_usalign(const double *moving, const char *moving_names,
    unsigned moving_len, const double *reference, const char *reference_names,
    unsigned reference_len, const char *pivot, double *matrix,
    double *scores, char *error, unsigned error_len) noexcept {
  try {
    auto chain = [](const double *xyz, const char *names, unsigned len) {
      molanl::usalign::Chain result;
      result.xyz.assign(xyz, xyz + len * 3);
      for (unsigned i = 0; i < len; ++i) {
        const char code = molanl::usalign::residueCode(std::string(names + i * 3, 3));
        result.seq.push_back(code);
        result.molScore += (code >= 'a' && code <= 'z') ? 1 : -1;
      }
      return result;
    };
    auto result = molanl::usalign::align(chain(moving, moving_names, moving_len),
        chain(reference, reference_names, reference_len),
        molanl::usalign::padAtomName(pivot));
    for (int row = 0; row < 3; ++row) {
      for (int col = 0; col < 3; ++col) matrix[row * 4 + col] = result.u[row][col];
      matrix[row * 4 + 3] = result.t[row];
    }
    scores[0] = result.tmRef;
    scores[1] = result.tmMov;
    scores[2] = result.rmsd;
    scores[3] = result.nAligned;
    return 0;
  } catch (const std::exception &ex) {
    if (error_len) { std::strncpy(error, ex.what(), error_len - 1); error[error_len - 1] = 0; }
    return 1;
  } catch (...) {
    return 2;
  }
}
