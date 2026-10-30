import {test, expect} from '@playwright/test';
import {readFile} from 'node:fs/promises';

const shellRoot = new URL('../../crates/cuemol-desktop/tauri-ui/', import.meta.url);

async function openShell(page, storage = {}, options = {}) {
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.route('http://tauri.test/**', async route => {
    const file = new URL(route.request().url()).pathname.slice(1) || 'index.html';
    const contentType = file.endsWith('.png') ? 'image/png' : file.endsWith('.js') ? 'text/javascript' : file.endsWith('.css') ? 'text/css' : 'text/html';
    await route.fulfill({body: await readFile(file === 'logo.png' ? new URL('../../cuemol-rs.png', import.meta.url) : new URL(file, shellRoot)), contentType});
  });
  await page.addInitScript(({storage, options}) => {
    if (!sessionStorage.getItem('seeded')) {
      for (const [key, value] of Object.entries(storage)) localStorage.setItem(key, value);
      sessionStorage.setItem('seeded', '1');
    }
    const scene = {uid: 1, name: 'Test scene', source: '', modified: false, active: true, objectCount: options.empty ? 0 : 1, mapCount: 0};
    const renderer = {
      uid: 4, name: 'Atoms', rendererType: 'cpk', group: null, uiCollapsed: false,
      selection: '*', visible: true, locked: false, style: '', material: '', alpha: 1,
      edgeType: 'none', edgeWidth: 0, edgeColor: '#000000', lineWidth: 1, detail: 4,
      supportsColoring: true, supportsSolidColoring: true, supportsMultiGradient: true,
      coloringType: 'CPKColoring', coloringAttrs: {}, paintEntries: [], gradientEntries: [],
      defaultColor: '#cccccc', specific: {},
    };
    const molecule = {
      uid: 3, sceneUid: 1, name: 'Probe', source: '', sourceType: 'pdb', uiOrder: 0,
      uiCollapsed: false, selection: '*', visible: true, locked: false,
      atomCount: 3, residueCount: 1, bondCount: 2, coloringType: 'CPKColoring',
      coloringAttrs: {}, paintEntries: [], defaultColor: '#cccccc', renderers: [renderer],
    };
    const snapshot = {
      scenes: [scene], views: [{uid: 2, sceneUid: 1, name: 'Main', active: true}],
      cameras: [], styles: [], objects: options.empty ? [] : [molecule], structureKey: 'initial', sequenceKey: 'initial',
      crystals: [], densityMaps: [], surfaces: [], lwObjects: [], activeSceneUid: 1,
      selection: '*', measureResult: null, perspective: false, background: 'black',
      showUnitCell: false, showSymmetryMates: false, showDensityContour: false, colorProofing: false,
      densityLevel: 1.5, animationObjects: [], animationElapsed: 0, animationDuration: 0,
      animationPlaying: false, animationLoop: false, animationStartCamera: '', morphs: [], trajectories: [],
      canUndo: false, canRedo: false, undoDescriptions: [], redoDescriptions: [], openedPaths: [],
      materialNames: [], logLines: [], status: 'Ready',
    };
    const viewport = {
      image: null, width: 320, height: 240, center: [0, 0, 0], zoom: 10, distance: 100, slab: 50,
      perspective: false, centerMark: options.centerMark || 'none', centerMarkEmbedded: true, lines: [], labels: [], status: 'Ready',
    };
    const sequenceResidues = options.sequence ? [-2, -1, 0, 1, '1A', '1B', ...Array.from({length: 4999}, (_, index) => index + 2)].map((index, position) => ({
      letter: 'G', resId: Number.parseInt(index, 10), residueIndex: String(index), resName: 'GLY',
      selection: `'A'.${index}.*`, selected: false, column: position,
    })) : [];
    const sequence = Array.from({length: options.sequence ? 40 : 0}, (_, index) => ({
      objectUid: 3 + index, objectName: `Probe${index}`, chainId: 'A', residues: sequenceResidues,
    }));
    const sequenceLayout = {minimum: -2, columns: 5014, insertions: [[1, 'A', 4], [1, 'B', 5]]};
    window.__shellCalls = [];
    window.__TAURI__ = {core: {invoke: async (command, args) => {
      window.__shellCalls.push({command, args});
      if (command === 'get_initial_state') return {snapshot, viewport, preferences: {}, startupPathsPending: false};
      if (command === 'render_viewport') return viewport;
      if (command === 'update_camera') return {...viewport, width: args.width, height: args.height};
      if (command === 'pick_atom' || command === 'select_screen_rect') return {snapshot, viewport};
      if (command === 'hover_at_screen') return {...viewport, image: null};
      if (command === 'get_molecule_details') return {structureKey: 'initial', sequenceKey: snapshot.sequenceKey, structureObjectUid: 3, structure: [], sequence, sequenceLayout};
      if (command === 'sequence_action') {
        snapshot.sequenceKey = `sequence-${window.__shellCalls.length}`;
        return {snapshot: structuredClone(snapshot), viewport};
      }
      if (command === 'get_pdb_history') return [];
      if (command === 'execute_script') return structuredClone(snapshot);
      if (command === 'rename_workspace_node') {
        if (args.input.objectUid !== 3) throw new Error('wrong inspector object');
        if (args.input.kind === 'renderer') renderer.name = args.input.name;
        else molecule.name = args.input.name;
        snapshot.canUndo = true;
        snapshot.undoDescriptions = ['Rename'];
        return structuredClone(snapshot);
      }
      if (command === 'set_workspace_visible') {
        const target = args.target.rendererUid === 4 ? renderer : molecule;
        target.visible = args.visible;
        return structuredClone(snapshot);
      }
      throw new Error(`Unexpected IPC: ${command}`);
    }}};
  }, {storage, options});
  await page.goto('http://tauri.test/');
  await expect(page.locator('#status-text')).toHaveText('Ready');
  await expect(page.locator('#error-alert-dialog')).not.toBeVisible();
  return errors;
}

test('Tauri empty scenes render their center axes and settle after camera motion', async ({page}) => {
  const errors = await openShell(page, {}, {empty: true, centerMark: 'axis'});
  const rendered = () => page.evaluate(() => window.__shellCalls.filter(call => call.command === 'render_viewport').length);
  await expect.poll(rendered).toBeGreaterThan(0);
  const before = await rendered();
  const rect = await page.locator('#viewport').boundingBox();
  await page.mouse.move(rect.x + rect.width / 2, rect.y + rect.height / 2);
  await page.mouse.down();
  await page.mouse.move(rect.x + rect.width / 2 + 60, rect.y + rect.height / 2 + 30, {steps: 6});
  await page.mouse.up();
  await expect.poll(rendered).toBeGreaterThan(before);
  expect(await page.evaluate(() => window.__shellCalls.some(call => call.command === 'update_camera'))).toBe(true);
  expect(errors).toEqual([]);
});

test('Tauri fresh storage opens the CueMol3 panels and theme survives reload', async ({page}) => {
  const errors = await openShell(page);
  await expect(page).toHaveTitle('Test scene — cuemol-rs');
  expect(await page.locator('.app-brand img').evaluate(image => image.naturalWidth)).toBeGreaterThan(0);
  expect((await page.locator('.side-panel').boundingBox()).width).toBeGreaterThan(200);
  expect((await page.locator('.bottom-panel').boundingBox()).height).toBeGreaterThan(100);
  await expect(page.getByRole('button', {name: 'Explorer', exact: true})).toHaveAttribute('aria-pressed', 'true');
  await expect(page.locator('[data-panel-name="Workspace"]')).toBeVisible();
  await expect(page.locator('[data-panel-name="Coloring"]')).toBeVisible();
  await expect(page.locator('[data-panel-name="Selection"]')).not.toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await page.getByRole('button', {name: 'Switch color theme'}).click();
  await page.reload();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  await expect(page.locator('#error-alert-dialog')).not.toBeVisible();
  expect(errors).toEqual([]);
});

test('Tauri activities keep molecule panels reachable and collapse reversibly', async ({page}) => {
  const errors = await openShell(page);
  for (const [name, panel] of [['Selection', 'Selection'], ['Crystal', 'Symmetry'], ['View', 'View']]) {
    await page.getByRole('button', {name, exact: true}).click();
    await expect(page.locator(`[data-panel-name="${panel}"]`)).toBeVisible();
    await expect(page.locator('[data-panel-name="Workspace"]')).not.toBeVisible();
  }
  await page.getByRole('button', {name: 'Actions', exact: true}).click();
  await expect(page.getByRole('button', {name: 'Superposition', exact: true})).toBeVisible();
  await page.getByRole('button', {name: 'Actions', exact: true}).click();
  await expect(page.locator('.side-panel')).not.toBeVisible();
  await page.getByRole('button', {name: 'Explorer', exact: true}).click();
  await expect(page.locator('[data-panel-name="Workspace"]')).toBeVisible();
  expect(errors).toEqual([]);
});

test('Tauri inspector follows selection and edits the selected renderer', async ({page}) => {
  const errors = await openShell(page);
  await page.locator('.tree-object').filter({hasText: 'Probe'}).click();
  await expect(page.getByRole('complementary', {name: 'Inspector'})).toBeVisible();
  await expect(page.getByLabel('Inspected name')).toHaveValue('Probe');
  await page.locator('.tree-renderer').filter({hasText: 'Atoms'}).click();
  await expect(page.getByLabel('Inspected name')).toHaveValue('Atoms');
  await page.getByLabel('Inspected name').fill('Visible atoms');
  await page.getByLabel('Inspected name').press('Tab');
  await expect(page.locator('.tree-renderer')).toContainText('Visible atoms');
  const rename = await page.evaluate(() => window.__shellCalls.find(call => call.command === 'rename_workspace_node'));
  expect(rename.args.input).toEqual({kind: 'renderer', objectUid: 3, rendererUid: 4, name: 'Visible atoms'});
  await page.getByRole('button', {name: 'Close inspector'}).click();
  await expect(page.getByRole('complementary', {name: 'Inspector'})).not.toBeVisible();
  await page.locator('.tree-object').filter({hasText: 'Probe'}).click();
  await expect(page.getByRole('complementary', {name: 'Inspector'})).toBeVisible();
  expect(errors).toEqual([]);
});

test('Tauri bottom docks expose animation, sequence and trajectory controls', async ({page}) => {
  const errors = await openShell(page);
  await page.locator('[data-bottom-select="animation"]').click();
  await expect(page.locator('#animation-play')).toBeVisible();
  await expect(page.locator('#animation-list')).toContainText('No animation objects.');
  await page.locator('[data-bottom-select="sequence"]').click();
  await expect(page.locator('#sequence-view')).toBeVisible();
  await page.locator('[data-bottom-select="trajectory"]').click();
  await expect(page.locator('#trajectory-slider')).toBeVisible();
  await page.locator('[data-bottom-select="output"]').click();
  await expect(page.getByLabel('JavaScript command')).toBeVisible();
  await expect(page.locator('#error-alert-dialog')).not.toBeVisible();
  expect(errors).toEqual([]);
});

test('Tauri saved collapsed panels reopen and a damaged shell preference uses defaults', async ({page}) => {
  const errors = await openShell(page, {
    'cuemol2.sidePanelWidth.v1': '0',
    'cuemol2.bottomPanelHeight.v1': '0',
    'cuemol3.shell.v1': 'null',
  });
  await expect(page.locator('.side-panel')).not.toBeVisible();
  await page.getByRole('button', {name: 'Explorer', exact: true}).click();
  await expect(page.locator('[data-panel-name="Workspace"]')).toBeVisible();
  await page.locator('summary').filter({hasText: /^Help$/}).click();
  await page.getByRole('button', {name: 'Console', exact: true}).click();
  await expect(page.getByLabel('JavaScript command')).toBeVisible();
  expect(errors).toEqual([]);
});

test('Tauri drag updates the camera before release without applying motion twice', async ({page}) => {
  const errors = await openShell(page);
  const rect = await page.locator('#viewport').boundingBox();
  const x = rect.x + 100, y = rect.y + 140;
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x + 100, y + 40, {steps: 8});
  await expect.poll(() => page.evaluate(() => window.__shellCalls.filter(call => call.command === 'update_camera').length), {timeout: 1500}).toBeGreaterThan(0);
  await page.mouse.up();
  await page.waitForTimeout(100);
  const calls = await page.evaluate(() => window.__shellCalls.filter(call => call.command === 'update_camera'));
  expect(calls.reduce((sum, call) => sum + call.args.x, 0)).toBeCloseTo(35, 5);
  expect(calls.reduce((sum, call) => sum + call.args.y, 0)).toBeCloseTo(14, 5);
  expect(calls.every(call => call.args.renderScale === 1)).toBe(true);
  expect(errors).toEqual([]);
});

test('Tauri HiDPI picking uses render pixels and the tool palette does not rotate the scene', async ({browser}) => {
  const context = await browser.newContext({viewport: {width: 1280, height: 800}, deviceScaleFactor: 2});
  try {
    const page = await context.newPage();
    const errors = await openShell(page);
    const rect = await page.locator('#viewport').boundingBox();
    await expect.poll(() => page.evaluate(() => window.__shellCalls.filter(call => call.command === 'render_viewport').length)).toBeGreaterThan(0);
    const render = await page.evaluate(() => window.__shellCalls.find(call => call.command === 'render_viewport'));
    expect(render.args.pixelRatio).toBe(2);
    await page.mouse.click(rect.x + 150, rect.y + 150);
    await expect.poll(() => page.evaluate(() => window.__shellCalls.filter(call => call.command === 'pick_atom').length)).toBe(1);
    const pick = await page.evaluate(() => window.__shellCalls.find(call => call.command === 'pick_atom'));
    expect(pick.args.x).toBeCloseTo(300, 0);
    expect(pick.args.y).toBeCloseTo(300, 0);
    await page.getByRole('button', {name: 'Rect select', exact: true}).click();
    const calls = await page.evaluate(() => window.__shellCalls.filter(call => call.command === 'pick_atom' || call.command === 'update_camera'));
    expect(calls).toHaveLength(1);
    expect(errors).toEqual([]);
  } finally { await context.close(); }
});


test('Tauri console preserves literal quotes and submits through Enter', async ({page}) => {
  const errors = await openShell(page);
  const input = page.getByRole('textbox', {name: 'JavaScript command'});
  await expect(input).toHaveAttribute('spellcheck', 'false');
  await expect(input).toHaveAttribute('autocorrect', 'off');
  await expect(input).toHaveAttribute('autocapitalize', 'off');
  const script = 'print("literal quotes");';
  await input.fill(script);
  await input.press('Enter');
  await expect(input).toHaveValue('');
  expect(await page.evaluate(() => window.__shellCalls.find(call => call.command === 'execute_script').args.script)).toBe(script);
  expect(errors).toEqual([]);
});

test('Tauri cancelled drag schedules a settled frame without replaying movement', async ({page}) => {
  const errors = await openShell(page);
  await page.waitForTimeout(200);
  const viewport = page.locator('#viewport');
  const rect = await viewport.boundingBox();
  const x = rect.x + 100, y = rect.y + 140;
  await viewport.dispatchEvent('pointerdown', {pointerId: 1, button: 0, clientX: x, clientY: y});
  await viewport.dispatchEvent('pointermove', {pointerId: 1, clientX: x + 30, clientY: y + 10});
  await expect.poll(() => page.evaluate(() => window.__shellCalls.filter(call => call.command === 'update_camera').length)).toBe(1);
  await page.evaluate(() => { window.__shellCalls = []; });
  await viewport.dispatchEvent('pointercancel', {pointerId: 1});
  await expect(viewport).not.toHaveClass(/dragging/);
  await expect.poll(() => page.evaluate(() => window.__shellCalls.filter(call => call.command === 'render_viewport').length)).toBeGreaterThan(0);
  expect(await page.evaluate(() => window.__shellCalls.filter(call => call.command === 'update_camera'))).toEqual([]);
  expect(errors).toEqual([]);
});


test('Tauri sequence virtualizes both axes and ranges include offscreen residues', async ({page}, testInfo) => {
  const errors = await openShell(page, {}, {sequence: true});
  await page.locator('[data-bottom-select="sequence"]').click();
  const firstRow = page.locator('.sequence-chain').first();
  const cell = index => firstRow.locator(`.sequence-cell[data-residue-index="${index}"]`);
  for (const index of [-2, -1, 0, 1, '1A', '1B']) {
    await cell(index).click();
    await expect.poll(() => page.evaluate(() => window.__shellCalls.filter(call => call.command === 'sequence_action').at(-1)?.args.input.targetSelection)).toBe(`'A'.${index}.*`);
  }
  const insertionA = await cell('1A').boundingBox(), insertionB = await cell('1B').boundingBox();
  expect(insertionB.x).toBeGreaterThanOrEqual(insertionA.x + insertionA.width - 0.1);
  await cell(1).click();
  await page.locator('.sequence-placeholder').evaluate(element => { element.scrollLeft = 50000; });
  await expect(cell(5000)).toBeVisible();
  await page.keyboard.down('Shift');
  await cell(5000).click();
  await page.keyboard.up('Shift');
  const range = await page.evaluate(() => window.__shellCalls.filter(call => call.command === 'sequence_action').at(-1).args.input);
  expect(range.action).toBe('range-select');
  expect(range.targetSelection).toBe("'A'.1.*");
  expect(range.rangeSelection.split('|')).toHaveLength(5002);
  expect(range.rangeSelection).toContain("'A'.1A.*");
  expect(range.rangeSelection.endsWith("'A'.5000.*")).toBe(true);
  await expect(page.locator('.sequence-cell.marked')).toHaveAttribute('data-residue-index', '5000');
  await page.locator('.sequence-placeholder').evaluate(element => { element.scrollTop = element.scrollHeight; });
  await expect(page.locator('.sequence-chain strong').last()).toHaveText('A:Probe39');
  expect(await page.locator('.sequence-cell').count()).toBeLessThan(1000);
  expect(await page.locator('.sequence-chain').count()).toBeLessThan(20);
  const bottom = await page.locator('.sequence-chain').last().boundingBox();
  const viewport = await page.locator('.sequence-placeholder').boundingBox();
  expect(bottom.y + bottom.height).toBeLessThanOrEqual(viewport.y + viewport.height + 1);
  expect(bottom.y + bottom.height).toBeGreaterThan(viewport.y + viewport.height - 20);
  const canvasWidth = await page.locator('.sequence-ruler-canvas').evaluate(element => element.width);
  expect(canvasWidth).toBeLessThanOrEqual(page.viewportSize().width * 2);
  await page.screenshot({path: testInfo.outputPath('sequence-virtual.png')});
  expect(errors).toEqual([]);
});
