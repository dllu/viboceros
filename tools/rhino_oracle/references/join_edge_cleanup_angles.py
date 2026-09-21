"""Independent high-precision directions of the stored clamped seam pieces."""
import argparse, json, math
from pathlib import Path
import mpmath as mp

mp.mp.dps = 100
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('exports', type=Path, help='eight-case brep_join source-export audit response')
args = parser.parse_args()
exports = json.loads(args.exports.read_text())
assert len(exports['outcomes']) == 8
rows = []
for outcome in exports['outcomes']:
    assert outcome['status'] == 'success'
    result = outcome['result']
    for source_index, brep in enumerate(result['value']['inputs']):
        assert len(brep['edges']) == 7
        source_edge = 3 if '-uv1-' in result['id'] else 0
        pieces = [brep['edges'][i]['curve']['definition'] for i in [source_edge, 4, 5, 6]]
        pieces.sort(key=lambda c: c['domain'][0])
        angles, gaps = [], []
        for a, b in zip(pieces, pieces[1:]):
            def endpoint(c, end):
                p = c['degree']; points = c['control_points']
                t = c['domain'][int(end)]
                assert all(k == t for k in (c['knots'][-p-1:] if end else c['knots'][:p+1]))
                w0, w1 = [mp.mpf(points[i]['weight']) for i in ([-2, -1] if end else [0, 1])]
                assert w0 * w1 > 0
                # Clamped endpoint derivatives are positive multiples of the
                # Euclidean last/first control chord for coherent weights.
                p0, p1 = [[mp.mpf(x) for x in points[i]['point']] for i in ([-2, -1] if end else [0, 1])]
                v = [y-x for x,y in zip(p0,p1)]
                assert any(v)
                return (p1 if end else p0), v
            pa, va = endpoint(a, True); pb, vb = endpoint(b, False)
            cross = [va[1]*vb[2]-va[2]*vb[1], va[2]*vb[0]-va[0]*vb[2], va[0]*vb[1]-va[1]*vb[0]]
            angle = mp.atan2(mp.sqrt(sum(x*x for x in cross)), sum(x*y for x,y in zip(va,vb)))
            angles.append(mp.nstr(angle, 30))
            gaps.append(mp.nstr(mp.sqrt(sum((x-y)**2 for x,y in zip(pa,pb))), 30))
        rows.append(dict(id=result['id'], source=source_index, junction_angles_radians=angles, junction_gaps=gaps))
print(json.dumps(dict(decimal_digits=mp.mp.dps, method='Exact binary64 controls converted to 100-digit mpmath; clamped coherent-weight endpoint directions from Euclidean control chords, cross/dot atan2. Numerical reference, not a certified enclosure.', cosine_at_1e_minus_10=math.cos(1e-10), cosine_at_1e_minus_8=math.cos(1e-8), rows=rows), indent=2))
