#!/usr/bin/env python3
"""Print exact-dyadic-UV saddle and cylinder surface-wire fixtures."""
from copy import deepcopy
import json
from math import sqrt


def main():
    saddle = dict(degree_u=1,degree_v=1,control_point_count_u=2,control_point_count_v=2,
                  knots_u=[0,0,1,1],knots_v=[0,0,1,1],control_points=[
                      dict(point=p,weight=w) for p,w in [([0,0,0],1),([1,0,0],2),([0,1,0],4),([1,1,1],8)]])
    circle=[(2,0,1),(2,2,sqrt(0.5)),(0,2,1),(-2,2,sqrt(0.5)),(-2,0,1),
            (-2,-2,sqrt(0.5)),(0,-2,1),(2,-2,sqrt(0.5)),(2,0,1)]
    cylinder=dict(degree_u=2,degree_v=1,control_point_count_u=9,control_point_count_v=2,
                  knots_u=[0,0,0,0.25,0.25,0.5,0.5,0.75,0.75,1,1,1],knots_v=[0,0,1,1],
                  control_points=[dict(point=[x,y,z],weight=w) for z in [0,1] for x,y,w in circle])
    operations=[]
    for name,definition in [("saddle",saddle),("cylinder",cylinder)]:
        for density in [-1,1,3]:
            for label,offset in [("local",[0,0]),("translated",[1e12,-2e12])]:
                surface=deepcopy(definition)
                for axis,key in enumerate(["knots_u","knots_v"]):
                    surface[key]=[k+offset[axis] for k in surface[key]]
                    assert [k-offset[axis] for k in surface[key]] == definition[key]
                operations.append(dict(op="surface_wires",id=f"{name}-density-{density}-{label}",surface=surface,density=density))
    print(json.dumps(dict(protocol_version=1,iterations=3,tolerance=dict(absolute=1e-9,relative=1e-12,angular=1e-10),operations=operations),indent=2))


if __name__ == "__main__":
    main()
