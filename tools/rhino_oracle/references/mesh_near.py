"""Independent exact-rational mesh Near endpoint-depth weighting reference.

Away from endpoint capture boxes, measured Rhino targets minimize
|Hxy(t) - cursor * W(t)|² / W(1-t)² on each visible wire. Curve Near instead
divides by W(t)². A wire endpoint inside the square aperture selects the
ordinary screen-distance calculation. This reference
implements the mathematical hypothesis; it does not read observed target values.
The retained snap_capture_box short-wire cases contradict this hypothesis's
endpoint behavior. Those differences remain explicit, not fitted away here.
"""
from fractions import Fraction as F
import itertools
from .projected_lines import homogeneous, interpolate, closest as screen_closest


def closest(matrix, a, b, cursor, radius=12):
    ha, hb = homogeneous(matrix, a), homogeneous(matrix, b)
    wa, wb = ha[2], hb[2]
    if min(wa, wb) <= 0: raise ValueError("mesh reference requires fully visible wires")
    if any(all(abs(h[i]/h[2]-F(cursor[i])) <= radius for i in range(2)) for h in (ha,hb)):
        return screen_closest(matrix,a,b,cursor,min(wa,wb)/2)
    # pa*wa² and pb*wb² are homogeneous cursor-relative X/Y times
    # their own endpoint depth.
    qa = [(ha[i]-F(cursor[i])*wa)*wa for i in range(2)]
    qb = [(hb[i]-F(cursor[i])*wb)*wb for i in range(2)]
    delta = [y-x for x,y in zip(qa,qb)]
    squared = sum(x*x for x in delta)
    s = F(0) if squared == 0 else max(F(0), min(F(1), -sum(x*y for x,y in zip(qa,delta))/squared))
    # The virtual projection uses the opposite endpoint depths.
    t = s*wb/((1-s)*wa+s*wb)
    target = interpolate(a,b,t)
    h = homogeneous(matrix,target)
    actual_squared = sum((h[i]/h[2]-F(cursor[i]))**2 for i in range(2))
    return target, actual_squared


def csv_text():
    rows = ["# X,Y,W rows (3x4), a.xyz, b.xyz, cursor.xy, radius, target.xyz, squared_distance"]
    for axis,depth,x,radius,reverse in itertools.product(range(3),[0.125,1.,32.,1e6,1e12,1e100],
                                                       [-1.,0.,0.25,0.75,1.,2.],[0.125,0.5,2.,12.],[False,True]):
        matrix = [[0.]*4 for _ in range(3)]
        matrix[0][axis],matrix[1][(axis+1)%3],matrix[2][(axis+2)%3] = 2.,-0.5,1.
        a,b = [0.]*3,[0.]*3
        for j,v in enumerate([-1.,0.25,1.]): a[(axis+j)%3] = v
        for j,v in enumerate([2.,-0.5,depth]): b[(axis+j)%3] = v
        if reverse: a,b = b,a
        cursor = [x,0.125]
        target,squared = closest(matrix,a,b,cursor,radius)
        # Keep clear captures; aperture-boundary rounding is a separate issue.
        if squared >= F(radius)**2 * F(999999,1000000): continue
        values = sum(matrix,[])+a+b+cursor+[radius]+list(map(float,target))+[float(squared)]
        rows.append(",".join(format(v,".17g") for v in values))
    return "\n".join(rows)+"\n"


if __name__ == "__main__":
    print(csv_text(),end="")
