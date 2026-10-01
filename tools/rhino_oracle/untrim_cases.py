"""Source-only UntrimAll fixtures; no observed definitions are input recipes."""
import copy
import json
import math
from pathlib import Path


def request():
    def controls(points, weights=None):
        return [dict(point=list(p), weight=1 if weights is None else weights[i]) for i, p in enumerate(points)]
    surface = dict(degree_u=1, degree_v=1, control_point_count_u=2, control_point_count_v=2,
        control_points=controls([[0,0,0],[10,0,0],[0,10,0],[10,10,0]]),
        knots_u=[0,0,10,10], knots_v=[0,0,10,10])
    def loop(a,b,c,d,cw=False):
        p=[[a,b,0],[c,b,0],[c,d,0],[a,d,0],[a,b,0]]
        if cw: p.reverse()
        curve=dict(degree=1, control_points=controls(p), knots=[0,0,1,2,3,4,4])
        return dict(curve=curve, parameter_curve=copy.deepcopy(curve))
    def trimmed(boundaries):
        return dict(type="brep", surface=copy.deepcopy(surface), boundaries=boundaries, interior_uv=[2,2])
    outer=trimmed([loop(1,1,9,9)])
    hole=trimmed([loop(1,1,9,9),loop(3,3,5,5,True)])
    reversed_outer=copy.deepcopy(outer); reversed_outer["reversed"]=True
    reversed_hole=copy.deepcopy(hole); reversed_hole["reversed"]=True
    full_border=trimmed([loop(0,0,10,10),loop(3,3,5,5,True)])
    def circle(radius, cw=False):
        p=[[5+radius,5,0],[5+radius,5+radius,0],[5,5+radius,0],
           [5-radius,5+radius,0],[5-radius,5,0],[5-radius,5-radius,0],
           [5,5-radius,0],[5+radius,5-radius,0],[5+radius,5,0]]
        if cw: p.reverse()
        curve=dict(degree=2,control_points=controls(p,[1,math.sqrt(.5),1,math.sqrt(.5),1,math.sqrt(.5),1,math.sqrt(.5),1]),
            knots=[0,0,0,1,1,2,2,3,3,4,4,4])
        return dict(curve=curve,parameter_curve=copy.deepcopy(curve))
    annulus=trimmed([circle(4),circle(1,True)])
    # Choose a point in the annulus (not its hole).
    annulus["interior_uv"]=[2,5]
    oblique=copy.deepcopy(hole)
    def transform(p): return [p[0]+11,.6*p[1]-7,.8*p[1]+4]
    for control in oblique["surface"]["control_points"]: control["point"]=transform(control["point"])
    for boundary in oblique["boundaries"]:
        for control in boundary["curve"]["control_points"]: control["point"]=transform(control["point"])
    parent=json.loads(Path(__file__).with_name("fixtures").joinpath("area_centroid.json").read_text())
    curved={op["id"]:op["sources"][0] for op in parent["operations"] if op["id"].startswith("brep-paraboloid-")}
    circle_net=circle(3)["curve"]
    cylinder=dict(type="surface",degree_u=2,degree_v=1,control_point_count_u=9,control_point_count_v=2,
        knots_u=circle_net["knots"],knots_v=[0,0,5,5],
        control_points=[dict(point=[c["point"][0],c["point"][1],z],weight=c["weight"])
                        for z in (0,5) for c in circle_net["control_points"]])
    kinky=dict(type="surface",degree_u=1,degree_v=1,control_point_count_u=3,control_point_count_v=2,
        knots_u=[0,0,5,10,10],knots_v=[0,0,10,10],
        control_points=controls([[0,0,0],[5,0,0],[10,0,3],[0,10,0],[5,10,0],[10,10,3]]))
    triangle=dict(type="surface",**copy.deepcopy(surface))
    triangle["control_points"]=controls([[0,0,0],[6,0,0],[0,0,0],[0,4,0]])
    sources=[("outer",[outer]),("hole",[hole]),("natural",[dict(type="surface",**surface)]),
        ("box",[dict(type="box")]),("mixed",[hole,dict(type="point",point=[2,3,4])]),
        ("reversed-outer",[reversed_outer]),("reversed-hole",[reversed_hole]),("full-border",[full_border]),
        ("annulus",[annulus]),("oblique",[oblique]),("paraboloid-disk",[curved["brep-paraboloid-disk"]]),
        ("paraboloid-annulus",[curved["brep-paraboloid-annulus"]]),
        ("paraboloid-oblique",[curved["brep-paraboloid-rotated-translated"]]),
        ("multiple",[outer,hole]),("cylinder",[cylinder]),("kinky",[kinky]),("triangle",[triangle])]
    operations=[
        dict(op="untrim_all_command",id="%s-keep-%d-pre-%d"%(name,keep,pre),sources=copy.deepcopy(items),
             keep_trim_objects=bool(keep),preselect=bool(pre))
        for name,items in sources for keep in (0,1) for pre in (0,1)]
    for keep in (0,1):
        for pre in (0,1):
            operations.append(dict(op="untrim_all_command",id="source-layer-keep-%d-pre-%d"%(keep,pre),
                sources=[copy.deepcopy(hole)],keep_trim_objects=bool(keep),preselect=bool(pre),source_layer=True))
    return dict(protocol_version=1,iterations=1,operations=operations)


if __name__ == "__main__": print(json.dumps(request(),indent=2))
