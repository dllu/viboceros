"""Repeat fixed command inputs before assigning a meshing discrepancy to the kernel."""
import argparse
from .capture_family import capture
from .generate_insert_point import SOURCES
from .insert_point_cases import request as base

CASES=[
    'insert_point_full_mesh_same_edge','insert_point_regions_same_reverse',
    'insert_point_spatial_bottom_mid_left_high','insert_point_spatial_same_edge_warp',
    'insert_point_spatial_tilted_plane','insert_point_spatial_bottom_low_left_low',
    'insert_point_spatial_left_low_bottom_mid','insert_point_spatial_tall_warp',
    'insert_point_spatial_prefix_left_high_bottom_mid',
    'insert_point_spatial_prefix_same_edge_reverse','insert_point_spatial_prefix_same_edge_three',
    'insert_point_spatial_prefix_left_low','insert_point_spatial_prefix_left_high',
]

def request():
    q=base(full=True)
    q['operations']=[dict(op='insert_point_command',id=k,case=k)for k in CASES]
    return q

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--second',action='store_true')
    args=parser.parse_args()
    family='insert_point_mesh_repeats'+('_second'if args.second else'')
    capture(family,request(),['generate_insert_point_mesh_repeats.py',*SOURCES],
            'VibocerosOracleMeshInsertRepeat20261029'+('Second'if args.second else'First'),timeout=1200)
