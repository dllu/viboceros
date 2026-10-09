"""Canonicalize native reconstructed member permutations with consistent paths.

Repeated identical nested-editor captures demonstrate unstable ancestor member
order. Raw responses stay unchanged; only this explicit comparison policy remaps
member slots and matching leaf paths. Geometry sample order remains significant.
"""
import copy
import json

POLICY = 'block_context_member_permutation'

def canonical(value):
    value=copy.deepcopy(value)
    for state in value['states']:
        permutations={}
        for definition in state['definitions']:
            members=definition['members']
            ranked=sorted(enumerate(members),key=lambda item:json.dumps(item[1],sort_keys=True,separators=(',',':')))
            permutations[definition['name']]={old:new for new,(old,member) in enumerate(ranked)}
            definition['members']=[member for old,member in ranked]
        for object in state['objects']:
            geometry=object['geometry']
            if geometry.get('kind')!='block':continue
            for leaf in geometry['leaves']:
                leaf['path']=[[name,permutations[name][index]] for name,index in leaf['path']]
            geometry['leaves'].sort(key=lambda leaf:json.dumps(leaf,sort_keys=True,separators=(',',':')))
    return value
