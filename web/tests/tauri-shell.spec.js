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
      coloringType: options.paint ? 'PaintColoring' : 'CPKColoring', coloringAttrs: {},
      paintEntries: options.paint ? [{selection:'chain A',color:'red'},{selection:'*',color:'white'}] : [], gradientEntries: [],
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
    const styleDocuments = new Map();
    if (options.styles) {
      snapshot.styles = [
        {styleId:101,sceneUid:0,name:'user',localIndex:null},
        {styleId:102,sceneUid:1,name:'user',localIndex:0},
        {styleId:103,sceneUid:0,name:'(anonymous)',localIndex:null},
        {styleId:104,sceneUid:0,name:'(anonymous)',localIndex:null},
      ].map(style=>({ownerSceneUid:1,readOnly:false,modified:false,source:'',...style}));
      for (const style of snapshot.styles) {
        const name=style.styleId===103?'':style.name;
        styleDocuments.set(style.styleId,`<styles id="${name}"><color id="accent" value="#123456"/><sel id="focus" value="name C1"/><style id="KeepCPK" type="renderer"/></styles>`);
      }
      snapshot.styles[0].source='user.xml';
      window.__insertStyle=()=>snapshot.styles.splice(0,0,{ownerSceneUid:1,sceneUid:1,styleId:105,name:'inserted',localIndex:0,readOnly:false,modified:false,source:''});
    }
    const styleFor = target => {
      const style=snapshot.styles.find(style=>style.styleId===target.styleId&&style.sceneUid===target.sceneUid&&style.ownerSceneUid===target.ownerSceneUid);
      if(!style)throw new Error('Style target does not exist');
      return style;
    };
    if(options.selectionSecond)snapshot.objects.push({...molecule,uid:8,name:'Other',selection:''});
    const viewport = {
      image: null, width: 320, height: 240, center: [0, 0, 0], zoom: 10, distance: 100, slab: 50,
      perspective: false, centerMark: options.centerMark || 'none', centerMarkEmbedded: true, lines: [], labels: [], status: 'Ready',
    };
    const sceneValues = {name:scene.name,uid:'1',aoEnabled:'false',aoRadius:'4',aoSteps:'3',aoIntensity:'2.2',aoHalfRes:'false'};
    const selectionHistory=['name C1'], selectionNames=['carbon'];
    const selectionUndo=[], selectionRedo=[];
    const rememberSelection=expression=>{
      const value=expression.trim();
      if(!['','*','none'].includes(value))selectionHistory.splice(0,selectionHistory.length,...[value,...selectionHistory.filter(item=>item!==value)].slice(0,100));
    };
    const inspectorValues = options.inspectorPages ? {ring:'false',thickness:'0.2',material:'',font_name:'Portable face','section.width':'1','section.tuber':'0.5',helix_ribbon:'false',loopsize2:'-1',stipple0:'-1',stipple1:'-1',stipple2:'-1',stipple3:'-1',stipple4:'-1',stipple5:'-1',bndry_molname:'',bndry_sel:'',autoupdate:'true',dragupdate:'true'} : {};
    if (options.inspectorPages) renderer.detail=24;
    const sequenceIndices = options.sequenceExtremes ? [-2147483648, '-2147483648A', 0, 2147483647, '2147483647B'] : [-2, -1, 0, 1, '1A', '1B', ...Array.from({length: 4999}, (_, index) => index + 2)];
    const sequenceResidues = options.sequence ? sequenceIndices.map((index, position) => ({
      letter: 'G', resId: Number.parseInt(index, 10), residueIndex: String(index), resName: 'GLY',
      selection: `'A'.${index}.*`, selected: false, column: options.sequenceExtremes ? [0,1,3,5,6][position] : position,
    })) : [];
    const sequence = Array.from({length: options.sequence ? 40 : 0}, (_, index) => ({
      objectUid: 3 + index, objectName: `Probe${index}`, chainId: 'A', residues: sequenceResidues,
    }));
    const sequenceLayout = options.sequenceExtremes ? {minimum: -2147483648, columns:16, insertions:[[-2147483648,'A',1],[2147483647,'B',6]],gaps:[{first:-2147483647,last:-1,column:2,removed:2147483646},{first:1,last:2147483646,column:4,removed:4294967291}]} : {minimum: -2, columns: 5014, insertions: [[1, 'A', 4], [1, 'B', 5]]};
    window.__shellCalls = [];
    window.__actionsRunning = false;
    window.__TAURI__ = {core: {invoke: async (command, args) => {
      window.__shellCalls.push({command, args});
      if (command === 'get_initial_state') return {snapshot, viewport, preferences: {}, startupPathsPending: false};
      if (command === 'get_snapshot') return snapshot;
      if (options.styles && command === 'get_style_properties') {
        const style=styleFor(args.target), xml=styleDocuments.get(style.styleId);
        const name=new DOMParser().parseFromString(xml,'application/xml').documentElement.getAttribute('id');
        return {...style,name,xml};
      }
      if (options.styles && command === 'set_style_properties') {
        const properties=args.properties, style=styleFor(properties);
        if(properties.editContent)throw new Error('Editor must send entry changes, not a stale XML document');
        style.name=properties.name||'(anonymous)';style.readOnly=properties.readOnly;
        const doc=new DOMParser().parseFromString(styleDocuments.get(style.styleId),'application/xml');
        doc.documentElement.setAttribute('id',properties.name);
        styleDocuments.set(style.styleId,new XMLSerializer().serializeToString(doc));
        return structuredClone(snapshot);
      }
      if (options.styles && command === 'save_style_file') {
        const style=styleFor(args.target);
        if(style.source!==args.path)style.modified=false;
        style.source=args.path;
        return structuredClone(snapshot);
      }
      if (command === 'render_viewport') return viewport;
      if (command === 'update_camera') {
        if (args.action === 'zoom') viewport.zoom *= args.x;
        return {...viewport, width: args.width, height: args.height};
      }
      if (command === 'pick_atom' || command === 'select_screen_rect') return {snapshot, viewport};
      if (command === 'hover_at_screen') return {...viewport, image: null};
      if (command === 'get_molecule_details') return {structureKey: 'initial', sequenceKey: snapshot.sequenceKey, structureObjectUid: 3, structure: [], sequence, sequenceLayout};
      if (command === 'sequence_action') {
        snapshot.sequenceKey = `sequence-${window.__shellCalls.length}`;
        return {snapshot: structuredClone(snapshot), viewport};
      }
      if (command === 'get_pdb_history') return [];
      if (command === 'clipboard_kind') return window.__clipboardKind || '';
      if (command === 'clipboard_text') return window.__clipboardText || '';
      if (command === 'set_clipboard_text') { window.__clipboardText = args.text; window.__clipboardKind = ''; return 'Copied text'; }
      if (options.clipboard && command === 'copy_workspace_object') {window.__clipboardKind='object';return 'Copied object';}
      if (options.clipboard && command === 'paste_workspace_object') {
        snapshot.objects.push({...molecule,uid:5,name:'Probe_1'});
        snapshot.canUndo = true;
        snapshot.undoDescriptions = ['Paste object'];
        return structuredClone(snapshot);
      }
      if (options.paint && command === 'set_coloring') {
        renderer.coloringType = args.update.coloringType;
        renderer.paintEntries = args.update.paintEntries;
        return {snapshot: structuredClone(snapshot), viewport};
      }
      if (command === 'actions_request') {
        if (args.operation === 'list') {
          const value = {steps: options.actions ? [{id:7,label:'Cartoon + ligand',entryCount:3,undone:false},{id:8,label:'Hide',entryCount:1,undone:false}] : [],done:options.actions ? 2 : 0,running:window.__actionsRunning,
            files:[{name:'Presets',bundled:true,actionCount:0,macros:[{name:'cartoon-ligand',label:'Cartoon + ligand',target_type:'MolCoord',description:'Show cartoon and ligand.'}]}]};
          if (window.__extraAction) value.files.push({key:'extra.qax',name:window.__extraAction,bundled:false,actionCount:1,macros:[]});
          if (window.__delayActionList) {
            window.__delayActionList = false;
            return new Promise(resolve => { window.__releaseActionList = () => resolve(value); });
          }
          return value;
        }
        if (args.operation === 'inspect') return {id:args.args.id,label:'Cartoon + ligand',source:'cmd',entries:[{command:'create_renderer',args:[['objId','Probe']],note:false}]};
        if (args.operation === 'run' || args.operation === 'goto') {
          window.__actionsRunning = args.operation === 'run' && Boolean(options.actionsAsync);
          return {snapshot:structuredClone(snapshot),running:window.__actionsRunning};
        }
        if (args.operation === 'save_history') return {ok:true};
      }
      if (command === 'execute_script') return structuredClone(snapshot);
      if (options.selectionPane && ['undo','redo'].includes(command)) {
        const from=command==='undo'?selectionUndo:selectionRedo, to=command==='undo'?selectionRedo:selectionUndo;
        if(from.length){to.push(molecule.selection);molecule.selection=from.pop();}
        snapshot.canUndo=selectionUndo.length>0;snapshot.canRedo=selectionRedo.length>0;
        snapshot.actionHistoryKey=`selection-${window.__shellCalls.length}`;
        return structuredClone(snapshot);
      }
      if (options.shortcuts && ['undo', 'redo', 'close_active_scene'].includes(command)) return structuredClone(snapshot);
      if (options.shortcuts && command === 'molecule_selection_action') return {snapshot:structuredClone(snapshot),viewport};
      if (command === 'console_start' || command === 'console_stop' || command === 'console_poll') {
        if (command === 'console_stop') window.__actionsRunning = false;
        return {snapshot: structuredClone(snapshot), running: window.__actionsRunning || command !== 'console_stop' && Boolean(options.consoleRunning)};
      }
      if (command === 'console_complete') return [{display: 'Probe', replacement: 'select Probe'}];
      if (command === 'inspector_request') {
        if(options.styles && args.tool==='style_edit') {
          const request=args.args, style=styleFor(request.target);
          if(window.__failStyleEdit){window.__failStyleEdit=false;throw new Error('Style edit rejected');}
          if(style.readOnly)throw new Error('Style is read-only');
          const doc=new DOMParser().parseFromString(styleDocuments.get(style.styleId),'application/xml');
          const tag=request.kind==='selection'?'sel':request.kind;
          let node=[...doc.documentElement.children].find(node=>node.tagName===tag&&node.getAttribute('id')===request.name);
          if(request.value===undefined){node?.remove();}
          else {
            if(!node){node=doc.createElement(tag);node.setAttribute('id',request.name);doc.documentElement.append(node);}
            node.setAttribute('value',request.kind==='color' && /^#[0-9a-f]{6}$/i.test(request.value) ? request.value.toUpperCase() : request.value);
          }
          styleDocuments.set(style.styleId,new XMLSerializer().serializeToString(doc));
          style.modified=true;
          return {snapshot:structuredClone(snapshot),result:{ok:true}};
        }
        if(args.tool==='selection_history') {
          if(args.args.action==='import')for(const value of args.args.entries){if(value.trim()&&!['*','none'].includes(value.trim())&&!selectionHistory.includes(value.trim()))selectionHistory.push(value.trim());}
          if(args.args.action==='remember')rememberSelection(args.args.expression);
          return {history:[...selectionHistory]};
        }
        if(args.tool==='selection_edit') {
          const request=args.args;
          if(request.expression==='chain (')throw new Error('Invalid selection expression');
          if(request.action==='apply') {
            selectionUndo.push(molecule.selection);selectionRedo.splice(0);molecule.selection=request.expression;
            snapshot.canUndo=true;snapshot.canRedo=false;
            if(request.remember)rememberSelection(request.expression);
          }
          if(request.action==='define'){
            if(request.name==='fail')throw new Error('Definition failed');
            selectionNames.push(request.name);rememberSelection(request.expression);
          }
          snapshot.actionHistoryKey=`selection-${window.__shellCalls.length}`;
          const result={snapshot:structuredClone(snapshot),history:[...selectionHistory]};
          if(window.__delaySelectionEdit){window.__delaySelectionEdit=false;return new Promise(resolve=>{window.__releaseSelectionEdit=()=>resolve(result);});}
          return result;
        }
        if (args.tool === 'selection_info') {
          const request=args.args;
          if(request.action==='context') return {current:request.moleculeId?molecule.selection:'name O1',scene:[...selectionNames],global:['protein'],history:[...selectionHistory],keywords:[
            {key:'hierarchical',label:'hier',kind:'hierarchical'},
            {key:'name',label:'name',kind:'nameList',candidates:'aname'},
          ]};
          if(request.action==='values') return window.__selectionCandidateValues || ['C1','O1'];
          if(request.action==='compose') {
            const term=request.draft.fields.value ? `name ${request.draft.fields.value}` : null;
            const result={expression:term===null ? null : request.operation==='add' ? `(${request.current}) or (${term})` : term};
            if(window.__delayCompose) {window.__delayCompose=false;return new Promise((resolve,reject)=>{window.__releaseCompose=()=>window.__composeError?reject(new Error('Composition failed')):resolve(result);});}
            return result;
          }
          if(request.action==='evaluate') {
            const result={valid:request.expression!=='chain (',count:1,error:request.expression==='chain ('?'Invalid selection':null};
            if(window.__delaySelection) {window.__delaySelection=false;return new Promise(resolve=>{window.__releaseSelection=()=>resolve(result);});}
            return result;
          }
          throw new Error(`Unexpected selection action: ${request.action}`);
        }
        if (['scene','view'].includes(args.args.nodeType)) {
          const isScene=args.args.nodeType==='scene';
          const values=isScene ? sceneValues : {name:snapshot.views[0].name,uid:'2',zoom:String(viewport.zoom)};
          const sections=isScene ? [
            {title:'Basic settings',expanded:true,rows:[{key:'name',label:'Name',editable:true}]},
            {title:'Ambient occlusion',expanded:true,rows:[{key:'aoEnabled',label:'Enabled'},{key:'ao-preset',label:'Preset',enabledWhen:'aoEnabled',options:[
              {label:'Low',patch:{aoRadius:'4',aoSteps:'3',aoIntensity:'2.2',aoHalfRes:'false'}},
              {label:'High',patch:{aoRadius:'12',aoSteps:'5',aoIntensity:'1.7',aoHalfRes:'true'}},
            ]}]},
          ] : [];
          const rows=()=>Object.entries(values).map(([key,value])=>({key,value,type:key==='aoEnabled' || key==='aoHalfRes' ? 'boolean' : key==='name' ? 'string' : 'real',readonly:key==='uid' || isScene && key==='name',category:'Common',hasdefault:false}));
          if(args.tool==='list_node_props') return {properties:rows(),sections};
          for(const {prop,value} of args.tool==='set_node_props' ? args.args.updates : [args.args]) {
            values[prop]=value;
            if(isScene && prop==='name') scene.name=value;
            if(!isScene && prop==='name') snapshot.views[0].name=value;
            if(!isScene && prop==='zoom') viewport.zoom=Number(value);
          }
          return {snapshot:structuredClone(snapshot),result:{properties:rows()}};
        }
        const target = args.args.nodeId === 4 ? renderer : molecule;
        const defaults = {alpha:'1', visible:'true', detail:'4', egtype:'none',...(options.inspectorPages ? {ring:'false',thickness:'0.2','section.width':'1','section.tuber':'0.5',helix_ribbon:'false',loopsize2:'-1',autoupdate:'true',dragupdate:'false'} : {})};
        const values = {name:target.name, visible:String(target.visible), uid:String(target.uid),
          ...(target === renderer ? {...inspectorValues,alpha:String(renderer.alpha), detail:String(renderer.detail), egtype:renderer.edgeType, style:renderer.style} : {})};
        if(options.selectionPicker) values.sel=target.selection;
        const rows = () => Object.entries(values).map(([key,value]) => ({key,value,
          type: key==='sel' ? 'selection' : ['visible','ring','helix_ribbon','autoupdate','dragupdate'].includes(key) ? 'boolean' : ['alpha','thickness','section.width','section.tuber','loopsize2'].includes(key) || key.startsWith('stipple') ? 'real' : key === 'detail' || key === 'uid' ? 'integer' : key === 'egtype' ? 'enum' : 'string',
          category: ['name','visible'].includes(key) ? 'Common' : key === 'uid' ? 'Identity' : 'Appearance',
          readonly: ['uid','style'].includes(key), hasdefault:Object.hasOwn(defaults,key),
          isdefault:value === defaults[key], defaultValue:defaults[key], allowed:key === 'egtype' ? ['none','edges','silhouette'] : undefined,
        }));
        if (args.tool === 'list_node_props') {
          const properties=rows();
          if (window.__delayInspector) { window.__delayInspector=false; return new Promise(resolve=>{window.__releaseInspector=()=>resolve({properties});}); }
          const sections=options.inspectorPages && target===renderer ? [{title:'Basic settings',expanded:true,rows:[
            {key:'name',label:'Name'},{key:'alpha',label:'Opacity',min:0,max:1,step:0.1},
            {key:'detail',label:'Detail',choices:['2','4','8','16','24','32']},
            {key:'material',label:'Material',choices:['','Matte'],labels:{'':'(none)'}},
            {key:'font_name',label:'Font name',font:true,choices:['Portable face','8514oem','serif','monospace']},
          ]},{title:'Geometry',expanded:true,rows:[
            {key:'ring',label:'Show ring'},{key:'thickness',label:'Thickness',min:0,max:3,step:0.01,unit:'Å',disabled:values.ring!=='true'},
            {key:'section.width',label:'Width1',min:0.01,max:3,step:0.01,writes:[{prop:'section.width'},{prop:'section.tuber',inverse:Number(values['section.width'])*Number(values['section.tuber'])}]},
            {key:'section.tuber',label:'Width2',display:String(Number(values['section.width'])*Number(values['section.tuber'])),min:0.01,max:3,step:0.01,writes:[{prop:'section.tuber',scale:1/Number(values['section.width'])}]},
            {key:'helix_ribbon',label:'Type',choices:['false','true'],labels:{false:'Cylinder',true:'Ribbon'}},
          ]},{title:'Optional and composite controls',expanded:true,rows:[
            {key:'loopsize2',label:'Loop size 2',min:0,max:10,step:0.1,optional:{off:-1,on:2,label:'Use a separate loop size 2'}},
            {key:'dash-pattern',kind:'toggle',label:'Dashed',display:String(Array.from({length:6},(_,i)=>Number(values[`stipple${i}`])).some(value=>value>=0)),patches:{true:Object.fromEntries(Array.from({length:6},(_,i)=>[`stipple${i}`,i<2?'1':'-1'])),false:Object.fromEntries(Array.from({length:6},(_,i)=>[`stipple${i}`,'-1']))}},
            {key:'stipple2',label:'Dash 2',blankNegative:true,enabledWhen:'dash-pattern'},
            {key:'limit-display',kind:'toggle',label:'Limit display by',display:String(Boolean(values.bndry_molname)),local:true,patches:{true:{},false:{bndry_molname:'',bndry_sel:''}}},
            {key:'bndry_molname',label:'Target',choices:[''],enabledWhen:'limit-display'},
            {key:'center-update',label:'Center update',reset:['autoupdate','dragupdate'],options:[
              {label:'None',patch:{autoupdate:'false',dragupdate:'false'}},
              {label:'Automatic',patch:{autoupdate:'true',dragupdate:'false'}},
              {label:'Automatic (drag)',patch:{autoupdate:'true',dragupdate:'true'}},
            ]},
          ]}] : options.selectionPicker ? [{title:'Basic settings',expanded:true,rows:[{key:'sel',label:'Selection'}]}] : [];
          return {properties,sections};
        }
        const set = (key,value) => {
          values[key]=value;
          if(key==='name') target.name=value;
          if(key==='sel') target.selection=value;
          if(key==='visible') target.visible=value==='true';
          if(key==='alpha') renderer.alpha=Number(value);
          if(key==='detail') renderer.detail=Number(value);
          if(key==='egtype') renderer.edgeType=value;
          if(Object.hasOwn(inspectorValues,key)) inspectorValues[key]=value;
        };
        if(args.tool==='set_node_prop') set(args.args.prop,args.args.value);
        else if(args.tool==='set_node_props') for(const {prop,value} of args.args.updates) set(prop,value);
        else for(const key of args.args.props || Object.keys(defaults)) if(Object.hasOwn(values,key)) set(key,defaults[key]);
        snapshot.canUndo=true; snapshot.undoDescriptions.push('Change properties');
        return {snapshot:structuredClone(snapshot),result:{properties:rows()}};
      }
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
    const backendInvoke=window.__TAURI__.core.invoke;
    window.__TAURI__.core.invoke=async(command,args)=>structuredClone(await backendInvoke(command,structuredClone(args)));
  }, {storage, options});
  await page.goto('http://tauri.test/');
  await expect(page.locator('#status-text')).toHaveText('Ready');
  await expect(page.locator('#error-alert-dialog')).not.toBeVisible();
  return errors;
}

test('Tauri style editor sends scoped entry changes and keeps other drafts', async ({page}, testInfo) => {
  const errors=await openShell(page,{}, {styles:true});
  await page.locator('.style-node').filter({hasText:'user ↗ (global)'}).dblclick();
  const dialog=page.locator('#reference-dialog');
  await expect(dialog.getByLabel('Style set name')).toHaveValue('user');
  await expect(dialog.getByRole('button',{name:'Rename',exact:true})).toBeDisabled();
  await dialog.getByLabel('Style set name').press('Enter');
  expect(await page.evaluate(()=>window.__shellCalls.filter(call=>call.command==='set_style_properties'))).toHaveLength(0);
  await dialog.getByLabel('Color accent',{exact:true}).fill('rgba(0.2,0.4,0.6,0.5)');
  await dialog.getByLabel('Style set name').fill(' unfinished ');
  await page.evaluate(()=>window.__insertStyle());
  await dialog.getByRole('button',{name:'Apply color accent',exact:true}).click();
  await expect(dialog.getByLabel('Color accent',{exact:true})).toHaveValue('rgba(0.2,0.4,0.6,0.5)');
  await expect(dialog.getByLabel('Style set name')).toHaveValue(' unfinished ');
  await dialog.getByRole('button',{name:'Selection',exact:true}).click();
  await dialog.getByLabel('Selection focus',{exact:true}).fill('name O1');
  await dialog.getByRole('button',{name:'Apply selection focus',exact:true}).click();
  await expect(dialog.getByLabel('Selection focus',{exact:true})).toHaveValue('name O1');
  await dialog.getByRole('button',{name:'Rename',exact:true}).click();
  await expect(page.locator('#dialog-title')).toContainText('unfinished');
  await expect(dialog.getByLabel('Style set name')).toHaveValue('unfinished');
  await dialog.getByRole('button',{name:'Save style...',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>window.__shellCalls.filter(call=>call.command==='save_style_file').length)).toBe(1);
  const calls=await page.evaluate(()=>window.__shellCalls);
  const edits=calls.filter(call=>call.command==='inspector_request'&&call.args.tool==='style_edit');
  expect(edits.map(call=>call.args.args)).toEqual([
    {target:{ownerSceneUid:1,sceneUid:0,styleId:101},kind:'color',name:'accent',value:'rgba(0.2,0.4,0.6,0.5)'},
    {target:{ownerSceneUid:1,sceneUid:0,styleId:101},kind:'selection',name:'focus',value:'name O1'},
  ]);
  expect(calls.find(call=>call.command==='save_style_file').args).toEqual({target:{ownerSceneUid:1,sceneUid:0,styleId:101},path:'user.xml'});
  await page.screenshot({path:testInfo.outputPath('tauri-style-editor.png')});
  expect(errors).toEqual([]);
});

test('Tauri anonymous style editing preserves identity and failed drafts', async ({page}) => {
  const errors=await openShell(page,{}, {styles:true});
  await page.locator('.style-node').filter({hasText:'(anonymous) (global)'}).first().dblclick();
  const dialog=page.locator('#reference-dialog');
  await expect(dialog.getByLabel('Style set name')).toHaveValue('');
  await dialog.getByLabel('Style set name').press('Enter');
  expect(await page.evaluate(()=>window.__shellCalls.filter(call=>call.command==='set_style_properties'))).toHaveLength(0);
  await dialog.getByRole('checkbox',{name:'Read-only',exact:true}).check();
  await expect(dialog.getByLabel('Color accent',{exact:true})).toBeDisabled();
  await dialog.getByRole('checkbox',{name:'Read-only',exact:true}).uncheck();
  await expect(dialog.getByLabel('Color accent',{exact:true})).toBeEnabled();
  await dialog.getByLabel('Color accent',{exact:true}).fill('#abcdef');
  await page.evaluate(()=>{window.__failStyleEdit=true;});
  await dialog.getByRole('button',{name:'Apply color accent',exact:true}).click();
  await expect(page.locator('#error-alert-dialog')).toContainText('Style edit rejected');
  await page.locator('#error-alert-dialog').getByRole('button').last().click();
  await expect(dialog.getByLabel('Color accent',{exact:true})).toHaveValue('#abcdef');
  await dialog.getByRole('button',{name:'Apply color accent',exact:true}).click();
  await expect(dialog.getByRole('checkbox',{name:'Read-only',exact:true})).toBeDisabled();
  await expect(dialog.getByLabel('Color accent',{exact:true})).toHaveValue('#ABCDEF');
  await expect(dialog.getByLabel('Style set name')).toHaveValue('');
  const calls=await page.evaluate(()=>window.__shellCalls.filter(call=>call.command==='inspector_request'&&call.args.tool==='style_edit'));
  expect(calls).toHaveLength(2);
  expect(calls.every(call=>call.args.args.target.styleId===103&&call.args.args.target.sceneUid===0)).toBe(true);
  expect(errors).toEqual([]);
});

test('Tauri style additions retain failed and unrelated pending names', async ({page}) => {
  const errors=await openShell(page,{}, {styles:true});
  await page.locator('.style-node').filter({hasText:'user ↗ (global)'}).dblclick();
  const dialog=page.locator('#reference-dialog');
  await dialog.getByPlaceholder('New color name').fill('new-accent');
  await dialog.getByLabel('Color accent',{exact:true}).fill('#abcdef');
  await dialog.getByRole('button',{name:'Apply color accent',exact:true}).click();
  await expect(dialog.getByPlaceholder('New color name')).toHaveValue('new-accent');
  await dialog.getByRole('button',{name:'Selection',exact:true}).click();
  await dialog.getByPlaceholder('New selection name').fill('new-focus');
  await dialog.getByRole('button',{name:'Color',exact:true}).click();
  await expect(dialog.getByPlaceholder('New color name')).toHaveValue('new-accent');
  await page.evaluate(()=>{window.__failStyleEdit=true;});
  await dialog.getByRole('button',{name:'Add',exact:true}).click();
  await expect(page.locator('#error-alert-dialog')).toContainText('Style edit rejected');
  await page.locator('#error-alert-dialog').getByRole('button').last().click();
  await expect(dialog.getByPlaceholder('New color name')).toHaveValue('new-accent');
  await dialog.getByRole('button',{name:'Add',exact:true}).click();
  await expect(dialog.getByLabel('Color new-accent',{exact:true})).toHaveValue('#FFFFFF');
  await expect(dialog.getByPlaceholder('New color name')).toBeEmpty();
  await dialog.getByRole('button',{name:'Selection',exact:true}).click();
  await expect(dialog.getByPlaceholder('New selection name')).toHaveValue('new-focus');
  expect(errors).toEqual([]);
});

test('Tauri Edit clipboard routes tree, text and modal focus separately', async ({page}) => {
  const errors = await openShell(page, {}, {clipboard:true,shortcuts:true});
  const edit = page.locator('.menu').filter({has:page.locator('summary', {hasText:/^Edit$/})});
  async function menu(action) {await edit.locator('summary').click();await edit.locator(`[data-action="${action}"]`).click();}
  await page.evaluate(() => {window.__clipboardKind='renderer';});
  await page.locator('#scene-tree button').filter({hasText:'Probe'}).first().click({button:'right'});
  await expect(page.getByRole('menuitem',{name:'Paste Renderer',exact:true})).toBeEnabled();
  await page.keyboard.press('Escape');
  await page.evaluate(() => {window.__clipboardKind='';});
  await page.locator('#scene-tree button').filter({hasText:'Probe'}).first().click({button:'right'});
  await expect(page.getByRole('menuitem',{name:'Paste Renderer',exact:true})).toBeDisabled();
  await page.keyboard.press('Escape');
  await page.locator('#scene-tree button').filter({hasText:'Probe'}).first().click();
  await menu('copy');
  await expect.poll(() => page.evaluate(() => window.__clipboardKind)).toBe('object');
  await menu('paste');
  await expect(page.locator('#scene-tree')).toContainText('Probe_1');
  const text = page.locator('#output-command');
  await text.fill('selected text');
  await text.click();
  await text.selectText();
  await menu('copy');
  await expect.poll(() => page.evaluate(() => window.__clipboardText)).toBe('selected text');
  await menu('cut');
  await expect(text).toHaveValue('');
  await menu('paste');
  await expect(text).toHaveValue('selected text');
  await page.locator('#viewport').focus();
  await menu('undo');
  expect(await page.evaluate(() => window.__shellCalls.filter(call=>call.command==='undo').length)).toBe(1);
  await page.evaluate(() => {
    const dialog=document.createElement('dialog');dialog.id='clipboard-probe-dialog';dialog.textContent='Modal';document.body.append(dialog);dialog.showModal();
  });
  await page.keyboard.press('Control+v');
  expect(await page.evaluate(() => window.__shellCalls.filter(call=>call.command==='paste_workspace_object').length)).toBe(1);
  expect(errors).toEqual([]);
});

test('Tauri paint clipboard reads reference keys, BOM, CRLF and wrapped base64', async ({page}) => {
  const errors = await openShell(page, {}, {paint:true});
  await page.locator('#scene-tree button').filter({hasText:'Atoms'}).first().click();
  await page.locator('#coloring-target').selectOption('renderer:3:4');
  await expect(page.locator('#coloring-paint-list')).toBeVisible();
  await page.locator('.coloring-paint-row').first().click();
  const edit = page.locator('.menu').filter({has:page.locator('summary', {hasText:/^Edit$/})});
  async function menu(action) {await edit.locator('summary').click();await edit.locator(`[data-action="${action}"]`).click();}
  await menu('copy');
  await expect.poll(() => page.evaluate(() => window.__clipboardText || '')).toContain('CueMolClipboard/1');
  const copied = await page.evaluate(() => JSON.parse(new TextDecoder().decode(Uint8Array.from(atob(window.__clipboardText.split('\n')[2]), char => char.charCodeAt(0)))));
  expect(copied).toEqual([{sel:'chain A',col:'red'}]);
  const data = Buffer.from(JSON.stringify([null,{sel:'chain 日本',col:'blue'},{sel:'*',col:'green'},{sel:7,col:'red'}])).toString('base64').match(/.{1,16}/g).join('\r\n');
  await page.evaluate(text => {window.__clipboardText=text;}, `\uFEFFCueMolClipboard/1\r\n{"kind":"paint","form":"single"}\r\n${data}\r\n`);
  await menu('paste');
  await expect.poll(() => page.evaluate(() => window.__shellCalls.filter(call => call.command==='set_coloring').length)).toBe(1);
  const rows = await page.evaluate(() => window.__shellCalls.find(call => call.command==='set_coloring').args.update.paintEntries);
  expect(rows).toEqual([{selection:'chain 日本',color:'blue'},{selection:'*',color:'green'},{selection:'chain A',color:'red'},{selection:'*',color:'white'}]);
  expect(errors).toEqual([]);
});

test('Tauri shortcuts distinguish redo, close, text editing and modified viewport letters', async ({page}) => {
  const errors = await openShell(page, {}, {shortcuts: true});
  await page.locator('#viewport').focus();
  await page.keyboard.press('Control+Shift+z');
  await expect.poll(() => page.evaluate(() => window.__shellCalls.filter(call => call.command === 'redo').length)).toBe(1);
  await page.keyboard.press('Control+w');
  await expect.poll(() => page.evaluate(() => window.__shellCalls.filter(call => call.command === 'close_active_scene').length)).toBe(1);
  const cameraCalls = await page.evaluate(() => window.__shellCalls.filter(call => call.command === 'camera_command').length);
  await page.keyboard.press('Control+f');
  await page.keyboard.press('Control+p');
  await page.keyboard.press('Alt+f');
  expect(await page.evaluate(() => window.__shellCalls.filter(call => call.command === 'camera_command').length)).toBe(cameraCalls);
  await page.keyboard.press('c');
  await expect.poll(() => page.evaluate(() => window.__shellCalls.filter(call => call.command === 'molecule_selection_action').map(call => call.args.mode))).toEqual(['center-view']);
  await page.keyboard.press('f');
  await expect.poll(() => page.evaluate(() => window.__shellCalls.filter(call => call.command === 'molecule_selection_action').map(call => call.args.mode))).toEqual(['center-view','fit-view']);
  await page.keyboard.press('Control+Shift+p');
  await expect(page.getByRole('button', {name:'Rect select', exact:true})).toBeHidden();
  await page.getByRole('button', {name:'Show tool palette', exact:true}).click();
  await expect(page.getByRole('button', {name:'Rect select', exact:true})).toBeVisible();
  await page.locator('#output-command').focus();
  await page.keyboard.press('Control+Shift+z');
  await page.evaluate(() => window.dispatchEvent(new KeyboardEvent('keydown', {key:'w',ctrlKey:true,isComposing:true,bubbles:true})));
  expect(await page.evaluate(() => window.__shellCalls.filter(call => call.command === 'redo').length)).toBe(1);
  expect(await page.evaluate(() => window.__shellCalls.filter(call => call.command === 'close_active_scene').length)).toBe(1);
  expect(errors).toEqual([]);
});

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
  await expect(page.locator('#action-history-list')).toBeVisible();
  await expect(page.locator('#action-library-list')).toContainText('Cartoon + ligand');
  await page.getByRole('button', {name: 'Actions', exact: true}).click();
  await expect(page.locator('.side-panel')).not.toBeVisible();
  await page.getByRole('button', {name: 'Explorer', exact: true}).click();
  await expect(page.locator('[data-panel-name="Workspace"]')).toBeVisible();
  expect(errors).toEqual([]);
});

test('Tauri Actions selects ranges, inspects operations and runs on the selected object', async ({page}) => {
  const errors = await openShell(page, {}, {actions:true});
  await page.getByRole('button', {name:'Actions',exact:true}).click();
  const history = page.locator('#action-history-list');
  await history.locator('[data-step-id="7"]').click();
  await history.locator('[data-step-id="8"]').click({modifiers:['Shift']});
  await expect(history.locator('[aria-selected="true"]')).toHaveCount(2);
  await page.getByRole('button', {name:'Save to Actions…',exact:true}).click();
  await page.getByLabel('Action name').fill('Saved view');
  await page.getByRole('button', {name:'Save',exact:true}).click();
  await expect.poll(() => page.evaluate(() => window.__shellCalls.filter(call => call.command === 'actions_request' && call.args.operation === 'save_history').length)).toBe(1);
  const save = await page.evaluate(() => window.__shellCalls.find(call => call.command === 'actions_request' && call.args.operation === 'save_history').args.args);
  expect(save).toEqual({ids:[7,8],name:'Saved view',asMacro:true});
  await history.locator('[data-step-id="7"]').dblclick();
  await expect(page.locator('#inspector-fields')).toContainText('create_renderer');
  await page.locator('#action-library-list').getByRole('treeitem',{name:'Cartoon + ligand',exact:true}).click();
  await page.locator('#action-library-controls').getByRole('button',{name:'Run',exact:true}).click();
  await expect.poll(() => page.evaluate(() => window.__shellCalls.filter(call => call.command === 'actions_request' && call.args.operation === 'run').length)).toBe(1);
  const run = await page.evaluate(() => window.__shellCalls.find(call => call.command === 'actions_request' && call.args.operation === 'run').args.args);
  expect(run).toEqual({file:'Presets',bundled:true,macro:'cartoon-ligand',targetIds:[3]});
  expect(errors).toEqual([]);
});

test('Tauri Actions unlocks after cancellation and retains a queued refresh', async ({page}) => {
  const errors = await openShell(page, {}, {actions:true,actionsAsync:true});
  await page.getByRole('button', {name:'Actions',exact:true}).click();
  const controls = page.locator('#action-library-controls');
  await page.locator('#action-library-list').getByRole('treeitem',{name:'Cartoon + ligand',exact:true}).click();
  await controls.getByRole('button',{name:'Run',exact:true}).click();
  await expect(page.locator('#console-stop')).toBeVisible();
  await controls.getByRole('button',{name:'Refresh',exact:true}).click();
  await expect.poll(() => page.evaluate(() => window.__shellCalls.filter(call => call.command === 'actions_request' && call.args.operation === 'list').length)).toBeGreaterThan(2);
  await page.locator('#console-stop').click();
  await expect(page.locator('#console-stop')).toBeHidden();
  await expect(page.locator('#action-history-controls').getByRole('button',{name:'Undo',exact:true})).toBeEnabled();
  await page.evaluate(() => { window.__delayActionList = true; });
  await controls.getByRole('button',{name:'Refresh',exact:true}).click();
  await expect.poll(() => page.evaluate(() => typeof window.__releaseActionList)).toBe('function');
  await page.evaluate(() => { window.__extraAction = 'Added while refreshing'; });
  await controls.getByRole('button',{name:'Refresh',exact:true}).click();
  await page.evaluate(() => { window.__releaseActionList(); });
  await expect(page.locator('#action-library-list')).toContainText('Added while refreshing');
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
  const rename = await page.evaluate(() => window.__shellCalls.find(call => call.command === 'inspector_request' && call.args.tool === 'set_node_prop'));
  expect(rename.args.args).toEqual({nodeType:'renderer',nodeId:4,prop:'name',value:'Visible atoms',cascadeGroupVisibility:true});
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
  await expect(page.getByLabel('Console command')).toBeVisible();
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
  await expect(page.getByLabel('Console command')).toBeVisible();
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
  await page.getByRole('radio', {name:'JavaScript', exact:true}).click();
  const input = page.getByRole('textbox', {name: 'Console command'});
  await expect(input).toHaveAttribute('spellcheck', 'false');
  await expect(input).toHaveAttribute('autocorrect', 'off');
  await expect(input).toHaveAttribute('autocapitalize', 'off');
  const script = 'print("literal quotes");';
  await input.fill(script);
  await input.press('Enter');
  await expect(input).toHaveValue('');
  expect(await page.evaluate(() => window.__shellCalls.find(call => call.command === 'console_start').args.source)).toBe(script);
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


test('Tauri sequence selects signed integer limits through compact gaps', async ({page}, testInfo) => {
  await page.addInitScript(() => {
    window.__sequenceRulerLabels = [];
    const original = CanvasRenderingContext2D.prototype.fillText;
    CanvasRenderingContext2D.prototype.fillText = function (text, ...args) {
      if (this.canvas.classList.contains('sequence-ruler-canvas')) window.__sequenceRulerLabels.push(String(text));
      return original.call(this, text, ...args);
    };
  });
  const errors = await openShell(page, {}, {sequence: true, sequenceExtremes: true});
  await page.locator('[data-bottom-select="sequence"]').click();
  const row = page.locator('.sequence-chain').first();
  for (const index of ['-2147483648', '-2147483648A', '0', '2147483647', '2147483647B']) {
    const cell = row.locator(`.sequence-cell[data-residue-index="${index}"]`);
    await expect(cell).toBeInViewport();
    await cell.click();
    await expect.poll(() => page.evaluate(() => window.__shellCalls.filter(call => call.command === 'sequence_action').at(-1)?.args.input.targetSelection)).toBe(`'A'.${index}.*`);
  }
  const extent = await page.locator('.sequence-placeholder').evaluate(element => element.scrollWidth);
  expect(extent).toBeLessThan(2000);
  const labels = await page.evaluate(() => window.__sequenceRulerLabels);
  expect(labels).toContain('…');
  expect(labels.filter(label => /^-?\d+$/.test(label)).every(label => Number(label) >= -2147483648 && Number(label) <= 2147483647)).toBe(true);
  await page.screenshot({path: testInfo.outputPath('sequence-integer-limits.png')});
  expect(errors).toEqual([]);
});

test('Tauri sequence virtualizes both axes and ranges include offscreen residues', async ({page}, testInfo) => {
  const errors = await openShell(page, {}, {sequence: true});
  await page.locator('[data-bottom-select="sequence"]').click();
  const firstRow = page.locator('.sequence-chain').first();
  const cell = index => firstRow.locator(`.sequence-cell[data-residue-index="${index}"]`);
  await page.keyboard.down('Shift');
  await cell(-2).click();
  await page.keyboard.up('Shift');
  await expect.poll(() => page.evaluate(() => window.__shellCalls.filter(call => call.command === 'sequence_action').at(-1)?.args.input.action)).toBe('toggle-center');
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


test('Tauri console keeps multiline drafts, IME, completion, dialect and stop controls', async ({page}) => {
  const errors = await openShell(page, {}, {consoleRunning: true});
  await page.locator('[data-bottom-select="output"]').click();
  const input = page.locator('#output-command');
  await input.fill('select Pr');
  await input.press('Tab');
  await expect(input).toHaveValue('select Probe');
  await input.press('Shift+Enter');
  await input.pressSequentially('hide Probe');
  let source = await input.inputValue();
  expect(source).toBe('select Probe\nhide Probe');
  await input.dispatchEvent('compositionstart');
  await input.press('Enter');
  expect(await page.evaluate(() => window.__shellCalls.filter(call => call.command === 'console_start'))).toEqual([]);
  await input.dispatchEvent('compositionend');
  source = await input.inputValue();
  await page.waitForTimeout(60);
  await input.press('Enter');
  await expect(page.locator('#console-stop')).toBeVisible();
  expect(await page.evaluate(() => window.__shellCalls.find(call => call.command === 'console_start').args)).toEqual({source, dialect: 'CueMol'});
  await page.locator('#console-stop').click();
  await expect(page.locator('#console-stop')).toBeHidden();
  await input.fill('unfinished');
  await input.press('ArrowUp');
  await expect(input).toHaveValue(source);
  await input.evaluate(element => element.setSelectionRange(element.value.length, element.value.length));
  await input.press('ArrowDown');
  await expect(input).toHaveValue('unfinished');
  await page.locator('[data-console-dialect="PyMOL"]').click();
  await page.reload();
  await page.locator('[data-bottom-select="output"]').click();
  await expect(page.locator('[data-console-dialect="PyMOL"]')).toHaveAttribute('aria-checked', 'true');
  await input.press('ArrowUp');
  await expect(input).toHaveValue(source);
  expect(errors).toEqual([]);
});


test('Tauri inspector uses typed fields and rejects late results after changing selection', async ({page}) => {
  const errors=await openShell(page);
  await page.locator('.tree-renderer').filter({hasText:'Atoms'}).click();
  const alpha=page.getByLabel('Inspected alpha',{exact:true});
  await expect(alpha).toHaveAttribute('type','number');
  await alpha.fill('0.4'); await alpha.press('Tab');
  await expect(page.getByRole('button',{name:'Reset alpha',exact:true})).toBeEnabled();
  await page.getByRole('button',{name:'Reset alpha',exact:true}).click();
  await expect(alpha).toHaveValue('1');
  await page.getByLabel('Inspected egtype',{exact:true}).selectOption('silhouette');
  await expect(page.getByLabel('Inspected egtype',{exact:true})).toHaveValue('silhouette');
  await page.getByRole('button',{name:'Reset all to default',exact:true}).click();
  await expect(page.getByLabel('Inspected egtype',{exact:true})).toHaveValue('none');
  await page.getByRole('tab',{name:'Generic',exact:true}).click();
  await expect(page.getByLabel('Inspected uid',{exact:true})).toBeDisabled();
  await page.evaluate(()=>{window.__delayInspector=true;});
  await page.locator('.tree-object').filter({hasText:'Probe'}).click();
  await expect.poll(()=>page.evaluate(()=>typeof window.__releaseInspector)).toBe('function');
  await page.locator('.tree-renderer').filter({hasText:'Atoms'}).click();
  await expect(page.getByLabel('Inspected name')).toHaveValue('Atoms');
  await page.evaluate(()=>window.__releaseInspector());
  await expect(page.getByLabel('Inspected name')).toHaveValue('Atoms');
  expect(errors).toEqual([]);
});

test('Tauri selection picker commits on close, cancels drafts, and ignores late validation', async ({page}) => {
  const errors=await openShell(page,{}, {selectionPicker:true});
  await page.locator('.tree-renderer').filter({hasText:'Atoms'}).click();
  const input=page.getByLabel('Inspected sel',{exact:true});
  const popup=page.getByRole('dialog',{name:'Build selection',exact:true});
  const writes=()=>page.evaluate(()=>window.__shellCalls.filter(call=>call.command==='inspector_request'&&call.args.tool==='set_node_prop'&&call.args.args.prop==='sel'));
  await page.getByRole('button',{name:'Build sel',exact:true}).click();
  await page.getByRole('button',{name:'Selection tab Term',exact:true}).click();
  await page.getByLabel('Selection keyword',{exact:true}).selectOption('name');
  await page.getByLabel('Selection value',{exact:true}).fill('C1');
  await page.getByRole('button',{name:'Selection Set',exact:true}).click();
  await expect(input).toHaveValue('name C1');
  expect(await writes()).toHaveLength(0);
  await page.keyboard.press('Escape');
  await expect(popup).not.toBeVisible();
  await expect(input).toHaveValue('*');
  expect(await writes()).toHaveLength(0);
  await page.getByRole('button',{name:'Build sel',exact:true}).click();
  await page.getByRole('button',{name:'Selection tab Term',exact:true}).click();
  await expect(page.getByLabel('Selection keyword',{exact:true})).toHaveValue('name');
  await expect(page.getByLabel('Selection value',{exact:true})).toHaveValue('C1');
  await page.getByRole('button',{name:'Selection tab Named',exact:true}).click();
  await page.getByRole('button',{name:'Selection choice carbon',exact:true}).click();
  await expect(popup).not.toBeVisible();
  await expect(input).toHaveValue('carbon');
  expect((await writes()).map(call=>call.args.args.value)).toEqual(['carbon']);
  await page.getByRole('button',{name:'Build sel',exact:true}).click();
  await input.fill('chain (');
  await page.getByRole('button',{name:'Build sel',exact:true}).click();
  await expect(popup).toBeVisible();
  await expect(popup).toContainText('Invalid selection');
  expect(await writes()).toHaveLength(1);
  await page.keyboard.press('Escape');
  await expect(input).toHaveValue('carbon');
  await page.getByRole('button',{name:'Build sel',exact:true}).click();
  await page.getByRole('button',{name:'Selection tab History',exact:true}).click();
  await page.evaluate(()=>{window.__delaySelection=true;});
  await page.getByRole('button',{name:'Selection choice name C1',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>typeof window.__releaseSelection)).toBe('function');
  await page.locator('#scene-tree button').filter({hasText:'Probe'}).first().click();
  await page.evaluate(()=>window.__releaseSelection());
  await expect(popup).not.toBeVisible();
  expect(await writes()).toHaveLength(1);
  expect(errors).toEqual([]);
});

test('Tauri selection builder preserves queued operations before committing on close', async ({page}) => {
  const errors=await openShell(page,{}, {selectionPicker:true});
  await page.locator('.tree-renderer').filter({hasText:'Atoms'}).click();
  const input=page.getByLabel('Inspected sel',{exact:true});
  const popup=page.getByRole('dialog',{name:'Build selection',exact:true});
  const writes=()=>page.evaluate(()=>window.__shellCalls.filter(call=>call.command==='inspector_request'&&call.args.tool==='set_node_prop'&&call.args.args.prop==='sel'));
  await page.getByRole('button',{name:'Build sel',exact:true}).click();
  await page.getByRole('button',{name:'Selection tab Term',exact:true}).click();
  await page.getByLabel('Selection keyword',{exact:true}).selectOption('name');
  await page.getByLabel('Selection value',{exact:true}).fill('C1');
  await page.evaluate(()=>{
    window.__delayCompose=true;
    document.querySelector('[aria-label="Selection Set"]').click();
  });
  await expect.poll(()=>page.evaluate(()=>typeof window.__releaseCompose)).toBe('function');
  await page.getByLabel('Selection value',{exact:true}).fill('O1');
  await page.getByRole('button',{name:'Selection Add',exact:true}).click();
  await page.mouse.click(600,130);
  await expect(popup).toBeVisible();
  expect(await writes()).toHaveLength(0);
  await page.evaluate(()=>window.__releaseCompose());
  await expect(input).toHaveValue('(name C1) or (name O1)');
  await expect(popup).not.toBeVisible();
  expect((await writes()).map(call=>call.args.args.value)).toEqual(['(name C1) or (name O1)']);
  await page.getByRole('button',{name:'Build sel',exact:true}).click();
  await page.getByRole('button',{name:'Selection tab Term',exact:true}).click();
  await page.evaluate(()=>{
    window.__releaseCompose=undefined;window.__delayCompose=true;
    document.querySelector('[aria-label="Selection Set"]').click();
  });
  await expect.poll(()=>page.evaluate(()=>typeof window.__releaseCompose)).toBe('function');
  await input.fill('name O1');
  await page.mouse.click(600,130);
  await page.evaluate(()=>window.__releaseCompose());
  await expect(input).toHaveValue('name O1');
  await expect(popup).not.toBeVisible();
  expect((await writes()).at(-1).args.args.value).toBe('name O1');
  await page.getByRole('button',{name:'Build sel',exact:true}).click();
  await page.getByRole('button',{name:'Selection tab Term',exact:true}).click();
  await page.evaluate(()=>{
    window.__releaseCompose=undefined;window.__delayCompose=true;
    document.querySelector('[aria-label="Selection Set"]').click();
  });
  await expect.poll(()=>page.evaluate(()=>typeof window.__releaseCompose)).toBe('function');
  await page.keyboard.press('Escape');
  await page.evaluate(()=>window.__releaseCompose());
  await expect(popup).not.toBeVisible();
  await expect(input).toHaveValue('name O1');
  expect(await writes()).toHaveLength(2);
  expect(errors).toEqual([]);
});

test('Tauri Enter ignores empty, failed and superseded selection terms', async ({page}) => {
  const errors=await openShell(page,{}, {selectionPicker:true});
  await page.locator('.tree-renderer').filter({hasText:'Atoms'}).click();
  const input=page.getByLabel('Inspected sel',{exact:true});
  const popup=page.getByRole('dialog',{name:'Build selection',exact:true});
  const writes=()=>page.evaluate(()=>window.__shellCalls.filter(call=>call.command==='inspector_request'&&call.args.tool==='set_node_prop'&&call.args.args.prop==='sel'));
  for(const mode of ['empty','failed','superseded']) {
    await page.getByRole('button',{name:'Build sel',exact:true}).click();
    await page.getByRole('button',{name:'Selection tab Term',exact:true}).click();
    await page.getByLabel('Selection keyword',{exact:true}).selectOption('name');
    await input.fill('');
    const term=page.getByLabel('Selection value',{exact:true});
    await term.fill(mode==='empty'?'':'C1');
    await page.evaluate(mode=>{
      window.__delayCompose=true;window.__composeError=mode==='failed';window.__releaseCompose=undefined;
      document.querySelector('[aria-label="Selection value"]').dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',bubbles:true}));
    },mode);
    await expect.poll(()=>page.evaluate(()=>typeof window.__releaseCompose)).toBe('function');
    if(mode==='superseded') await input.fill('name O1');
    await page.evaluate(async()=>{window.__releaseCompose();await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));});
    await expect(popup).toBeVisible();
    await expect(input).toHaveValue(mode==='superseded'?'name O1':'');
    if(mode==='failed') await expect(popup).toContainText('Composition failed');
    expect(await writes()).toHaveLength(0);
    await page.keyboard.press('Escape');
    await expect(input).toHaveValue('*');
  }
  await page.getByRole('button',{name:'Build sel',exact:true}).click();
  await page.getByRole('button',{name:'Selection tab Term',exact:true}).click();
  await input.fill('');
  const term=page.getByLabel('Selection value',{exact:true});
  await term.fill('C1');
  await term.press('Enter');
  await expect(popup).not.toBeVisible();
  await expect(input).toHaveValue('name C1');
  expect((await writes()).map(call=>call.args.args.value)).toEqual(['name C1']);
  expect(errors).toEqual([]);
});

test('Tauri selection pane separates typed drafts, applied selection and shared history', async ({page}) => {
  const errors=await openShell(page,{}, {selectionPane:true});
  await page.getByRole('button',{name:'Selection',exact:true}).click();
  await page.locator('[data-panel-name="MolStruct"] > .side-title').click();
  await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  const detailRequests=()=>page.evaluate(()=>window.__shellCalls.filter(call=>call.command==='get_molecule_details').length);
  const initialDetails=await detailRequests();
  const panel=page.locator('[data-panel-name="Selection"]');
  const input=panel.getByLabel('Selection expression');
  const edits=()=>page.evaluate(()=>window.__shellCalls.filter(call=>call.command==='inspector_request'&&call.args.tool==='selection_edit').map(call=>call.args.args));
  await expect(input).toHaveValue('*');
  await expect(panel.getByRole('button',{name:'Apply selection',exact:true})).toBeDisabled();
  await input.fill('name O1');
  await page.getByRole('button',{name:'Explorer',exact:true}).click();
  await page.getByRole('button',{name:'Selection',exact:true}).click();
  await expect(input).toHaveValue('name O1');
  await panel.getByRole('button',{name:'Center view on selection',exact:true}).click();
  await expect(input).toHaveValue('name O1');
  expect((await edits()).at(-1)).toMatchObject({action:'center',expression:'*'});
  await input.press('Enter');
  await expect(panel.getByRole('button',{name:'Apply selection',exact:true})).toBeDisabled();
  expect((await edits()).at(-1)).toMatchObject({action:'apply',expression:'name O1',remember:true});
  await panel.getByRole('button',{name:'Selection tab History',exact:true}).click();
  await expect(panel.getByRole('button',{name:'Selection choice name O1',exact:true})).toBeVisible();
  await page.keyboard.press('ControlOrMeta+z');
  await expect(input).toHaveValue('*');
  await expect(panel.getByRole('button',{name:'Selection choice name O1',exact:true})).toBeVisible();
  await input.fill('chain (');
  await input.press('Enter');
  await expect(panel.getByRole('alert')).toContainText('Invalid selection');
  await expect(input).toHaveValue('chain (');
  await panel.getByRole('button',{name:'Clear selection',exact:true}).click();
  await expect(input).toHaveValue('');
  await expect(panel.getByRole('button',{name:'Center view on selection',exact:true})).toBeDisabled();
  await input.fill('name C1');
  await panel.getByRole('button',{name:'Define name',exact:true}).click();
  await panel.getByLabel('Selection definition name').fill('fail');
  await panel.getByRole('button',{name:'Define',exact:true}).click();
  await expect(panel.getByRole('alert')).toContainText('Definition failed');
  await expect(panel.getByLabel('Selection definition name')).toHaveValue('fail');
  await panel.getByLabel('Selection definition name').fill('carbon2');
  await panel.getByRole('button',{name:'Define',exact:true}).click();
  await expect(panel.getByLabel('Selection definition name')).not.toBeVisible();
  await panel.getByRole('button',{name:'Selection tab Named',exact:true}).click();
  await panel.getByRole('button',{name:'Selection choice carbon2',exact:true}).click();
  await expect(input).toHaveValue('carbon2');
  expect((await edits()).at(-1)).toMatchObject({action:'apply',expression:'carbon2',remember:false});
  expect(await detailRequests()).toBe(initialDetails);
  expect(errors).toEqual([]);
});

test('Tauri inline selection builder serializes live edits and cancels stale target drafts', async ({page}) => {
  const errors=await openShell(page,{}, {selectionPane:true});
  await page.getByRole('button',{name:'Selection',exact:true}).click();
  const panel=page.locator('[data-panel-name="Selection"]');
  const input=panel.getByLabel('Selection expression');
  await panel.getByRole('button',{name:'Selection tab Term',exact:true}).click();
  await panel.getByLabel('Selection keyword',{exact:true}).selectOption('name');
  await panel.getByLabel('Selection value',{exact:true}).fill('C1');
  await page.evaluate(()=>{window.__delayCompose=true;document.querySelector('.selection-builder-inline [aria-label="Selection Set"]').click();});
  await expect.poll(()=>page.evaluate(()=>typeof window.__releaseCompose)).toBe('function');
  await panel.getByLabel('Selection value',{exact:true}).fill('O1');
  await panel.getByRole('button',{name:'Selection Add',exact:true}).click();
  await page.evaluate(()=>window.__releaseCompose());
  await expect(input).toHaveValue('(name C1) or (name O1)');
  const edits=()=>page.evaluate(()=>window.__shellCalls.filter(call=>call.command==='inspector_request'&&call.args.tool==='selection_edit').map(call=>call.args.args));
  expect((await edits()).map(edit=>edit.expression)).toEqual(['name C1','(name C1) or (name O1)']);
  expect((await edits()).every(edit=>edit.remember===false)).toBe(true);
  await page.getByRole('button',{name:'Explorer',exact:true}).click();
  await page.getByRole('button',{name:'Selection',exact:true}).click();
  await expect(panel.getByLabel('Selection value',{exact:true})).toHaveValue('O1');
  await page.evaluate(()=>{window.__releaseCompose=undefined;window.__delayCompose=true;document.querySelector('.selection-builder-inline [aria-label="Selection Set"]').click();});
  await expect.poll(()=>page.evaluate(()=>typeof window.__releaseCompose)).toBe('function');
  await input.fill('name manual');
  await page.evaluate(async()=>{window.__releaseCompose();await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));});
  expect(await edits()).toHaveLength(2);
  await expect(input).toHaveValue('name manual');
  await page.evaluate(()=>{window.__selectionCandidateValues=['P1'];snapshot.structureKey='edited';renderShell();});
  await expect(panel.getByLabel('Selection candidates').locator('option')).toHaveText(['▾','P1']);
  await expect(panel.getByLabel('Selection value',{exact:true})).toHaveValue('O1');
  await expect(input).toHaveValue('name manual');
  await page.evaluate(()=>{snapshot.objects.push({...snapshot.objects[0],uid:8,name:'Other',selection:''});renderShell();});
  await panel.locator('#selection-object').selectOption('8');
  await expect(input).toHaveValue('');
  await panel.getByRole('button',{name:'Selection tab Term',exact:true}).click();
  await expect(panel.getByLabel('Selection keyword',{exact:true})).toHaveValue('hierarchical');
  expect(errors).toEqual([]);
});

test('Tauri selection pane refreshes deferred queries after reopening the activity', async ({page}) => {
  const errors=await openShell(page,{}, {selectionPane:true});
  await page.getByRole('button',{name:'Selection',exact:true}).click();
  const panel=page.locator('[data-panel-name="Selection"]');
  await panel.getByRole('button',{name:'Selection tab Term',exact:true}).click();
  await panel.getByLabel('Selection keyword',{exact:true}).selectOption('name');
  await expect(panel.getByLabel('Selection candidates').locator('option')).toHaveText(['▾','C1','O1']);
  await page.evaluate(()=>{
    window.__selectionCandidateValues=['P1'];snapshot.structureKey='hidden-edit';renderShell();
    selectCueMolActivity('explorer',false);
  });
  await page.waitForTimeout(350);
  await page.getByRole('button',{name:'Selection',exact:true}).click();
  await expect(panel.getByLabel('Selection candidates').locator('option')).toHaveText(['▾','P1']);
  await expect(panel.locator('#selection-validation')).toHaveText('1 atoms');
  expect(errors).toEqual([]);
});

test('Tauri selection pane retains text typed while an applied edit response is pending', async ({page}) => {
  const errors=await openShell(page,{}, {selectionPane:true});
  await page.getByRole('button',{name:'Selection',exact:true}).click();
  const panel=page.locator('[data-panel-name="Selection"]');
  const input=panel.getByLabel('Selection expression');
  await input.fill('name C1');
  await page.evaluate(()=>{window.__delaySelectionEdit=true;});
  await input.press('Enter');
  await expect.poll(()=>page.evaluate(()=>typeof window.__releaseSelectionEdit)).toBe('function');
  await input.fill('name O1');
  await page.evaluate(async()=>{window.__releaseSelectionEdit();await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));});
  await expect(input).toHaveValue('name O1');
  await expect(panel.getByRole('button',{name:'Apply selection',exact:true})).toBeEnabled();
  expect(await page.evaluate(()=>snapshot.objects[0].selection)).toBe('name C1');
  await input.press('Enter');
  await expect(panel.getByRole('button',{name:'Apply selection',exact:true})).toBeDisabled();
  expect(await page.evaluate(()=>snapshot.objects[0].selection)).toBe('name O1');
  expect(errors).toEqual([]);
});

test('Tauri selection pane refreshes a completed edit after switching molecules', async ({page}) => {
  const errors=await openShell(page,{}, {selectionPane:true,selectionSecond:true});
  await page.getByRole('button',{name:'Selection',exact:true}).click();
  const panel=page.locator('[data-panel-name="Selection"]');
  const input=panel.getByLabel('Selection expression');
  await input.fill('name C1');
  await page.evaluate(()=>{window.__delaySelectionEdit=true;});
  await input.press('Enter');
  await expect.poll(()=>page.evaluate(()=>typeof window.__releaseSelectionEdit)).toBe('function');
  await panel.locator('#selection-object').selectOption('8');
  await input.fill('name O1');
  await page.evaluate(()=>window.__releaseSelectionEdit());
  await expect.poll(()=>page.evaluate(()=>snapshot.objects.find(object=>object.uid===3).selection)).toBe('name C1');
  await expect(panel.locator('#selection-object')).toHaveValue('8');
  await expect(input).toHaveValue('name O1');
  await panel.locator('#selection-object').selectOption('3');
  await expect(input).toHaveValue('name C1');
  expect(errors).toEqual([]);
});

test('Tauri renderer schema applies ranges, gates, choices and derived atomic writes', async ({page}) => {
  const errors=await openShell(page,{}, {inspectorPages:true});
  await page.locator('.tree-renderer').filter({hasText:'Atoms'}).click();
  const input=key=>page.getByLabel(`Inspected ${key}`,{exact:true});
  await expect(input('detail')).toHaveValue('24');
  await input('detail').selectOption('16');
  await expect(input('detail')).toHaveValue('16');
  await expect(input('thickness')).toBeDisabled();
  await input('ring').check();
  await expect(input('thickness')).toBeEnabled();
  await input('thickness').fill('9'); await input('thickness').press('Tab');
  await expect(input('thickness')).toHaveValue('3');
  await input('alpha').fill('0.35'); await input('alpha').press('Tab');
  await expect(input('alpha')).toHaveValue('0.35');
  await input('alpha').fill('2'); await input('alpha').press('Tab');
  await expect(input('alpha')).toHaveValue('1');
  await input('material').selectOption('Matte');
  await expect(input('material')).toHaveValue('Matte');
  await expect(input('font_name')).toHaveValue('Portable face');
  await expect(input('font_name').locator('option[value="8514oem"]')).toHaveCSS('font-family','"8514oem"');
  await input('font_name').selectOption('8514oem');
  await expect(input('font_name')).toHaveCSS('font-family','"8514oem"');
  await expect(input('font_name').locator('option[value="serif"]')).toHaveCSS('font-family','serif');
  await input('font_name').selectOption('serif');
  await expect(input('font_name')).toHaveCSS('font-family','serif');
  await input('section.width').fill('2'); await input('section.width').press('Tab');
  await expect(input('section.width')).toHaveValue('2');
  await expect(input('section.tuber')).toHaveValue('0.5');
  const batch=await page.evaluate(()=>window.__shellCalls.find(call=>call.command==='inspector_request' && call.args.tool==='set_node_props').args.args.updates);
  expect(batch).toEqual([{prop:'section.width',value:'2'},{prop:'section.tuber',value:'0.25'}]);
  await input('helix_ribbon').selectOption('true');
  await expect(input('helix_ribbon')).toHaveValue('true');
  const flag=await page.evaluate(()=>window.__shellCalls.filter(call=>call.command==='inspector_request' && call.args.args.prop==='helix_ribbon').at(-1).args.args.value);
  expect(flag).toBe('true');
  await page.getByRole('tab',{name:'Generic',exact:true}).click();
  await expect(input('section.tuber')).toHaveValue('0.25');
  await expect(input('font_name')).toHaveAttribute('type','text');
  await expect(input('font_name')).toHaveValue('serif');
  expect(errors).toEqual([]);
});

test('Tauri optional and compound controls retain local state and submit complete edits', async ({page}) => {
  const errors=await openShell(page,{}, {inspectorPages:true});
  await page.locator('.tree-renderer').filter({hasText:'Atoms'}).click();
  const input=key=>page.getByLabel(`Inspected ${key}`,{exact:true});
  const gate=page.getByLabel('Use a separate loop size 2',{exact:true});
  await expect(input('loopsize2')).toBeDisabled();
  await gate.check();
  await expect(input('loopsize2')).toHaveValue('2');
  await input('loopsize2').fill('3.4'); await input('loopsize2').press('Tab');
  await expect(input('loopsize2')).toHaveValue('3.4');
  await gate.uncheck();
  await expect(input('loopsize2')).toBeDisabled();
  await gate.check();
  await expect(input('loopsize2')).toHaveValue('3.4');
  await page.getByRole('button',{name:'Reset loopsize2',exact:true}).click();
  await gate.check();
  await expect(input('loopsize2')).toHaveValue('2');
  await input('dash-pattern').check();
  const writes=await page.evaluate(()=>window.__shellCalls.filter(call=>call.command==='inspector_request' && call.args.tool==='set_node_props').at(-1).args.args.updates);
  expect(writes).toEqual(Array.from({length:6},(_,i)=>({prop:`stipple${i}`,value:i<2?'1':'-1'})));
  await expect(input('stipple2')).toBeEnabled();
  await expect(input('stipple2')).toHaveValue('');
  const writesBefore=await page.evaluate(()=>window.__shellCalls.filter(call=>call.command==='inspector_request' && call.args.tool!=='list_node_props').length);
  await input('stipple2').focus(); await input('stipple2').press('Tab');
  await input('stipple2').fill('8'); await input('stipple2').press('Escape');
  await expect(input('stipple2')).toHaveValue('');
  expect(await page.evaluate(()=>window.__shellCalls.filter(call=>call.command==='inspector_request' && call.args.tool!=='list_node_props').length)).toBe(writesBefore);
  await page.getByRole('tab',{name:'Generic',exact:true}).click();
  await input('stipple2').fill('-2.5'); await input('stipple2').press('Tab');
  await expect(input('stipple2')).toHaveValue('-2.5');
  await page.getByRole('tab',{name:'Properties',exact:true}).click();
  await input('stipple2').fill('8'); await input('stipple2').press('Escape');
  await expect(input('stipple2')).toHaveValue('');
  await page.getByRole('tab',{name:'Generic',exact:true}).click();
  await expect(input('stipple2')).toHaveValue('-2.5');
  await page.getByRole('tab',{name:'Properties',exact:true}).click();
  await input('stipple2').fill('2.75'); await input('stipple2').press('Tab');
  await expect(input('stipple2')).toHaveValue('2.75');
  await input('stipple2').fill(''); await input('stipple2').press('Tab');
  const blank=await page.evaluate(()=>window.__shellCalls.filter(call=>call.command==='inspector_request' && call.args.args.prop==='stipple2').at(-1).args.args.value);
  expect(blank).toBe('-1');
  await expect(input('bndry_molname')).toBeDisabled();
  await input('limit-display').check();
  await expect(input('bndry_molname')).toBeEnabled();
  await input('limit-display').uncheck();
  await expect(input('bndry_molname')).toBeDisabled();
  await input('alpha').fill('0.7'); await input('alpha').press('Tab');
  await expect(input('alpha')).toHaveValue('0.7');
  await expect(input('bndry_molname')).toBeDisabled();
  await expect(input('center-update')).toHaveValue('Automatic (drag)');
  const resetCenter=page.getByRole('button',{name:'Reset center-update',exact:true});
  await expect(resetCenter).toBeEnabled();
  await resetCenter.click();
  await expect(input('center-update')).toHaveValue('Automatic');
  await expect(resetCenter).toBeDisabled();
  const reset=await page.evaluate(()=>window.__shellCalls.filter(call=>call.command==='inspector_request' && call.args.tool==='reset_node_props').at(-1).args.args.props);
  expect(reset).toEqual(['autoupdate','dragupdate']);
  await page.getByRole('tab',{name:'Generic',exact:true}).click();
  await expect(input('dash-pattern')).toHaveCount(0);
  expect(errors).toEqual([]);
});

test('Tauri Scene and View inspectors share presets and refresh after camera input', async ({page}) => {
  const errors=await openShell(page);
  await page.locator('.menu').filter({has:page.locator('[data-dialog="Scene Properties"]')}).locator('summary').click();
  await page.locator('[data-dialog="Scene Properties"]').click();
  const name=page.getByLabel('Inspected name',{exact:true});
  await expect(name).toBeEnabled();
  await name.fill('Scene from Inspector'); await name.press('Tab');
  await expect(name).toHaveValue('Scene from Inspector');
  await page.getByLabel('Inspected aoEnabled',{exact:true}).check();
  await page.getByLabel('Inspected ao-preset',{exact:true}).selectOption('High');
  await expect.poll(()=>page.evaluate(()=>window.__shellCalls.filter(call=>call.command==='inspector_request' && call.args.tool==='set_node_props').length)).toBe(1);
  const batch=await page.evaluate(()=>window.__shellCalls.find(call=>call.command==='inspector_request' && call.args.tool==='set_node_props').args.args);
  expect(batch.nodeType).toBe('scene');
  expect(batch.updates).toEqual([{prop:'aoRadius',value:'12'},{prop:'aoSteps',value:'5'},{prop:'aoIntensity',value:'1.7'},{prop:'aoHalfRes',value:'true'}]);
  await page.getByRole('tab',{name:'Generic',exact:true}).click();
  await expect(name).toBeDisabled();
  await page.locator('.menu').filter({has:page.locator('[data-dialog="View Property"]')}).locator('summary').click();
  await page.locator('[data-dialog="View Property"]').click();
  const zoom=page.getByLabel('Inspected zoom',{exact:true});
  await expect(zoom).toHaveValue('10');
  const rect=await page.locator('#viewport').boundingBox();
  await page.mouse.move(rect.x+rect.width/2,rect.y+rect.height/2);
  await page.mouse.wheel(0,160);
  await expect(zoom).not.toHaveValue('10');
  await name.fill('Pending view');
  const queries=await page.evaluate(()=>window.__shellCalls.filter(call=>call.command==='inspector_request' && call.args.tool==='list_node_props').length);
  await page.mouse.wheel(0,160);
  await expect.poll(()=>page.evaluate(()=>window.__shellCalls.filter(call=>call.command==='inspector_request' && call.args.tool==='list_node_props').length)).toBeGreaterThan(queries);
  await expect(name).toHaveValue('Pending view');
  await page.evaluate(()=>window.__TAURI__.core.invoke('inspector_request',{tool:'set_node_prop',args:{nodeType:'view',nodeId:2,prop:'name',value:'External view'}}));
  await page.mouse.wheel(0,160);
  await expect(name).toHaveValue('External view');
  await page.locator('.tree-object').filter({hasText:'Probe'}).click();
  await expect(name).toHaveValue('Probe');
  expect(errors).toEqual([]);
});
