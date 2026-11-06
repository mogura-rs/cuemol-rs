#include <common.h>
#include <fstream>
#include <libcuemol2_api/loader.hpp>
#include <modules/xtal/DensityMap.hpp>
#include <qsys/TTYView.hpp>

int main(int argc, char **argv) {
    if (argc != 3) return 2;
    if (cuemol2::init_qlib() < 0 || cuemol2::init(argv[1], false, false) < 0) return 3;
    std::ofstream out(argv[2]);
    for (int i = 0; i < 16; ++i) {
        xtal::DensityMap map;
        unsigned char bytes[8] = {};
        map.setMapByteArray(bytes, 2, 2, 2, 0., 1., 0.5, 0.25);
        map.setMapParams(-3, 2, 4, 10, 12, 14);
        map.setXtalParams(10., 12., 14., 90., i & 1 ? 110. : 90., i & 1 ? 80. : 90.);
        if (i & 2) map.setOrigin(qlib::Vector4D(3., -2., 5.));
        if (i & 4) map.setXformMatrix(qlib::Matrix4D::makeTransMat(qlib::Vector4D(4., 5., 6.)));
        qsys::ViewPtr view(new qsys::TTYView());
        view->resetAllProps();
        view->setViewSize(100, 200);
        if (i & 8) view->rotateView(17., -31., 23.);
        out << map.getFitBoxJSON(view) << '\n';
    }
}
