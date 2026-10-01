"""Replay saved UntrimAll/UntrimBorder captures from source-only native requests.

Command events/history remain evidence in the raw capture, not comparison
outputs. Independent box factories are compared for rejection and unchanged
geometry within each engine, rather than claiming identical topology tables.
"""
import argparse
import copy
import json

from .client import OracleClient, OracleProtocolError, compare_responses, load_request
from .untrim_probe import validate


def canonical_response(request, response):
    operations=request["operations"]
    if (response.get("protocol_version")!=1 or response.get("iterations")!=1
            or len(response.get("results",[]))!=len(operations)):
        raise OracleProtocolError("incomplete untrim response")
    result=copy.deepcopy(response)
    for op,row in zip(operations,result["results"]):
        if row.get("id")!=op["id"]:
            raise OracleProtocolError("untrim operation order mismatch")
        value=row["value"]
        for field in ("events","history"):value.pop(field,None)
        if any(source["type"]=="box" for source in op["sources"]):
            if op["sources"]!=[dict(type="box")] or value.get("succeeded") is not False:
                raise OracleProtocolError("expected rejected whole-box selection")
            before,after=value["before"],value["after"]
            if (len(before)!=1 or len(after)!=1
                    or before[0]["geometry"]!=after[0]["geometry"]):
                raise OracleProtocolError("rejected untrim command changed box geometry")
            value.pop("constructed")
            for field in ("before","after"):
                for obj in value[field]:obj.pop("geometry")
    return result


def replay(request, observed, client=None, timeout=180):
    if (request.get("protocol_version")!=1 or type(request.get("iterations")) is not int
            or request["iterations"]!=1 or not request.get("operations")
            or observed.get("engine")!="rhino"):
        raise OracleProtocolError("expected one-iteration untrim request and Rhino capture")
    for operation in request["operations"]:validate(operation)
    expected=canonical_response(request,observed)
    native=(client or OracleClient()).run_viboceros(copy.deepcopy(request),timeout)
    return compare_responses(canonical_response(request,native),expected,1e-9,0.)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("request")
    parser.add_argument("observations")
    parser.add_argument("--timeout",type=float,default=180)
    args=parser.parse_args()
    report=replay(load_request(args.request),load_request(args.observations),timeout=args.timeout)
    print(json.dumps(report.as_dict(),indent=2,allow_nan=False))
    return 0 if report.passed else 1


if __name__=="__main__":raise SystemExit(main())
