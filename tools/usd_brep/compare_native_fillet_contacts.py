"""Retain the actual native contact C ABI and complete sanitizer outcomes."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

ROOT=Path(__file__).resolve().parents[2]


def run(path,sanitize):
    env=dict(os.environ)
    if sanitize:env["ASAN_OPTIONS"]="detect_leaks=0"
    r=subprocess.run([str(path.resolve())],capture_output=True,text=True,env=env,timeout=120,check=False)
    response=json.loads(r.stdout) if r.returncode==0 else r.stdout
    passed=r.returncode==0 and isinstance(response,dict) and response.get("passed") is True
    if passed and (len(response["cases"])!=36 or not all(c["passed"] for c in response["cases"])):
        raise ValueError("contact probe omitted or failed a case")
    return dict(return_code=r.returncode,stderr=r.stderr,response=response,passed=passed,
                binary_sha256=hashlib.sha256(path.read_bytes()).hexdigest())


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output",type=Path)
    parser.add_argument("--probe",type=Path,required=True)
    parser.add_argument("--asan",type=Path,required=True)
    args=parser.parse_args()
    jobs=dict(native=run(args.probe,False),address_sanitizer=run(args.asan,True))
    sources=["tools/usd_brep/fillet_contacts_probe.cpp","tools/usd_brep/compare_native_fillet_contacts.py",
             "tools/usd_brep/CMakeLists.txt","crates/viboceros-smlib/native/CMakeLists.txt",
             "crates/viboceros-smlib/native/bridge.cpp","crates/viboceros-smlib/native/bridge.h",
             "crates/viboceros-smlib/src/native/curve_fillet_contacts.rs",
             "third_party/patches/usd-brep-curve-fillet-sides.patch",
             "third_party/patches/usd-brep-curve-fillet-seed.patch",
             "third_party/patches/usd-brep-curve-fillet-seed-header.patch"]
    record=dict(upstream_commit="c9c5979f04cea8c38213004d4f04df0dcfb40d05",cases_per_job=36,
                scope="Actual C ABI: line/arc/NURBS and earlier-crossing corner, three charts and three planes. "
                      "Contact positions, retained source endpoints, parameter ranges and invalid-input rejection; "
                      "no Rhino or speed qualification. Unit-radius inputs; Rust regressions cover world restoration.",
                sanitizer_scope="Complete SMLib, SM_API and C bridge; only process-global context leak accounting disabled.",
                passed=all(job["passed"] for job in jobs.values()),
                source_sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in sources},jobs=jobs)
    args.output.write_text(json.dumps(record,indent=2,allow_nan=False)+"\n")
    print("Native contacts and ASan:",record["passed"])
    if not record["passed"]:raise ValueError("native contact qualification failed; retained all outcomes")


if __name__=="__main__":main()
