import {test, expect} from '@playwright/test';
import {call} from './support.js';

const status = page => page.evaluate(() => window.cuemol.status());
const present = page => page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
async function command(page, name, args = {}) {
  const id = await page.evaluate(([name,args]) => window.cuemol.command(name,args),[name,args]);
  await expect.poll(async () => {const state=await status(page);return state.lastCommand===id&&!state.busy;}).toBe(true);
  const message=(await status(page)).message;
  try {return JSON.parse(message);} catch(cause) {throw new Error(`${name}: ${message}`,{cause});}
}
async function click(page, key, options={}) {
  await expect.poll(async () => (await status(page)).ui.rects[key],{message:`Missing control: ${key}`}).toBeTruthy();
  await present(page);
  const [x,y,w,h]=(await status(page)).ui.rects[key];
  await page.mouse.move(x+w/2,y+h/2);
  await present(page);
  await page.mouse.click(x+w/2,y+h/2,options);
  await present(page);
}
async function fill(page, key, text) {
  await click(page,key);
  await page.keyboard.press('ControlOrMeta+A');
  await page.keyboard.insertText(text);
  await present(page);
}
async function ready(page, backend) {
  await page.goto(`./?renderer=${backend}`);
  await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.scene?.sceneId),{timeout:60000}).toBeGreaterThan(0);
  expect(await page.evaluate(() => window.cuemol.backend)).toBe(backend);
}
async function xmlDownload(download) {
  const chunks=[];
  for await(const chunk of await download.createReadStream())chunks.push(chunk);
  return Buffer.concat(chunks).toString('utf8');
}

for(const backend of ['webgl','webgpu']) {
  test(`style editor keeps live color references in ${backend}`, async ({page}) => {
    await ready(page,backend);
    await command(page,'create_style_set',{name:'aliases'});
    await command(page,'define_color',{styleSet:'aliases',name:'accent',color:'#ff0000'});
    await click(page,'Style scene aliases',{clickCount:2});
    await fill(page,'Style new name','copy');
    await fill(page,'Style new value','accent{alpha:0.25;mod_b:-0.5}');
    await click(page,'Style add entry');
    await expect.poll(async () => (await command(page,'get_style_set',{styleSet:'aliases'})).xml).toContain('accent{alpha:0.25;mod_b:-0.5}');
    await click(page,'Style tab Selection');
    await fill(page,'Style new name','focus');
    await fill(page,'Style new value','name C1');
    await click(page,'Style add entry');
    await expect.poll(async () => (await command(page,'get_style_set',{styleSet:'aliases'})).xml).toContain('name C1');
    await fill(page,'Style name','renamed');
    await click(page,'Style Rename');
    await expect.poll(async () => (await status(page)).ui.rects['Style scene renamed']).toBeTruthy();
    await command(page,'define_color',{styleSet:'renamed',name:'accent',color:'#00ff00'});
    const color=await command(page,'execute_script',{source:"var c=cuemol.getService('StyleManager').getColor('copy',scene.uid); print(c.toString()); print([c.r(),c.g(),c.b(),c.a()].join(','));"});
    expect(color.log).toBe('accent{alpha:0.25;mod_b:-0.5}\n0,128,0,64\n');
    await click(page,'Style tab Color');
    await expect.poll(async () => (await status(page)).ui.rects['Style value 0 copy']).toBeTruthy();
    await click(page,'Style close');
    const xml=(await command(page,'get_style_set',{styleSet:'renamed'})).xml;
    expect(xml).toContain('accent{alpha:0.25;mod_b:-0.5}');
    expect(xml).toContain('name C1');
  });

  test(`imported named palette colors remain fixed in ${backend}`, async ({page}) => {
    await ready(page,backend);
    await command(page,'create_style_set',{name:'source'});
    await command(page,'define_color',{styleSet:'source',name:'accent',color:'#00ff00'});
    const xml='<styles id="imported"><color id="copy" value="accent"/><color id="shifted" value="red" mod_h="120" set_a="0.25"/></styles>';
    await page.locator('#styles').setInputFiles({name:'palette.xml',mimeType:'application/xml',buffer:Buffer.from(xml)});
    await expect.poll(async () => (await status(page)).ui.rects['Style readonly']).toBeTruthy();
    await expect.poll(async () => (await status(page)).loading).toBe(false);
    const imported=await command(page,'get_style_set',{styleSet:'imported'});
    expect(imported.xml).not.toContain('value="accent"');
    expect(imported.xml).not.toContain('mod_h=');
    expect(imported.xml).toContain('#00FF00');
    await command(page,'define_color',{styleSet:'source',name:'accent',color:'#ff0000'});
    const colors=await command(page,'execute_script',{source:"var palette=cuemol.getService('StyleManager'); var copy=palette.getColor('copy',scene.uid), shifted=palette.getColor('shifted',scene.uid); print([copy.r(),copy.g(),copy.b(),copy.a()].join(',')); print([shifted.r(),shifted.g(),shifted.b(),shifted.a()].join(','));"});
    expect(colors.log).toBe('0,255,0,255\n0,255,0,64\n');
    expect((await command(page,'get_style_set',{styleSet:'imported'})).xml).toBe(imported.xml);
  });

  test(`global style edits survive scene history in ${backend}`,async ({page}) => {
    test.setTimeout(180000);
    await ready(page,backend);
    await call(page,'clear_undo');
    await click(page,'Style global user',{clickCount:2});
    await click(page,'Style tab Selection');
    await fill(page,'Style new name','focus');
    await fill(page,'Style new value','name C1');
    await click(page,'Style add entry');
    await expect.poll(async () => (await command(page,'get_style_set',{styleSet:'user',scope:'global'})).xml).toContain('name C1');
    await click(page,'Style close');
    await call(page,'undo');
    expect((await command(page,'get_style_set',{styleSet:'user',scope:'global'})).xml).toContain('name C1');
    await command(page,'create_style_set',{name:'local-palette'});
    await command(page,'define_selection',{styleSet:'user',scope:'global',name:'focus',selection:'name O1'});
    await call(page,'undo');
    const exists=() => command(page,'execute_script',{source:"print(cuemol.getService('StyleManager').hasStyleSet('local-palette',scene.uid));"});
    expect((await exists()).log).toBe('0\n');
    expect((await command(page,'get_style_set',{styleSet:'user',scope:'global'})).xml).toContain('name O1');
    await call(page,'redo');
    expect(Number((await exists()).log)).toBeGreaterThan(0);
    expect((await command(page,'get_style_set',{styleSet:'user',scope:'global'})).xml).toContain('name O1');
  });

  test(`style file editor preserves definitions and camera in ${backend}`,async ({page},testInfo) => {
    test.setTimeout(180000);
    await ready(page,backend);
    await command(page,'set_view',{center:[1,2,3],zoom:42,distance:100,slab:50});
    const view=(await status(page)).view;
    const xml='<styles id="palette"><color id="accent" value="#1234ee"/><sel id="focus" value="name C1"/><style id="KeepCPK" type="renderer"><sphr>0.7</sphr></style></styles>';
    await page.locator('#styles').setInputFiles({name:'palette.xml',mimeType:'application/xml',buffer:Buffer.from(xml)});
    await expect.poll(async () => (await status(page)).ui.rects['Style readonly']).toBeTruthy();
    await expect.poll(async () => (await status(page)).loading).toBe(false);
    expect((await status(page)).view).toEqual(view);
    expect(await command(page,'get_style_set',{styleSet:'palette'})).toMatchObject({readonly:true,modified:false,source:''});
    await click(page,'Style readonly');
    await expect.poll(async () => (await command(page,'get_style_set',{styleSet:'palette'})).readonly).toBe(false);
    await fill(page,'Style value 0 accent','#22cc44');
    await command(page,'create_style_set',{name:'inserted'});
    await click(page,'Style apply 0 accent');
    await expect.poll(async () => (await command(page,'get_style_set',{styleSet:'palette'})).xml).toMatch(/#22CC44/i);
    expect((await command(page,'get_style_set',{styleSet:'inserted'})).xml).not.toContain('accent');
    await command(page,'define_color',{styleSet:'palette',name:'accent',color:'#abcdef'});
    await click(page,'Style apply 0 accent');
    await expect.poll(async () => (await command(page,'get_style_set',{styleSet:'palette'})).xml).toContain('#ABCDEF');
    await click(page,'Style tab Selection');
    await fill(page,'Style value 1 focus','name O1');
    await command(page,'set_style_readonly',{styleSet:'inserted',readonly:true});
    await click(page,'Style apply 1 focus');
    await expect.poll(async () => (await command(page,'get_style_set',{styleSet:'palette'})).xml).toContain('name O1');
    await fill(page,'Style name','renamed');
    await command(page,'set_style_readonly',{styleSet:'inserted',readonly:false});
    await click(page,'Style Rename');
    await expect.poll(async () => (await status(page)).ui.rects['Style scene renamed']).toBeTruthy();
    const saved=page.waitForEvent('download');
    await click(page,'Style save');
    const download=await saved;
    expect(download.suggestedFilename()).toBe('renamed.xml');
    const savedXml=await xmlDownload(download);
    expect(savedXml).toContain('id="renamed"');
    expect(savedXml).toContain('name O1');
    expect(savedXml).toContain('KeepCPK');
    expect(savedXml).not.toMatch(/readonly=|modified=|src=/);
    await page.screenshot({path:testInfo.outputPath('style-editor-selection.png')});
    await click(page,'Style tab Styles');
    await click(page,'Style delete 2 KeepCPK');
    await expect.poll(async () => (await command(page,'get_style_set',{styleSet:'renamed'})).xml).not.toContain('KeepCPK');
    await click(page,'Style close');
    await expect.poll(async () => (await status(page)).ui.rects['Style name']).toBeUndefined();
    const scope=(await status(page)).scene.sceneId;
    const script=await command(page,'execute_script',{source:`print(cuemol.getService('StyleManager').getStrData('sel','focus',${scope}));`});
    expect(script.log).toBe('name O1\n');
    const bytes=Buffer.from(savedXml.replace('id="renamed"','id="reimported"'));
    await page.locator('#styles').setInputFiles({name:'saved.xml',mimeType:'application/xml',buffer:bytes});
    await expect.poll(async () => (await status(page)).ui.rects['Style scene reimported']).toBeTruthy();
    expect((await command(page,'get_style_set',{styleSet:'reimported'})).xml).toContain('name O1');
  });

  test(`global style editor keeps its scope through local shadowing in ${backend}`,async ({page},testInfo) => {
    test.setTimeout(180000);
    await ready(page,backend);
    expect(await command(page,'get_style_set',{styleSet:'system',scope:'global'})).toMatchObject({source:'',readonly:true,scope:0});
    await click(page,'Style global user',{clickCount:2});
    await fill(page,'Style new name','accent');
    await fill(page,'Style new value','#123456');
    await click(page,'Style add entry');
    await expect.poll(async () => (await command(page,'get_style_set',{styleSet:'user',scope:'global'})).xml).toContain('accent');
    await click(page,'Style tab Selection');
    await fill(page,'Style new name','focus');
    await fill(page,'Style new value','name C1');
    await click(page,'Style add entry');
    await expect.poll(async () => (await command(page,'get_style_set',{styleSet:'user',scope:'global'})).xml).toContain('name C1');
    await command(page,'create_style_set',{name:'user'});
    await fill(page,'Style value 1 focus','name O1');
    await click(page,'Style apply 1 focus');
    await expect.poll(async () => (await command(page,'get_style_set',{styleSet:'user',scope:'global'})).xml).toContain('name O1');
    expect((await command(page,'get_style_set',{styleSet:'user',scope:'scene'})).xml).not.toContain('focus');
    await fill(page,'Style name','global-renamed');
    await click(page,'Style Rename');
    await expect.poll(async () => (await status(page)).ui.rects['Style global global-renamed']).toBeTruthy();
    await page.screenshot({path:testInfo.outputPath('style-editor-global.png')});
    await click(page,'Style delete 1 focus');
    await expect.poll(async () => (await command(page,'get_style_set',{styleSet:'global-renamed',scope:'global'})).xml).not.toContain('focus');
    await click(page,'Style close');
    await command(page,'define_selection',{styleSet:'user',scope:'scene',name:'focus',selection:'name C1'});
    expect((await command(page,'get_style_set',{styleSet:'user',scope:'scene'})).xml).toContain('name C1');
    expect((await command(page,'get_style_set',{styleSet:'global-renamed',scope:'global'})).xml).not.toContain('focus');
    const anonymous = Number((await command(page,'execute_script',{source:"print(cuemol.getService('StyleManager').createStyleSet('',0));"})).log);
    await click(page,'Style global (anonymous)',{clickCount:2});
    await command(page,'execute_script',{source:"cuemol.getService('StyleManager').createStyleSet('(anonymous)',0);"});
    await click(page,'Style readonly');
    await expect.poll(async () => (await command(page,'get_style_set',{styleSet:'',styleId:anonymous,scope:'global'})).readonly).toBe(true);
    await click(page,'Style close');
    await click(page,`Style global #${anonymous}`,{button:'right'});
    await click(page,'Style context readonly');
    await expect.poll(async () => (await command(page,'get_style_set',{styleSet:'',styleId:anonymous,scope:'global'})).readonly).toBe(false);
    expect((await command(page,'get_style_set',{styleSet:'(anonymous)',scope:'global'})).readonly).toBe(false);
    await expect.poll(async () => (await status(page)).ui.rects['Style context readonly']).toBeUndefined();
    await click(page,`Style global #${anonymous}`,{button:'right'});
    await click(page,'Edit style');
    await click(page,'Style tab Selection');
    await fill(page,'Style new name','unique');
    await fill(page,'Style new value','name C1');
    await click(page,'Style add entry');
    await expect.poll(async () => (await command(page,'get_style_set',{styleSet:'',styleId:anonymous,scope:'global'})).xml).toContain('unique');
    expect((await command(page,'get_style_set',{styleSet:'',styleId:anonymous,scope:'global'})).name).toBe('');
    expect((await command(page,'get_style_set',{styleSet:'(anonymous)',scope:'global'})).xml).not.toContain('unique');
    await click(page,'Style close');
    await click(page,`Style global #${anonymous}`,{button:'right'});
    await click(page,'Style context delete');
    await expect.poll(async () => (await command(page,'execute_script',{source:"print(cuemol.getService('StyleManager').hasStyleSet('',0));"})).log).toBe('0\n');
    expect((await command(page,'get_style_set',{styleSet:'(anonymous)',scope:'global'})).name).toBe('(anonymous)');
  });
}
