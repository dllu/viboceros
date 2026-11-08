"""Recipe admission, owned API cleanup, and immutable matching evidence."""
import copy,gzip,json,shutil,tempfile,unittest
from pathlib import Path
from types import SimpleNamespace as NS
from unittest import mock
from . import match_surface_sdk as sdk
from .match_surface_cases import PILOT,request
from .structural_sdk import validate_operations
from .surface_match_evidence import PROOF,collect


class SurfaceMatchTests(unittest.TestCase):
    def test_recipe_admission(self):
        query=request(list(PILOT));validate_operations(query);op=query['operations'][0]
        for change in [dict(extra=True),dict(case='missing'),dict(id=False),dict(op='match')]:
            with self.subTest(change=change),self.assertRaises(ValueError):sdk.validate(dict(op,**change))

    def test_capture_inventory_and_frozen_sources(self):
        rows,diagnostics,audits,averages,captures,_=collect()
        self.assertEqual([len(rows),len(diagnostics),len(audits),len(averages),len(captures)],[66,16,9,6,5])
        self.assertIsNone(captures[2]['exit_status'])
        self.assertTrue(all(v['source_unchanged'] for v in rows.values()))

    def test_capture_receipts_cannot_hide_missing_launch_status(self):
        with tempfile.TemporaryDirectory() as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof)
            path=proof/'captures.json';receipts=json.loads(path.read_bytes())
            for index,key,value in [(0,'command',['desktop-launcher']), (0,'exit_status',None),
                    (2,'exit_status',0),(0,'observation_sha256','0'*64)]:
                changed=copy.deepcopy(receipts);changed[index][key]=value;path.write_text(json.dumps(changed))
                with self.subTest(index=index,key=key),self.assertRaises(ValueError):collect(proof)

    def test_missing_duplicate_and_invalid_replays_do_not_qualify(self):
        from .report_surface_match import comparisons
        comparisons()
        with tempfile.TemporaryDirectory() as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof)
            path=proof/'surface-portable.log.gz';lines=gzip.decompress(path.read_bytes()).decode().splitlines()
            index=next(i for i,line in enumerate(lines) if line.startswith('MATCH_SURFACE '))
            row=json.loads(lines[index].split(' ',1)[1])
            variants=[lines[:index]+lines[index+1:],lines+[lines[index]]]
            for key,bad in [('maximum_geometry_error',True),('maximum_geometry_error',float('nan')),
                            ('maximum_geometry_error',1.1e-9),('archives',False),('qualified_capture',False)]:
                altered=copy.deepcopy(row);altered[key]=bad;variant=lines[:]
                variant[index]='MATCH_SURFACE '+json.dumps(altered);variants.append(variant)
            for variant in variants:
                path.write_bytes(gzip.compress('\n'.join(variant).encode(),mtime=0))
                with self.assertRaises(ValueError):comparisons(proof)

    def host(self):
        class Brep:
            def __init__(self,name):
                self.name=name;self.changed=False;self.IsValid=True;self.Dispose=mock.Mock()
                trim=NS(IsoStatus=1,Edge=NS(name=name+' edge'))
                self.Faces=[NS(Loops=[NS(Trims=[trim])])]
        first,second=Brep('first'),Brep('second');output=Brep('matched')
        sources=[NS(ToBrep=mock.Mock(return_value=b),Dispose=mock.Mock()) for b in [first,second]]
        create=mock.Mock(return_value=(True,output,None));Brep.CreateFromMatch=create
        setting=NS(EnableRefinement=mock.Mock());constructor=mock.Mock(return_value=setting)
        geometry=NS(Brep=Brep,IsoStatus=NS(East=1,West=1),
            Continuity=NS(**{'None':0,'C0_continuous':1,'G1_continuous':2,'G2_continuous':3}),
            PreserveIsoCurveMethod=NS(Automatic=0),MatchSrfSettings=constructor)
        host=dict(Rhino=NS(Geometry=geometry),_object_source=mock.Mock(side_effect=sources))
        helper=NS(geometry_record=lambda b,*_:dict(name=b.name,changed=b.changed))
        return host,sources,[first,second],output,create,constructor,setting,helper

    def test_owned_call_and_decline(self):
        op=request(['match_position_none'])['operations'][0];tolerance=request([])['tolerance']
        for decline in [False,True]:
            host,sources,breps,output,create,constructor,setting,helper=self.host()
            if decline:create.return_value=(False,None,None)
            with mock.patch.dict('sys.modules',{'brep_join_probe':helper}):value,_=sdk.run(op,host,1,tolerance)
            self.assertIs(value['success'],not decline);constructor.assert_called_once_with(1,0)
            self.assertFalse(setting.Average);self.assertFalse(setting.MatchClosestPoints)
            setting.EnableRefinement.assert_called_once_with(False,1e-6,1e-6,.05)
            create.assert_called_once_with(breps[0].Faces[0].Loops[0].Trims[0].Edge,
                breps[1].Faces[0].Loops[0].Trims[0].Edge,setting)
            for item in sources+breps:item.Dispose.assert_called_once()
            self.assertEqual(output.Dispose.call_count,int(not decline))

    def test_native_failure_invalid_output_and_mutation_dispose_owned_geometry(self):
        op=request(['match_position_none'])['operations'][0]
        for stage in ['native','invalid','mutation']:
            host,sources,breps,output,create,_,_,helper=self.host()
            if stage=='native':create.side_effect=ValueError('native failure')
            elif stage=='invalid':output.IsValid=False
            else:
                def changed(*_):breps[0].changed=True;return True,output,None
                create.side_effect=changed
            with self.subTest(stage=stage),mock.patch.dict('sys.modules',{'brep_join_probe':helper}),self.assertRaises(ValueError):
                sdk.run(op,host,1,request([])['tolerance'])
            for item in sources+breps:item.Dispose.assert_called_once()
            self.assertEqual(output.Dispose.call_count,int(stage!='native'))


    def test_prefit_hypothesis_disposes_curves_and_checks_owned_surfaces(self):
        key='match_prefit_full_gap_1e-06';recipe=dict(sdk.CASES[key],audit=False)
        for stage in ['success','mutation','native']:
            host,sources,breps,output,_,_,_,helper=self.host()
            curve=NS(Dispose=mock.Mock())
            breps[1].Faces[0].Loops[0].Trims[0].Edge.DuplicateCurve=mock.Mock(return_value=curve)
            checks=[type(b)(b.name)for b in breps]
            for source,b,check in zip(sources,breps,checks):source.ToBrep.side_effect=[b,check]
            sources[0].MatchToCurve=mock.Mock(return_value=None)
            if stage=='mutation':checks[0].changed=True
            if stage=='native':sources[0].MatchToCurve.side_effect=ValueError('native failure')
            with self.subTest(stage=stage),mock.patch.dict(sdk.CASES,{key:recipe}),mock.patch.dict('sys.modules',{'brep_join_probe':helper}):
                if stage=='success':
                    value,_=sdk.run(dict(op='match_surface_sdk',id=key,case=key),host,1,request([])['tolerance'])
                    self.assertEqual([q['level']for q in value['prefit_audit']],[0,1,2,3,4])
                    self.assertTrue(all(q['output'] is None for q in value['prefit_audit']))
                else:
                    with self.assertRaises(ValueError):sdk.run(dict(op='match_surface_sdk',id=key,case=key),host,1,request([])['tolerance'])
            curve.Dispose.assert_called_once()
            for obj in sources+breps+[output]:obj.Dispose.assert_called_once()
            checks[0].Dispose.assert_has_calls([] if stage=='native' else [mock.call()])
            self.assertEqual(checks[1].Dispose.call_count,int(stage=='success'))

if __name__=='__main__':unittest.main()
