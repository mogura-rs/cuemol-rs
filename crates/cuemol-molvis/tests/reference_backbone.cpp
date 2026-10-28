#include <common.h>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <string>
#include <libcuemol2_api/loader.hpp>
#include <modules/molstr/MolCoord.hpp>
#include <modules/molstr/PDBFileReader.hpp>
#include <modules/molstr/MolResidue.hpp>
#include <modules/molvis/RibbonRenderer.hpp>
#include <modules/molvis/Ribbon2Renderer.hpp>
#include <modules/molvis/TubeRenderer.hpp>
#include <modules/molvis/TubeMeshBuilder.hpp>
#include <modules/molvis/smospl/SmoothSpline.hpp>

template<class Base> class MeshOracle : public Base {
public:
    void write(std::ostream &out, bool faces, bool color) {
        auto &builder = this->tubeGpuBuilder();
        builder.startBuild(this->getSceneID());
        this->render(nullptr);
        builder.finish();
        if (faces) {
            const auto triangle = [&](int a, int b, int c) {
                for (int i : {a, b, c}) {
                    const auto p = builder.getPos(i);
                    out << p.x() << ' ' << p.y() << ' ' << p.z() << ' ';
                }
                out << '\n';
            };
            for (const auto &p : builder.getPrims()) {
                if (p.type == molvis::TubeMeshBuilder::PRIM_FAN) {
                    for (int j = 2; j < p.count; ++j) triangle(p.first, p.first + j - 1, p.first + j);
                } else {
                    const int count = p.type == molvis::TubeMeshBuilder::PRIM_BAND ? p.count * 2 : p.count;
                    const auto index = [&](int j) {
                        if (p.type != molvis::TubeMeshBuilder::PRIM_BAND) return p.first + j;
                        return j % 2 ? p.ref + (j / 2) * p.refStride : p.first + j / 2;
                    };
                    for (int j = 2; j < count; ++j) {
                        if (j % 2) triangle(index(j - 1), index(j - 2), index(j));
                        else triangle(index(j - 2), index(j - 1), index(j));
                    }
                }
            }
            return;
        }
        for (int i = 0; i < builder.getVertexSize(); ++i) {
            const auto p = builder.getPos(i);
            const auto n = builder.getNorm(i);
            out << p.x() << ' ' << p.y() << ' ' << p.z() << ' '
                << n.x() << ' ' << n.y() << ' ' << n.z();
            if (color) {
                const auto c = builder.getColor(i);
                out << ' ' << int(gfx::getRCode(c)) << ' ' << int(gfx::getGCode(c))
                    << ' ' << int(gfx::getBCode(c)) << ' ' << int(gfx::getACode(c));
            }
            out << '\n';
        }
    }
};

template<class Base> void mesh(molstr::MolCoordPtr mol, const char *style, std::ostream &out, int argc, char **argv) {
    auto *renderer = new MeshOracle<Base>();
    renderer->resetAllProps();
    mol->attachRenderer(qsys::RendererPtr(renderer));
    renderer->applyStyles(style);
    for (int i = 7; i < argc; ++i) {
        const std::string setting(argv[i]);
        const auto split = setting.find('=');
        if (split == std::string::npos) throw std::runtime_error("Expected PROPERTY=VALUE");
        qlib::LVariant value;
        value.setStringValue(setting.substr(split + 1).c_str());
        const auto key = setting.substr(0, split);
        const bool ok = key.rfind("object.", 0) == 0
            ? mol->setNestedProperty(key.substr(7).c_str(), value)
            : renderer->setNestedProperty(key.c_str(), value);
        if (!ok)
            throw std::runtime_error("Unknown property: " + setting);
    }
    renderer->write(out, std::string(argv[2]) == "faces", std::string(argv[2]) == "color");
}

int main(int argc, char **argv) {
    if (argc < 4) return 2;
    if (cuemol2::init_qlib() < 0 || cuemol2::init(argv[1], false, false) < 0) return 3;
    std::ofstream out(argv[3]);
    out << std::setprecision(17);
    const std::string mode(argv[2]);
    if ((mode == "mesh" || mode == "faces" || mode == "color") && argc >= 7) {
        molstr::MolCoordPtr mol(new molstr::MolCoord());
        molstr::PDBFileReader reader;
        reader.resetAllProps();
        reader.setPath(argv[4]);
        reader.attach(qsys::ObjectPtr(mol));
        static_cast<qsys::ObjReader &>(reader).read();
        reader.detach();
        const std::string kind(argv[5]);
        if (kind == "ribbon") mesh<molvis::RibbonRenderer>(mol, argv[6], out, argc, argv);
        else if (kind == "cartoon") mesh<molvis::Ribbon2Renderer>(mol, argv[6], out, argc, argv);
        else mesh<molvis::TubeRenderer>(mol, argv[6], out, argc, argv);
    } else if (mode == "profile") {
        int type, detail;
        double width, tuber, sharp;
        while (std::cin >> type >> width >> tuber >> sharp >> detail) {
            molvis::TubeSection section;
            section.setType(type); section.setWidth(width); section.setTuber(tuber);
            section.setSharp(sharp); section.setDetail(detail); section.setupSectionTable();
            out << section.getSize() << '\n';
            for (int i = 0; i < section.getSize(); ++i) {
                const auto v = section.getSectTab(i);
                out << v.x() << ' ' << v.y() << ' ' << v.z() << ' ' << v.w() << ' '
                    << int(section.getSfType(i)) << '\n';
            }
        }
    } else if (mode == "smooth") {
        int n, start, weighted;
        double rho;
        while (std::cin >> n >> rho >> start >> weighted) {
            molvis::SmoothSpline spline;
            spline.setSize(n); spline.setStartId(start); spline.setRho(rho);
            spline.setUseWeight(weighted != 0);
            for (int i = 0; i < n; ++i) {
                double x, y, z, w;
                std::cin >> x >> y >> z >> w;
                spline.setPoint(i + start, qlib::Vector4D(x, y, z, w));
            }
            if (!spline.generate()) return 4;
            for (int i = -4; i <= n * 4; ++i) {
                qlib::Vector4D p, d, dd;
                spline.interpolate(start + i * 0.25, &p, &d, &dd);
                out << p.x() << ' ' << p.y() << ' ' << p.z() << ' '
                    << d.x() << ' ' << d.y() << ' ' << d.z() << ' '
                    << dd.x() << ' ' << dd.y() << ' ' << dd.z() << '\n';
            }
        }
    } else return 2;
    out.close();
    cuemol2::fini();
    cuemol2::fini_qlib();
}
