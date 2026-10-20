"""Independent graph-metric check for the general-patch isocurve witness.

Requires SciPy. No Rust NURBS evaluation or native geometry producer is used.
The polynomial patch is z = 0.2*x*y*(1 + y/2), with 0 <= x,y <= 2.
"""
import json
import math
from pathlib import Path
from scipy.integrate import solve_ivp

def reference():
    # At (1,1,0.3), the source follows (1,0,0.3). Cross its unit
    # tangent with the oriented surface normal to travel distance -0.05.
    root=math.sqrt(1.25)*math.sqrt(1.09)
    initial=[1.,1.,.12/root,-1.09/root]
    def rhs(_,state):
        x,y,dx,dy=state
        hx=.2*y*(1.+.5*y);hy=.2*x*(1.+y)
        hessian=2.*.2*(1.+y)*dx*dy+.2*x*dy*dy
        factor=-hessian/(1.+hx*hx+hy*hy)
        return [dx,dy,hx*factor,hy*factor]
    result=solve_ivp(rhs,[0.,.05],initial,method='DOP853',atol=1e-13,rtol=1e-13)
    if not result.success:raise ValueError(result.message)
    x,y=result.y[:2,-1]
    return [float(x),float(y),float(.2*x*y*(1.+.5*y))]

def main():
    path=Path(__file__).resolve().parents[1]/'rhino_oracle/fixtures/surface_curve_offset_general_verify.json'
    expected=json.loads(path.read_text())['operations'][0]['samples'][0]
    point=reference();error=math.dist(point,expected)
    if error>5e-13:raise ValueError('independent graph witness changed: '+str(error))
    print(json.dumps(dict(method='DOP853 on the independent graph metric',point=point,maximum_query_point_difference=error),allow_nan=False))

if __name__=='__main__':main()
