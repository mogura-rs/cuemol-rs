#include <common.h>
#include <fstream>
#include <iomanip>
#include <libcuemol2_api/loader.hpp>
#include <modules/xtal/CCP4MapReader.hpp>
#include <modules/xtal/DensityMap.hpp>
#include <qlib/LVariant.hpp>

int main(int argc, char **argv) {
    if (argc != 8) return 2;
    if (cuemol2::init_qlib() < 0 || cuemol2::init(argv[1], false, false) < 0) return 3;
    {
        std::ofstream out(argv[3]);
        out << std::setprecision(17);
        try {
            qlib::LScrSp<xtal::DensityMap> map(new xtal::DensityMap());
            xtal::CCP4MapReader reader;
            reader.resetAllProps();
            qlib::LVariant value;
            value.setIntValue(std::stoi(argv[4])); reader.setProperty("subsample", value);
            value.setBoolValue(std::stoi(argv[5]) != 0); reader.setProperty("use_header_stats", value);
            const int transform = std::stoi(argv[6]);
            value.setBoolValue((transform & 1) != 0); reader.setProperty("normalize", value);
            value.setBoolValue((transform & 2) != 0);
            reader.setProperty("truncate_min", value); reader.setProperty("truncate_max", value);
            value.setRealValue(-0.5); reader.setProperty("min", value);
            value.setRealValue(1.5); reader.setProperty("max", value);
            value.setRealValue(std::stod(argv[7])); reader.setProperty("max_voxels", value);
            reader.setPath(argv[2]);
            reader.attach(qsys::ObjectPtr(map));
            static_cast<qsys::ObjReader &>(reader).read();
            reader.detach();
            out << "ok\n";
            out << map->getColNo() << ' ' << map->getRowNo() << ' ' << map->getSecNo() << '\n';
            out << map->getStartCol() << ' ' << map->getStartRow() << ' ' << map->getStartSec() << '\n';
            auto origin = map->getOrigin();
            auto corner = map->convToOrth(qlib::Vector4D(1, 2, 3));
            out << origin.x() << ' ' << origin.y() << ' ' << origin.z() << '\n';
            out << corner.x() << ' ' << corner.y() << ' ' << corner.z() << '\n';
            out << map->getDetectedMapType() << ' ' << map->getMinDensity() << ' ' << map->getMaxDensity()
                << ' ' << map->getMeanDensity() << ' ' << map->getRmsdDensity() << '\n';
            out << map->getLevelBase() << ' ' << map->getLevelStep() << '\n';
            auto sample = map->convToOrth(qlib::Vector4D(0.4, 0.6, 0.8));
            out << map->atFloat(0, 0, 0) << ' ' << map->getValueAt(sample) << ' ' << map->getQuantStep() << '\n';
            for (int z = 0; z < map->getSecNo(); ++z)
                for (int y = 0; y < map->getRowNo(); ++y)
                    for (int x = 0; x < map->getColNo(); ++x)
                        out << int(map->atByte(x, y, z)) << ' ';
            out << '\n';
        } catch (const qlib::LException &) { out << "error\n"; }
    }
    cuemol2::fini();
    cuemol2::fini_qlib();
}
