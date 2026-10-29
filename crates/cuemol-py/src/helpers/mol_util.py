import cuemol


class AtomIter:
    iterator_type = "AtomIterator"

    def __init__(self, aMol, aSel="*"):
        molecule = cuemol.obj(aMol)
        self.mIter = cuemol.createObj(self.iterator_type)
        self.mIter.target = molecule
        self.mIter.sel = cuemol.sel(aSel, molecule.scene_uid)
        self.mIter.first()

    def __iter__(self):
        return self

    def __next__(self):
        if not self.mIter.hasMore():
            raise StopIteration
        value = self.mIter.get()
        self.mIter.next()
        return value


class ResidIter(AtomIter):
    iterator_type = "ResidIterator"


def forEachResid(aMol, aSel, aFn):
    for residue in ResidIter(aMol, aSel):
        if aFn(residue):
            break


def sameAtom(aMol, aAtom):
    return aMol.getAtom(aAtom.chainName, aAtom.residIndex, aAtom.name)


def sameResid(aMol, aResid):
    return aMol.getResidue(aResid.chainName, aResid.sindex)


def _transform(molecule, matrix, selection, notify):
    molecule = cuemol.obj(molecule)
    molecule.xformByMat(matrix, cuemol.sel("*" if selection is None else selection, molecule.scene_uid))
    if notify:
        molecule.fireAtomsMoved()


def rotate(aMol, aCen, aAxis, aDeg, aSel=None, aNotify=True):
    matrix = cuemol.createObj("Matrix")
    matrix.setRotate(aCen, aAxis, aDeg)
    _transform(aMol, matrix, aSel, aNotify)


def rotateZ(aMol, aCen, aDeg, aSel=None, aNotify=True):
    rotate(aMol, aCen, cuemol.vec(0, 0, 1), aDeg, aSel, aNotify)


def shift(aMol, aShift, aSel=None, aNotify=True):
    matrix = cuemol.createObj("Matrix")
    matrix.setTranslate(aShift)
    _transform(aMol, matrix, aSel, aNotify)


def showArrow(aMol, aRendName, aPos1, aPos2, aMol2=None):
    renderer = aMol.getRendererByName(aRendName)
    if renderer is None or renderer.type_name != "atomintr":
        renderer = aMol.createRenderer("atomintr")
        renderer.name = aRendName
    renderer.mode = "fancy"
    renderer.captype_end = renderer.captype_start = "arrow"
    renderer.width = 0.2
    renderer.stipple0 = renderer.stipple1 = 0
    if cuemol.isimpl(aPos1, "Vector") and cuemol.isimpl(aPos2, "Vector"):
        renderer.appendBy2Vecs(aPos1, aPos2)
    elif cuemol.isimpl(aPos1, "MolAtom") and cuemol.isimpl(aPos2, "MolAtom"):
        renderer.appendById(aPos1.id, (aMol if aMol2 is None else aMol2).uid, aPos2.id, False)
    else:
        raise RuntimeError("showArrow() requires two vectors or two atoms")


def del_atoms(aMol, aSel):
    molecule = cuemol.obj(aMol)
    cuemol.svc("MolAnlManager").deleteAtoms(molecule, cuemol.sel(aSel, molecule.getScene()))


def _alignment_args(aRefMol, aRefSel, aMovMol, aMovSel):
    reference, moving = cuemol.obj(aRefMol), cuemol.obj(aMovMol)
    return reference, cuemol.sel(aRefSel, reference.getScene()), moving, cuemol.sel(aMovSel, moving.getScene())


def ssm_fit(aRefMol, aRefSel, aMovMol, aMovSel):
    return cuemol.svc("MolAnlManager").superposeSSM1(*_alignment_args(aRefMol, aRefSel, aMovMol, aMovSel), False)


def usalign_fit(aRefMol, aRefSel, aMovMol, aMovSel, aPivot="auto"):
    return cuemol.svc("MolAnlManager").superposeUSalign1(*_alignment_args(aRefMol, aRefSel, aMovMol, aMovSel), aPivot, False)


def calc_rmsd(aRefMol, aRefSel, aMovMol, aMovSel):
    return cuemol.svc("MolAnlManager").calcRMSD(*_alignment_args(aRefMol, aRefSel, aMovMol, aMovSel), "")


def merge(aToMol, aFromMol, aFromSel, aCopy=False):
    manager = cuemol.svc("MolAnlManager")
    destination, source = cuemol.obj(aToMol), cuemol.obj(aFromMol)
    selection = cuemol.sel(aFromSel, source.getScene())
    manager.copyAtoms(destination, source, selection)
    if not aCopy:
        manager.deleteAtoms(source, selection)


def chg_chain(aFromMol, aFromSel, aNewChName):
    molecule = cuemol.obj(aFromMol)
    cuemol.svc("MolAnlManager").changeChainName(molecule, cuemol.sel(aFromSel, molecule.getScene()), aNewChName)
