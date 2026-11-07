import {expect} from '@playwright/test';
export async function load(page,name,buffer){
  await page.locator('#files').setInputFiles({name,mimeType:'application/octet-stream',buffer});
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
