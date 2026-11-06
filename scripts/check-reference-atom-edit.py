#!/usr/bin/env python3
"""Run the current reference's molecule deletion/merge regressions read-only."""
import argparse
import os
from pathlib import Path
import shlex
import subprocess

ROOT = Path(__file__).resolve().parent.parent
PREAMBLE = r'''
#include <cmath>
#include <iostream>
#include <stdexcept>
#include <libcuemol2_api/loader.hpp>
struct Check {
    Check(bool value, const char *expression) {
        if (!value) throw std::runtime_error(expression);
    }
    template<class T> Check &operator<<(const T &) { return *this; }
};
#define TEST(suite, name) void suite##_##name()
#define EXPECT_TRUE(value) Check(bool(value), #value)
#define ASSERT_TRUE(value) EXPECT_TRUE(value)
#define EXPECT_FALSE(value) Check(!bool(value), "not " #value)
#define ASSERT_FALSE(value) EXPECT_FALSE(value)
#define EXPECT_EQ(a, b) Check((a) == (b), #a " == " #b)
#define ASSERT_EQ(a, b) EXPECT_EQ(a, b)
#define ASSERT_LT(a, b) Check((a) < (b), #a " < " #b)
#define EXPECT_NEAR(a, b, epsilon) Check(std::abs((a) - (b)) <= (epsilon), #a " ~= " #b)
'''
MAIN = r'''
int main(int argc, char **argv) {
    if (argc != 2 || cuemol2::init_qlib() < 0 || cuemol2::init(argv[1], false, false) < 0) return 2;
    try {
        MolDeleteUndo_PartialResidueComesBackUnderItsIDs();
        std::cout << "Partial residue and coordinate history passed\n";
        MolDeleteUndo_NeighbourhoodSelectionComesBackWhole();
        std::cout << "Recorded neighbourhood passed\n";
        MolDeleteUndo_AltConfsDraggedAlongComeBack();
        std::cout << "Alternate conformations passed\n";
        MolDeleteUndo_MergeRedoKeepsIDsForLaterEdits();
        std::cout << "Merge identities and later deletion passed\n";
    } catch (const std::exception &error) {
        std::cerr << error.what() << '\n';
        return 1;
    }
    return 0;
}
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reference', type=Path, default=ROOT.parent / 'cuemol2')
    parser.add_argument('--build-dir', type=Path, required=True)
    parser.add_argument('--config', type=Path, required=True)
    args = parser.parse_args()
    reference, build, config = args.reference.resolve(), args.build_dir.resolve(), args.config.resolve()
    source = reference / 'src/tests/modules/molstr/test_mol_delete_undo.cpp'
    work = ROOT / '.cache/atom-edit-reference'
    work.mkdir(parents=True, exist_ok=True)
    probe = work / 'probe.cpp'
    probe.write_text(PREAMBLE + source.read_text().replace('#include <gtest/gtest.h>', '') + MAIN)
    binary = work / 'probe'
    subprocess.run(shlex.split(os.environ.get('CXX', 'c++')) + [
        '-O2', '-std=c++20', '-DHAVE_CONFIG_H', '-DNO_SCRIPT',
        '-I' + str(reference / 'src'), '-I' + str(reference / 'src/modules'),
        '-I' + str(build / 'src'), '-I' + str(build / '_deps/spdlog-src/include'),
        str(probe), '-L' + str(build / 'src'), '-lcuemol2',
        '-Wl,-rpath,' + str(build / 'src'), '-o', str(binary)], check=True, cwd=ROOT)
    subprocess.run([str(binary), str(config)], check=True, cwd=ROOT)


if __name__ == '__main__':
    main()
