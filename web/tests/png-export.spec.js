import {readFile, copyFile} from 'node:fs/promises';
import {PNG} from 'pngjs';
import {test, expect} from '@playwright/test';
import {call, load} from './support.js';

for (const renderer of ['webgl', 'webgpu']) {
  test(`GPU PNG export on ${renderer} preserves scene and ignores the raytrace backend`, async ({page}, testInfo) => {
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.goto(`./?renderer=${renderer}`);
    await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.scene?.sceneId), {timeout:60000}).toBeGreaterThan(0);
    expect(await page.evaluate(() => window.cuemol.backend)).toBe(renderer);
    await load(page, 'atom.pdb', Buffer.from('HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n'));
    await call(page, 'set_view', {zoom:6,center:[0,0,0],slab:50,distance:100});
    const before = await call(page, 'set_render_settings', {settings:'backend=povray; width=800; height=600; dpi=600'});
    const exportImage = async (name, width=96, height=64) => {
      const pending = page.waitForEvent('download');
      const state = await call(page, 'export_scene', {path:name,format:'png',width,height,dpi:150,transparent:true});
      expect(state.message).toBe(`Saved ${name}`);
      const file = await pending;
      expect(file.suggestedFilename()).toBe(name);
      await copyFile(await file.path(), testInfo.outputPath(name));
      return readFile(await file.path());
    };
    const first = await exportImage('gpu.png');
    const image = PNG.sync.read(first);
    expect([image.width,image.height,image.alpha]).toEqual([96,64,true]);
    expect(image.data[3]).toBe(0);
    let visible = 0;
    for(let i=3;i<image.data.length;i+=4) if(image.data[i]>128) visible++;
    expect(visible).toBeGreaterThan(10);
    const phys = first.indexOf(Buffer.from('pHYs'));
    expect(phys).toBeGreaterThan(0);
    expect(first.readUInt32BE(phys+4)).toBe(5906);
    const after = await page.evaluate(() => window.cuemol.status());
    expect(after.scene).toEqual(before.scene);
    expect(after.view).toEqual(before.view);
    await call(page, 'set_render_settings', {settings:'backend=umbreon_npr'});
    expect(await exportImage('same.png')).toEqual(first);
    const wide = PNG.sync.read(await exportImage('wide.png', 513, 384));
    expect([wide.width,wide.height]).toEqual([513,384]);
    expect(wide.data[3]).toBe(0);
    expect(wide.data[(192*513+256)*4+3]).toBe(255);
    const failed = await call(page, 'export_scene', {path:'invalid.png',format:'png',width:0,height:64});
    expect(failed.message).toContain('dimensions must be positive');
    expect(failed.view).toEqual(before.view);
    expect(errors).toEqual([]);
  });
}
