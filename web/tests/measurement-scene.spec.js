import {test, expect} from '@playwright/test';
import {PNG} from 'pngjs';

const pdb = `HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C
HETATM    2  C2  LIG A   1       1.000   0.000   0.000  1.00 10.00           C
HETATM    3  C3  LIG A   1       2.000   1.000   0.000  1.00 10.00           C
HETATM    4  C4  LIG A   1       3.000   2.000   1.000  1.00 10.00           C
HETATM    5  C5  LIG A   1      20.000  20.000  20.000  1.00 10.00           C
HETATM    6  C6  LIG A   1      30.000  30.000  30.000  1.00 10.00           C
END
`;
const cross = 'name C1 around 1.5 [Target]';
const stages = [
  [[0,0,0], [0.5,135,90]],
  [[1,0,0], [1/3,45,90]],
  [[2,1,1], [2.179449471770337,13.262676008304892,0]],
];
const status = page => page.evaluate(() => window.cuemol.status());
async function command(page, name, args = {}) {
  const id = await page.evaluate(([name,args]) => window.cuemol.command(name,args), [name,args]);
  await expect.poll(async () => {
    const state = await status(page);
    return state.lastCommand === id && !state.busy;
  }).toBe(true);
  const message = (await status(page)).message;
  try { return JSON.parse(message); } catch (cause) { throw new Error(`${name}: ${message}`, {cause}); }
}

for (const backend of ['webgl','webgpu']) {
  test(`scene measurements update ${backend} values, labels and pixels through edits and reload`, async ({page},testInfo) => {
    test.setTimeout(120000);
    await page.goto(`./?renderer=${backend}`);
    await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.scene?.sceneId), {timeout:60000}).toBeGreaterThan(0);
    expect(await page.evaluate(() => window.cuemol.backend)).toBe(backend);
    for (const [index,name] of ['Probe','Target'].entries()) {
      await page.locator('#files').setInputFiles({name:`${name}.pdb`,mimeType:'application/octet-stream',buffer:Buffer.from(pdb)});
      await expect.poll(async () => (await status(page)).scene.objects.length).toBe(index+1);
    }
    let [probe,target] = (await status(page)).scene.objects;
    await command(page,'rename_node',{nodeType:'object',nodeId:probe.id,name:'Probe'});
    await command(page,'rename_node',{nodeType:'object',nodeId:target.id,name:'Target'});
    await command(page,'set_visible',{nodeType:'object',nodeId:target.id,visible:false});
    for (const renderer of probe.renderers) await command(page,'delete_node',{nodeType:'renderer',nodeId:renderer.id});
    await command(page,'define_selection',{styleSet:'measurements',name:'focus',selection:cross});
    const created = await command(page,'execute_script',{source:`var r=cuemol.getObject(${probe.id}).createRenderer('atomintr','measure'); r.showlabel=true; r.mode='simple'; r.width=2; print(JSON.stringify([r.append('focus','name C2'),r.appendAngle('focus','name C2','name C3'),r.appendTorsion('focus','name C2','name C3','name C4')]));`});
    expect(created.log).toBe('[0,1,2]\n');
    let renderer = (await status(page)).scene.objects.find(object => object.id === probe.id).renderers.find(renderer => renderer.name === 'measure').id;
    await command(page,'set_projection',{perspective:false});
    await command(page,'set_view',{center:[1.5,1,0],zoom:12,distance:200,slab:50});
    await command(page,'set_visible',{nodeType:'renderer',nodeId:renderer,visible:false});
    const [x,y,w,h] = (await status(page)).ui.rects.Viewport;
    const clip = {x:Math.floor(x+w/2-160),y:Math.floor(y+h/2-110),width:320,height:220};
    const capture = async name => {
      await page.mouse.move(5,5);
      await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
      return PNG.sync.read(await page.screenshot({clip,path:testInfo.outputPath(name)})).data;
    };
    const empty = await capture('measurement-empty.png');
    await command(page,'set_visible',{nodeType:'renderer',nodeId:renderer,visible:true});
    const shift = xyz => command(page,'set_node_prop',{nodeType:'object',nodeId:target.id,prop:'xformMat',value:`(1,0,0,0,0,1,0,0,0,0,1,0,${xyz.join(',')},1)`});
    const check = async expected => {
      const result = await command(page,'execute_script',{source:`var r=cuemol.getObject(${probe.id}).getRendererByName('measure'); print(JSON.stringify([r.getValue(0),r.getValue(1),r.getValue(2)]));`});
      const values = JSON.parse(result.log);
      for (const [index,value] of values.entries()) expect(value).toBeCloseTo(expected[index],5);
      await expect.poll(async () => (await status(page)).ui.navigation.measurementLabels.map(Number)).toEqual(expected.map(value => Number(value.toFixed(2))));
    };
    let first;
    for (const [index,[xyz,expected]] of stages.entries()) {
      await shift(xyz);
      await check(expected);
      const pixels = await capture(`measurement-stage-${index}.png`);
      expect(pixels.equals(empty)).toBe(false);
      if (index === 0) first = pixels;
      else expect(pixels.equals(first)).toBe(false);
    }
    await shift([20,0,0]);
    await expect.poll(async () => (await status(page)).ui.navigation.measurementLabels).toEqual([]);
    expect((await capture('measurement-no-atoms.png')).equals(empty)).toBe(true);
    await shift([0,0,0]);
    await check(stages[0][1]);
    expect((await capture('measurement-restored.png')).equals(first)).toBe(true);
    await command(page,'remove_selection',{styleSet:'measurements',name:'focus'});
    await expect.poll(async () => (await status(page)).ui.navigation.measurementLabels).toEqual([]);
    expect((await capture('measurement-removed-definition.png')).equals(empty)).toBe(true);
    await command(page,'define_selection',{styleSet:'measurements',name:'focus',selection:cross});
    await check(stages[0][1]);
    const download = page.waitForEvent('download');
    const button = (await status(page)).ui.rects['Save Scene'];
    await page.mouse.click(button[0]+button[2]/2,button[1]+button[3]/2);
    const stream = await (await download).createReadStream(), chunks = [];
    for await (const chunk of stream) chunks.push(chunk);
    const bytes = Buffer.concat(chunks);
    expect(bytes.toString()).toContain('sel1="focus"');
    await command(page,'create_scene',{name:'reload'});
    await page.locator('#files').setInputFiles({name:'measurement.qsc',mimeType:'application/octet-stream',buffer:bytes});
    await expect.poll(async () => (await status(page)).message).toBe('Loaded measurement.qsc');
    const objects = (await status(page)).scene.objects;
    probe = objects.find(object => object.name === 'Probe');
    target = objects.find(object => object.name === 'Target');
    renderer = probe.renderers.find(renderer => renderer.name === 'measure').id;
    await shift(stages[1][0]);
    await check(stages[1][1]);
    await command(page,'set_node_prop',{nodeType:'renderer',nodeId:renderer,prop:'width',value:'0.1'});
    await command(page,'set_node_prop',{nodeType:'renderer',nodeId:renderer,prop:'mode',value:'fancy'});
    const fancy = await capture('measurement-fancy.png');
    expect(fancy.equals(empty)).toBe(false);
    await command(page,'remove_selection',{styleSet:'measurements',name:'focus'});
    await expect.poll(async () => (await status(page)).ui.navigation.measurementLabels).toEqual([]);
    expect((await capture('measurement-fancy-removed.png')).equals(empty)).toBe(true);
  });

  test(`fancy angle arrows in ${backend} end at the outer atoms`, async ({page},testInfo) => {
    await page.goto(`./?renderer=${backend}`);
    await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.scene?.sceneId), {timeout:60000}).toBeGreaterThan(0);
    expect(await page.evaluate(() => window.cuemol.backend)).toBe(backend);
    const arrows = `HETATM    1  C1  LIG A   1      -3.000  -2.000   0.000  1.00 10.00           C
HETATM    2  C2  LIG A   1       0.000   1.000   0.000  1.00 10.00           C
HETATM    3  C3  LIG A   1       4.000  -2.000   0.000  1.00 10.00           C
END
`;
    await page.locator('#files').setInputFiles({name:'arrows.pdb',mimeType:'application/octet-stream',buffer:Buffer.from(arrows)});
    await expect.poll(async () => (await status(page)).scene.objects.length).toBe(1);
    const object = (await status(page)).scene.objects[0];
    for (const renderer of object.renderers) await command(page,'delete_node',{nodeType:'renderer',nodeId:renderer.id});
    const result = await command(page,'execute_script',{source:`var r=cuemol.getObject(${object.id}).createRenderer('atomintr','arrows'); r.mode='fancy'; r.showlabel=false; r.stipple0=-1; r.width=0.1; r.captype_start='flat'; r.captype_end='arrow'; r.arrowheight=0.8; r.arrowwidth=3; print(r.appendAngle('name C1','name C2','name C3'));`});
    expect(result.log).toBe('0\n');
    await command(page,'set_projection',{perspective:false});
    await command(page,'set_view',{center:[0.5,-0.5,0],zoom:10,distance:200,slab:50});
    await page.mouse.move(5,5);
    await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    const [x,y,w,h] = (await status(page)).ui.rects.Viewport;
    const png = PNG.sync.read(await page.screenshot({path:testInfo.outputPath('angle-arrows.png')}));
    const ink = point => {
      const px = Math.round(x+w/2+(point[0]-0.5)*h/10);
      const py = Math.round(y+h/2-(point[1]+0.5)*h/10);
      let count = 0;
      for (let dy=-1;dy<=1;dy++) for (let dx=-1;dx<=1;dx++) {
        const offset = ((py+dy)*png.width+px+dx)*4;
        if (png.data[offset]+png.data[offset+1]+png.data[offset+2]>60) count++;
      }
      return count;
    };
    const unit = Math.SQRT1_2;
    const outerArrow = [-3+unit*0.6-unit*0.17,-2+unit*0.6+unit*0.17];
    const innerArrow = [-unit*0.6-unit*0.17,1-unit*0.6+unit*0.17];
    expect(ink(outerArrow)).toBeGreaterThan(4);
    expect(ink(innerArrow)).toBe(0);
  });
}
