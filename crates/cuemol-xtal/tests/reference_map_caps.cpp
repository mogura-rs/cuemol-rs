#include <common.h>
#include <fstream>
#include <iomanip>
#include <libcuemol2_api/loader.hpp>
#include <modules/surface/MolSurfObj.hpp>
#include <modules/xtal/DensityMap.hpp>
#include <modules/xtal/MapSurfRenderer.hpp>
#include <qsys/Scene.hpp>
#include <qsys/SceneManager.hpp>

static double area(const surface::MolSurfObj &mesh) {
    double result = 0;
    for (int i = 0; i < mesh.getFaceSize(); ++i) {
        const auto &f = mesh.getFaceAt(i);
        const auto a = mesh.getVertAt(f.id1).v3d();
        const auto b = mesh.getVertAt(f.id2).v3d();
        const auto c = mesh.getVertAt(f.id3).v3d();
        result += (b-a).cross(c-a).length() * 0.5;
    }
    return result;
}
int main(int argc, char **argv) {
    if (argc != 3) return 2;
    if (cuemol2::init_qlib() < 0 || cuemol2::init(argv[1], false, false) < 0) return 3;
    std::ofstream out(argv[2]);
    out << std::setprecision(17);
    for (int mask = 0; mask < 256; ++mask) {
        auto scene = qsys::SceneManager::getInstance()->createScene();
        unsigned char values[8];
        for (int i = 0; i < 8; ++i) values[i] = mask & (1 << i) ? 64 : 192;
        auto *map = new xtal::DensityMap();
        map->setMapByteArray(values, 2, 2, 2, 0, 256, 128, 64);
        map->setMapParams(0, 0, 0, 2, 2, 2);
        map->setXtalParams(2, 2, 2, 90, 90, 90);
        qsys::ObjectPtr object(map);
        scene->addObject(object);
        auto base = object->createRenderer("isosurf");
        auto *renderer = dynamic_cast<xtal::MapSurfRenderer *>(base.get());
        renderer->setRegionMode(xtal::MapRenderer::REGION_FULL);
        renderer->setUseAbsLev(true);
        renderer->setSigLevel(2);
        renderer->setCapMode(xtal::MapSurfRenderer::CAP_OFF);
        auto open = renderer->generateSurfObj();
        auto *a = dynamic_cast<surface::MolSurfObj *>(open.get());
        renderer->setCapMode(xtal::MapSurfRenderer::CAP_ON);
        auto closed = renderer->generateSurfObj();
        auto *b = dynamic_cast<surface::MolSurfObj *>(closed.get());
        out << b->getFaceSize() - a->getFaceSize() << ' ' << area(*b) - area(*a) << '\n';
        qsys::SceneManager::getInstance()->destroyScene(scene->getUID());
    }
    cuemol2::fini();
    cuemol2::fini_qlib();
}
