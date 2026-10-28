#include <common.h>
#include <fstream>
#include <iomanip>
#include <libcuemol2_api/loader.hpp>
#include <modules/surface/ElePotMap.hpp>
#include <vector>

int main(int argc, char **argv) {
    if (argc != 4) return 2;
    if (cuemol2::init_qlib() < 0 || cuemol2::init(argv[1], false, false) < 0) return 3;
    {
        std::ifstream input(argv[2]);
        int nx, ny, nz;
        input >> nx >> ny >> nz;
        std::vector<float> values(size_t(nx) * ny * nz);
        for (auto &value : values) input >> value;
        surface::ElePotMap map;
        map.setMapFloatArray(values.data(), nx, ny, nz, 0.5, 2., 3., qlib::Vector4D(1, 2, 3));
        std::ofstream out(argv[3]);
        out << std::setprecision(17);
        out << map.getMinDensity() << ' ' << map.getMaxDensity() << ' ' << map.getMeanDensity()
            << ' ' << map.getRmsdDensity() << ' ' << map.getLevelBase() << ' ' << map.getLevelStep()
            << ' ' << map.getQuantStep() << '\n';
        for (int z = 0; z < nz; ++z)
            for (int y = 0; y < ny; ++y)
                for (int x = 0; x < nx; ++x)
                    out << map.atFloat(x, y, z) << ' ';
        out << '\n';
        for (int z = 0; z < nz; ++z)
            for (int y = 0; y < ny; ++y)
                for (int x = 0; x < nx; ++x)
                    out << int(map.atByte(x, y, z)) << ' ';
        out << '\n';
        for (double z : {-0.2, 0.4, 1.2, double(nz - 1)})
            for (double y : {-0.1, 0.7, 1.1, double(ny - 1)})
                for (double x : {-1.1, -0.2, 0.3, double(nx - 1)})
                    out << map.getValueAt(map.convToOrth(qlib::Vector4D(x, y, z))) << ' ';
        out << '\n';
        const double low = map.getMinDensity() - 0.1;
        const double high = map.getMaxDensity() + 0.1;
        out << map.getHistogramJSON(low, high, 31).c_str();
    }
    cuemol2::fini();
    cuemol2::fini_qlib();
}
