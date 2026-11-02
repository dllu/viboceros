"""Freeze native analysis-mesh extraction, presentation attributes and history."""
from .capture_family import capture, HERE
import gzip
from .extract_analysis_mesh_probe import validate

FAMILY='extract_analysis_mesh'


def request():
    operations=[]
    def add(source, mode, preselect, steps=None):
        op=dict(op='extract_analysis_mesh',id=source+'-'+mode+'-'+str(int(preselect))+('-repeat' if steps else ''),
                source=source,mode=mode,preselect=preselect,steps=steps or ['Extract','Undo','Redo'])
        validate(op);operations.append(op)
    for i,source in enumerate(('plane','warped','holed','box','sphere','subd_plane','subd_box','extrusion')):
        for j,mode in enumerate(('None','Draft','DraftOff','Curvature','CurvatureOff')):
            add(source,mode,(i+j)%2==0)
    for source,mode in (('mesh','None'),('mesh','Draft'),('line','None'),('block','None'),('block_multi','None')):
        for preselect in (False,True):add(source,mode,preselect)
    for source,mode in (('plane','Draft'),('box','Curvature'),('subd_plane','None')):
        add(source,mode,True,['Extract','Extract','Undo','Redo'])
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-8,relative=1e-12,angular=1e-8),operations=operations)


def main():
    compressed=HERE/('observations/'+FAMILY+'.json.gz')
    if compressed.exists():raise ValueError('successful analysis-mesh captures are immutable')
    capture(FAMILY,request(),['generate_extract_analysis_mesh.py','extract_analysis_mesh_probe.py',
        'client.py','rhino_worker.py','structural_sdk.py','join_probe.py','merge_edges_probe.py',
        *['fixtures/brep_direction/'+name+'.3dm' for name in ('warped','holed','box','sphere')]],
        'VibocerosOracleExtractAnalysisQualified20261102',timeout=1200)
    raw=HERE/('observations/'+FAMILY+'.json')
    original=raw.read_bytes()
    with compressed.open('xb') as output:output.write(gzip.compress(original,mtime=0))
    if gzip.decompress(compressed.read_bytes())!=original:raise ValueError('capture compression changed bytes')
    raw.unlink()


if __name__=='__main__':main()
