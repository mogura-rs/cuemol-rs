import {test, expect} from '@playwright/test';
import {PNG} from 'pngjs';
import {load, call} from './support.js';

for (const backend of ['webgl', 'webgpu']) {
  test(`line ending variants preserve label pixels on ${backend}`, async ({page}) => {
    await page.addInitScript(() => {
      const contexts = new WeakSet();
      window.multilineImages = [];
      const prototype = Object.getPrototypeOf(new OffscreenCanvas(1, 1).getContext('2d'));
      const fill = prototype.fillText, read = prototype.getImageData;
      prototype.fillText = function (text, ...args) {
        if (text.startsWith('Top')) contexts.add(this);
        return fill.call(this, text, ...args);
      };
      prototype.getImageData = function (...args) {
        const image = read.apply(this, args);
        if (contexts.has(this)) window.multilineImages.push({width: image.width, height: image.height,
          alpha: Array.from(image.data.filter((_, i) => i % 4 === 3))});
        return image;
      };
    });
    await page.goto(`./?renderer=${backend}`);
    await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.scene?.sceneId), {timeout: 60000}).toBeGreaterThan(0);
    await load(page, 'lines.pdb', Buffer.from('HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n'));
    await expect.poll(() => page.evaluate(() => window.cuemol.status().message)).toBe('Loaded lines.pdb');
    const object = (await page.evaluate(() => window.cuemol.status())).scene.objects[0];
    const state = await call(page, 'execute_script', {source: `var r=cuemol.getObject(${object.id}).createRenderer('*namelabel');r.name='lines';r.font_size=24;r.font_name='serif';`});
    const renderer = state.scene.objects[0].renderers.find(r => r.name === 'lines').id;
    for (const variants of [
      ['Top\nBottom', 'Top\rBottom', 'Top\r\nBottom', 'Top\n\rBottom'],
      ['Top\n\nBottom', 'Top\r\n\rBottom', 'Top\n\r\nBottom'],
    ]) {
      let expected;
      for (const text of variants) {
        await page.evaluate(() => { window.multilineImages = []; });
        await call(page, 'execute_script', {source: `cuemol.getRenderer(${renderer}).setLabel(1,${JSON.stringify(text)});`});
        await expect.poll(() => page.evaluate(() => window.multilineImages.length)).toBeGreaterThan(0);
        const actual = await page.evaluate(() => window.multilineImages.at(-1));
        expect(actual.alpha.filter(value => value > 0).length).toBeGreaterThan(100);
        if (expected) {
          expect([actual.width, actual.height], JSON.stringify(text)).toEqual([expected.width, expected.height]);
          expect(actual.alpha, JSON.stringify(text)).toEqual(expected.alpha);
        } else expected = actual;
      }
    }
  });

  test(`font families and styles use reference Canvas glyphs on ${backend}`, async ({page}, testInfo) => {
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.addInitScript(() => {
      const labels = new WeakMap();
      window.labelImages = [];
      const prototypes = new Set([CanvasRenderingContext2D.prototype,
        Object.getPrototypeOf(new OffscreenCanvas(1, 1).getContext('2d'))]);
      for (const prototype of prototypes) {
        const fill = prototype.fillText, read = prototype.getImageData;
        prototype.fillText = function (text, ...args) {
          if (text === 'Glyph Ω') labels.set(this, text);
          return fill.call(this, text, ...args);
        };
        prototype.getImageData = function (...args) {
          const image = read.apply(this, args);
          if (labels.has(this)) {
            window.labelImages.push({text: labels.get(this), font: this.font, width: image.width,
              height: image.height, alpha: Array.from(image.data.filter((_, i) => i % 4 === 3))});
          }
          return image;
        };
      }
    });
    await page.goto(`./?renderer=${backend}`);
    await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.scene?.sceneId), {timeout: 60000}).toBeGreaterThan(0);
    await load(page, 'font.pdb', Buffer.from('HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n'));
    await expect.poll(() => page.evaluate(() => window.cuemol.status().message)).toBe('Loaded font.pdb');
    const object = (await page.evaluate(() => window.cuemol.status())).scene.objects[0];
    const hide = object.renderers.map(r => `cuemol.getRenderer(${r.id}).visible=false;`).join('');
    let state = await call(page, 'execute_script', {source: hide + `var r=cuemol.getObject(${object.id}).createRenderer('*namelabel');r.name='fonts';r.font_size=32;r.font_name='serif';r.halign='left';r.valign='top';r.dispx=30;r.dispy=30;r.color=cuemol.makeColor('#ff00ff');r.setLabel(1,'Glyph Ω');r.label_on_top=true;`});
    const renderer = state.scene.objects[0].renderers.find(r => r.name === 'fonts').id;
    async function glyph(family, style, weight) {
      await page.evaluate(() => { window.labelImages = []; });
      await call(page, 'execute_script', {source: `var r=cuemol.getRenderer(${renderer});r.font_name=${JSON.stringify(family)};r.font_style=${JSON.stringify(style)};r.font_weight=${JSON.stringify(weight)};`});
      await expect.poll(() => page.evaluate(() => window.labelImages.length)).toBeGreaterThan(0);
      const result = await page.evaluate(() => {
        const actual = window.labelImages.at(-1);
        const measure = new OffscreenCanvas(1, 1).getContext('2d');
        measure.font = actual.font;
        const width = Math.ceil(Math.ceil(measure.measureText(actual.text).width) / 4) * 4;
        const canvas = new OffscreenCanvas(width, 32);
        const context = canvas.getContext('2d');
        context.font = actual.font;
        context.textBaseline = 'bottom';
        context.fillStyle = 'white';
        context.fillText(actual.text, 0, 32);
        const expected = context.getImageData(0, 0, width, 32).data.filter((_, i) => i % 4 === 3);
        return {actual, width, mismatch: actual.alpha.filter((value, i) => value !== expected[i]).length};
      });
      expect(result.actual.width).toBe(result.width);
      expect(result.actual.height).toBe(32);
      expect(result.mismatch).toBe(0);
      expect(result.actual.alpha.filter(value => value > 0).length).toBeGreaterThan(100);
      return result.actual;
    }
    const mono = await glyph('monospace', 'normal', 'normal');
    const serif = await glyph('serif', 'italic', 'bold');
    expect(mono.alpha).not.toEqual(serif.alpha);
    const upper = await glyph('serif', 'ITALIC', 'BOLD');
    expect(upper.alpha).toEqual(serif.alpha);
    const named = await glyph('"Times New Roman", serif', 'normal', 'normal');
    expect(named.alpha).not.toEqual(serif.alpha);
    const rect = (await page.evaluate(() => window.cuemol.status())).ui.rects.Viewport;
    await expect.poll(async () => {
      const image = PNG.sync.read(await page.screenshot({clip: {x: rect[0], y: rect[1], width: rect[2], height: rect[3]}}));
      let count = 0;
      for (let i = 0; i < image.data.length; i += 4) {
        if (image.data[i] > 150 && image.data[i + 1] < 60 && image.data[i + 2] > 150) count++;
      }
      return count;
    }).toBeGreaterThan(100);
    await page.screenshot({path: testInfo.outputPath(`fonts-${backend}.png`)});
    expect(errors).toEqual([]);
  });
}
