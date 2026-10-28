#include <common.h>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <libcuemol2_api/loader.hpp>
#include <qsys/TTYView.hpp>
#include <qsys/ViewInputConfig.hpp>
#include <qsys/InDevEvent.hpp>
#include <qsys/MomentumScroll.hpp>

int main(int argc, char **argv) {
    if (argc != 3) return 2;
    if (cuemol2::init_qlib() < 0 || cuemol2::init(argv[1], false, false) < 0) return 3;
    std::ofstream out(argv[2]);
    out << std::setprecision(17);
    qsys::TTYView view;
    view.resetAllProps();
    auto *config = qsys::ViewInputConfig::getInstance();
    std::string command;
    while (std::cin >> command) {
        if (command == "reset") {
            std::string style; double x, y, z, radius; int width, height;
            std::cin >> style >> width >> height >> x >> y >> z >> radius;
            config->resetAllProps(); config->applyStyle(style.c_str()); config->setTbRad(radius);
            view.setViewSize(width, height); view.setZoom(50); view.setViewDist(200);
            view.setSlabDepth(50); view.setViewCenter(qlib::Vector4D(1,2,3));
            view.setRotQuat(qlib::LQuat(1,0,0,0)); view.rotateView(x,y,z);
            qsys::InDevEvent event; view.mouseDragStart(event);
        } else if (command == "binding") {
            std::string key, value; std::cin >> key >> value;
            qlib::LVariant prop; prop.setStringValue(value == "-" ? "" : value.c_str());
            config->setProperty(key.c_str(), prop);
        } else {
            qsys::InDevEvent event;
            int modifiers, x, y, dx, dy;
            if (command == "drag") {
                std::cin >> modifiers >> x >> y >> dx >> dy;
                event.setModifier(modifiers); event.setX(x); event.setY(y);
                event.setDeltaX(dx); event.setDeltaY(dy); view.mouseDragMove(event);
            } else if (command == "wheel") {
                std::cin >> modifiers >> dx >> dy;
                event.setModifier(modifiers); event.setDeltaX(dx); event.setDeltaY(dy); view.mouseWheel(event);
            } else if (command == "gesture") {
                int axis; std::cin >> modifiers >> axis >> dy;
                event.setModifier(modifiers); event.setGestureAxis(axis); event.setDeltaY(dy); view.mouseGesture(event);
            } else if (command == "momentum") {
                int kind; double vx, vy, rho;
                std::cin >> kind >> x >> y >> vx >> vy >> rho;
                event.setX(x); event.setY(y); event.setVeloX(vx); event.setVeloY(vy);
                qsys::MomentumScroll scroll(&view);
                scroll.setType(kind == 1 ? qsys::MomentumScroll::MMS_ROTXY : qsys::MomentumScroll::MMS_TRANSXY);
                scroll.setupXY(event); scroll.onTimer(rho);
            } else if (command == "start") { view.mouseDragStart(event); }
            else return 4;
        }
        const auto center = view.getViewCenter(); const auto q = view.getRotQuat();
        out << center.x() << ' ' << center.y() << ' ' << center.z() << ' '
            << q.Vx() << ' ' << q.Vy() << ' ' << q.Vz() << ' ' << q.a() << ' '
            << view.getZoom() << ' ' << view.getViewDist() << ' ' << view.getSlabDepth() << '\n';
    }
}
