"""Admission, frozen native receipts and MatchSrf replay evidence gates."""
import copy,gzip,json,shutil,tempfile,unittest
from pathlib import Path
from .match_surface_command import PILOT,request,validate
from .match_surface_command_evidence import PROOF,collect
from .structural_sdk import validate_operations


class MatchSurfaceCommandTests(unittest.TestCase):
    def test_prescribed_recipe_admission(self):
        query=request(list(PILOT));validate_operations(query);op=query['operations'][0]
        for change in [dict(extra=True),dict(case='missing'),dict(id=True),dict(op='match')]:
            with self.subTest(change=change),self.assertRaises(ValueError):validate(dict(op,**change))

    def test_frozen_command_captures_and_diagnostic_timeout(self):
        rows,diagnostics,captures,_=collect()
        self.assertEqual([len(rows),len(diagnostics),len(captures)],[21,4,3])
        self.assertEqual(sum(v['history_changed'] for v in rows.values()),17)
        self.assertTrue(diagnostics['match_cmd_option_menus']['input_trace'][0]['errors'])
        self.assertTrue(all(not v['input_trace'][0]['errors'] for v in rows.values()))

    def test_failed_missing_and_changed_capture_receipts_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof)
            path=proof/'captures.json';receipts=json.loads(path.read_bytes())
            for change in [receipts[:-1],receipts+receipts[-1:]]:
                path.write_text(json.dumps(change))
                with self.assertRaises(ValueError):collect(proof)
            for key,value in [('command',['desktop-launcher']),('exit_status',None),('observation_sha256','0'*64)]:
                changed=copy.deepcopy(receipts);changed[0][key]=value;path.write_text(json.dumps(changed))
                with self.subTest(key=key),self.assertRaises(ValueError):collect(proof)

    def test_missing_duplicate_and_invalid_replays_are_rejected(self):
        from .report_match_surface_command import comparisons
        comparisons()
        with tempfile.TemporaryDirectory() as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof)
            path=proof/'ui-portable.log.gz';lines=gzip.decompress(path.read_bytes()).decode().splitlines()
            i=next(i for i,line in enumerate(lines) if line.startswith('MATCH_SURFACE_UI '))
            row=json.loads(lines[i].split(' ',1)[1])
            variants=[lines[:i]+lines[i+1:],lines+[lines[i]]]
            for key,bad in [('maximum_geometry_error',True),('maximum_geometry_error',float('nan')),
                            ('maximum_geometry_error',1.1e-9),('archives',False),('changed',None)]:
                changed=copy.deepcopy(row);changed[key]=bad;variant=lines[:]
                variant[i]='MATCH_SURFACE_UI '+json.dumps(changed);variants.append(variant)
            for variant in variants:
                path.write_bytes(gzip.compress('\n'.join(variant).encode(),mtime=0))
                with self.assertRaises(ValueError):comparisons(proof)


if __name__=='__main__':unittest.main()
