#include <common.h>
#include <fstream>
#include <iomanip>
#include <libcuemol2_api/loader.hpp>
#include <qsys/TTYView.hpp>

int main(int argc, char **argv) {
    if (argc != 3) return 2;
    if (cuemol2::init_qlib() < 0 || cuemol2::init(argv[1], false, false) < 0) return 3;
    std::ofstream out(argv[2]);
    out << std::setprecision(17);
    const auto emit = [&](qsys::TTYView &view) {
        const auto center = view.getViewCenter();
        out << view.getViewDist() << ' ' << view.getSlabDepth() << ' ' << view.getZoom()
            << ' ' << center.x() << ' ' << center.y() << ' ' << center.z() << '\n';
    };
    for (double distance : {0., 0.05, 0.1, 10., 10000., 20000.}) {
        qsys::TTYView view;
        view.resetAllProps();
        view.setViewDist(200.);
        view.setSlabDepth(300.);
        view.setViewDist(distance);
        emit(view);
        view.setSlabDepth(400.);
        emit(view);
        view.setSlabDepth(25000.);
        emit(view);
    }
    for (double zoom : {-1., 0., 0.005, 0.01, 100.}) {
        qsys::TTYView view;
        view.resetAllProps();
        view.setZoom(zoom);
        emit(view);
    }
    for (int axis = 0; axis < 3; ++axis) {
        qsys::TTYView view;
        view.resetAllProps();
        view.setViewSize(800, 200);
        view.setZoom(50.);
        view.rotateView(30., 45., 90.);
        view.translateView(axis == 0 ? 4. : 0., axis == 1 ? 4. : 0., axis == 2 ? 4. : 0.);
        emit(view);
    }
}
