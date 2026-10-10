// Independent OCCT STEP-reader check; no OCCT code is included in the application.
#include <STEPControl_Reader.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepGProp.hxx>
#include <GProp_GProps.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Solid.hxx>
#include <cmath>
#include <filesystem>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <stdexcept>

int main(int argc, char **argv) {
    try {
        if (argc < 3) throw std::runtime_error("usage: inspect_step_regions OUTPUT INPUT...");
        std::ofstream output(argv[1]);
        if (!output) throw std::runtime_error("cannot create output");
        output << std::setprecision(17) << "[\n";
        for (int argument = 2; argument < argc; ++argument) {
            STEPControl_Reader reader;
            if (reader.ReadFile(argv[argument]) != IFSelect_RetDone || reader.TransferRoots() == 0)
                throw std::runtime_error("STEP read/transfer failed");
            const auto shape = reader.OneShape();
            const auto name = std::filesystem::path(argv[argument]).stem().string();
            output << (argument == 2 ? "" : ",\n") << "  {\"id\":\"" << name << "\",\"solids\":[";
            int count = 0;
            for (TopExp_Explorer explorer(shape, TopAbs_SOLID); explorer.More(); explorer.Next()) {
                const auto solid = TopoDS::Solid(explorer.Current());
                const bool valid = BRepCheck_Analyzer(solid, true).IsValid();
                GProp_GProps properties;
                BRepGProp::VolumeProperties(solid, properties, 1e-10, true, false);
                const double volume = properties.Mass();
                if (!valid || !std::isfinite(volume) || volume <= 0.)
                    throw std::runtime_error(name + ": invalid native material solid");
                output << (count++ ? "," : "") << "{\"valid\":true,\"volume\":" << volume << "}";
                std::cout << name << ": valid material solid, volume " << std::setprecision(17) << volume << '\n';
            }
            if (!count) throw std::runtime_error(name + ": no material solids");
            output << "]}";
        }
        output << "\n]\n";
        if (!output) throw std::runtime_error("output write failed");
        return 0;
    } catch (const std::exception &error) {
        std::cerr << error.what() << '\n';
        return 1;
    }
}
