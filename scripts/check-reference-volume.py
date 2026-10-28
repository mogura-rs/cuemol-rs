#!/usr/bin/env python3
"""Execute the unchanged C++ volume transfer function with captured texture upload."""
import argparse
import os
from pathlib import Path
import shlex
import subprocess

ROOT = Path(__file__).resolve().parent.parent


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reference', type=Path, default=ROOT.parent / 'cuemol2')
    parser.add_argument('--build-dir', type=Path, required=True)
    args = parser.parse_args()
    reference, build = args.reference.resolve(), args.build_dir.resolve()
    source = (reference / 'src/modules/xtal/GLSLMapVolRenderer.cpp').read_text()
    signature = 'void GLSLMapVolRenderer::genXferFunMap()'
    start = source.index('{', source.index(signature))
    depth, end = 1, start + 1
    while depth:
        depth += (source[end] == '{') - (source[end] == '}')
        end += 1
    body = source[start:end]
    adapter = r'''
#include <common.h>
#include <gfx/AbstractColor.hpp>
#include <qlib/Vector4D.hpp>
#include <fstream>
#include <algorithm>
using qlib::Vector4D;
#define CHK_GLERROR(...) ((void)0)
constexpr int GL_TEXTURE0=0, GL_TEXTURE1=1, GL_TEXTURE_1D=0;
constexpr int GL_TEXTURE_ENV=0, GL_TEXTURE_ENV_MODE=0, GL_REPLACE=0;
constexpr int GL_UNPACK_ALIGNMENT=0, GL_RGBA=0, GL_UNSIGNED_BYTE=0;
static void glActiveTexture(...) {}
static void glEnable(...) {}
static void glDisable(...) {}
static void glBindTexture(...) {}
static void glTexEnvf(...) {}
static void glPixelStorei(...) {}
static unsigned char colors[1024];
static const unsigned char *allocation;
static void glTexImage1D(int,int,int,int,int,int,int,const void *data) {
    allocation = static_cast<const unsigned char *>(data);
    std::copy_n(allocation, 1024, colors);
}
static int m_nXferType, m_nXfunTexID;
static unsigned m_isolevel;
constexpr int GLMV_AUTO1=1;
void transfer_function()
'''
    adapter += body
    adapter += r'''
int main(int argc, char **argv) {
    if (argc != 2) return 2;
    std::ofstream output(argv[1]);
    for (int mode=0; mode<3; ++mode) {
        m_nXferType=mode;
        for (unsigned level: {0,64,128,254,255}) {
            m_isolevel=level;
            transfer_function();
            for (int i=0; i<256; ++i) {
                for (int c=0; c<4; ++c) output << unsigned(colors[4*i+c]) << ' ';
                output << '\n';
            }
            delete[] allocation;
        }
    }
}
'''
    directory = ROOT / '.cache/volume-reference'
    directory.mkdir(parents=True, exist_ok=True)
    cpp, binary = directory / 'transfer.cpp', directory / 'transfer'
    cpp.write_text(adapter)
    subprocess.run(shlex.split(os.environ.get('CXX', 'c++')) + [
        '-O2', '-std=c++20', '-DHAVE_CONFIG_H', '-DNO_SCRIPT',
        '-I' + str(reference / 'src'), '-I' + str(build / 'src'),
        '-I' + str(build / '_deps/spdlog-src/include'), str(cpp),
        '-L' + str(build / 'src'), '-lcuemol2', '-Wl,-rpath,' + str(build / 'src'),
        '-o', str(binary)], check=True, cwd=ROOT)
    subprocess.run(['cargo', 'test', '--locked', '-p', 'cuemol-xtal', '--test',
                    'volume_transfer_compat', '--', '--nocapture'], cwd=ROOT,
                   env=dict(os.environ, CUEMOL_VOLUME_ORACLE=str(binary)), check=True)


if __name__ == '__main__':
    main()
