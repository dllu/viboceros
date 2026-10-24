"""Fixed B-rep component, trim, seam and owner selection scenes."""

# B-rep controls include a rational face and a triangular trim whose support
# surface has control points outside the face. Repeated picks test identity,
# rather than coordinate-based merging across distinct components.
BREP = {}
for kind in ['brep_box', 'brep_surface', 'brep_trimmed']:
    for label, picks in [
        ('face', [('face', 0)]), ('edge', [('edge', 0)]),
        ('edges_reverse', [('edge', 2), ('edge', 0)]),
        ('mixed', [('face', 0), ('edge', 0)]),
        ('repeat', [('face', 0), ('face', 0)]),
    ]:
        BREP['component_'+kind+'_'+label] = dict(
            sources=[dict(kind=kind, mode=None, grips=[])], whole=[], order=[0],
            output='Points', layer='Input', flat=False,
            components=[[0, k, i] for k, i in picks],
            component_capture=True, component_records=True)
BREP['component_brep_box_faces_reverse'] = dict(BREP['component_brep_box_face'],
    components=[[0, 'face', 5], [0, 'face', 0]])
BREP['component_brep_box_whole_owner'] = dict(BREP['component_brep_box_face'], whole=[0])
BREP['component_brep_two_owners'] = dict(BREP['component_brep_box_face'],
    sources=[dict(kind='brep_box', mode=None, grips=[]), dict(kind='brep_surface', mode=None, grips=[])],
    order=[0, 1], components=[[1, 'face', 0], [0, 'edge', 0]])
BREP_PICKED = {}
for kind in ['brep_box', 'brep_surface', 'brep_trimmed']:
    for label in ['face', 'edge']:
        name = 'component_'+kind+'_'+label
        BREP_PICKED['postselected_'+name] = dict(BREP[name], component_getter=True)

HOLDOUTS = {}
for kind in ['brep_closed', 'brep_periodic_u', 'brep_periodic_v', 'brep_cylinder']:
    for label, components, whole in [
        ('face', [[0, 'face', 0]], []),
        ('edge', [[0, 'edge', 0]], []),
        ('whole', [], [0]),
    ]:
        HOLDOUTS['component_seams_'+kind+'_'+label] = dict(
            sources=[dict(kind=kind, mode=None, grips=[])], whole=whole, order=[0],
            output='Points', layer='Input', flat=False, components=components,
            component_capture=True, component_records=True)
    HOLDOUTS['component_seams_'+kind+'_cloud'] = dict(
        HOLDOUTS['component_seams_'+kind+'_face'], output='PointCloud',
        layer='Current', output_first=True)
HOLDOUTS['component_seams_brep_cylinder_edges'] = dict(
    HOLDOUTS['component_seams_brep_cylinder_edge'],
    components=[[0, 'edge', 2], [0, 'edge', 0]])

PERIODIC_EDGES = {}
for label, picks in [('edge', [('edge', 0)]),
                     ('edges', [('edge', 2), ('edge', 0)]),
                     ('faces', [('face', 1), ('face', 0)])]:
    PERIODIC_EDGES['component_seams_joined_periodic_'+label] = dict(
        sources=[dict(kind='brep_periodic_joined', mode=None, grips=[])],
        whole=[], order=[0], output='Points', layer='Input', flat=False,
        components=[[0,k,i] for k,i in picks],
        component_capture=True, component_records=True)
