// Direct producer qualification, independent of the Rust extension algorithm.
#include <SmNurbsCrv.h>
#include <nurbs.h>
#include <algorithm>
#include <cmath>
#include <cstdlib>
#include <iomanip>
#include <iostream>

int main(int argc, char **argv) {
    if (argc > 2) return 2;
    int requested = -1;
    if (argc == 2) {
        char *end = nullptr;
        const auto value = std::strtol(argv[1], &end, 10);
        if (*end || value < 0 || value > 7) return 2;
        requested = static_cast<int>(value);
    }
    int index = 0;
    bool passed = true;
    std::cout << std::setprecision(17) << "{\"cases\":[";
    bool first = true;
    for (bool full_break : {false, true}) {
        for (bool distance : {false, true}) {
            for (bool at_start : {false, true}) {
                const auto current = index++;
                if (requested >= 0 && requested != current) continue;
                NL_STACKS stack;
                SmStackHandler cleanup(&stack);
                NL_CPOINT points[] = {{0,0,0,1},{1,1,0,1},{2,2,0,1},
                                      {3,0,0,1},{4,1,0,1},{5,2,0,1}};
                double break_knots[] = {0,0,0,.5,.5,.5,1,1,1};
                NL_CPOINT regular_points[] = {{0,0,0,1},{2,2,0,1},{4,4,0,1}};
                double regular_knots[] = {0,0,0,1,1,1};
                NL_CURVE source, result;
                N_CrvInitArrays(&source);
                N_CrvInitArrays(&result);
                if (N_CrvFromCPtsAndKnots(&source,
                        full_break ? points : regular_points, full_break ? 5 : 2, 2,
                        full_break ? break_knots : regular_knots, full_break ? 8 : 5,
                        &stack) != NL_NO) return 2;
                const auto side = at_start ? NL_START : NL_END;
                const auto status = distance
                    ? N_CrvExtendByDist(&source, std::sqrt(2.), NL_ABSOLUTE,
                                       side, NL_CMAX, &result, &stack, &stack)
                    : N_CrvExtendByParamDist(&source, at_start ? -.25 : 1.25,
                                            side, NL_CMAX, &result, &stack, &stack);
                if (!first) std::cout << ',';
                first = false;
                std::cout << "{\"full_order_break\":" << (full_break ? "true" : "false")
                          << ",\"method\":\"" << (distance ? "distance" : "parameter")
                          << "\",\"side\":\"" << (at_start ? "start" : "end")
                          << "\",\"status\":" << status;
                if (status == NL_NO) {
                    const auto begin = result.knt->U[result.p];
                    const auto end = result.knt->U[result.pol->n + 1];
                    double maximum = 0;
                    bool evaluated = true;
                    for (int i = 0; i <= 128; ++i) {
                        const auto t = begin + (end - begin) * i / 128.;
                        // Full-order breaks have independent one-sided values.
                        if (full_break && t == .5) continue;
                        NL_POINT actual;
                        if (N_CrvEval(&result, t, NL_LEFT, &actual) != NL_NO) {
                            evaluated = false;
                            break;
                        }
                        const auto x = full_break && t > .5 ? 1 + 4*t : 4*t;
                        const auto y = full_break && t > .5 ? 4*t - 2 : 4*t;
                        maximum = std::max(maximum, std::hypot(std::hypot(actual.x-x, actual.y-y), actual.z));
                    }
                    const auto domain_error = std::max(std::abs(begin - (at_start ? -.25 : 0.)),
                                                       std::abs(end - (at_start ? 1. : 1.25)));
                    std::cout << ",\"domain\":[" << begin << ',' << end
                              << "],\"maximum_point_error\":" << maximum
                              << ",\"domain_error\":" << domain_error
                              << ",\"evaluation_succeeded\":" << (evaluated ? "true" : "false");
                    passed = passed && evaluated && maximum < 1e-10 && domain_error < 1e-10;
                } else passed = false;
                std::cout << '}';
            }
        }
    }
    std::cout << "],\"passed\":" << (passed ? "true" : "false") << "}\n";
    return passed ? 0 : 1;
}
