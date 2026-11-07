import {expect} from '@playwright/test';
export async function acceptFileOptions(page, {fixtureNames = false} = {}) {
  const frame = () => page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  await frame();
  const deadline = Date.now() + 60000;
  while (Date.now() < deadline) {
    const state = await page.evaluate(() => window.cuemol.status());
    let rect = state.ui.rects['File options Open'];
    if (rect) {
      if (fixtureNames && state.message.startsWith('Choose options for ')) {
        // Keep existing script targets explicit when loading named test fixtures.
        const base = state.message.slice('Choose options for '.length).replace(/\.(gz|bz2|xz)$/i, '');
        const names = new Set(state.scene.objects.map(object => object.name));
        let name = base;
        for (let suffix = 1; names.has(name); suffix++) name = `${base}(${suffix})`;
        await expect.poll(async () => {
          await frame();
          const field = await page.evaluate(() => window.cuemol.status().ui.rects['File object name']);
          if (!field) return false;
          await page.mouse.click(field[0] + field[2] / 2, field[1] + field[3] / 2);
          await frame();
          return page.evaluate(() => window.cuemol.status().ui.keyboardInput);
        }).toBe(true);
        await page.keyboard.press('ControlOrMeta+A');
        await page.keyboard.type(name);
        await frame();
      }
      rect = await page.evaluate(() => window.cuemol.status().ui.rects['File options Open']);
      if (rect) await page.mouse.click(rect[0] + rect[2] / 2, rect[1] + rect[3] / 2);
    } else if (!state.loading && !state.busy) {
      return;
    }
    await frame();
  }
  throw new Error('File loading did not finish after accepting its options');
}
export async function load(page,name,buffer,{accept = true} = {}) {
  await page.locator('#files').setInputFiles({name,mimeType:'application/octet-stream',buffer});
  if (accept) await acceptFileOptions(page, {fixtureNames:true});
}
export async function call(page,name,args={}){
  const id=await page.evaluate(([name,args])=>window.cuemol.command(name,args),[name,args]);
  await expect.poll(()=>page.evaluate(id=>{
    const state=window.cuemol.status();return state.lastCommand===id&&!state.busy;
  },id),{timeout:60000}).toBe(true);
  return page.evaluate(()=>window.cuemol.status());
}

export async function enablePlugins(page, ids) {
  await page.addInitScript(ids => {
    let config = localStorage.getItem('cuemol.config') || '# cuemol-config-v2';
    for (const id of ids) config += `\nplugins:${id}:enabled\ttrue`;
    localStorage.setItem('cuemol.config', config);
  }, ids);
}
