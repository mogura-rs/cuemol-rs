import {test, expect} from '@playwright/test';
import {PNG} from 'pngjs';
import {load, call} from './support.js';

const protein = Array.from({length:6}, (_, residue) => [
  [' N  ', 'N', -1.3, 0], [' CA ', 'C', 0, 0], [' C  ', 'C', 1.2, 0], [' O  ', 'O', 1.2, 1.2],
].map(([name, element, dx, dy], atom) => {
  const serial = residue * 4 + atom + 1;
  const x = (residue * 3.8 - 9.5 + dx).toFixed(3).padStart(8);
  const y = (Math.sin(residue * 0.5) * 2 + dy).toFixed(3).padStart(8);
  return `ATOM  ${String(serial).padStart(5)} ${name} ALA A${String(residue + 1).padStart(4)}    ${x}${y}   0.000  1.00 10.00           ${element}`;
}).join('\n')).join('\n') + '\nEND\n';
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
        entries.push([paint.getSelAt(i).toString(), color.toString()]);
      }
      print(JSON.stringify(entries));
      var ribbon = object.getRendererByType('ribbon');
      print(JSON.stringify([ribbon.helix.tuber, ribbon.helix.width, ribbon.sheet.tuber, ribbon.sheet.width, ribbon.coil.width]));
      ribbon.visible = false;
    `});
    const output = JSON.parse(state.message).log.trim().split('\n').map(line => JSON.parse(line));
    expect(output[0]).toEqual([
      ['sheet','okabeito_1'], ['helix','okabeito_1'],
      ['nucleic','okabeito_1{mod_h:15}'], ['*','okabeito_1{mod_b:0.1;mod_s:-0.6}'],
    ]);
    expect(output[1]).toEqual([6.5,0.18,6.5,0.18,0.2]);
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
      let warm = 0;
      for (let i = 0; i < image.data.length; i += 4) {
        const [r,g,b] = image.data.subarray(i,i+3);
        if (r > 80 && g > r*0.6 && r > b*1.15) warm++;
      }
      return warm;
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
    expect(JSON.parse(JSON.parse(state.message).log.trim())).toEqual(['roundsquare',0.4,2.5,0.3,0.3]);
  });
}
