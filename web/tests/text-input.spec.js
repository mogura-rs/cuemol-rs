import {test, expect} from '@playwright/test';
import {PNG} from 'pngjs';
import {load} from './support.js';

const molecule = 'HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C  \nEND\n';

async function clickRect(page, name) {
  await expect.poll(() => page.evaluate(name => window.cuemol.status().ui.rects[name]?.length, name)).toBe(4);
  const rect = await page.evaluate(name => window.cuemol.status().ui.rects[name], name);
  await page.mouse.click(rect[0] + rect[2] / 2, rect[1] + rect[3] / 2);
}

async function replaceName(page, name) {
  const field = await page.evaluate(() => window.cuemol.status().ui.inspector.name);
  await page.mouse.click(field[0] + field[2] / 2, field[1] + field[3] / 2);
  await expect(page.locator('input[type="text"]')).toBeFocused();
  await page.keyboard.press('ControlOrMeta+a');
  await present(page);
  await page.keyboard.insertText(name);
  await present(page);
  await page.keyboard.press('Tab');
  await expect.poll(() => page.evaluate(() => window.cuemol.status().scene.objects[0].name)).toBe(name);
  return page.screenshot({clip:{x:field[0], y:field[1], width:field[2], height:field[3]}});
}

async function present(page) {
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
}

async function editObjectName(page) {
  await page.goto('./?renderer=webgl');
  await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.scene?.sceneId), {timeout:60000}).toBeGreaterThan(0);
  await load(page, 'input.pdb', Buffer.from(molecule));
  await expect.poll(() => page.evaluate(() => window.cuemol.status().scene.objects.length)).toBe(1);
  const object = await page.evaluate(() => window.cuemol.status().scene.objects[0]);
  const row = await page.evaluate(id => window.cuemol.status().ui.rects[`object:${id}`], object.id);
  await page.mouse.click(row[0] + row[2] / 2, row[1] + row[3] / 2);
  await expect.poll(() => page.evaluate(() => window.cuemol.status().ui.inspector.name?.length)).toBe(4);
  const field = await page.evaluate(() => window.cuemol.status().ui.inspector.name);
  await page.mouse.click(field[0] + field[2] / 2, field[1] + field[3] / 2);
  await expect(page.locator('input[type="text"]')).toBeFocused();
  await page.keyboard.press('ControlOrMeta+a');
  return object;
}

test('browser text input commits Unicode without physical key events', async ({page}, testInfo) => {
  await editObjectName(page);
  const name = '分子 α-chain 日本語';
  await page.keyboard.insertText(name);
  await present(page);
  await page.keyboard.press('Tab');
  await expect.poll(() => page.evaluate(() => window.cuemol.status().scene.objects[0].name)).toBe(name);
  await page.screenshot({path:testInfo.outputPath('unicode-input.png')});
});

test('Japanese input renders glyphs instead of missing-character boxes', async ({page}, testInfo) => {
  await editObjectName(page);
  const boxes = PNG.sync.read(await replaceName(page, '□□ α-chain □□□'));
  const glyphs = PNG.sync.read(await replaceName(page, '分子 α-chain 日本語'));
  expect([glyphs.width, glyphs.height]).toEqual([boxes.width, boxes.height]);
  let changed = 0;
  for (let i = 0; i < glyphs.data.length; i += 4) {
    if (Math.abs(glyphs.data[i] - boxes.data[i]) + Math.abs(glyphs.data[i+1] - boxes.data[i+1])
        + Math.abs(glyphs.data[i+2] - boxes.data[i+2]) > 30) changed++;
  }
  expect(changed).toBeGreaterThan(100);
  await page.screenshot({path:testInfo.outputPath('japanese-glyphs.png')});
});

test('browser IME composition replaces preedit text with the committed name', async ({page, browserName}) => {
  test.skip(browserName !== 'chromium', 'Chromium exposes the browser IME protocol.');
  await editObjectName(page);
  const session = await page.context().newCDPSession(page);
  await session.send('Input.imeSetComposition', {text:'にほん', selectionStart:3, selectionEnd:3});
  await present(page);
  await session.send('Input.insertText', {text:'日本語'});
  await present(page);
  await page.keyboard.press('Tab');
  await expect.poll(() => page.evaluate(() => window.cuemol.status().scene.objects[0].name)).toBe('日本語');
});

test('Console accepts Unicode text and runs the entered script', async ({page}) => {
  const object = await editObjectName(page);
  await clickRect(page, 'Console');
  await clickRect(page, 'Console input');
  await expect(page.locator('input[type="text"]')).toBeFocused();
  await page.keyboard.insertText(`cuemol.getObject(${object.id}).name = "実行した分子";`);
  await present(page);
  await clickRect(page, 'Console Run');
  await expect.poll(() => page.evaluate(() => window.cuemol.status().scene.objects[0].name)).toBe('実行した分子');
  await clickRect(page, 'Undo');
  await expect.poll(() => page.evaluate(() => window.cuemol.status().scene.objects[0].name)).toBe(object.name);
});
