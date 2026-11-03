"""Public headless load/get/save behavior for owned analysis parameter profiles."""
import binascii
import os

ARCHIVES = ('zero','zero-default','zero-constructor','zero-tolerance','zero-grid64',
            'zero-simple','zero-subd2','zero-render-quality','zero-render-custom',
            'tiny','epsilon','positive')
IDENTITY_ARCHIVES = ('zero-compute-off','zero-texture2','zero-custom','zero-absolute4',
                     'default-compute-on','default-texture1','default-custom')
PARAMETERS = ('RelativeTolerance','Tolerance','GridMinCount','GridMaxCount','GridAngle',
              'GridAspectRatio','RefineAngle','RefineGrid','SimplePlanes',
              'ComputeCurvature','MinimumEdgeLength','MaximumEdgeLength','JaggedSeams')


def validate(op):
    if (not isinstance(op,dict) or set(op) != {'op','id','archive'}
            or op['op'] not in ('analysis_profile','analysis_profile_identity')
            or op['archive'] not in (ARCHIVES if op['op']=='analysis_profile' else IDENTITY_ARCHIVES)
            or op['id'] != op['archive']):
        raise ValueError('fixed analysis profile archive required')


def run(op,host,iterations,tolerance):
    validate(op)
    R,S=host['Rhino'],host['System'];G=R.Geometry
    active=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(active.Objects) or R.Commands.Command.InCommand():
        raise ValueError('analysis profile capture requires empty owned idle document')

    def parameters(doc):
        mp=doc.GetAnalysisMeshingParameters();empty=G.SubD();subd=mp.SubDDisplayParameters()
        try:
            result={key:getattr(mp,key) for key in PARAMETERS}
            result.update(subd_level=int(subd.DisplayDensity(empty)),subd_adaptive=bool(subd.DisplayDensityIsAdaptive))
            return result
        finally:subd.Dispose();empty.Dispose();mp.Dispose()

    source=os.path.join(os.path.dirname(__file__),'fixtures','analysis_profiles',op['archive']+'.3dm')
    outputs=[]
    for read_first in (False,True):
        loaded=R.RhinoDoc.OpenHeadless(source)
        if loaded is None:raise ValueError('owned analysis profile load failed')
        output=S.IO.Path.Combine(S.IO.Path.GetTempPath(),'VibocerosAnalysisProfile_'+str(S.Guid.NewGuid())+'.3dm')
        try:
            before=parameters(loaded) if read_first else None
            options=R.FileIO.FileWriteOptions()
            options.FileVersion=8;options.SuppressDialogBoxes=True;options.SuppressAllInput=True
            options.UpdateDocumentPath=False;options.WriteUserData=True
            try:
                if not loaded.Write3dmFile(output,options):raise ValueError('owned analysis profile save failed')
            finally:options.Dispose()
            after=parameters(loaded)
            reopened=R.RhinoDoc.OpenHeadless(output)
            if reopened is None:raise ValueError('owned saved analysis profile load failed')
            try:restored=parameters(reopened)
            finally:reopened.Dispose()
            with open(output,'rb') as stream:encoded=stream.read()
            outputs.append(dict(read_first=read_first,before=before,after=after,restored=restored,
                                archive_hex=binascii.hexlify(encoded).decode('ascii')))
        finally:
            loaded.Dispose()
            if S.IO.File.Exists(output):S.IO.File.Delete(output)
    if R.RhinoDoc.ActiveDoc.RuntimeSerialNumber!=active.RuntimeSerialNumber or list(active.Objects):
        raise ValueError('headless profile reads changed the active document')
    return dict(variants=outputs),0
