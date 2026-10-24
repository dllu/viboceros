"""Fixed grip extraction scenes; requests cannot supply executable input."""
from copy import deepcopy
try:
    from .extract_points_component_cases import CASES as COMPONENTS, CURVES as COMPONENT_CURVES, EDGE_HOLDOUTS, SELECTION as COMPONENT_SELECTION, MIXED as COMPONENT_MIXED, MIXED_VERIFIED, PICKED_CONTROLS, PICKED
except (ImportError, ValueError):
    from extract_points_component_cases import CASES as COMPONENTS, CURVES as COMPONENT_CURVES, EDGE_HOLDOUTS, SELECTION as COMPONENT_SELECTION, MIXED as COMPONENT_MIXED, MIXED_VERIFIED, PICKED_CONTROLS, PICKED

try:
    from .extract_points_brep_cases import BREP, BREP_PICKED, HOLDOUTS as BREP_HOLDOUTS, PERIODIC_EDGES
except (ImportError, ValueError, SystemError):
    from extract_points_brep_cases import BREP, BREP_PICKED, HOLDOUTS as BREP_HOLDOUTS, PERIODIC_EDGES

try:
    from .extract_points_target_cases import DISCOVERY as TARGET_DISCOVERY, ECHO as TARGET_ECHO, MENU as TARGET_MENU
except (ImportError, ValueError, SystemError):
    from extract_points_target_cases import DISCOVERY as TARGET_DISCOVERY, ECHO as TARGET_ECHO, MENU as TARGET_MENU


def source(kind='rational', mode='control', grips=None):
    return dict(kind=kind, mode=mode, grips=[2, 0] if grips is None else grips)


CASES = {}
for kind in ['curve', 'rational', 'closed', 'periodic', 'polyline', 'circle', 'arc',
             'surface', 'surface_closed', 'surface_periodic_u', 'surface_periodic_v',
             'mesh', 'polycurve', 'subd']:
    CASES[kind+'_control'] = dict(sources=[source(kind)])
for kind in ['rational', 'closed', 'periodic', 'circle', 'polycurve', 'subd']:
    CASES[kind+'_edit'] = dict(sources=[source(kind, 'edit')])
for output in ['Points', 'PointCloud']:
    for layer in ['Input', 'Current']:
        CASES['options_'+output.lower()+'_'+layer.lower()] = dict(
            sources=[source()], output=output, layer=layer)
for name, extra in [
    ('mixed_grips', {}),
    ('mixed_grips_reverse', dict(order=[2, 1, 0])),
    ('mixed_grips_cloud', dict(output='PointCloud')),
    ('mixed_grips_cloud_reverse', dict(output='PointCloud', order=[2, 1, 0])),
    ('whole_and_grips', dict(whole=[2])),
    ('whole_and_grips_cloud', dict(whole=[2], output='PointCloud')),
    ('owner_and_grips', dict(whole=[0])),
    ('owner_and_grips_cloud', dict(whole=[0], output='PointCloud')),
]:
    CASES[name] = dict(sources=[source(), source('circle', 'edit'), source('mesh', grips=[])], **extra)
CASES['whole_displayed_owner'] = dict(sources=[source(grips=[])], whole=[0])
for name, case in CASES.items():
    case.setdefault('output', 'Points')
    case.setdefault('layer', 'Input')
    case.setdefault('whole', [])
    case.setdefault('order', list(range(len(case['sources']))))


CONTROLS = {}
for output in ['Points', 'PointCloud']:
    for layer in ['Input', 'Current']:
        CONTROLS['whole_'+output.lower()+'_'+layer.lower()] = dict(
            sources=[source(grips=[])], whole=[0], output=output, layer=layer, order=[0])
for layer in ['Input', 'Current']:
    CONTROLS['grips_output_first_'+layer.lower()] = dict(sources=[source()],
        whole=[], output='PointCloud', layer=layer, order=[0], output_first=True)
CONTROLS['mixed_whole_cloud'] = dict(sources=[source(grips=[]), source('circle', grips=[])],
    whole=[1, 0], output='PointCloud', layer='Input', order=[0, 1])
VERIFIED = {'getter_'+name:dict(deepcopy(case), getter=True) for name, case in CASES.items()}
METADATA = {
    'metadata_whole_first': dict(sources=[source(grips=[]), source('circle')], whole=[0], order=[1,0]),
    'metadata_unselected_first': dict(sources=[source(grips=[]), source(), source('mesh', grips=[])], whole=[2], order=[1,0,2]),
    'metadata_whole_first_two_grips': dict(sources=[source(grips=[]), source('circle'), source('mesh')], whole=[0], order=[2,1,0]),
}
for case in METADATA.values():
    case.update(output='PointCloud', layer='Input', getter=True)
FAMILIES = dict(grips=CASES, controls=CONTROLS, verified=VERIFIED, metadata=METADATA,
                components=COMPONENTS, component_curves=COMPONENT_CURVES, component_edge_holdouts=EDGE_HOLDOUTS, component_selection=COMPONENT_SELECTION, component_mixed=COMPONENT_MIXED, component_mixed_verified=MIXED_VERIFIED, component_picked_controls=PICKED_CONTROLS, component_picked=PICKED,
                component_brep=BREP, component_brep_picked=BREP_PICKED,
                component_brep_holdouts=BREP_HOLDOUTS,
                component_brep_periodic_edges=PERIODIC_EDGES,
                target_discovery=TARGET_DISCOVERY, target_echo=TARGET_ECHO, target_menu=TARGET_MENU)
ALL_CASES = {name:case for cases in FAMILIES.values() for name, case in cases.items()}


def request(family='grips'):
    return dict(protocol_version=1, iterations=1, operations=[
        dict(op='extract_points_command', id=name, case=name) for name in FAMILIES[family]])


def validate(operation):
    if (not isinstance(operation, dict) or set(operation) != {'op', 'id', 'case'} or
            operation.get('op') != 'extract_points_command' or
            not isinstance(operation.get('case'), str) or operation['case'] not in ALL_CASES or
            operation['id'] != operation['case']):
        raise ValueError('fixed ExtractPt grip scene required')
