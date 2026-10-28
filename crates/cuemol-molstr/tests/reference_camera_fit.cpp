#include <common.h>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <libcuemol2_api/loader.hpp>
#include <modules/molstr/MolCoord.hpp>
#include <modules/molstr/MolAtom.hpp>
#include <qsys/TTYView.hpp>

int main(int argc, char **argv) {
    if (argc != 3) return 2;
    if (cuemol2::init_qlib() < 0 || cuemol2::init(argv[1], false, false) < 0) return 3;
    std::ofstream out(argv[2]);
    out << std::setprecision(17);
    int orient, width, height, count;
    while (std::cin >> orient >> width >> height >> count) {
        molstr::MolCoordPtr molecule(new molstr::MolCoord());
        for (int i = 0; i < count; ++i) {
            double x, y, z;
            std::cin >> x >> y >> z;
            molstr::MolAtomPtr atom(new molstr::MolAtom());
            atom->setName("CA");
            atom->setPos(qlib::Vector4D(x,y,z));
            molecule->appendAtomScr1(atom, "A", i + 1, "ALA");
        }
        qsys::ViewPtr viewPtr(new qsys::TTYView());
        auto &view = *viewPtr;
        view.resetAllProps();
        view.setViewSize(width, height);
        view.setViewCenter(qlib::Vector4D(1,2,3));
        view.rotateView(17,-31,23);
        if (orient) molecule->orientView(viewPtr, molstr::SelectionPtr());
        else molecule->fitView2(viewPtr, molstr::SelectionPtr());
        auto center = view.getViewCenter();
        auto q = view.getRotQuat();
        out << center.x() << ' ' << center.y() << ' ' << center.z() << ' '
            << q.Vx() << ' ' << q.Vy() << ' ' << q.Vz() << ' ' << q.a() << ' '
            << view.getZoom() << ' ' << view.getSlabDepth() << '\n';
    }
}
