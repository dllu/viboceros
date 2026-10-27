"""Public Sweep1 interval choices at Greville sites and inserted rail knots."""
from copy import deepcopy
from .capture_family import capture
from .generate_sweep1_arc_stations import request as arcs
from .generate_sweep1_automatic_curved import request as curved


def evaluate(curve, t):
    p, knots = curve['degree'], curve['knots']
    t = knots[p] + t*(knots[-p-1]-knots[p])
    controls = [[x*c['weight'] for x in c['point']] + [c['weight']] for c in curve['control_points']]
    span = min(len(controls)-1, max(i for i, k in enumerate(knots) if k <= t))
    work = deepcopy(controls[span-p:span+1])
    for r in range(1, p+1):
        for j in range(p, r-1, -1):
            i = span-p+j
            alpha = (t-knots[i])/(knots[i+p-r+1]-knots[i])
            work[j] = [(1-alpha)*a+alpha*b for a, b in zip(work[j-1], work[j])]
    return [x/work[p][3] for x in work[p][:3]]


def insert_half(curve):
    curve = deepcopy(curve)
    p, knots = curve['degree'], curve['knots']
    controls = [c['point'] for c in curve['control_points']]
    k = max(i for i, t in enumerate(knots) if t <= .5)
    multiplicity = knots.count(.5)
    result = controls[:k-p+1]
    for i in range(k-p+1, k-multiplicity+1):
        alpha = (.5-knots[i])/(knots[i+p]-knots[i])
        result.append([(1-alpha)*a+alpha*b for a, b in zip(controls[i-1], controls[i])])
    result.extend(controls[k-multiplicity:])
    curve['knots'] = knots[:k+1]+[.5]+knots[k+1:]
    curve['control_points'] = [dict(point=point, weight=1.) for point in result]
    return curve


def request():
    query = arcs()
    operations = []
    def add(name, rail, point):
        for refit in [False, True]:
            end = [point[0]+.2, point[1]+.4, point[2]+.3]
            operations.append(dict(op='sweep1', id=name+('-refit' if refit else '-basis'),
                rail=rail, sections=[dict(type='line', start=point, end=end)],
                command=True, refit_rail=refit, inspect_definition=True))
    pair = {kind:next(op['rail'] for op in query['operations']
                     if op['id'].startswith('arc-station-oriented') and op['rail']['type']==kind)
            for kind in ['arc','nurbs']}
    for fraction in [.25,.5,.75]:
        point = evaluate(pair['nurbs'], fraction)
        for kind, rail in pair.items():add('greville-{}-{}'.format(kind, fraction), rail, point)
    spatial = deepcopy(curved()['operations'][4]['rail'])
    straight = dict(type='nurbs', degree=3, knots=[0.]*4+[1.]*4,
                    control_points=[dict(point=[0.,0.,z], weight=1.) for z in [0.,5/3,10/3,5.]])
    for name, source in [('straight',straight),('spatial',spatial)]:
        single = insert_half(source)
        full = insert_half(insert_half(single))
        for mult, rail in [('simple',single),('full',full)]:
            for t in [.2,.7]:add('inserted-{}-{}-{}'.format(name,mult,t),rail,evaluate(rail,t))
    query['operations'] = operations
    return query


if __name__ == '__main__':
    capture('sweep1_refit_intervals', request(),
            ['generate_sweep1_refit_intervals.py', 'generate_sweep1_arc_stations.py',
             'generate_sweep1_automatic_curved.py', 'rhino_worker.py', 'client.py', 'structural_sdk.py'],
            'VibocerosOracleSweep1RefitIntervals20261027')
