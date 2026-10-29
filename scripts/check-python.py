#!/usr/bin/env python3
"""Check Python helpers and persistent live-scene bindings in a native binary."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    binary = parser.parse_args().binary.resolve()
    with tempfile.TemporaryDirectory(prefix='cuemol-python-check-') as directory:
        work = Path(directory)
        fixture = work / 'molecule λ.pdb'
        fixture.write_text(
            'HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\n'
            'HETATM    2  O1  LIG A   1       1.400   0.000   0.000  1.00 10.00           O\nEND\n',
            encoding='utf-8')
        source = """
import json
import cuemol as cm
from cuemol import cmd, fileio, logging, mol_util, net_fetch, renderer, scene_util
from cuemol.undo_txn import UndoTxn
objects = json.loads(cm.scene().getObjectTreeJSON())
molecule = cm.obj(objects[0]['ID'])
assert isinstance(molecule, cm.getWrpClass('MolCoord'))
assert len(list(mol_util.AtomIter(molecule))) == 2
assert len(list(mol_util.ResidIter(molecule))) == 1
fileio.saveObject(molecule, 'selected λ.pdb', 'pdb',
                  {'sel': cm.sel('name O1', molecule.scene_uid)})
retained_value = 'persistent λ'
"""
        second = "assert retained_value == 'persistent λ'\nassert molecule.getAtomSize() == 2"
        calls = [
            ('load_file', {'path': str(fixture)}),
            ('execute_script', {'source': f'getService("PythonBridge").runString({json.dumps(source)});'}),
            ('execute_script', {'source': f'getService("PythonBridge").runString({json.dumps(second)});'}),
        ]
        requests = [{'jsonrpc': '2.0', 'id': 0, 'method': 'initialize', 'params': {}}]
        requests.extend({'jsonrpc': '2.0', 'id': index, 'method': 'tools/call',
                         'params': {'name': name, 'arguments': arguments}}
                        for index, (name, arguments) in enumerate(calls, 1))
        env = {key: value for key, value in os.environ.items() if not key.startswith('CUEMOL_')}
        env['CUEMOL_CONFIG_DIR'] = str(work / 'config')
        result = subprocess.run([str(binary), 'mcp', 'serve'], cwd=work, env=env,
                                input=''.join(json.dumps(request) + '\n' for request in requests),
                                capture_output=True, encoding='utf-8', timeout=90)
        if result.returncode:
            raise RuntimeError(f'Python check failed ({result.returncode}):\n{result.stderr}')
        responses = {response['id']: response for response in
                     (json.loads(line) for line in result.stdout.splitlines())}
        for request in requests:
            response = responses.get(request['id'])
            if response is None or response.get('error') or response.get('result', {}).get('isError'):
                raise RuntimeError(f'Python request failed: {response}\n{result.stderr}')
        atoms = [line for line in (work / 'selected λ.pdb').read_text().splitlines()
                 if line.startswith(('ATOM  ', 'HETATM'))]
        if len(atoms) != 1 or atoms[0][12:16].strip() != 'O1':
            raise RuntimeError('Python selection did not export exactly the oxygen atom')
        print('Python helpers, live scene queries, selected PDB output and persistent state passed')


if __name__ == '__main__':
    main()
