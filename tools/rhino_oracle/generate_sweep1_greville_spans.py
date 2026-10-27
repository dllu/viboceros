"""Greville anchoring on single-span and exactly knot-refined spatial rails."""
from .capture_family import capture
from .generate_sweep1_automatic_curved import request as curved
from .generate_sweep1_refit_intervals import evaluate, insert_half


def request():
    query = curved()
    source = query['operations'][4]['rail']
    simple = insert_half(source)
    full = insert_half(insert_half(simple))
    operations = []
    for name, rail in [('bezier',source), ('simple',simple), ('full',full)]:
        degree, knots = rail['degree'], rail['knots']
        sites = [sum(knots[i+1:i+degree+1])/degree for i in range(1,len(rail['control_points'])-1)]
        for t in sites:
            point = evaluate(rail,t)
            for refit in [False,True]:
                operations.append(dict(op='sweep1',id='greville-{}-{}-{}'.format(name,t,'refit' if refit else 'basis'),
                    rail=rail,sections=[dict(type='line',start=point,end=[point[0]+.2,point[1]+.4,point[2]+.3])],
                    command=True,refit_rail=refit,inspect_definition=True))
    query['operations'] = operations
    return query


if __name__ == '__main__':
    capture('sweep1_greville_spans',request(),
            ['generate_sweep1_greville_spans.py','generate_sweep1_refit_intervals.py',
             'generate_sweep1_arc_stations.py','generate_sweep1_automatic_curved.py',
             'rhino_worker.py','client.py','structural_sdk.py'],
            'VibocerosOracleSweep1GrevilleSpans20261027')
