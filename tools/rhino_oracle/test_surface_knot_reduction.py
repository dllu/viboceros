"""Schema, staging, source ownership and older-runtime public API use."""
import copy
import math
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

from . import surface_knot_reduction_probe as probe
from .generate_surface_knot_reduction import request
from .structural_sdk import stage_helpers,validate_operations


class SurfaceKnotReductionTests(unittest.TestCase):
    def test_schema_and_standalone_helper_staging(self):
        query=request();original=copy.deepcopy(query);validate_operations(query)
        op=query['operations'][0]
        for changes in [dict(extra=True),dict(direction='invalid'),dict(minimum_multiplicity=True),
                        dict(maximum_multiplicity=1),dict(absolute_tolerance=-1.),
                        dict(absolute_tolerance=float('nan')),dict(absolute_tolerance=float('inf'))]:
            with self.assertRaises(ValueError):probe.validate(dict(op,**changes))
        with tempfile.TemporaryDirectory()as directory:
            stage_helpers(query,Path(__file__).parent,Path(directory))
            self.assertEqual([p.name for p in Path(directory).iterdir()],['surface_knot_reduction_probe.py'])
        self.assertEqual(query,original)

    def host(self):
        candidate=SimpleNamespace(KnotsU=SimpleNamespace(RemoveMultipleKnots=mock.Mock(return_value=2)),
            KnotsV=SimpleNamespace(RemoveMultipleKnots=mock.Mock(return_value=3)),IsValid=True,
            Dispose=mock.Mock(),Domain=lambda _:SimpleNamespace(ParameterAt=lambda t:t),PointAt=lambda u,v:[u,v,0.])
        source=SimpleNamespace(Degree=lambda _:3,Duplicate=mock.Mock(return_value=candidate),Dispose=mock.Mock())
        host=dict(_nurbs_surface_from_definition=mock.Mock(return_value=source),
            _nurbs_surface_definition=lambda s:dict(identity='source'if s is source else'candidate'),
            _xyz=lambda p:p,_measure=lambda iterations,build:(build(),17))
        return host,source,candidate

    def test_public_duplicate_and_math_subset_support_success_and_every_failure(self):
        op=request()['operations'][0];op=dict(op,direction='both')
        for stage in ['success','duplicate','native','invalid','mutation']:
            host,source,candidate=self.host()
            if stage=='duplicate':source.Duplicate.return_value=None
            if stage=='native':candidate.KnotsU.RemoveMultipleKnots.side_effect=ValueError('native failure')
            if stage=='invalid':candidate.IsValid=False
            if stage=='mutation':
                calls=[0]
                def definition(s):
                    if s is source:calls[0]+=1
                    return dict(identity='changed'if calls[0]>1 else'original')
                host['_nurbs_surface_definition']=definition
            with self.subTest(stage=stage),mock.patch.object(probe,'math',SimpleNamespace(isnan=math.isnan,isinf=math.isinf)):
                if stage=='success':
                    value,elapsed=probe.run(op,host,1,request()['tolerance'])
                    self.assertEqual((value['removed'],len(value['samples']),elapsed),(5,81,17))
                    self.assertEqual(value['before'],value['after'])
                else:
                    with self.assertRaises(ValueError):probe.run(op,host,1,request()['tolerance'])
            source.Dispose.assert_called_once()
            self.assertEqual(candidate.Dispose.call_count,int(stage!='duplicate'))

    def test_null_and_absent_defaults_match_the_protocol(self):
        for explicit_null in [False,True]:
            op=request()['operations'][0]
            for name in ['direction','minimum_multiplicity','maximum_multiplicity','absolute_tolerance']:op.pop(name,None)
            if explicit_null:op.update(maximum_multiplicity=None,absolute_tolerance=None)
            host,_,candidate=self.host();probe.run(op,host,1,request()['tolerance'])
            for knots in [candidate.KnotsU,candidate.KnotsV]:
                knots.RemoveMultipleKnots.assert_called_once_with(1,4,1e-6)


if __name__=='__main__':unittest.main()
