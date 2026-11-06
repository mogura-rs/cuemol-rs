import {test, expect} from '@playwright/test';
import {load, call} from './support.js';

const targetPdb = `ATOM      1  N   ALA A   1       0.000   0.000   0.000  1.00 10.00           N
ATOM      2  CA  ALA A   1       1.000   0.000   0.000  1.00 10.00           C
ATOM      3  C   ALA A   1       2.000   0.000   0.000  1.00 10.00           C
ATOM      4  CA AALA A   1       1.000   0.100   0.000  0.50 10.00           C
ATOM      5  CA BALA A   1       1.000  -0.100   0.000  0.50 10.00           C
END
`;
const sourcePdb = `ATOM      1  N   ALA B   1       0.000   2.000   0.000  1.00 10.00           N
ATOM      2  CA  ALA B   1       2.000   2.000   0.000  1.00 10.00           C
END
`;

for (const backend of ['webgl', 'webgpu']) {
  test(`Atom edits ${backend} keep identities through worker loading and application history`, async ({page}) => {
    await page.goto(`./?renderer=${backend}`);
    await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.scene?.sceneId), {timeout:60000}).toBeGreaterThan(0);
    expect(await page.evaluate(() => window.cuemol.backend)).toBe(backend);
    await load(page, 'target.pdb', Buffer.from(targetPdb));
    await expect.poll(() => page.evaluate(() => window.cuemol.status().scene.objects.length)).toBe(1);
    await load(page, 'source.pdb', Buffer.from(sourcePdb));
    await expect.poll(() => page.evaluate(() => window.cuemol.status().scene.objects.length)).toBe(2);
    const [target, source] = await page.evaluate(() => window.cuemol.status().scene.objects);
    const script = async body => {
      const state = await call(page, 'execute_script', {source:`var m=cuemol.getObject(${target.id}), source=cuemol.getObject(${source.id}); ${body}`});
      expect(state.message).not.toMatch(/^Error:/);
      return JSON.parse(state.message).log.trim();
    };
    const step = async name => {
      expect((await call(page, name)).message).not.toMatch(/^Error:/);
    };
    expect(await script("m.copyAtoms(source,cuemol.makeSel('*'));print(m.getAtomSize()+':'+m.getAtomByID(6).chainName+':'+m.getAtomByID(7).name);")).toBe('7:B:CA');
    await script("m.getAtomByID(7).pos=cuemol.Vector('(9,2,0)');");
    expect(await script("m.deleteAtoms(cuemol.makeSel('aid 2'));print(m.getAtomSize()+':'+(m.getAtomByID(2)===null)+':'+(m.getAtomByID(4)===null)+':'+(m.getAtomByID(5)===null));")).toBe('4:true:true:true');
    await step('undo');
    expect(await script('print(m.getAtomSize()+":"+m.getAtomByID(7).pos.x);')).toBe('7:9');
    await step('undo');
    expect(await script('print(m.getAtomByID(7).pos.x);')).toBe('2');
    await step('undo');
    expect(await script('print(m.getAtomSize());')).toBe('5');
    await step('redo');
    expect(await script('print(m.getAtomByID(6).chainName+":"+m.getAtomByID(7).pos.x);')).toBe('B:2');
    await step('redo');
    expect(await script('print(m.getAtomByID(7).pos.x);')).toBe('9');
    await step('redo');
    expect(await script('print(m.getAtomSize());')).toBe('4');
    for (let index = 0; index < 3; ++index) await step('undo');
    expect(await script("print(m.appendAtom1(source.getAtomByID(1),'C',1,'ALA'));")).toBe('8');
    expect(await script('print(m.getAtomByID(1).chainName+":"+m.getAtomByID(8).chainName+":"+source.getAtomSize());')).toBe('A:C:2');
  });
}
