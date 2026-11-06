#include <common.h>
#include <fstream>
#include <memory>
#include <sstream>
#include <libcuemol2_api/loader.hpp>
#include <modules/anim/SimpleSpin.hpp>
#include <modules/anim/CamMotion.hpp>
#include <modules/anim/ShowHideAnim.hpp>
#include <qlib/LScrVector4D.hpp>
#include <qlib/LScriptable.hpp>
#include <qlib/LVariant.hpp>

std::string decode(const std::string &hex) {
    if (hex == "-") return "";
    std::string text;
    for (size_t i = 0; i < hex.size(); i += 2)
        text += static_cast<char>(std::stoul(hex.substr(i, 2), nullptr, 16));
    return text;
}

int main(int argc, char **argv) {
    if (argc != 4 || cuemol2::init_qlib() < 0 || cuemol2::init(argv[1], false, false) < 0) return 2;
    std::ifstream input(argv[2]);
    std::ofstream output(argv[3]);
    size_t index;
    std::string kind, prop, type, encoded;
    while (input >> index >> kind >> prop >> type >> encoded) {
        std::unique_ptr<qsys::AnimObj> object;
        if (kind == "spin") {
            auto spin = new anim::SimpleSpin;
            spin->setAxis(qlib::Vector4D(1, 0, 0));
            object.reset(spin);
        } else if (kind == "camera") object.reset(new anim::CamMotion);
        else object.reset(new anim::ShowHideAnim);
        bool success = false;
        try {
            auto text = decode(encoded);
            qlib::LVariant value;
            if (type == "string") value.setStringValue(text);
            else if (type == "number") value.setRealValue(std::stod(text));
            else if (type == "boolean") value.setBoolValue(text == "true");
            else if (type == "vector") {
                double x, y, z;
                std::istringstream(text) >> x >> y >> z;
                value.setObjectPtr(new qlib::LScrVector4D(x, y, z));
            }
            if (type != "error") success = object->setProperty(prop, value);
        } catch (...) {}
        output << "{\"index\":" << index << ",\"success\":" << (success ? "true" : "false")
               << ",\"properties\":" << qlib::getPropsJSONImpl(object.get()) << "}\n";
    }
    cuemol2::fini();
    cuemol2::fini_qlib();
}
