"""Immutable tangent boundary and public SDK capture inventories."""
import gzip,json
from pathlib import Path
HERE=Path(__file__).parent
UI_FAMILIES={'line_tangent_arcs':18,'line_tangent_arcs_ends':10,'line_tangent_arcs_controls':5}
SDK_FAMILIES={'line_tangent_arcs_sdk':8,'line_tangent_arcs_bounds':9}
FAMILIES=dict(UI_FAMILIES,**SDK_FAMILIES)
DIAGNOSTICS={'arc_scaled':'The pilot divided integer coordinates by 2 under IronPython 2, producing a different arc from the Python 3 recipe. Retained as an input diagnostic; arc_scaled_corrected uses float multiplication.'}
KNOWN={k:'The public local tangent query fails from the measured station although an interior tangent exists. Native Line falls back to its closest endpoint tangent; the independent solver finds the interior tangent.'for k in ['arc_station_5','arc_middle_control','arc_reversed','arc_as_nurbs','arc_translated','arc_hover_contact','arc_hover_endpoint','arc_scaled_corrected']}
def read(family):return json.loads(gzip.decompress((HERE/('observations/'+family+'.json.gz')).read_bytes()))
def observations():return {r['id']:r['value']for family in FAMILIES for r in read(family)['results']}
def sdk_variants(value):return value.get('contact_sdk',value.get('contacts'))
def failure_ranges(queries):
 ranges=[]
 for i,q in enumerate(queries):
  if q['success']:continue
  if ranges and i==ranges[-1][-1]+1:ranges[-1].append(i)
  else:ranges.append([i])
 return [[queries[r[0]]['seed'],queries[r[-1]]['seed']]for r in ranges]
