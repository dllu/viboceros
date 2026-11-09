"""Keep Native mismatches, runtime failures and the complete replay scope visible."""
import copy,gzip,json,shutil,tempfile,unittest
from pathlib import Path
from .report_surface_knot_reduction import ROOT,PROOF,build,collect,sha


class SurfaceKnotReductionRecordTests(unittest.TestCase):
    def test_record_reproduces_and_declines_are_not_native_matches(self):
        value=build()
        self.assertEqual(value,json.loads((ROOT/'docs/surface-knot-reduction-validation.json').read_bytes()))
        self.assertFalse(value['full_rhino_parity'])
        self.assertEqual((value['native_observations'],value['retained_failed_launches']),(88,2))
        for result in value['backends'].values():
            self.assertEqual((result['matched'],result['declined']),(64,24))
        for path,digest in value['evidence_sha256'].items():self.assertEqual(sha((ROOT/path).read_bytes()),digest,path)

    def test_missing_captures_sources_and_failed_attempts_cannot_be_hidden(self):
        with tempfile.TemporaryDirectory()as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof)
            for name,key in [('captures.json',None),('attempts.json',None),('sources.json','overrides')]:
                path=proof/name;original=path.read_bytes();value=json.loads(original)
                if key is None:changed=value[:-1]
                else:changed=copy.deepcopy(value);changed[key].pop(next(iter(changed[key])))
                path.write_text(json.dumps(changed))
                with self.assertRaises(ValueError):build(proof)
                path.write_bytes(original)

    def test_backend_rows_and_the_error_gate_cannot_be_erased(self):
        with tempfile.TemporaryDirectory()as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);path=proof/'native.log.gz'
            lines=gzip.decompress(path.read_bytes()).decode().splitlines();index=next(i for i,s in enumerate(lines)if s.startswith('SURFACE_REDUCTION '))
            changed=lines[:];row=json.loads(lines[index].split(' ',1)[1]);row['maximum_native_bound']=1e-3;changed[index]='SURFACE_REDUCTION '+json.dumps(row)
            for variant in [changed,lines[:index]+lines[index+1:]]:
                path.write_bytes(gzip.compress('\n'.join(variant).encode(),mtime=0))
                with self.assertRaises(ValueError):build(proof)


if __name__=='__main__':unittest.main()
