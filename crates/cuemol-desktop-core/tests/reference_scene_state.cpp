#include <common.h>
#include <fstream>
#include <libcuemol2_api/loader.hpp>
#include <qsys/Scene.hpp>
#include <qsys/SceneManager.hpp>

int main(int argc, char **argv) {
    if (argc != 3) return 2;
    if (cuemol2::init_qlib() < 0 || cuemol2::init(argv[1], false, false) < 0) return 3;
    std::ofstream out(argv[2]);
    const auto scene = qsys::SceneManager::getInstance()->createScene();
    const auto report = [&](const char *step) {
        out << step << ' ' << scene->isModified() << ' '
            << scene->isJustCreated() << ' ' << scene->getUndoSize() << '\n';
    };
    report("initial");
    scene->setName("untracked");
    report("untracked");
    scene->startUndoTxn("rename");
    scene->setName("edited");
    report("pending");
    scene->commitUndoTxn();
    report("committed");
    scene->undo(0);
    report("undo");
    scene->redo(0);
    report("redo");
    scene->clearUndoDataScr();
    report("cleared");
    scene->startUndoTxn("rollback");
    scene->setName("cancelled");
    scene->rollbackUndoTxn();
    report("rollback");
    scene->startUndoTxn("empty");
    scene->commitUndoTxn();
    report("empty");
}
