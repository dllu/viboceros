"""Retain isolated curved blend outcomes before and after derivative scaling."""

import argparse
import hashlib
import itertools
import json
import os
from pathlib import Path
import subprocess


def capture(binary, sanitizer=False):
    records = []
    env = os.environ.copy()
    if sanitizer:
        env["ASAN_OPTIONS"] = "detect_leaks=0:halt_on_error=1"
    for case, flag, rational, direction, takeoff in itertools.product(
        range(4), range(3), range(2), (1, 2), (0.2, 0.5, 1.7)
    ):
        arguments = [str(case), str(flag), str(rational), str(direction), str(takeoff)]
        record = dict(case=case, blend_flag=flag, rational=bool(rational),
                      direction_flag=direction, takeoff=takeoff)
        try:
            result = subprocess.run([str(binary.resolve()), *arguments], env=env,
                                    capture_output=True, text=True, timeout=30)
            record.update(returncode=result.returncode, stderr=result.stderr)
            try:
                record.update(json.loads(result.stdout))
            except json.JSONDecodeError:
                record["stdout"] = result.stdout
        except subprocess.TimeoutExpired as error:
            record.update(returncode=None, stderr=str(error))
        records.append(record)
    return records


def qualify(records):
    for record in records:
        if record["returncode"] != 0 or record.get("status") != 1000 or not record.get("all_finite"):
            raise ValueError(f"native process or evaluation failure: {record}")
        epsilons = {"boundary_correspondence_error": 1e-12}
        if record["direction_flag"] == 1:
            epsilons["independent_hermite_error"] = 1e-12
        if record["blend_flag"] != 0:
            epsilons["tangent_plane_error"] = 2e-8
            if not record["minimum_cross_speed"] > 0.:
                raise ValueError(f"singular boundary: {record}")
        if record["blend_flag"] == 2:
            epsilons.update(sample_first_derivative_error=1e-10,
                            sample_second_derivative_error=1e-10,
                            sample_third_derivative_error=1e-10,
                            cross_normal_curvature_error=1e-7,
                            along_normal_curvature_error=1e-10)
        for key, epsilon in epsilons.items():
            if not record[key] < epsilon:
                raise ValueError(f"{key} exceeds {epsilon}: {record}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("baseline", type=Path)
    parser.add_argument("patched", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--asan", type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    result = dict(
        upstream_commit="c9c5979f04cea8c38213004d4f04df0dcfb40d05",
        baseline="Rail alignment repair enabled; derivative scaling repair disabled.",
        fixture="Quadratic polynomial/rational Bezier extrusions with unequal degrees, knots, domains; "
                "both explicit correspondences and takeoff factors 0.2, 0.5, 1.7.",
        sample_grid=[41, 41],
        direct_jet_orders=[1, 2, 3],
        scope="Exact direct chain-rule samples and endpoint correspondence; same-direction Hermite shape. "
              "Reverse-direction mixed curvature remains a fitting diagnostic, not a G2 certificate.",
        source_sha256={str(path): hashlib.sha256((root/path).read_bytes()).hexdigest() for path in [
            Path("tools/usd_brep/surface_blend_curved_probe.cpp"),
            Path("tools/usd_brep/compare_surface_blend_producer.py"),
            Path("third_party/patches/usd-brep-blend-rail-parameterization.patch"),
            Path("third_party/patches/usd-brep-blend-derivative-scaling.patch"),
        ]},
        before=capture(args.baseline),
        patched=capture(args.patched),
    )
    if args.asan:
        result["asan"] = capture(args.asan, sanitizer=True)
        result["asan_options"] = "detect_leaks=0:halt_on_error=1; process-wide context leak accounting excluded"
    # Retain failures as evidence even if qualification stops this command.
    args.output.write_text(json.dumps(result, indent=2, allow_nan=False)+"\n")
    for suite in ["patched"] + (["asan"] if args.asan else []):
        qualify(result[suite])
        print(f"Qualified {len(result[suite])} {suite} curved constructions and independent sampled jets")


if __name__ == "__main__":
    main()
