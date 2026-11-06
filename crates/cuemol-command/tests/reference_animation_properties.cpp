#include <common.h>
#include <fstream>
#include <libcuemol2_api/loader.hpp>
#include <modules/anim/SimpleSpin.hpp>
#include <modules/anim/CamMotion.hpp>
#include <modules/anim/RealPropAnim.hpp>
#include <modules/anim/MolAnim.hpp>
#include <modules/anim/RendXformAnim.hpp>
#include <modules/anim/ShowHideAnim.hpp>
#include <modules/anim/SlideInOutAnim.hpp>
#include <modules/anim/NoopAnimObj.hpp>
#include <qlib/LScriptable.hpp>
#include <qlib/LVariant.hpp>

template<class T> void emit(std::ostream &out, const char *name) {
    T object;
    std::set<qlib::LString> properties;
    object.getPropNames(properties);
    for (int phase = 0; phase < 3; ++phase) {
        for (const auto &key : properties) {
            if (!object.hasPropDefault(key)) continue;
            if (phase == 1) {
                qlib::LVariant value;
                if (!object.getProperty(key, value) || !object.setProperty(key, value)) throw std::runtime_error("property write failed");
            } else if (phase == 2 && !object.resetProperty(key)) throw std::runtime_error("property reset failed");
        }
        out << "{\"type\":\"" << name << "\",\"phase\":\"" << (phase == 0 ? "initial" : phase == 1 ? "explicit" : "reset")
            << "\",\"properties\":" << qlib::getPropsJSONImpl(&object) << "}\n";
    }
}
int main(int argc, char **argv) {
    if (argc != 3 || cuemol2::init_qlib() < 0 || cuemol2::init(argv[1], false, false) < 0) return 2;
    std::ofstream out(argv[2]);
    emit<anim::NoopAnimObj>(out,"NoopAnimObj");
    emit<anim::SimpleSpin>(out,"SimpleSpin");
    emit<anim::CamMotion>(out,"CamMotion");
    emit<anim::RealPropAnim>(out,"RealPropAnim");
    emit<anim::MolAnim>(out,"MolAnim");
    emit<anim::RendXformAnim>(out,"RendXformAnim");
    emit<anim::ShowHideAnim>(out,"ShowHideAnim");
    emit<anim::SlideInOutAnim>(out,"SlideInOutAnim");
    cuemol2::fini();
    cuemol2::fini_qlib();
}
