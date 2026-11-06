#include <common.h>
#include <fstream>
#include <libcuemol2_api/loader.hpp>
#include <modules/molstr/MolCoord.hpp>
#include <qsys/Scene.hpp>
#include <qsys/SceneManager.hpp>
#include <qsys/TTYView.hpp>
#include <qlib/LVariant.hpp>

int main(int argc, char **argv) {
    if (argc != 3) return 2;
    if (cuemol2::init_qlib() < 0 || cuemol2::init(argv[1], false, false) < 0) return 3;
    std::ofstream out(argv[2]);
    const auto scene = qsys::SceneManager::getInstance()->createScene();
    qsys::ViewPtr view(new qsys::TTYView());
    view->resetAllProps();
    view->setSceneID(scene->getUID());
    molstr::MolCoordPtr molecule(new molstr::MolCoord());
    molecule->setProperty("name", qlib::LVariant("probe"));
    scene->addObject(molecule);
    scene->startUndoTxn("camera");
    view->setZoom(71);
    scene->commitUndoTxn();
    out << "camera-only=" << scene->getUndoSize() << ':' << view->getZoom() << '\n';
    scene->startUndoTxn("edit");
    molecule->setProperty("name", qlib::LVariant("changed"));
    view->setZoom(75);
    scene->commitUndoTxn();
    view->setSlabDepth(33);
    scene->undo(0);
    out << "undo=" << molecule->getName().c_str() << ':' << view->getZoom() << ':' << view->getSlabDepth() << '\n';
    view->setZoom(91);
    scene->redo(0);
    out << "redo=" << molecule->getName().c_str() << ':' << view->getZoom() << '\n';
    scene->startUndoTxn("rollback");
    molecule->setProperty("name", qlib::LVariant("cancelled"));
    view->setZoom(101);
    scene->rollbackUndoTxn();
    out << "rollback=" << molecule->getName().c_str() << ':' << view->getZoom() << '\n';
}
