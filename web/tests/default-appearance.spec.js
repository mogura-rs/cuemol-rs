import {test, expect} from '@playwright/test';
import {PNG} from 'pngjs';
import {load, call} from './support.js';

const protein = `ATOM      1  CA  ALA A   1       0.000   0.000   0.000  1.00 10.00           C
ATOM      2  CA  ALA A   2       3.800   0.000   0.000  1.00 10.00           N
ATOM      3  CA  ALA A   3       7.600   0.000   0.000  1.00 10.00           C
END
`;
const nucleic = `ATOM      1  P     A B   1       0.000   0.000   0.000  1.00 10.00           P
ATOM      2  P     A B   2       6.000   0.000   0.000  1.00 10.00           P
ATOM      3  P     A B   3      12.000   0.000   0.000  1.00 10.00           P
END
`;

for (const backend of ['webgl', 'webgpu']) {
  test(`default molecular colors and styles match CueMol on ${backend}`, async ({page}, testInfo) => {
    await page.goto(`./?renderer=${backend}`);
    await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.scene?.sceneId), {timeout:60000}).toBeGreaterThan(0);
    expect(await page.evaluate(() => window.cuemol.backend)).toBe(backend);
    await load(page, 'protein.pdb', Buffer.from(protein));
    await expect.poll(() => page.evaluate(() => window.cuemol.status().scene.objects.length)).toBe(1);
    let state = await page.evaluate(() => window.cuemol.status());
    const object = state.scene.objects[0];
    expect(object.renderers.some(renderer => renderer.type === 'ribbon')).toBe(true);
    state = await call(page, 'execute_script', {source:`
      var object = cuemol.getObject(${object.id});
      var paint = object.coloring;
      var entries = [];
      for (var i = 0; i < paint.size; i++) {
        var color = paint.getColorAt(i);
        entries.push([paint.getSelAt(i).toString(), color.r(), color.g(), color.b()]);
      }
      print(JSON.stringify(entries));
      var ribbon = object.getRendererByType('ribbon');
      print(JSON.stringify([ribbon.helix.tuber, ribbon.helix.width, ribbon.sheet.tuber, ribbon.sheet.width, ribbon.coil.width]));
      ribbon.visible = false;
    `});
    const output = JSON.parse(state.message).log.trim().split('\n').map(line => JSON.parse(line));
    expect(output[0]).toEqual([['sheet',70,130,180], ['helix',240,230,140], ['nucleic',255,255,0], ['*',255,250,240]]);
    expect(output[1]).toEqual([6,0.2,7,0.2,0.35]);
    state = await call(page, 'create_renderer', {objId:object.id, rendererType:'tracestick'});
    const trace = state.scene.objects[0].renderers.find(renderer => renderer.type === 'tracestick');
    state = await call(page, 'execute_script', {source:`
      var trace = cuemol.getRenderer(${trace.id});
      print(JSON.stringify([trace.style, trace.sphr, trace.bondw]));
    `});
    expect(JSON.parse(JSON.parse(state.message).log.trim())).toEqual(['DefaultTraceStick,DefaultHSCPaint',0.25,0.25]);
    const viewport = state.ui.rects.Viewport;
    const clip = {x:viewport[0]+viewport[2]/4, y:viewport[1]+viewport[3]/4, width:viewport[2]/2, height:viewport[3]/2};
    await expect.poll(async () => {
      const image = PNG.sync.read(await page.screenshot({clip}));
      let neutral = 0;
      for (let i = 0; i < image.data.length; i += 4) {
        const [r,g,b] = image.data.subarray(i,i+3);
        if (r > 80 && g > r*0.9 && b > r*0.9) neutral++;
      }
      return neutral;
    }).toBeGreaterThan(100);
    await page.screenshot({path:testInfo.outputPath(`default-tracestick-${backend}.png`)});
    await load(page, 'nucleic.pdb', Buffer.from(nucleic));
    await expect.poll(() => page.evaluate(() => window.cuemol.status().scene.objects.length)).toBe(2);
    state = await page.evaluate(() => window.cuemol.status());
    const bases = state.scene.objects[1].renderers.find(renderer => renderer.type === 'nucl');
    expect(bases).toBeDefined();
    state = await call(page, 'execute_script', {source:`
      var bases = cuemol.getRenderer(${bases.id});
      print(JSON.stringify([bases.section.type, bases.section.width, bases.section.tuber, bases.base_size, bases.base_thick]));
    `});
    expect(JSON.parse(JSON.parse(state.message).log.trim())).toEqual(['elliptical',0.5,2.5,0.5,0.5]);
  });
}
