#include <common.h>
#include <fstream>
#include <iomanip>
#include <dlfcn.h>
#include <libcuemol2_api/loader.hpp>
#include <modules/molstr/AtomIterator.hpp>
#include <modules/molstr/MolCoord.hpp>
#include <modules/molstr/PDBFileReader.hpp>
#include <modules/surface/DirectSurfRenderer.hpp>
#include <modules/surface/edtsurf/CommonPara.h>
#include <modules/surface/edtsurf/ProteinSurface.h>

// Compare defined zero-initialized flags, not EDTSurf's allocator-dependent bits.
void edtsurf::ProteinSurface::laplaciansmooth(int iterations) {
    for (int i = 0; i < vertnumber; ++i) verts[i].inout = false;
    using Function = void (*)(ProteinSurface *, int);
    static auto original = reinterpret_cast<Function>(
        dlsym(RTLD_NEXT, "_ZN7edtsurf14ProteinSurface15laplaciansmoothEi"));
    if (!original) throw std::runtime_error("EDTSurf smoothing symbol is unavailable");
    original(this, iterations);
}

class SurfaceOracle : public surface::DirectSurfRenderer {
public:
    void write(std::ostream &out) {
        buildMeshCache();
        std::map<int, unsigned> atom_index;
        molstr::AtomIterator atoms(getClientMol());
        for (atoms.first(); atoms.hasMore(); atoms.next())
            atom_index[atoms.get()->getID()] = atom_index.size();
        out << m_verts.size() << ' ' << m_faces.size() << '\n';
        for (int i = 0; i < m_verts.size(); ++i) {
            const auto &v = m_verts[i];
            out << v.x << ' ' << v.y << ' ' << v.z << ' '
                << v.nx << ' ' << v.ny << ' ' << v.nz << ' '
                << (atom_index.count(v.info) ? atom_index.at(v.info) : UINT32_MAX) << '\n';
        }
        for (int i = 0; i < m_faces.size(); ++i) {
            const auto &f = m_faces[i];
            out << f.id1 << ' ' << f.id2 << ' ' << f.id3 << '\n';
        }
    }
};

int main(int argc, char **argv) {
    if (argc < 4) return 2;
    if (cuemol2::init_qlib() < 0 || cuemol2::init(argv[1], false, false) < 0) return 3;
    {
        molstr::MolCoordPtr molecule(new molstr::MolCoord());
        molstr::PDBFileReader reader;
        reader.resetAllProps();
        reader.setPath(argv[2]);
        reader.attach(qsys::ObjectPtr(molecule));
        static_cast<qsys::ObjReader &>(reader).read();
        reader.detach();
        auto *renderer = new SurfaceOracle();
        renderer->resetAllProps();
        molecule->attachRenderer(qsys::RendererPtr(renderer));
        for (int i = 4; i < argc; ++i) {
            const std::string setting(argv[i]);
            const auto split = setting.find('=');
            if (split == std::string::npos) return 4;
            qlib::LVariant value;
            value.setStringValue(setting.substr(split + 1).c_str());
            if (!renderer->setNestedProperty(setting.substr(0, split).c_str(), value)) return 5;
        }
        std::ofstream out(argv[3]);
        out << std::setprecision(17);
        renderer->write(out);
        out.close();
    }
    cuemol2::fini();
    cuemol2::fini_qlib();
}
