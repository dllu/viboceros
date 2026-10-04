"""Bounded public SDK fitting timings, with prebuilt .NET point arrays."""
import math


def request():
    return dict(protocol_version=1,iterations=1,operations=[dict(op="circle_fit_benchmark",id="circle-fit-benchmark",counts=[100,10000])])


def validate(op):
    if (not isinstance(op,dict) or set(op)!={"op","id","counts"} or op["op"]!="circle_fit_benchmark"
        or op["id"]!="circle-fit-benchmark" or op["counts"]!=[100,10000]):
        raise ValueError("invalid bounded circle fit benchmark")


def run(op,host):
    validate(op)
    Rhino,System=host["Rhino"],host["System"]
    if list(Rhino.RhinoDoc.ActiveDoc.Objects):raise ValueError("benchmark requires empty owned document")
    result=[]
    for count in op["counts"]:
        points=System.Array[Rhino.Geometry.Point3d]([Rhino.Geometry.Point3d(1+(3+.03*math.cos(3*i))*math.cos(.4*i),2+(3+.03*math.cos(3*i))*math.sin(.4*i),.2*math.sin(2.3*i)) for i in range(count)])
        for _ in range(10):Rhino.Geometry.Circle.TryFitCircleToPoints(points)
        times=[]
        for _ in range(3):
            watch=System.Diagnostics.Stopwatch.StartNew()
            ok,circle=Rhino.Geometry.Circle.TryFitCircleToPoints(points)
            watch.Stop()
            if not ok:raise ValueError("native benchmark fit failed")
            times.append(watch.ElapsedTicks*1000./System.Diagnostics.Stopwatch.Frequency)
        result.append(dict(count=count,milliseconds=times,median_ms=sorted(times)[1],origin=host["_xyz"](circle.Center),radius=circle.Radius))
    return dict(records=result),0
