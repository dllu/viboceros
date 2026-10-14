"""Compare matching portable/native fillet harness work, excluding Rhino speed claims."""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import statistics

from tools.rhino_oracle import OracleClient, load_request
from tools.rhino_oracle.client import _compare_value
from tools.numerics.compare_native_curve_connections import ROOT, CORNERS_NURBS_KINKS, CORNERS_ARC_KINKS

FIXTURES=("curve_fillet_pair","curve_fillet_pair_parts","curve_fillet_nonmeeting","curve_fillet_corners")


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output",type=Path)
    parser.add_argument("--iterations",type=int,default=100)
    args=parser.parse_args()
    if not 1<=args.iterations<=1000:raise ValueError("iterations must be 1..1000")
    operations=[]
    for fixture in FIXTURES:
        r=load_request(ROOT/f"tools/rhino_oracle/fixtures/{fixture}.json")
        for op in r["operations"]:
            op=copy.deepcopy(op);op["id"]=fixture+"/"+op["id"];operations.append(op)
    request=dict(protocol_version=1,iterations=args.iterations,operations=operations)
    portable=OracleClient(native_smlib=False).run_viboceros(request,timeout=1800)
    native=OracleClient(native_smlib=True).run_viboceros(request,timeout=1800)
    if [r["id"] for r in native["results"]]!=[r["id"] for r in portable["results"]]:
        raise ValueError("result identifiers differ")
    checks=[]
    for a,b in zip(native["results"],portable["results"]):
        fixture,name=a["id"].split("/",1)
        epsilon=1e-7 if "nurbs" in name else 1e-10
        if fixture=="curve_fillet_corners":
            epsilon=1e-7 if name in CORNERS_NURBS_KINKS else 1e-8 if name in CORNERS_ARC_KINKS else 1e-10
        differences=[]
        error=_compare_value(a["id"],a["value"],b["value"],epsilon,0.,differences)
        checks.append(dict(id=a["id"],passed=not differences,max_absolute_error=error,differences=differences,
                           native_ns_per_iteration=a["elapsed_ns"]/args.iterations,
                           portable_ns_per_iteration=b["elapsed_ns"]/args.iterations,
                           portable_to_native_ratio=b["elapsed_ns"]/a["elapsed_ns"] if a["elapsed_ns"] else None))
    sources=["tools/numerics/benchmark_curve_fillet_contacts.py","tools/numerics/compare_native_curve_connections.py",
             "crates/viboceros-geometry/src/polycurve/fillet.rs","crates/viboceros-geometry/src/curve_fillet_operations.rs",
             "crates/viboceros-smlib/src/native/curve_fillet_contacts.rs","crates/viboceros-smlib/native/bridge.cpp"]
    sources += [f"tools/rhino_oracle/fixtures/{f}.json" for f in FIXTURES]
    nurbs=[r["portable_to_native_ratio"] for r in checks if "nurbs" in r["id"] and r["portable_to_native_ratio"] is not None]
    record=dict(iterations=args.iterations,cases=len(checks),passed=all(c["passed"] for c in checks),
                scope="Same fixtures and timed curve preparation/contact/extraction work. Source construction and "
                      "output sampling/export are outside the loop. Host harness measurements; no Rhino speed claim.",
                median_nurbs_portable_to_native_ratio=statistics.median(nurbs),
                source_sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in sources},
                checks=checks,native_response=native,portable_response=portable)
    args.output.write_text(json.dumps(record,indent=2,allow_nan=False)+"\n")
    print("Matches",record["passed"],"NURBS median portable/native ratio",record["median_nurbs_portable_to_native_ratio"])
    if not record["passed"]:raise ValueError("backend outputs differ; retained all measurements")


if __name__=="__main__":main()
