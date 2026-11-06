import {test, expect} from '@playwright/test';
import {load, call} from './support.js';

const pdb = 'ATOM      1  CA  ALA A   1       1.000   0.000   0.000  1.00 10.00           C\nEND\n';
for (const renderer of ['webgl', 'webgpu']) {
  test(`Scene undo ${renderer} preserves later camera navigation and redo`, async ({page}) => {
    await page.goto(`./?renderer=${renderer}`);
    await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.scene?.sceneId), {timeout:60000}).toBeGreaterThan(0);
    await load(page, 'history.pdb', Buffer.from(pdb));
    await expect.poll(() => page.evaluate(() => window.cuemol.status().scene.objects.length)).toBe(1);
    const state = await page.evaluate(() => window.cuemol.status());
    const object = state.scene.objects[0];
    await call(page, 'rename_node', {nodeType:'object', nodeId:object.id, name:'edited'});
    const navigated = await call(page, 'set_view', {center:[12, 23, 34], zoom:51, slab:67, rotation:[0.5,0.5,0.5,0.5]});
    const undone = await call(page, 'undo');
    expect(undone.scene.objects[0].name).toBe(object.name);
    expect(undone.view).toEqual(navigated.view);
    const later = await call(page, 'set_view', {center:[-1, 2, 7], zoom:21});
    const redone = await call(page, 'redo');
    expect(redone.scene.objects[0].name).toBe('edited');
    expect(redone.view).toEqual(later.view);
    const scripted = await call(page, 'execute_script', {source:"scene.getView(parseInt(scene.view_uids.split(',')[0])).zoom=97;"});
    expect(scripted.view.zoom).toBe(97);
    const scriptUndo = await call(page, 'undo');
    expect(scriptUndo.scene.objects[0].name).toBe(object.name);
    expect(scriptUndo.view.zoom).toBe(97);
    const scriptRedo = await call(page, 'redo');
    expect(scriptRedo.scene.objects[0].name).toBe('edited');
    expect(scriptRedo.view.zoom).toBe(97);
  });
}
