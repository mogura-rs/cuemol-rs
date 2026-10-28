#include <common.h>
#include <fstream>
#include <iomanip>
#include <libcuemol2_api/loader.hpp>
#include <modules/surface/ElePotMap.hpp>
#include <modules/xtal/DensityMap.hpp>
#include <modules/xtal/MapRenderer.hpp>
#include <qlib/LVariant.hpp>
#include <qsys/Scene.hpp>
#include <qsys/SceneManager.hpp>
#include <vector>

int main(int argc, char **argv) {
    if (argc != 3) return 2;
    if (cuemol2::init_qlib() < 0 || cuemol2::init(argv[1], false, false) < 0) return 3;
    std::ofstream out(argv[2]);
    out << std::setprecision(17);
    for (int kind = 0; kind < 5; ++kind) {
        auto scene = qsys::SceneManager::getInstance()->createScene();
        qsys::ObjectPtr object;
        if (kind == 2) {
            std::vector<float> values(4096);
            for (int i = 0; i < 4096; ++i) values[i] = float(i % 17 - 8) * 0.125f;
            auto *map = new surface::ElePotMap();
            map->setMapFloatArray(values.data(), 16, 16, 16, 1., 1., 1., qlib::Vector4D());
            object = qsys::ObjectPtr(map);
        } else {
            std::vector<unsigned char> bytes(4096, 0);
            for (int i = 0; i < 4096; ++i) bytes[i] = kind == 4 ? i % 256 : kind == 3 ? 0 : i < 10 ? 230 + i : 0;
            auto *map = new xtal::DensityMap();
            map->setMapByteArray(bytes.data(), 16, 16, 16,
                kind == 3 ? 5. : kind == 4 ? 0. : -2.,
                kind == 3 ? 5. : kind == 4 ? 2.56e-18 : 254.,
                kind == 3 ? 5. : kind == 4 ? 1.27e-18 : 0.5,
                kind == 3 ? 0. : kind == 4 ? 7.4e-19 : 10.);
            map->setMapParams(0, 0, 0, 16, 16, 16);
            map->setXtalParams(16, 16, 16, 90, 90, 90);
            map->setDetectedMapType(kind == 1 || kind == 3 ? 2 : 1);
            object = qsys::ObjectPtr(map);
        }
        scene->addObject(object);
        auto base = object->createRenderer("isosurf");
        auto *renderer = dynamic_cast<xtal::MapRenderer *>(base.get());
        if (!renderer) return 4;
        for (int phase = 0; phase < 5; ++phase) {
            if (phase == 1) { qlib::LVariant value; value.setRealValue(1.1); renderer->setProperty("siglevel", value); }
            if (phase == 2) renderer->resetProperty("siglevel");
            if (phase == 3) renderer->setLevel(kind == 4 ? 1e-18 : 42.);
            if (phase == 4) renderer->setUseAbsLev(true);
            out << renderer->getSigLevel() << ' ' << renderer->getEffectiveSigLevelProp() << ' '
                << renderer->getLevel() << ' ' << renderer->getMinLevel() << ' ' << renderer->getMaxLevel() << ' '
                << renderer->getUiAbsLevelMin() << ' ' << renderer->getUiAbsLevelMax() << ' '
                << renderer->getUiLevelMin() << ' ' << renderer->getUiLevelMax() << ' '
                << renderer->isPropDefault("siglevel") << '\n';
        }
        qsys::SceneManager::getInstance()->destroyScene(scene->getUID());
    }
    cuemol2::fini();
    cuemol2::fini_qlib();
}
