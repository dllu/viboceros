"""Owned MergeSrf input admission and diagnostic response retention."""
import gzip
import json
import shutil
import tempfile
from pathlib import Path
import unittest
from unittest import mock
from .client import OracleProtocolError, _record_owned_input_diagnostics
from .merge_surface_command import PILOT, request, validate
from .structural_sdk import validate_operations


class MergeSurfaceCommandTests(unittest.TestCase):
    def test_authored_command_recipes_are_admitted(self):
        query = request(list(PILOT))
        validate_operations(query)
        op = query['operations'][0]
        for change in [dict(extra=1), dict(case='unknown'), dict(id=False)]:
            with self.subTest(change=change), self.assertRaises(ValueError):
                validate(dict(op, **change))

    def test_failed_owned_receipts_retain_the_complete_native_response(self):
        response = dict(results=[dict(id='owned', value=dict(history='native prompt'))])
        interaction = mock.Mock()
        interaction.record_diagnostics.side_effect = OracleProtocolError('missing owned input')
        with self.assertRaises(OracleProtocolError) as caught:
            _record_owned_input_diagnostics(interaction, response)
        self.assertIs(caught.exception.raw_response, response)
        interaction.record_diagnostics.assert_called_once_with(response)


class MergeSurfaceEvidenceTests(unittest.TestCase):
    def test_retained_capture_inventory_and_completion_classification(self):
        from .merge_surface_command_evidence import collect
        rows, orientation, captures, _ = collect()
        self.assertEqual((len(rows),len(orientation),len(captures)),(20,10,5))
        self.assertEqual(sum(v['success'] for v in rows.values()),13)
        self.assertNotIn('merge_q_getters',rows)
        self.assertNotIn('merge_d_post_sharp',rows)

    def test_capture_receipt_mutation_is_rejected(self):
        from .merge_surface_command_evidence import PROOF, collect
        with tempfile.TemporaryDirectory() as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof)
            path=proof/'captures.json'; rows=json.loads(path.read_bytes())
            for key,bad in [('exit_status',1),('observation_sha256','0'*64),('command',['python3'])]:
                altered=json.loads(json.dumps(rows));altered[0][key]=bad
                path.write_text(json.dumps(altered))
                with self.subTest(key=key),self.assertRaises(ValueError):collect(proof)

    def test_replay_inventory_and_error_gates(self):
        from .merge_surface_command_evidence import PROOF
        from .report_merge_surface_command import comparisons
        # The runner creates comparison logs before invoking this suite.
        comparisons()
        with tempfile.TemporaryDirectory() as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof)
            path=proof/'ui-portable.log.gz';text=gzip.decompress(path.read_bytes()).decode()
            lines=text.splitlines();index=next(i for i,s in enumerate(lines) if s.startswith('MERGE_SURFACE_UI '))
            row=json.loads(lines[index].split(' ',1)[1])
            variants=[lines[:index]+lines[index+1:],lines+[lines[index]]]
            for bad in [True,float('nan'),1.1e-9]:
                altered=json.loads(json.dumps(row));altered['maximum_geometry_error']=bad
                variant=lines[:];variant[index]='MERGE_SURFACE_UI '+json.dumps(altered);variants.append(variant)
            for variant in variants:
                path.write_bytes(gzip.compress('\n'.join(variant).encode(),mtime=0))
                with self.assertRaises(ValueError):comparisons(proof)


if __name__ == '__main__': unittest.main()
