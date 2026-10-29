import urllib.parse
import urllib.request

import cuemol as cm
from . import renderer


def load_url(aURL):
    path = urllib.parse.urlsplit(aURL).path.lower()
    compression = None
    for suffix, mode in ((".gz", "gzip"), (".xz", "xz"), (".bz2", "bzip2")):
        if path.endswith(suffix):
            path, compression = path[:-len(suffix)], mode
            break
    reader = cm.strMgr().createHandler("mmcif" if path.endswith(".cif") else "pdb", 0)
    if compression:
        reader.compress = compression
    streams = cm.strMgr()
    job = streams.loadObjectAsync(reader)
    try:
        with urllib.request.urlopen(aURL, timeout=30) as response:
            while chunk := response.read(65536):
                data = cm._internal.createBAryFromBytes(chunk)
                streams.supplyDataAsync(job, data, len(chunk))
    except BaseException:
        try:
            streams.waitLoadAsync(job)
        except Exception:
            pass
        raise
    return streams.waitLoadAsync(job)


def fetch(pdbid, scene=None):
    if not isinstance(pdbid, str) or not pdbid.isalnum():
        raise ValueError("PDB ID must be alphanumeric")
    molecule = load_url(f"https://files.rcsb.org/download/{pdbid.lower()}.cif.gz")
    molecule.name = pdbid
    cm.scene(scene).addObject(molecule)
    renderer.setupDefaultRenderer(molecule)
    return molecule
