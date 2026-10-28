#include <common.h>
#include <fstream>
#include <libcuemol2_api/loader.hpp>
#include <modules/xtal/DensityMap.hpp>
#include <modules/xtal/MapRenderer.hpp>
#include <modules/xtal/MapLod.hpp>
#include <qsys/Scene.hpp>
#include <qsys/SceneManager.hpp>
#include <vector>

int main(int argc, char **argv) {
    if (argc != 3) return 2;
    if (cuemol2::init_qlib() < 0 || cuemol2::init(argv[1], false, false) < 0) return 3;
    std::ofstream out(argv[2]);
    for (int kind = 0; kind < 3; ++kind) {
        auto scene = qsys::SceneManager::getInstance()->createScene();
        auto *map = new xtal::DensityMap();
        std::vector<unsigned char> bytes(33 * 35 * 37, 128);
        map->setMapByteArray(bytes.data(), 33, 35, 37, 0., 256., 128., 10.);
        map->setMapParams(2, -3, 5, 40, 40, 40);
        map->setXtalParams(40., 48., 56., kind ? 83. : 90., kind ? 77. : 90., kind ? 120. : 90.);
        map->setOrigin(qlib::Vector4D(7., -5., 3.));
        qlib::Matrix4D transform;
        if (kind == 2) {
            transform.aij(1,1) = 0.; transform.aij(1,2) = -2.;
            transform.aij(2,1) = 2.; transform.aij(2,2) = 0.;
            transform.aij(3,3) = 1.5;
            transform.setTransPart(qlib::Vector4D(20., -10., 5.));
            map->setXformMatrix(transform);
        }
        qsys::ObjectPtr object(map);
        scene->addObject(object);
        auto base = object->createRenderer("isosurf");
        auto *renderer = dynamic_cast<xtal::MapRenderer *>(base.get());
        renderer->setRegionMode(xtal::MapRenderer::REGION_FULL);
        for (int phase = 0; phase < 7; ++phase) {
            const double half[] = {100., 3., 8., 0., 5., 5., 5.};
            qlib::Vector4D center = phase == 3 ? qlib::Vector4D(1000.,1000.,1000.) : qlib::Vector4D(14.,10.,24.);
            renderer->setViewBox(center, half[phase]);
            renderer->setZoomRefine(phase != 5);
            renderer->setLod(phase == 4 ? 4 : phase == 6 ? 8 : 0);
            int lo[3], hi[3], step;
            renderer->computeFullRegion(map, lo, hi, step);
            const int st[] = {2,-3,5}, n[] = {33,35,37};
            for (int a=0; a<3; ++a) {
                auto range = xtal::lodAlignRange(lo[a], hi[a], st[a], n[a], step);
                out << range.start-st[a] << ' ' << range.start-st[a]+range.span+1 << ' ';
            }
            out << step << '\n';
        }
        qsys::SceneManager::getInstance()->destroyScene(scene->getUID());
    }
    for (long long n : {1LL, 33LL, 256LL, 512LL, 10000LL}) {
        for (long long budget : {1LL, 1048576LL, 16777216LL}) {
            out << xtal::lodStepForBudget(n,n+2,n+4,budget) << '\n';
        }
    }
    cuemol2::fini();
    cuemol2::fini_qlib();
}
