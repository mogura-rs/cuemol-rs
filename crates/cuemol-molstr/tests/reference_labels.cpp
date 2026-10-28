#include <common.h>
#include <fstream>
#include <iomanip>
#include <libcuemol2_api/loader.hpp>
#include <modules/molstr/AtomIterator.hpp>
#include <modules/molstr/LabelFormat.hpp>
#include <modules/molstr/MolCoord.hpp>
#include <modules/molstr/MolResidue.hpp>
#include <modules/molstr/PDBFileReader.hpp>

int main(int argc, char **argv) {
    if (argc != 6) return 2;
    if (cuemol2::init_qlib() < 0 || cuemol2::init(argv[1], false, false) < 0) return 3;
    {
        molstr::MolCoordPtr molecule(new molstr::MolCoord());
        molstr::PDBFileReader reader;
        reader.resetAllProps();
        reader.setPath(argv[2]);
        reader.attach(qsys::ObjectPtr(molecule));
        static_cast<qsys::ObjReader &>(reader).read();
        reader.detach();
        molecule->setName("sample");
        molstr::AtomIterator atoms(molecule);
        atoms.first();
        auto atom = atoms.get();
        atom->setBfac(std::stod(argv[5]));
        atom->setOcc(0.75);
        atom->setAtomPropInt("flag", -7);
        atom->setAtomPropReal("score", 1.25);
        atom->getParentResidue()->setPropStr("phase", "helix λ界");
        std::ifstream formats(argv[3]);
        std::ofstream output(argv[4]);
        output << atom->getID() << '\n';
        std::string format;
        while (std::getline(formats, format)) {
            try {
                const auto text = molstr::LabelFormat(format.c_str()).apply(atom);
                output << "ok ";
                for (unsigned char ch : std::string(text.c_str()))
                    output << std::hex << std::setw(2) << std::setfill('0') << int(ch);
                output << '\n';
            } catch (const qlib::LException &) {
                output << "error\n";
            }
        }
    }
    cuemol2::fini();
    cuemol2::fini_qlib();
}
