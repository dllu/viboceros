"""Replay area API and actual AreaCentroid output without hiding differences."""
import argparse
import copy
import json
import math
from .area_centroid_probe import validate
from .client import OracleClient, OracleError, OracleProtocolError, _validate_response, compare_responses, load_request


def prepare(request, observed, tight_api=False, measure="area", source_api=False):
    if measure not in ("area","volume") or (measure=="volume" and tight_api) or (measure=="area" and source_api):
        raise OracleProtocolError("unsupported centroid replay scope")
    if (not isinstance(request,dict) or type(request.get("protocol_version")) is not int or request["protocol_version"]!=1
            or type(request.get("iterations")) is not int or request["iterations"]!=1
            or not isinstance(request.get("operations"),list) or not request["operations"]):
        raise OracleProtocolError("centroid replay requires one iteration")
    for operation in request["operations"]: validate(operation, measure)
    if any("open_confirmation" in operation for operation in request["operations"]):
        raise OracleProtocolError("open-volume confirmation captures are not yet supported by native replay")
    ids=[op["id"] for op in request["operations"]]
    _validate_response(observed,"rhino")
    if len(set(ids))!=len(ids) or observed["iterations"]!=1 or ids!=[r["id"] for r in observed["results"]]:
        raise OracleProtocolError("centroid observations do not match requested order")
    evidence=copy.deepcopy(observed)
    keys={"constructed_inputs","history","inputs","insertions","points","properties","selected","succeeded","tight_properties"}
    if measure=="volume": keys.update(("source_properties","source_first_moments"))
    finite=lambda x:type(x) in (int,float) and math.isfinite(x)
    for op,row in zip(request["operations"],evidence["results"]):
        v=row["value"]; count=len(op["sources"])
        if not isinstance(v,dict) or set(v)!=keys or type(v["succeeded"]) is not bool or not isinstance(v["history"],str):
            raise OracleProtocolError("invalid centroid observation fields")
        fields=["constructed_inputs","inputs","insertions","properties","tight_properties"]
        if measure=="volume": fields.extend(("source_properties","source_first_moments"))
        for field in fields:
            if not isinstance(v[field],list) or len(v[field])!=count: raise OracleProtocolError("incomplete centroid source evidence")
        for source,created,stored,insertion in zip(op["sources"],v["constructed_inputs"],v["inputs"],v["insertions"]):
            if insertion=="unchanged":
                if created!=stored: raise OracleProtocolError("centroid insertion changed source")
            elif insertion=="reversed":
                if source["type"]!="brep" or not source.get("cap_surface") or created==stored:
                    raise OracleProtocolError("unexpected centroid insertion reversal")
                # The source face flag is not a solid orientation: the graph
                # below its cap is inward when that flag is false. Verify the
                # entire recorded reversal, not a presumed flag or case ID.
                flipped=copy.deepcopy(created)
                try:
                    faces=flipped["topology"]["faces"]
                    if not isinstance(faces,list) or not faces: raise ValueError()
                    for face in faces:
                        if type(face["reversed"]) is not bool: raise ValueError()
                        face["reversed"]=not face["reversed"]
                except (KeyError,TypeError,ValueError):
                    raise OracleProtocolError("invalid centroid insertion reversal") from None
                if flipped!=stored: raise OracleProtocolError("centroid insertion changed more than face orientation")
            else: raise OracleProtocolError("unverified centroid insertion")
            if source["type"]=="mesh" and stored!={k:source[k] for k in ("vertices","faces")}:
                raise OracleProtocolError("centroid mesh source was changed or rounded")
        for field in (["properties","tight_properties","source_properties"] if measure=="volume" else ["properties","tight_properties"]):
            for mass in v[field]:
                if mass is None: continue
                if (not isinstance(mass,dict) or set(mass)!={measure,"centroid"} or not finite(mass[measure]) or (measure=="area" and mass[measure]<0)
                        or not isinstance(mass["centroid"],list) or len(mass["centroid"])!=3 or not all(map(finite,mass["centroid"]))):
                    raise OracleProtocolError("invalid centroid mass observation")
        if measure=="volume":
            for moment in v["source_first_moments"]:
                if moment is not None and (not isinstance(moment,list) or len(moment)!=3 or not all(map(finite,moment))):
                    raise OracleProtocolError("invalid volume first-moment observation")
        if (not isinstance(v["selected"],list) or any(type(i) is not int or not 0<=i<count for i in v["selected"])
                or len(set(v["selected"]))!=len(v["selected"])):
            raise OracleProtocolError("invalid centroid selection observation")
        if not isinstance(v["points"],list): raise OracleProtocolError("invalid centroid point observations")
        for point in v["points"]:
            if (not isinstance(point,dict) or set(point)!={"point","selected","current_layer","groups","name"}
                    or not isinstance(point["point"],list) or len(point["point"])!=3 or not all(map(finite,point["point"]))
                    or type(point["selected"]) is not bool or type(point["current_layer"]) is not bool
                    or type(point["groups"]) is not int or point["groups"]<0 or not isinstance(point["name"],str)):
                raise OracleProtocolError("invalid centroid point observation")
        if measure=="volume":
            row["value"] = {"properties":v["source_properties"]} if source_api else {k:v[k] for k in ("points","selected","succeeded")}
        else:
            row["value"] = {"properties":v["tight_properties"]} if tight_api else {k:v[k] for k in ("properties","points","selected","succeeded")}
    return copy.deepcopy(request),evidence


def replay(request,observed,client=None,tight_api=False,measure="area",source_api=False):
    native,evidence=prepare(request,observed,tight_api,measure,source_api)
    actual=(client or OracleClient()).run_viboceros(native)
    if tight_api or source_api:
        for row in actual["results"]: row["value"]={"properties":row["value"]["properties"]}
    elif measure=="volume":
        for row in actual["results"]: row["value"]={k:row["value"][k] for k in ("points","selected","succeeded")}
    return compare_responses(actual,evidence,absolute_epsilon=1e-9,relative_epsilon=0.)


def main(measure="area"):
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("request"); parser.add_argument("observations")
    if measure=="area": parser.add_argument("--tight-api",action="store_true",help="compare per-object API only, not command markers or selection")
    else: parser.add_argument("--source-api",action="store_true",help="compare raw API on constructed sources, not command outputs or post-insertion geometry")
    args=parser.parse_args()
    try:
        report=replay(load_request(args.request),load_request(args.observations),tight_api=getattr(args,"tight_api",False),
                      measure=measure,source_api=getattr(args,"source_api",False))
        output=report.as_dict()
        if measure=="volume": output["scope"]="raw API on constructed source geometry" if args.source_api else "actual command marker/selection/attributes; API diagnostics retained separately"
        else: output["scope"]="explicit-tolerance per-object API only" if args.tight_api else "default per-object API and command marker/selection/attributes; no history-string comparison"
        print(json.dumps(output,indent=2,allow_nan=False))
        return 0 if report.passed else 1
    except (ValueError,OSError,OracleError) as error: parser.exit(2,"centroid replay failed: %s\n"%error)


if __name__=="__main__": raise SystemExit(main())
