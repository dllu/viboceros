"""Capture immutable native Loft splitting and document-history observations."""
import argparse,gzip,hashlib,json
from pathlib import Path
from .client import OracleClient
from .loft_splitting_cases import domain_speed_jets, domain_quadrature, open_basis_holdouts, orientation_holdouts, orientation_witnesses, closed_orientation_regular
from .loft_splitting_cases import request,diagnostics,reorder_request,angle_request,manual_diagnostics,seam_diagnostics,basis_diagnostics,simplified_diagnostics,reorder_inspection,tangent_jets,chart_diagnostics,domain_holdouts,domain_compatibility,domain_preparation,domain_matching,domain_aspect_holdouts,domain_geometric_matching,domain_fastpath_holdouts,domain_speed_samples

HERE=Path(__file__).resolve().parent
SOURCES=['generate_loft_splitting.py','loft_splitting_cases.py','loft_splitting_probe.py','curve_definition_validation.py',
         'loft_compatibility_probe.py',
         'surface_apex_tangent_cases.py','fixtures/loft_command.json','client.py','rhino_worker.py','structural_sdk.py','join_probe.py','merge_edges_probe.py']


def main():
    recipes = {
        'main': ('loft_splitting', request),
        'smoke': ('loft_splitting_smoke', lambda: request(True)),
        'diagnostics': ('loft_splitting_diagnostics', diagnostics),
        'reorder': ('loft_splitting_reorder', reorder_request),
        'reorder-inspection': ('loft_splitting_reorder_inspection', reorder_inspection),
        'angles': ('loft_splitting_angles', angle_request),
        'tangent-jets': ('loft_splitting_tangent_jets', tangent_jets),
        'chart-diagnostics': ('loft_splitting_chart_diagnostics', chart_diagnostics),
        'domain-holdouts': ('loft_splitting_domain_holdouts', domain_holdouts),
        'domain-compatibility': ('loft_splitting_domain_compatibility', domain_compatibility),
        'domain-preparation': ('loft_splitting_domain_preparation', domain_preparation),
        'domain-matching': ('loft_splitting_domain_matching', domain_matching),
        'domain-aspect-holdouts': ('loft_splitting_domain_aspect_holdouts', domain_aspect_holdouts),
        'domain-geometric-matching': ('loft_splitting_domain_geometric_matching', domain_geometric_matching),
        'domain-fastpath-holdouts': ('loft_splitting_domain_fastpath_holdouts', domain_fastpath_holdouts),
        'domain-speed-samples': ('loft_splitting_domain_speed_samples', domain_speed_samples),
        'domain-speed-jets': ('loft_splitting_domain_speed_jets', domain_speed_jets),
        'domain-quadrature': ('loft_splitting_domain_quadrature', domain_quadrature),
        'open-basis-holdouts': ('loft_splitting_open_basis_holdouts', open_basis_holdouts),
        'orientation-holdouts': ('loft_splitting_orientation_holdouts', orientation_holdouts),
        'orientation-witnesses': ('loft_splitting_orientation_witnesses', orientation_witnesses),
        'closed-orientation-regular': ('loft_splitting_closed_orientation_regular', closed_orientation_regular),
        'manual': ('loft_splitting_manual', manual_diagnostics),
        'seam-diagnostics': ('loft_splitting_seam_diagnostics', seam_diagnostics),
        'basis-diagnostics': ('loft_splitting_basis_diagnostics', basis_diagnostics),
        'simplified-diagnostics': ('loft_splitting_simplified_diagnostics', simplified_diagnostics),
    }
    parser=argparse.ArgumentParser(description=__doc__)
    modes=parser.add_mutually_exclusive_group()
    for name in recipes:
        if name!='main':modes.add_argument('--'+name,dest='family',action='store_const',const=name)
    parser.set_defaults(family='main')
    args=parser.parse_args();family,recipe=recipes[args.family]
    query=HERE/('fixtures/'+family+'.json');output=HERE/('observations/'+family+'.json')
    if output.exists():raise ValueError('native Loft splitting captures are immutable')
    q=recipe();query.write_text(json.dumps(q,indent=2)+'\n')
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    sources={p:digest(HERE/p)for p in SOURCES}
    print('Capturing {} Loft splitting commands'.format(len(q['operations'])),flush=True)
    response=OracleClient(settings_scheme='VibocerosOracleLoftSplitting20261024').run_rhino(q,timeout=1200)
    if sources!={p:digest(HERE/p)for p in SOURCES}:raise ValueError('native Loft producer changed during capture')
    if len(response['results'])!=len(q['operations']) or any('error'in row for row in response['results']):raise ValueError('incomplete Loft splitting capture')
    response['provenance']=dict(request_sha256=digest(query),sources=sources)
    output.write_text(json.dumps(response,indent=2,allow_nan=False)+'\n')
    for p,sha in sources.items():
        archive=HERE/'observations/source_snapshots'/(sha+'.gz')
        if not archive.exists():archive.write_bytes(gzip.compress((HERE/p).read_bytes(),mtime=0))
    print('Recorded '+family,flush=True)


if __name__=='__main__':main()
