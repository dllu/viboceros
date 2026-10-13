"""Independent continued B-spline evaluation of recorded point-grid nets.

Requires optional mpmath. Arguments: fixture.json portable-audit.json
native-audit.json. Uses exact binary64 inputs at 80 decimal digits; this is a
numerical diagnostic, not a certified enclosure or Rhino reference.
"""
import json
import sys
from pathlib import Path

import mpmath as mp

mp.mp.dps = 80


def basis(knots, degree, count, parameter):
    knots = list(map(mp.mpf, knots))
    parameter = mp.mpf(parameter)
    if parameter <= knots[degree]:
        span = degree
        while knots[span] == knots[span + 1]:
            span += 1
    elif parameter >= knots[count]:
        span = count - 1
        while knots[span] == knots[span + 1]:
            span -= 1
    else:
        span = max(i for i in range(degree, count) if knots[i] <= parameter)
    # Local Cox/de Boor triangular recursion continues the chosen polynomial
    # span when the parameter is outside the active interval.
    values = [mp.mpf(1)]
    for order in range(1, degree + 1):
        saved = mp.mpf(0)
        for index in range(order):
            left = parameter - knots[span + index + 1 - order]
            right = knots[span + index + 1] - parameter
            term = values[index] / (left + right)
            values[index] = saved + right * term
            saved = left * term
        values.append(saved)
    result = [mp.mpf(0)] * count
    result[span - degree:span + 1] = values
    return result


def values(audit):
    return {o['result']['id']: o['result']['value'] for o in audit['outcomes']
            if o['status'] == 'success'}


def residual(fixture, value):
    s = value['surface']
    n, m = s['control_count']
    u, v = value['constraints']['parameters']
    bu = mp.matrix([basis(s['knots_u'], s['degree'][0], n, t) for t in u])
    bv = mp.matrix([basis(s['knots_v'], s['degree'][1], m, t) for t in v])
    maxima = []
    for axis in range(3):
        controls = mp.matrix(n, m)
        for j in range(m):
            for i in range(n):
                controls[i, j] = mp.mpf(s['control_points'][j * n + i]['point'][axis])
        evaluated = bu * controls * bv.T
        maxima.append(float(max(abs(evaluated[i, j] - mp.mpf(
            fixture['points'][i * fixture['count'][1] + j][axis]))
            for i in range(len(u)) for j in range(len(v)))))
    samples = value['constraints']['samples']
    double_residual = max(abs(samples[j * len(u) + i][a] -
                              fixture['points'][i * fixture['count'][1] + j][a])
                          for j in range(len(v)) for i in range(len(u)) for a in range(3))
    return dict(high_precision_coordinate_residuals=maxima,
                high_precision_max_residual=max(maxima),
                binary64_evaluator_max_residual=double_residual)


def main():
    fixture, portable, native = [json.loads(Path(p).read_text()) for p in sys.argv[1:]]
    portable, native = values(portable), values(native)
    cases = []
    for f in fixture['operations']:
        if f.get('command', False):
            continue
        cases.append(dict(id=f['id'], portable=residual(f, portable[f['id']]),
                          native=residual(f, native[f['id']])))
    print(json.dumps(dict(decimal_digits=mp.mp.dps,
                          method='Exact binary64 definitions and recorded stations; independent local Cox/de Boor continued-span basis and tensor multiplication. Numerical diagnostic, not a certified enclosure or Rhino capture.',
                          cases=cases), indent=2))


if __name__ == '__main__':
    main()
