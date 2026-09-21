"""Independent tensor-surface area quadrature; requires optional mpmath.

Global basis functions and their derivatives are evaluated independently of
either geometry kernel. Increasing quadrature order is an accuracy witness,
not a formal integration error enclosure.
"""
import argparse
import json
from pathlib import Path

import mpmath as mp


def basis(knots, degree, t):
    values = [mp.mpf(int(a <= t < b)) for a, b in zip(knots, knots[1:])]
    for p in range(1, degree + 1):
        following = []
        for i in range(len(values) - 1):
            value = mp.mpf(0)
            if knots[i+p] != knots[i]:
                value += (t-knots[i])*values[i]/(knots[i+p]-knots[i])
            if knots[i+p+1] != knots[i+1]:
                value += (knots[i+p+1]-t)*values[i+1]/(knots[i+p+1]-knots[i+1])
            following.append(value)
        values = following
    return values


def stations(surface, axis, bounds, nodes, weights):
    degree = surface['degree_'+axis]
    knots = list(map(mp.mpf, surface['knots_'+axis]))
    count = surface['control_point_count_'+axis]
    left, right = map(mp.mpf, bounds)
    result = []
    for x, weight in zip(nodes, weights):
        t = (left+right)/2 + (right-left)*x/2
        values = basis(knots, degree, t)
        lower = basis(knots, degree-1, t)
        derivatives = []
        for i in range(count):
            derivative = mp.mpf(0)
            if knots[i+degree] != knots[i]:
                derivative += degree*lower[i]/(knots[i+degree]-knots[i])
            if knots[i+degree+1] != knots[i+1]:
                derivative -= degree*lower[i+1]/(knots[i+degree+1]-knots[i+1])
            derivatives.append(derivative)
        result.append((values, derivatives, weight*(right-left)/2))
    return result


def area(source, nodes, weights):
    surface = source['surface']
    bounds = source['trim_bounds']
    # These four fixtures have no interior knots in the integration rectangle.
    for axis, interval in zip(('u', 'v'), bounds):
        assert not any(interval[0] < t < interval[1] for t in surface['knots_'+axis])
    u = stations(surface, 'u', bounds[0], nodes, weights)
    v = stations(surface, 'v', bounds[1], nodes, weights)
    controls = [[mp.mpf(x)*mp.mpf(c['weight']) for x in c['point']]+[mp.mpf(c['weight'])]
                for c in surface['control_points']]
    nu = surface['control_point_count_u']
    total = mp.mpf(0)
    for bv, dv, wv in v:
        for bu, du, wu in u:
            h, hu, hv = [[mp.mpf(0) for _ in range(4)] for _ in range(3)]
            for j in range(len(bv)):
                for i in range(len(bu)):
                    for axis, value in enumerate(controls[j*nu+i]):
                        h[axis] += bu[i]*bv[j]*value
                        hu[axis] += du[i]*bv[j]*value
                        hv[axis] += bu[i]*dv[j]*value
            a = [(hu[i]*h[3]-h[i]*hu[3])/h[3]**2 for i in range(3)]
            b = [(hv[i]*h[3]-h[i]*hv[3])/h[3]**2 for i in range(3)]
            cross = [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]]
            total += wu*wv*mp.sqrt(mp.fsum(c*c for c in cross))
    return total


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--digits', type=int, default=80)
    parser.add_argument('--order', type=int, default=64)
    args = parser.parse_args()
    if not 30 <= args.digits <= 300 or not 8 <= args.order <= 128:
        parser.error('digits must be 30..300 and order 8..128')
    path = Path(__file__).resolve().parents[1]/'fixtures'/'join_isocurve_certificates.json'
    request = json.loads(path.read_text())
    with mp.workdps(args.digits):
        nodes, weights = mp.gauss_quadrature(args.order, 'legendre')
        values = {}
        for name in ('extrusion-clamped', 'extrusion-unclamped', 'tensor-clamped', 'tensor-unclamped'):
            operation = next(o for o in request['operations'] if o['id'] == name+'-uv0-origin0-Join-pre')
            source = operation['sources'][0]['brep']['source']
            values[name] = mp.nstr(area(source, nodes, weights), args.digits-5)
        print(json.dumps(values, indent=2))


if __name__ == '__main__':
    main()
