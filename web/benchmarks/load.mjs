import {chromium, firefox, webkit} from '@playwright/test';
import {mkdir, writeFile} from 'node:fs/promises';
import {dirname} from 'node:path';
import {parseArgs} from 'node:util';
import {browserLaunchArgs} from '../tests/browser-options.js';

const {values} = parseArgs({options: {
  url: {type: 'string'}, output: {type: 'string'},
  browser: {type: 'string', default: 'chromium'}, channel: {type: 'string'},
  atoms: {type: 'string', default: '1000,5000,20000'},
  samples: {type: 'string', default: '3'},
}});
if (!values.url) throw new Error('--url is required');
const counts = values.atoms.split(',').map(Number);
const samples = Number(values.samples);
if (!Number.isInteger(samples) || samples < 1 || !counts.every(n => Number.isInteger(n) && n > 0 && n <= 99999)) {
  throw new Error('Use a positive sample count and 1..99999 atoms per PDB');
}
const engine = {chromium, firefox, webkit}[values.browser];
if (!engine) throw new Error('Unknown browser');
const browser = await engine.launch({
  channel: values.channel,
  args: values.browser === 'chromium' ? browserLaunchArgs : [],
});
const report = {url: values.url, browser: values.browser, version: browser.version(), samples, results: []};

function pdb(atoms) {
  const lines = [];
  for (let i = 0; i < atoms; i++) {
    const serial = String(i + 1).padStart(5), residue = String(i % 9999 + 1).padStart(4);
    const chain = String.fromCharCode(65 + Math.floor(i / 9999));
    const xyz = [i % 100 * 4, Math.floor(i / 100) % 100 * 4, Math.floor(i / 10000) * 4]
      .map(value => value.toFixed(3).padStart(8)).join('');
    lines.push(`HETATM${serial}  C1  LIG ${chain}${residue}    ${xyz}  1.00 10.00           C  `);
  }
  return Buffer.from(lines.join('\n') + '\nEND\n');
}

async function measure(page, operation) {
  await page.evaluate(() => {
    const probe = window.__loadProbe = {gaps: [], longtasks: [], last: performance.now()};
    probe.timer = setInterval(() => {
      const now = performance.now(); probe.gaps.push(now - probe.last); probe.last = now;
    }, 10);
    if (PerformanceObserver.supportedEntryTypes.includes('longtask')) {
      probe.observer = new PerformanceObserver(list => probe.longtasks.push(...list.getEntries().map(entry => entry.duration)));
      probe.observer.observe({entryTypes: ['longtask']});
    }
  });
  const memoryBefore = await runtimeMemory(page);
  const started = performance.now();
  const value = await operation();
  const elapsedMs = performance.now() - started;
  await page.waitForTimeout(60);
  const timing = await page.evaluate(() => {
    const probe = window.__loadProbe;
    clearInterval(probe.timer); probe.observer?.disconnect();
    return {maxTimerGapMs: Math.max(0, ...probe.gaps), longtasksMs: probe.longtasks};
  });
  const memoryAfter = await runtimeMemory(page);
  return {value, timing: {elapsedMs, ...timing, memoryBefore, memoryAfter}};
}

async function runtimeMemory(page) {
  return page.evaluate(async () => {
    const module = await import(new URL('./pkg/cuemol_web.js', location.href));
    return (await module.default()).memory.buffer.byteLength;
  });
}

async function load(page, name, buffer) {
  await page.locator('#files').setInputFiles({name, mimeType: 'application/octet-stream', buffer});
  await page.waitForFunction(name => window.cuemol.status().message === `Loaded ${name}`, name, {timeout: 180000});
}

try {
  for (const atoms of counts) {
    const input = pdb(atoms);
    for (let sample = 0; sample < samples; sample++) {
      const context = await browser.newContext({viewport: {width: 1280, height: 900}});
      try {
        const page = await context.newPage();
        const errors = [];
        page.on('pageerror', error => errors.push(error.message));
        await page.goto(values.url);
        await page.waitForFunction(() => window.cuemol?.status()?.scene?.sceneId > 0, {}, {timeout: 60000});
        if (!report.manifest) {
          report.manifest = await page.evaluate(async () => {
            const response = await fetch('./build-info.json');
            return response.ok ? response.json() : null;
          });
        }
        const loaded = await measure(page, () => load(page, 'large.pdb', input));
        const loadWorker = await page.evaluate(() => window.cuemolLoader?.last);
        let state = await page.evaluate(() => window.cuemol.status());
        if (state.scene.objects[0]?.atoms !== atoms) throw new Error('PDB atom count changed');
        const exported = await measure(page, async () => {
          const pending = page.waitForEvent('download', {timeout: 180000});
          const rect = state.ui.rects['Save Scene'];
          await page.mouse.click(rect[0] + rect[2] / 2, rect[1] + rect[3] / 2);
          const download = await pending;
          const chunks = [];
          for await (const chunk of await download.createReadStream()) chunks.push(chunk);
          return Buffer.concat(chunks);
        });
        const restored = await measure(page, () => load(page, 'restored.qsc', exported.value));
        const reloadWorker = await page.evaluate(() => window.cuemolLoader?.last);
        state = await page.evaluate(() => window.cuemol.status());
        if (state.scene.objects[0]?.atoms !== atoms) throw new Error('Scene export changed the atom count');
        if (errors.length) throw new Error(errors.join('\n'));
        const result = {atoms, sample, inputBytes: input.length, sceneBytes: exported.value.length,
          load: {...loaded.timing, worker: loadWorker}, save: exported.timing,
          reload: {...restored.timing, worker: reloadWorker}};
        report.results.push(result);
        console.log(JSON.stringify(result));
      } finally { await context.close(); }
    }
  }
} finally {
  await browser.close();
  if (values.output) {
    await mkdir(dirname(values.output), {recursive: true});
    await writeFile(values.output, JSON.stringify(report, null, 2) + '\n');
  }
}
