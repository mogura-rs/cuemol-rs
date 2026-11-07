import {readFile, copyFile} from 'node:fs/promises';
import {test, expect} from '@playwright/test';
import {call, load} from './support.js';

async function click(page, key) {
  await expect.poll(() => page.evaluate(key => window.cuemol.status().ui.rects[key], key)).toBeTruthy();
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  const [x,y,w,h] = await page.evaluate(key => window.cuemol.status().ui.rects[key], key);
  await page.mouse.click(x+w/2, y+h/2);
}

for (const renderer of ['webgl', 'webgpu']) {
  test(`Scene geometry ${renderer} downloads STL MQO and standalone POV without changing history`, async ({page}, testInfo) => {
    await page.goto(`./?renderer=${renderer}`);
    await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.scene?.sceneId), {timeout:60000}).toBeGreaterThan(0);
    await load(page, 'probe.pdb', Buffer.from('HETATM    1  C1  LIG A   1       2.000   0.000   0.000  1.00 10.00           C\nEND\n'));
    const before = await page.evaluate(() => window.cuemol.status());
    const id = before.scene.objects[0].id;
    await call(page, 'rename_node', {nodeType:'object',nodeId:id,name:'exported object'});
    const current = await call(page, 'set_view', {center:[3,-5,7],zoom:31,slab:50,distance:100});
    for (const format of ['stl','mqo','pov']) {
      const pending = page.waitForEvent('download');
      await click(page, 'Rendering menu');
      await click(page, 'Export scene');
      await click(page, `Export ${format}`);
      const downloaded = await pending;
      expect(downloaded.suggestedFilename()).toBe(`scene.${format}`);
      const bytes = await readFile(await downloaded.path());
      await copyFile(await downloaded.path(), testInfo.outputPath(`scene.${format}`));
      if (format === 'stl') {
        const count = bytes.readUInt32LE(80);
        expect(count).toBeGreaterThan(0);
        expect(bytes.length).toBe(84+count*50);
        for(let offset=84;offset<bytes.length;offset+=50) for(let vertex=0;vertex<3;vertex++) {
          const point=[0,1,2].map(axis=>bytes.readFloatLE(offset+12+vertex*12+axis*4));
          for(const [axis,center] of [-1,5,-7].entries()) expect(Math.abs(point[axis]-center)).toBeLessThan(3);
        }
      } else if(format === 'mqo') {
        expect(bytes.toString()).toMatch(/^Metasequoia Document/);
        expect(bytes.toString()).toMatch(/vertex [1-9]/);
        expect(bytes.toString()).toMatch(/face [1-9]/);
      } else {
        expect(bytes.toString()).toContain('CueMol POV-Ray scene');
        expect(bytes.toString()).toContain('sphere {');
        expect(bytes.toString()).not.toContain('scene.inc');
      }
      const after = await page.evaluate(() => window.cuemol.status());
      expect(after.view).toEqual(current.view);
      expect(after.scene).toEqual(current.scene);
    }
    const failed = await call(page, 'export_scene', {path:'invalid.xyz',format:'xyz'});
    expect(failed.message).toContain('unsupported scene export format');
    const undone = await call(page, 'undo');
    expect(undone.scene.objects[0].name).toBe(before.scene.objects[0].name);
    expect(undone.view).toEqual(current.view);
    expect((await call(page, 'redo')).scene.objects[0].name).toBe('exported object');
  });
}
