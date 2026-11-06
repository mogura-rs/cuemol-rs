#include <common.h>
#include <fstream>
#include <iostream>
#include <string>
#include <libcuemol2_api/loader.hpp>
#include <modules/molstr/SelCommand.hpp>
int main(int argc, char **argv) {
    if (argc != 3) return 2;
    if (cuemol2::init_qlib() < 0 || cuemol2::init(argv[1], false, false) < 0) return 3;
    std::ofstream output(argv[2]);
    std::string expression;
    while (std::getline(std::cin, expression)) {
        molstr::SelCommand selection;
        if (!selection.compile(expression.c_str())) { output << "ERROR\n"; continue; }
        output << selection.simplify().c_str() << '\t' << selection.dumpNodes().c_str() << '\t'
               << selection.invertSimplified().c_str() << '\t' << selection.toggleSidechain().c_str() << '\t'
               << selection.rewriteAround(5., false).c_str() << '\n';
    }
}
