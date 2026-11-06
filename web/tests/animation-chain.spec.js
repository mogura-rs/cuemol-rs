import {test, expect} from '@playwright/test';
import {call, load} from './support.js';

for (const backend of ['webgl', 'webgpu']) {
  test(`animation reference edits ${backend} preserve times history and saved chains`, async ({page}) => {
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.goto(`./?renderer=${backend}`);
    await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.scene?.sceneId), {timeout:60000}).toBeGreaterThan(0);
    const timeline = async () => JSON.parse((await call(page, 'list_anims')).message);
    for (const name of ['base', 'child', 'last']) await call(page, 'add_anim', {type:'wait', name});
    await call(page, 'clear_undo');
    const before = await timeline();
    await call(page, 'set_anim_prop', {element:'base', prop:'name', value:'renamed'});
    let current = await timeline();
    expect(current.elements[1].timeRefName).toBe('renamed');
    expect(current.elements.map(element => [element.uid, element.absStartMs, element.absEndMs])).toEqual(before.elements.map(element => [element.uid, element.absStartMs, element.absEndMs]));
    await call(page, 'undo');
    const failed = await call(page, 'set_anim_prop', {element:'base', prop:'timeRefName', value:'last'});
    expect(failed.message).toContain('cycle');
    await call(page, 'redo');
    expect((await timeline()).elements[0].name).toBe('renamed');
    await call(page, 'set_anim_prop', {element:'child', prop:'timeRefName', value:''});
    current = await timeline();
    expect([current.elements[1].startMs, current.elements[1].endMs]).toEqual([1000, 2000]);
    await call(page, 'set_anim_prop', {element:'child', prop:'timeRefName', value:'renamed'});
    await call(page, 'remove_anim', {element:'renamed'});
    current = await timeline();
    expect(current.elements.map(element => [element.name, element.absStartMs, element.absEndMs])).toEqual([['child',1000,2000], ['last',2000,3000]]);
    expect(current.elements[0].timeRefName).toBe('');
    const download = page.waitForEvent('download');
    await call(page, 'save_scene', {path:'animation.qsc', embedAll:true});
    const stream = await (await download).createReadStream();
    const chunks = [];
    for await (const chunk of stream) chunks.push(chunk);
    await load(page, 'animation.qsc', Buffer.concat(chunks));
    await expect.poll(() => page.evaluate(() => window.cuemol.status().message)).toBe('Loaded animation.qsc');
    current = await timeline();
    expect(current.elements.map(element => [element.name, element.timeRefName, element.absStartMs, element.absEndMs])).toEqual([['child','',1000,2000], ['last','child',2000,3000]]);
    expect(errors).toEqual([]);
  });
}

for (const backend of ['webgl', 'webgpu']) {
  test(`animation broken chains ${backend} load with diagnostics and can be repaired`, async ({page}, testInfo) => {
    await page.goto(`./?renderer=${backend}`);
    await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.scene?.sceneId), {timeout:60000}).toBeGreaterThan(0);
    const data='<scene><animation length="3000000000"><motion type="NoopAnimObj" name="base" end="1000000000"/><motion type="NoopAnimObj" name="child" timeRefName="missing" end="1000000000"/><motion type="NoopAnimObj" name="last" timeRefName="child" end="1000000000"/></animation></scene>';
    await load(page, 'broken.qsc', Buffer.from(data));
    await expect.poll(() => page.evaluate(() => window.cuemol.status().message)).toBe('Loaded broken.qsc');
    let timeline=JSON.parse((await call(page,'list_anims')).message);
    expect(timeline.resolveError).toContain('missing');
    expect(timeline.elements.map(element=>element.timeRefState)).toEqual(['ok','missing','upstream']);
    const [x,y,w,h]=await page.evaluate(()=>window.cuemol.status().ui.rects.Animation);
    await page.mouse.click(x+w/2,y+h/2);
    await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
    await page.screenshot({path:testInfo.outputPath('animation-broken.png')});
    const failed=await call(page,'animate',{action:'play'});
    expect(failed.message).toContain('missing');
    await call(page,'set_anim_prop',{element:'child',prop:'timeRefName',value:'base'});
    timeline=JSON.parse((await call(page,'list_anims')).message);
    expect(timeline.resolveError).toBeUndefined();
    expect(timeline.elements.map(element=>element.absEndMs)).toEqual([1000,2000,3000]);
    await call(page,'animate',{action:'play'});
    timeline=JSON.parse((await call(page,'list_anims')).message);
    expect(timeline.mgr.playState).toBe('playing');
    await call(page,'animate',{action:'stop'});
  });
}
