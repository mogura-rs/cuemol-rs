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
      uid: 3, className:'MolCoord', sceneUid: 1, name: 'Probe', source: '', sourceType: 'pdb', uiOrder: 0,
      uiCollapsed: false, selection: options.selectionExpression || '*', visible: true, locked: false,
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
    if(options.apbsDialogs) {
      snapshot.objects.push({...molecule,uid:5,name:'Second',renderers:[]});
      window.__apbsSettings={apbsExecutable:'tools/apbs',pdb2pqrExecutable:'tools/pdb2pqr',forceField:'amber'};
      window.__apbsJob=null;
      window.__apbsResult=null;
      window.__apbsSceneChanged=()=>{snapshot.activeSceneUid=9;};
    }
    if(options.morphDialogs) {
      snapshot.objects.push({...molecule,uid:5,name:'Source',renderers:[]});
      snapshot.objects.push({...molecule,uid:7,name:'Existing morph',className:'MorphMol',renderers:[]});
      snapshot.morphs.push({objectUid:7,frame:0,frames:[{index:0,name:'(this)',source:'',sourceType:'<this>'}]});
      window.__morphSnapshot=()=>structuredClone(snapshot);
      window.__morphSceneChanged=()=>{snapshot.activeSceneUid=9;};
    }
    if(options.cameras) snapshot.cameras=['front','__current','back'].map(name=>({sceneUid:1,name,source:name==='front'?'front.cam':'',visibilityCount:name==='front'?2:0}));
    if (options.density) {
      scene.mapCount=1;
      const specific={color:'blue',colormode:'solid',use_abslevel:'false',siglevel:'1.1',siglevel_effective:'1.1',level:'0.183',ui_level_min:'-10',ui_level_max:'10',ui_abslevel_min:'0',ui_abslevel_max:'1',extent:'15',maxExtent:'50',map_type_resolved:'xtal',region_mode_resolved:'box'};
      if(options.density==='em')Object.assign(specific,{map_type_resolved:'em',region_mode_resolved:'full',siglevel:'20',siglevel_effective:'20',ui_level_min:'0',ui_level_max:'100'});
      const densityRenderer={...renderer,uid:11,name:'Contour',rendererType:'contour',specific};
      snapshot.densityMaps=[{uid:10,sceneUid:1,name:'Density',source:'density.dx',sourceType:'dx',className:'ElePotMap',uiOrder:1,uiCollapsed:false,visible:true,locked:false,counts:[9,9,9],starts:[0,0,0],intervals:[9,9,9],origin:[0,0,0],spacing:[1,1,1],minimum:0,maximum:1,mean:0.1,rmsd:0.16,renderers:[densityRenderer,{...densityRenderer,uid:12,name:'Other',specific:{...specific},alpha:0.5}]}];
    }
    if(options.symmetry && snapshot.densityMaps[0])snapshot.densityMaps[0].crystal={objectUid:10,a:8,b:8,c:8,alpha:90,beta:90,gamma:90,lattice:'TRICLINIC',spaceGroupName:'P 1',spaceGroupNumber:1};
    if(options.brokenAnimation) {
      snapshot.animationResolveError='"child" is relative to "missing", which does not exist';
      snapshot.animationObjects=[{uid:20,index:0,relativeStart:0,timeRefName:'missing',disabled:false,quadric:0,name:'child',typeName:'NoopAnimObj',start:0,end:1,duration:1,resolveError:snapshot.animationResolveError}];
      snapshot.animationDuration=1;
    }
    if(options.interactions) {
      snapshot.objects.push({...molecule,uid:5,name:'Other',renderers:[]});
      window.__interactionSceneChanged=()=>{snapshot.activeSceneUid=9;};
    }
    if(options.surfaceDialogs) {
      snapshot.surfaces=[{uid:10,sceneUid:1,name:'sf_Probe',className:'MolSurfObj',visible:true,locked:false,uiOrder:1,uiCollapsed:false,source:'',format:'qdfsurf',originalMolecule:'Probe',originalSelection:'chain A',originalDensity:20,originalProbeRadius:1.8,renderers:[]}];
    }
    const animationUndo=[],animationRedo=[];
    const resolveAnimation=()=>{
      const ends=new Map();
      const resolve=object=>{if(ends.has(object.uid))return ends.get(object.uid);const parent=snapshot.animationObjects.find(item=>item.name===object.timeRefName);const base=parent?resolve(parent):0;object.start=base+object.relativeStart;object.end=object.start+object.duration;ends.set(object.uid,object.end);return object.end;};
      snapshot.animationObjects.forEach((object,index)=>{object.index=index;resolve(object);});snapshot.animationDuration=Math.max(0,...snapshot.animationObjects.map(object=>object.end));
    };
    if(options.animationTimeline){
      snapshot.animationObjects=['base','child','last'].map((name,index)=>({uid:20+index,index,name,typeName:'SimpleSpin',disabled:false,timeRefName:index?['base','child'][index-1]:'',relativeStart:0,duration:1,quadric:0,angle:360,axis:[0,1,0],modifiedDefaults:['angle'],resolveError:null}));
      if(options.animationRealProperty)Object.assign(snapshot.animationObjects[0],{typeName:'RealPropAnim',startValue:-2,endValue:3,prop:'alpha',rend:'target',modifiedDefaults:[]});
      resolveAnimation();
      window.__animationState=()=>structuredClone(snapshot.animationObjects);
      window.__reverseAnimations=()=>{snapshot.animationObjects.reverse();resolveAnimation();};
    }
    const styleDocuments = new Map();
    if (options.styles) {
      snapshot.styles = [
        {styleId:101,sceneUid:0,name:'user',localIndex:null},
        {styleId:102,sceneUid:1,name:'user',localIndex:0},
        {styleId:103,sceneUid:0,name:'(anonymous)',localIndex:null},
        {styleId:104,sceneUid:0,name:'(anonymous)',localIndex:null},
      ].map(style=>({ownerSceneUid:1,readOnly:false,modified:false,source:'',...style}));
      if(options.styleMultiple)snapshot.styles.push({ownerSceneUid:1,sceneUid:1,styleId:105,name:'second',localIndex:1,readOnly:false,modified:false,source:''});
      for (const style of snapshot.styles) {
        const name=style.styleId===103?'':style.name;
        styleDocuments.set(style.styleId,`<styles id="${name}"><color id="accent" value="#123456"/><sel id="focus" value="name C1"/><style id="KeepCPK" type="renderer"/></styles>`);
      }
      snapshot.styles[0].source='user.xml';
      window.__insertStyle=()=>{
        const styleId=Math.max(...snapshot.styles.map(style=>style.styleId))+1;
        for(const style of snapshot.styles)if(style.localIndex!==null)style.localIndex+=1;
        styleDocuments.set(styleId,'<styles id="inserted"/>');
        snapshot.styles.splice(0,0,{ownerSceneUid:1,sceneUid:1,styleId,name:'inserted',localIndex:0,readOnly:false,modified:false,source:''});
      };
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
    window.__switchView = () => { snapshot.views[0].uid += 10; };
    let preferences = {
      dropTarget: options.files ? 'new' : 'active', shellTarget:'active',
      atomLabelFont:'sans-serif',atomLabelSize:12,atomLabelColor:'yellow',
      atomLabelItalic:false,atomLabelBold:false,uiLanguage:'en-US',useHidpi:false,
      mousePreset:'DefaultViewInConf',trackballRadius:0.8,hitPrecision:10,
      momentumScroll:false,multitouchPad:false,emulateRightButton:false,
      plugins:options.plugins || {},mcpAcceptConnections:true,mcpPort:0,
      darkMode:true,updateCheck:true,consoleRemote:false,tools:{},
      gpuPicking:true,hoverInfo:true,hoverHighlight:true,
      shortcuts:[{id:'view-trans-plusx',key:'ArrowRight',ctrl:true,alt:false,shift:false},{id:'view-trans-minusx',key:'ArrowLeft',ctrl:true,alt:false,shift:false}],
    };
    const agent = {running:false,message:'Ready',messages:[],models:[],refreshingModels:false,settings:{model:'openai:test-model',reasoning:'low',enter_sends:false,endpoint:'',enabled:true}};
    const events = new Map();
    window.__finishAgent=()=>{agent.running=false;agent.message='Complete';agent.messages.push({role:'Tool',text:'get_scene_state {}',image:null,tool:{name:'get_scene_state',input:'{}',result:'{\"objects\":[]}',ok:true}},{role:'Assistant',text:'<img src=x onerror=alert(1)> kept as text',image:null});events.get('cuemol-agent-changed')?.();};
    window.__shellCalls = [];
    if (options.nativeRenderingWindow) window.__TAURI_INTERNALS__ = {};
    window.__actionsRunning = false;
    window.__TAURI__ = {core: {invoke: async (command, args) => {
      window.__shellCalls.push({command, args});
      if (command === 'render_window_request') return;
      if (command === 'prepare_application_close') {
        if(window.__closePreparationError)throw new Error('Pending render settings could not be saved');
        if(options.closeGuard)snapshot.scenes[0].modified=true;
        return structuredClone(snapshot);
      }
      if (options.updates && command === 'update_request') {
        const checking=args.operation==='check';
        return {currentVersion:'0.1.0',message:checking?'Checking for updates…':'CueMol 0.2.0 is available.',checking,installing:false,release:checking?null:{version:'0.2.0',url:'https://github.com/example/cuemol/releases/tag/v0.2.0',newer:true}};
      }
      if (options.apbsDialogs && command === 'get_apbs_dialog_state') {
        return {...window.__apbsSettings,job:window.__apbsJob};
      }
      if (options.apbsDialogs && command === 'start_apbs_calculation') {
        if(args.input.selection==='invalid (((') throw new Error('Invalid selection');
        if(window.__apbsHoldStart) await new Promise(resolve=>{window.__apbsReleaseStart=resolve;});
        window.__apbsJob={requestId:args.input.requestId,status:'Running PDB2PQR...',log:'Starting calculation',stopping:false};
        return 'running';
      }
      if (options.apbsDialogs && command === 'cancel_apbs_calculation') {
        if(window.__apbsJob?.requestId!==args.requestId)return 'stale';
        window.__apbsJob.stopping=true;window.__apbsResult='error:APBS calculation cancelled';return 'stopping';
      }
      if (options.apbsDialogs && command === 'poll_apbs_calculation') {
        if(window.__apbsJob?.requestId!==args.requestId)return 'stale';
        if(!window.__apbsResult)return 'running';
        const result=window.__apbsResult;window.__apbsResult=null;window.__apbsJob=null;
        return result;
      }
      if(options.morphDialogs && ['initialize_morph_object','append_morph_frame','insert_morph_frame_from_file','remove_morph_frame'].includes(command)) {
        if(args.sceneUid!==snapshot.activeSceneUid)throw new Error('The active scene changed');
        const object=snapshot.objects.find(o=>o.uid===(args.objectUid??args.targetObjectUid));
        if(!object)throw new Error('Missing target');
        let morph=snapshot.morphs.find(m=>m.objectUid===object.uid);
        if(command==='initialize_morph_object') {
          object.className='MorphMol';
          if(!morph)snapshot.morphs.push({objectUid:object.uid,frame:0,frames:[{index:0,name:'(this)',source:'',sourceType:'<this>'}]});
        } else {
          if(!morph)throw new Error('Convert first');
          if(command==='remove_morph_frame') {
            if(morph.frames[args.index]?.sourceType==='<this>')throw new Error('Cannot remove base frame');
            morph.frames.splice(args.index,1);
          } else {
            if(args.path?.includes('bad'))throw new Error('Invalid PDB frame');
            const source=snapshot.objects.find(o=>o.uid===args.sourceObjectUid);
            const name=args.frameName||(source?source.name:args.path.split(/[\\/]/).pop());
            morph.frames.splice(args.insertBefore??morph.frames.length,0,{index:0,name,source:args.path||`object:${source.uid}`,sourceType:args.path?'pdb':'molecule'});
          }
          morph.frames.forEach((frame,index)=>{frame.index=index;});
        }
        snapshot.canUndo=true;return snapshot;
      }
      if (options.agent && command === 'agent_request') {
        const {operation, args:input} = args;
        if(operation==='configure') agent.settings=structuredClone(input.settings);
        if(operation==='models') agent.models=['openai:test-model','openai:second-model'];
        if(operation==='start') {agent.running=true;agent.message='Thinking…';agent.messages.push({role:'You',text:input.prompt,image:null});}
        if(operation==='cancel') {agent.running=false;agent.message='Cancelled; completed edits are kept.';}
        if(operation==='clear') {agent.messages=[];agent.message='';}
        return {agent};
      }
      if (command === 'get_initial_state') return {snapshot, viewport, preferences: options.picking ? structuredClone(preferences) : options.files ? {dropTarget:'new',plugins:preferences.plugins} : {plugins:preferences.plugins}, startupPathsPending: !!options.startupPaths?.length};
      if (command === 'take_initial_paths') return options.startupPaths || [];
      if (command === 'open_initial_paths') return {snapshot,viewport};
      if (command === 'get_preferences') return structuredClone(preferences);
      if (command === 'set_preferences') { preferences=structuredClone(args.preferences);return {preferences,status:'Options updated'}; }
      if (command === 'get_snapshot') return snapshot;
      if (options.moleculeEdits && ['delete_atoms','change_chain','renumber_residues','merge_molecules'].includes(command)) {
        if (args.selection === 'invalid (((') throw new Error('Invalid selection');
        snapshot.status = args.selection === 'none' && command !== 'change_chain' ? 'No atoms matched the selection' : 'Edited molecule';
        if (command === 'renumber_residues' && !args.shiftMode && args.value === 1) snapshot.status = 'Residue index change was rejected';
        return structuredClone(snapshot);
      }
      if (options.surfaceDialogs && ['create_molecular_surface','regenerate_surface','cut_surface'].includes(command)) {
        if(args.selection==='invalid (((') throw new Error('Invalid selection');
        if(command==='regenerate_surface' && window.__surfaceFailure) throw new Error('Surface generation failed');
        return structuredClone(snapshot);
      }
      if (options.superposition && command === 'superpose_objects') {
        if (args.referenceSelection === 'invalid (((') throw new Error('Invalid selection');
        snapshot.status = 'Molecular superposition applied';
        return structuredClone(snapshot);
      }
      if (options.secondary && command === 'assign_secondary_structure') {
        if (args.selection === 'invalid (((') throw new Error('Invalid selection');
        snapshot.status = 'Secondary structure assigned';
        return structuredClone(snapshot);
      }
      if (options.interactions && command === 'calculate_contacts') {
        if(args.selection1 === 'invalid (((') throw new Error('Invalid selection');
        snapshot.status=args.selection1 === 'none' ? 'No interaction was found' : 'Defined 1 interaction(s) in measure';
        return structuredClone(snapshot);
      }
      if ((options.rendererHistory || options.rendererSetup) && command === 'get_workspace_renderer_setup_options') {
        if (![3,10].includes(args.objectUid)) throw new Error('Renderer history needs the selected object class');
        const result = args.kind === 'densityRenderer'
          ? {rendererType:'isosurf',recenter:true,options:['contour','isosurf'].map(name=>({name,description:name,preset:false}))}
          : {rendererType:'ribbon',recenter:true,options:[...['simple','cpk','ribbon'].map(name=>({name,description:name,preset:false})),{name:'Default1RendPreset',description:'Default preset',preset:true}]};
        if (window.__holdRendererOptions) return new Promise(resolve => { window.__releaseRendererOptions = () => resolve(result); });
        return result;
      }
      if (options.rendererSetup && command === 'create_workspace_renderer') {
        const input = args.input;
        if (input.selection === 'invalid (((') throw new Error('Invalid selection');
        const objects = input.kind === 'densityRenderer' ? snapshot.densityMaps : snapshot.objects;
        const object = objects.find(item => item.uid === input.objectUid);
        object.renderers.push({...renderer,uid:40+object.renderers.length,name:input.rendererName,rendererType:input.rendererType,
          group:input.groupName,selection:input.selection || '*'});
        return structuredClone(snapshot);
      }
      if(options.fileDefaults && command==='get_file_renderer_setup')return {
        targetKind:'molecule',objectName:args.path.replace(/\.[^.]+$/,''),rendererType:'auto',rendererName:'auto1',recenter:true,
        options:[{name:'auto',description:'Automatic representations',preset:false},{name:'cpk',description:'CPK',preset:false}],
      };
      if(options.fileDefaults && command==='open_path_with_renderer_setup') {
        snapshot.openedPaths=[args.path];snapshot.status='Loaded '+args.path;return {snapshot,viewport};
      }
      if(options.files && command==='get_file_renderer_setup')return null;
      if(options.files && command==='open_paths') {
        const context=args.openContext;
        if(context?.sceneId == null && context?.target==='new') {
          snapshot.scenes.forEach(scene=>scene.active=false);
          snapshot.scenes.push({uid:7,name:'Dropped files',active:true,modified:true,objectCount:0,mapCount:0});
        }
        const active=snapshot.scenes.find(scene=>scene.active);
        active.objectCount++;
        snapshot.activeSceneUid=active.uid;
        snapshot.openedPaths=args.paths;
        return {snapshot,viewport};
      }

      if(options.cameras && command==='create_camera') {
        const name=args.name.trim();
        if(!snapshot.cameras.some(camera=>camera.name===name))snapshot.cameras.push({sceneUid:1,name,source:'',visibilityCount:0});
        return structuredClone(snapshot);
      }
      if(options.cameras && command==='inspector_request' && ['save_camera','apply_camera','delete_camera','rename_camera','move_camera','clear_camera_visibility'].includes(args.tool)) {
        const request=args.args;
        if(request.sceneId!==snapshot.activeSceneUid)throw new Error('Scene changed');
        const camera=snapshot.cameras.find(camera=>camera.name===request.name);
        if(!camera)throw new Error('Camera not found');
        if(args.tool==='save_camera')camera.visibilityCount=request.withVisibility?2:0;
        if(args.tool==='clear_camera_visibility')camera.visibilityCount=0;
        if(args.tool==='rename_camera')camera.name=request.newName;
        if(args.tool==='delete_camera')snapshot.cameras=snapshot.cameras.filter(item=>item!==camera);
        if(args.tool==='move_camera'){
          const cameras=snapshot.cameras.slice().sort((a,b)=>Number(b.name==='__current')-Number(a.name==='__current'));
          const from=cameras.indexOf(camera);cameras.splice(from,1);cameras.splice(Math.max(1,request.to-1),0,camera);snapshot.cameras=cameras;
        }
        return {snapshot:structuredClone(snapshot)};
      }

      if (command === 'get_application_info') return {
        productName: 'CueMol3', version: 'test-version', target: 'test-target', features: [], plugins: [], config: [],
        aboutRows: [['Version', 'test-version'], ['Commit', 'abc123'], ['Date', '2026-11-01T00:00:00Z'], ['Rust', 'rustc test'], ['wgpu', 'test-wgpu'], ['Tauri', 'test-tauri'], ['OS', 'test-os']],
      };
      if (options.styles && command === 'copy_workspace_style') {
        if(window.__holdStyleCopy)await window.__holdStyleCopy;
        const style=styleFor(args.input);
        window.__copiedStyleXml=styleDocuments.get(style.styleId);
        return `Copied style ${style.name}`;
      }
      if (options.styles && command === 'delete_workspace_items') {
        const styles=args.targets.map(target=>styleFor({ownerSceneUid:target.sceneUid,sceneUid:target.sceneUid,styleId:target.styleId}));
        snapshot.styles=snapshot.styles.filter(style=>!styles.includes(style));
        return structuredClone(snapshot);
      }
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
        window.__savedStyleXml=styleDocuments.get(style.styleId);
        if(style.source!==args.path)style.modified=false;
        style.source=args.path;
        return structuredClone(snapshot);
      }
      if (command === 'render_viewport' || command === 'fit_workspace_item' || command === 'fit_workspace_items') return viewport;
      if (command === 'update_camera') {
        if(options.delayCamera && !window.__cameraWasDelayed){window.__cameraWasDelayed=true;await new Promise(resolve=>window.__releaseCamera=resolve);}
        if (args.action === 'zoom') viewport.zoom *= args.x;
        if (args.action.startsWith('set-center-')) viewport.center['xyz'.indexOf(args.action.at(-1))]=args.x;
        if (args.action.startsWith('translate-')) viewport.center['xyz'.indexOf(args.action.at(-1))]+=args.x;
        for(const field of ['zoom','slab','distance']) {
          if(args.action===`set-${field}`)viewport[field]=args.x;
          if(args.action===`adjust-${field}`)viewport[field]+=args.x;
        }
        if(args.action==='projection')viewport.perspective=!viewport.perspective;
        if(args.action.startsWith('center-mark-'))viewport.centerMark=args.action.slice('center-mark-'.length);
        return {...viewport, width: args.width, height: args.height};
      }
      if(options.symmetry && command==='get_space_groups') {
        if(options.symmetryDelayed && args.lattice==='MONOCLINIC')await new Promise(resolve=>window.__releaseSpaceGroups=resolve);
        return JSON.stringify(({TRICLINIC:[{id:1,cname:'P 1'}],MONOCLINIC:[{id:3,cname:'P 2'}],CUBIC:[{id:195,cname:'P 2 3'}],HEXAGONAL:[{id:168,cname:'P 6'}],TRIGONAL:[{id:146,cname:'R 3'}]})[args.lattice]||[]);
      }
      if(options.symmetry && command==='change_crystal_info') {
        const crystal={...args,lattice:args.spaceGroupNumber===195?'CUBIC':'TRICLINIC',spaceGroupName:args.spaceGroupNumber===195?'P 2 3':'P 1'};
        snapshot.crystals=snapshot.crystals.filter(item=>item.objectUid!==args.objectUid);snapshot.crystals.push(crystal);
        snapshot.canUndo=true;return structuredClone(snapshot);
      }
      if(options.symmetry && command==='show_symmetry_molecules')return structuredClone(snapshot);
      if (command === 'pick_atom' || command === 'select_screen_rect') return {snapshot, viewport};
      if (command === 'hover_at_screen') return {...viewport, image: null};
      if (command === 'get_molecule_details') return {structureKey: 'initial', sequenceKey: snapshot.sequenceKey, structureObjectUid: 3, structure: [], sequence, sequenceLayout};
      if (command === 'sequence_action') {
        snapshot.sequenceKey = `sequence-${window.__shellCalls.length}`;
        return {snapshot: structuredClone(snapshot), viewport};
      }
      if (command === 'get_pdb_history') return [];
      if (options.pdbSearch && command === 'remember_pdb_history') return [];
      if (options.pdbSearch && command === 'get_pdb_info') {
        const q=args.query;
        const entry=(id,em=false)=>({id,title:`Structure ${id}`,methods:[em?'ELECTRON CRYSTALLOGRAPHY':'X-RAY DIFFRACTION'],resolution:2.1,emdbIds:em?['EMD-11638']:[],pdbFormatAvailable:!em,kind:em?'em':'xtal'});
        if(q.kind==='search') {
          if(q.query==='delayed')await new Promise(resolve=>window.__releasePdbSearch=resolve);
          return {kind:'search',total:30,entries:q.start===0?[entry('1crn'),entry('7a4m',true)]:[entry('1mbn')]};
        }
        if(q.kind==='entries') {
          if(q.pdbIds[0]==='9zzz')return {kind:'entries',entries:[]};
          if(q.pdbIds[0]==='1err')throw new Error('metadata offline');
          return {kind:'entries',entries:[entry(q.pdbIds[0])]};
        }
        return {kind:'emdb',info:{emdbId:q.emdbId,title:'Test EMDB map',recommendedLevel:20,sizeBytes:4096,dims:[4,4,4]}};
      }
      if(options.pdbSearch&&command==='prefetch_pdb')return null;
      if(options.pdbSearch&&command==='get_pdb_download_progress')return 4096;
      if(options.pdbSearch&&command==='open_pdb')return {snapshot,viewport};

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
      if (options.rewrite && command === 'molecule_selection_action') {
        molecule.selection=args.selection;
        return {snapshot:structuredClone(snapshot),viewport};
      }
      if (options.shortcuts && command === 'molecule_selection_action') return {snapshot:structuredClone(snapshot),viewport};
      if (command === 'console_start' || command === 'console_stop' || command === 'console_poll') {
        if (command === 'console_stop') window.__actionsRunning = false;
        return {snapshot: structuredClone(snapshot), running: window.__actionsRunning || command !== 'console_stop' && Boolean(options.consoleRunning)};
      }
      if (command === 'console_complete') return [{display: 'Probe', replacement: 'select Probe'}];
      if(options.density && command==='show_unit_cell') {
        snapshot.densityMaps[0].renderers.push({...renderer,uid:13,name:'Cell',rendererType:'*unitcell'});
        return structuredClone(snapshot);
      }
      if(options.density && command==='redraw_density_renderer') return structuredClone(snapshot);
      if(options.animationTimeline && command==='seek_animation'){snapshot.animationElapsed=Math.max(0,Math.min(snapshot.animationDuration,args.elapsed));return structuredClone(snapshot);}
      if(options.animationTimeline && ['undo','redo'].includes(command)){
        const from=command==='undo'?animationUndo:animationRedo,to=command==='undo'?animationRedo:animationUndo;
        if(from.length){to.push(structuredClone(snapshot.animationObjects));snapshot.animationObjects=from.pop();resolveAnimation();}snapshot.canUndo=animationUndo.length>0;snapshot.canRedo=animationRedo.length>0;return structuredClone(snapshot);
      }
      if (command === 'inspector_request') {
        if((options.animationTimeline||options.brokenAnimation) && (args.tool.includes('anim')||args.tool==='animation_selection')) {
          const request=args.args,uid=request.uid??Number(request.element?.slice(1));
          const object=snapshot.animationObjects.find(object=>object.uid===uid);
          const defaultNames=object?.typeName==='SimpleSpin'?['disabled','quadric','angle','axis']:['disabled','quadric'];
          const markDefault=(prop,isDefault)=>{if(!defaultNames.includes(prop))return;const changed=new Set(object.modifiedDefaults||[]);if(isDefault)changed.delete(prop);else changed.add(prop);object.modifiedDefaults=[...changed];};
          if(args.tool==='animation_element_detail') {
            const properties={uid:String(uid),name:object.name,disabled:String(object.disabled),timeRefName:object.timeRefName,start:String(object.relativeStart),end:String(object.relativeStart+object.duration),absStart:String(object.start),absEnd:String(object.end),quadric:String(object.quadric)};
            if(object.typeName==='SimpleSpin')Object.assign(properties,{angle:String(object.angle),axis:`(${(object.axis??[0,1,0]).join(',')})`});
            if(object.typeName==='RealPropAnim')Object.assign(properties,{startValue:String(object.startValue),endValue:String(object.endValue),prop:object.prop,rend:object.rend});
            const metadata=Object.fromEntries(Object.keys(properties).map(prop=>[prop,{type:prop==='disabled'?'boolean':['quadric','angle','startValue','endValue'].includes(prop)?'real':prop==='uid'?'integer':prop==='axis'?'object<Vector>':['start','end','absStart','absEnd'].includes(prop)?'object<TimeValue>':'string',readonly:['uid','start','end','absStart','absEnd'].includes(prop),hasDefault:defaultNames.includes(prop),isDefault:defaultNames.includes(prop)&&!(object.modifiedDefaults||[]).includes(prop)}]));
            return {sceneId:1,uid,type:object.typeName,startMs:object.relativeStart*1000,endMs:(object.relativeStart+object.duration)*1000,properties,metadata,references:snapshot.animationObjects.map(item=>({name:item.name,error:item.uid===uid?'Self reference':null})),cameras:[],renderers:[],molecules:[]};
          }
          const record=()=>{animationUndo.push(structuredClone(snapshot.animationObjects));animationRedo.splice(0);snapshot.canUndo=true;snapshot.canRedo=false;};
          const timing=value=>{object.relativeStart=value.startMs/1000;object.duration=(value.endMs-value.startMs)/1000;};
          if(args.tool==='edit_animation_property') {
            if(request.mode==='preview'&&window.__holdAnimationPreview)await window.__holdAnimationPreview;
            const write=(value,isDefault=false)=>{if(request.prop==='timing')timing(value);else {object[request.prop]=Number(value);markDefault(request.prop,isDefault);}};
            if(request.mode==='commit'){write(request.originalValue,request.originalWasDefault);if(JSON.stringify(request.value)!==JSON.stringify(request.originalValue)){record();write(request.value);}}
            else if(request.mode==='abort')write(request.originalValue,request.originalWasDefault);else write(request.value);
          } else if(args.tool==='add_anim'){record();const before=snapshot.animationObjects.findIndex(object=>`#${object.uid}`===request.before),index=before<0?snapshot.animationObjects.length:before,previous=snapshot.animationObjects[index-1];snapshot.animationObjects.splice(index,0,{uid:Math.max(0,...snapshot.animationObjects.map(object=>object.uid))+1,index,name:'SimpleSpin0',typeName:'SimpleSpin',disabled:false,timeRefName:previous?.name||'',relativeStart:0,duration:1,quadric:0,angle:360,axis:[0,1,0],modifiedDefaults:['angle'],resolveError:null});}
          else if(args.tool==='set_anim_time'){record();timing(request);}
          else if(args.tool==='set_anim_generic_prop'||args.tool==='set_anim_prop'){
            record();markDefault(request.prop,false);
            if(request.prop==='name'){const before=object.name;for(const child of snapshot.animationObjects)if(child.timeRefName===before)child.timeRefName=request.value;object.name=request.value;}
            else if(request.prop==='axis'){const axis=request.value.replace(/[()]/g,'').split(/[,\s]+/).map(Number),norm=Math.hypot(...axis);object.axis=axis.map(value=>value/norm);}
            else object[request.prop]=['quadric','angle','startValue','endValue'].includes(request.prop)?Number(request.value):request.prop==='disabled'?request.value==='true':request.value;
          }
          else if(args.tool==='reset_anim_prop'){record();for(const prop of request.props.split(',')){if(!defaultNames.includes(prop))throw new Error('No default');object[prop]=prop==='axis'?[0,1,0]:prop==='disabled'?false:0;markDefault(prop,true);}}
          else if(args.tool==='animation_selection'){record();if(request.action==='delete')snapshot.animationObjects=snapshot.animationObjects.filter(object=>!request.uids.includes(object.uid));else {const index=snapshot.animationObjects.findIndex(object=>object.uid===request.uids[0]),target=index+(request.action==='up'?-1:1);[snapshot.animationObjects[index],snapshot.animationObjects[target]]=[snapshot.animationObjects[target],snapshot.animationObjects[index]];}}
          resolveAnimation();return {snapshot:structuredClone(snapshot)};
        }
        if(options.density && [11,12].includes(args.args.nodeId)) {
          const target=snapshot.densityMaps[0].renderers.find(item=>item.uid===args.args.nodeId);
          const values=()=>({alpha:String(target.alpha),...target.specific});
          const rows=()=>Object.entries(values()).map(([key,value])=>({key,value,type:key==='color'?'color':'real',isdefault:key==='alpha'&&target.alpha===1,hasdefault:key==='alpha',rgba:key==='color'?[0,0,255,255]:undefined}));
          if(args.tool==='list_node_props')return {properties:rows()};
          const edit=args.args;
          if(args.tool==='edit_node_property' && edit.mode==='preview' && window.__holdNumericPreview)await window.__holdNumericPreview;
          const value=edit.mode==='abort'?edit.originalValue:edit.value;
          if(edit.prop==='alpha')target.alpha=Number(value);
          else {target.specific[edit.prop]=value;if(edit.prop==='siglevel')target.specific.siglevel_effective=value;}
          if(args.tool==='set_node_prop'||edit.mode==='commit'){snapshot.canUndo=true;snapshot.undoDescriptions.push('Change density');}
          return {snapshot:structuredClone(snapshot)};
        }
        if(options.styles && args.tool==='style_edit') {
          if(window.__holdStyleEdit)await window.__holdStyleEdit;
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
          if(request.action==='rewrite' && options.rewrite) {
            if(window.__rewriteError)throw new Error('Rewrite failed');
            return {expression:window.__rewriteResult};
          }
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
            {key:'name',label:'Name'},{key:'alpha',label:'Opacity',min:0,max:1,step:0.1,realtime:true},
            {key:'detail',label:'Detail',choices:['2','4','8','16','24','32']},
            {key:'material',label:'Material',choices:['','Matte'],labels:{'':'(none)'}},
            {key:'font_name',label:'Font name',font:true,choices:['Portable face','8514oem','serif','monospace']},
          ]},{title:'Geometry',expanded:true,rows:[
            {key:'ring',label:'Show ring'},{key:'thickness',label:'Thickness',min:0,max:3,step:0.01,unit:'Å',disabled:values.ring!=='true'},
            {key:'section.width',label:'Width1',min:0.01,max:1.5,step:0.01,writes:[{prop:'section.width'},{prop:'section.tuber',inverse:Number(values['section.width'])*Number(values['section.tuber'])}]},
            {key:'section.tuber',label:'Width2',display:String(Number(values['section.width'])*Number(values['section.tuber'])),min:0.01,max:1.5,step:0.01,writes:[{prop:'section.tuber',scale:1/Number(values['section.width'])}]},
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
        if(args.tool==='edit_node_property') {
          const edit=args.args;
          if(window.__holdNumericPreview && edit.mode==='preview') await window.__holdNumericPreview;
          set(edit.prop, edit.mode==='abort' ? edit.originalWasDefault ? defaults[edit.prop] : edit.originalValue : edit.value);
          if(edit.mode==='commit' && edit.value!==edit.originalValue) { snapshot.canUndo=true; snapshot.undoDescriptions.push('Change properties'); }
          return {snapshot:structuredClone(snapshot)};
        }
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
      if (command === 'image_export_request') {
        const {operation,args:input} = args;
        if (operation === 'begin') return {id:String(++window.__imageId || (window.__imageId=1)),width:800,height:600};
        if (operation === 'start') { window.__imageStarted=true; window.__imageCancelled=false; return {running:true}; }
        if (operation === 'cancel') { window.__imageCancelled=true; return {}; }
        if (operation === 'release') return {};
        if (operation === 'poll') {
          const completed=!window.__holdImage || window.__imageCancelled;
          snapshot.status=window.__imageCancelled?'Image export cancelled':'Exported image: cuemol.png';
          return {completed,running:!completed,cancelled:!!window.__imageCancelled,error:window.__imageError||null};
        }
        throw Error('Unknown image export operation');
      }
      if (command === 'export_scene_geometry') return structuredClone(snapshot);
      throw new Error(`Unexpected IPC: ${command}`);
    }}};
    if(options.closeGuard){
      window.__closedWindows=0;
      let handler;
      const appWindow={onCloseRequested:async callback=>{handler=callback;},close:async()=>{
        let prevented=false;
        await handler?.({preventDefault:()=>{prevented=true;}});
        if(!prevented)window.__closedWindows++;
      }};
      window.__requestApplicationClose=()=>appWindow.close();
      window.__TAURI__.webviewWindow={getCurrentWebviewWindow:()=>appWindow};
    }
    if(options.agent) window.__TAURI__.event={listen:async(name,callback)=>{events.set(name,callback);return ()=>events.delete(name);}};
    const backendInvoke=window.__TAURI__.core.invoke;
    window.__TAURI__.core.invoke=async(command,args)=>structuredClone(await backendInvoke(command,structuredClone(args)));
    if(options.files) window.__TAURI__.webviewWindow={getCurrentWebviewWindow:()=>({
      onDragDropEvent:async callback=>{window.__nativeFileDrop=callback;return ()=>{};}
    })};
    window.__TAURI__.dialog={open:async options=>{
      window.__morphPickerOptions=options;
      if(window.__delayMorphPicker)return new Promise(resolve=>{window.__releaseMorphPicker=resolve;});
      return window.__morphPaths||null;
    },save:async options=>{
      window.__saveDialogOptions=options;
      window.__optionsVisibleAtSave=document.querySelector('#reference-dialog')?.open || false;
      if(window.__holdSave)return new Promise(resolve=>window.__resolveSave=resolve);
      return window.__cancelSave ? null : options.defaultPath;
    }};
  }, {storage, options});
  await page.goto('http://tauri.test/');
  if (!options.startupPaths?.length) await expect(page.locator('#status-text')).toHaveText('Ready');
  await expect(page.locator('#error-alert-dialog')).not.toBeVisible();
  return errors;
}

test('Tauri style rows open on double click with their own scope', async ({page}) => {
  const errors = await openShell(page, {}, {styles:true});
  for (const scope of ['scene', 'global']) {
    await page.getByRole('button', {name:`Style set user (${scope})`, exact:true}).dblclick({delay:120});
    await expect(page.locator('#reference-dialog').getByLabel('Style set name')).toHaveValue('user');
    expect(await page.evaluate(() => window.__shellCalls.filter(call => call.command === 'get_style_properties').at(-1).args.target))
      .toEqual({ownerSceneUid:1,sceneUid:scope==='scene'?1:0,styleId:scope==='scene'?102:101});
    await page.locator('#reference-dialog').getByRole('button', {name:'Close', exact:true}).last().click();
    await expect(page.locator('#reference-dialog')).not.toBeVisible();
  }
  expect(errors).toEqual([]);
});

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

test('Tauri style save waits for accepted edits and close keeps queued edits', async ({page}) => {
  const errors=await openShell(page,{}, {styles:true});
  await page.locator('.style-node').filter({hasText:'user ↗ (global)'}).dblclick();
  const dialog=page.locator('#reference-dialog');
  await page.evaluate(()=>{window.__holdStyleEdit=new Promise(resolve=>{window.__releaseStyleEdit=resolve;});});
  await dialog.getByLabel('Color accent',{exact:true}).fill('#abcdef');
  await dialog.getByRole('button',{name:'Apply color accent',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>window.__shellCalls.filter(c=>c.command==='inspector_request'&&c.args.tool==='style_edit').length)).toBe(1);
  await dialog.getByRole('button',{name:'Selection',exact:true}).click();
  await dialog.getByLabel('Selection focus',{exact:true}).fill('name O1');
  await dialog.getByRole('button',{name:'Apply selection focus',exact:true}).click();
  await dialog.getByRole('button',{name:'Save style...',exact:true}).click();
  expect(await page.evaluate(()=>window.__shellCalls.filter(c=>c.command==='save_style_file'))).toHaveLength(0);
  await dialog.getByRole('button',{name:'Close',exact:true}).last().click();
  await expect(dialog).not.toBeVisible();
  await page.evaluate(()=>window.__releaseStyleEdit());
  await expect.poll(()=>page.evaluate(()=>window.__savedStyleXml||'')).toContain('#ABCDEF');
  expect(await page.evaluate(()=>window.__savedStyleXml)).toContain('name O1');
  expect(errors).toEqual([]);
});

test('Tauri anonymous style saves with a visible default filename', async ({page}) => {
  const errors=await openShell(page,{}, {styles:true});
  await page.locator('.style-node').filter({hasText:'(anonymous) (global)'}).first().dblclick();
  const dialog=page.locator('#reference-dialog');
  await dialog.getByRole('button',{name:'Save style...',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>window.__saveDialogOptions?.defaultPath)).toBe('style.xml');
  await expect.poll(()=>page.evaluate(()=>window.__savedStyleXml||'')).toContain('id=""');
  await dialog.getByRole('button',{name:'Close',exact:true}).last().click();
  await page.locator('.style-node').filter({hasText:/^\(anonymous\) \(global\)$/}).dblclick();
  await expect(dialog.getByLabel('Style set name')).toHaveValue('(anonymous)');
  await dialog.getByRole('button',{name:'Save style...',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>window.__saveDialogOptions?.defaultPath)).toBe('(anonymous).xml');
  expect(errors).toEqual([]);
});

test('Tauri queued style work preserves a later selection across insertion', async ({page}) => {
  const errors=await openShell(page,{}, {styles:true});
  await page.getByRole('button',{name:'Style set user (global)',exact:true}).dblclick();
  const dialog=page.locator('#reference-dialog');
  await page.evaluate(()=>{window.__holdStyleEdit=new Promise(resolve=>{window.__releaseStyleEdit=resolve;});});
  await dialog.getByLabel('Color accent',{exact:true}).fill('#abcdef');
  await dialog.getByRole('button',{name:'Apply color accent',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>window.__shellCalls.filter(c=>c.command==='inspector_request'&&c.args.tool==='style_edit').length)).toBe(1);
  await dialog.getByRole('button',{name:'Save style...',exact:true}).click();
  await dialog.getByRole('button',{name:'Close',exact:true}).last().click();
  const selected=page.getByRole('button',{name:'Style set user (scene)',exact:true});
  await selected.focus();
  await selected.press('Enter');
  await page.evaluate(()=>{window.__insertStyle();window.__releaseStyleEdit();});
  await expect.poll(()=>page.evaluate(()=>window.__savedStyleXml||'')).toContain('#ABCDEF');
  await expect(selected).toHaveClass(/active/);
  await page.locator('#workspace-properties').click();
  await expect(dialog.getByLabel('Style set name')).toHaveValue('user');
  await expect(dialog.locator('.style-set-metadata')).toContainText('Scene style set');
  expect(errors).toEqual([]);
});

test('Tauri style copy resolves its target after an insertion', async ({page}) => {
  const errors=await openShell(page,{}, {styles:true});
  await page.evaluate(()=>{window.__holdStyleCopy=new Promise(resolve=>{window.__releaseStyleCopy=resolve;});});
  await page.getByRole('button',{name:'Style set user (scene)',exact:true}).click({button:'right'});
  await page.locator('.workspace-context-menu').getByRole('menuitem',{name:'Copy',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>window.__shellCalls.some(c=>c.command==='copy_workspace_style'))).toBe(true);
  await page.evaluate(()=>{window.__insertStyle();window.__releaseStyleCopy();});
  await expect.poll(()=>page.evaluate(()=>window.__copiedStyleXml||'')).toContain('id="user"');
  expect(await page.evaluate(()=>window.__shellCalls.find(c=>c.command==='copy_workspace_style').args)).toEqual({input:{ownerSceneUid:1,sceneUid:1,styleId:102}});
  expect(errors).toEqual([]);
});

test('Tauri multiple style deletion retains identities during confirmation', async ({page}) => {
  const errors=await openShell(page,{}, {styles:true,styleMultiple:true});
  await page.getByRole('button',{name:'Style set user (scene)',exact:true}).click();
  await page.getByRole('button',{name:'Style set second (scene)',exact:true}).click({modifiers:['ControlOrMeta']});
  await page.locator('#workspace-delete').click();
  const confirmation=page.locator('#confirmation-dialog');
  await expect(confirmation).toContainText('Delete style: user');
  await page.evaluate(()=>window.__insertStyle());
  await confirmation.getByRole('button',{name:'Delete',exact:true}).click();
  await expect(confirmation).toContainText('Delete style: second');
  await confirmation.getByRole('button',{name:'Delete',exact:true}).click();
  await expect(page.getByRole('button',{name:'Style set inserted (scene)',exact:true})).toBeVisible();
  await expect(page.getByRole('button',{name:'Style set user (scene)',exact:true})).toHaveCount(0);
  await expect(page.getByRole('button',{name:'Style set second (scene)',exact:true})).toHaveCount(0);
  const targets=await page.evaluate(()=>window.__shellCalls.find(c=>c.command==='delete_workspace_items').args.targets);
  expect(targets.map(target=>target.styleId)).toEqual([102,105]);
  expect(targets.every(target=>target.localIndex===undefined)).toBe(true);
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
  await expect(page).toHaveTitle('Test scene — CueMol3');
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
  const errors = await openShell(page, {}, {plugins: {mdtools: true}});
  await page.locator('[data-bottom-select="animation"]').click();
  await expect(page.locator('#animation-play')).toBeVisible();
  await expect(page.locator('#animation-list')).toContainText('No animation objects.');
  await page.locator('[data-bottom-select="sequence"]').click();
  await expect(page.locator('#sequence-view')).toBeVisible();
  await page.locator('[data-bottom-select="trajectory"]').click();
  await expect(page.locator('#trajectory-slider')).toBeVisible();
  await page.locator('[data-bottom-select="console"]').click();
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
  await page.getByRole('separator', {name: 'Resize bottom panel', exact: true}).press('ArrowUp');
  await page.locator('[data-bottom-select="console"]').click();
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
  await page.locator('[data-bottom-select="console"]').click();
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
  await page.locator('[data-bottom-select="console"]').click();
  await expect(page.locator('[data-console-dialect="PyMOL"]')).toHaveAttribute('aria-checked', 'true');
  await input.press('ArrowUp');
  await expect(input).toHaveValue(source);
  await page.locator('[data-console-dialect="ChimeraX"]').click();
  await expect(page.locator('#console-prompt')).toHaveText('ChX>');
  await input.fill('select #1/A:2');
  await input.press('Enter');
  expect(await page.evaluate(() => window.__shellCalls.filter(call => call.command === 'console_start').at(-1).args)).toEqual({source: 'select #1/A:2', dialect: 'ChimeraX'});
  await page.locator('#console-stop').click();
  await page.reload();
  await expect(page.locator('[data-console-dialect="ChimeraX"]')).toHaveAttribute('aria-checked', 'true');
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
  await expect(input('section.width')).toHaveValue('1.5');
  await expect(input('section.tuber')).toHaveValue('0.5');
  const batch=await page.evaluate(()=>window.__shellCalls.find(call=>call.command==='inspector_request' && call.args.tool==='set_node_props').args.args.updates);
  expect(batch).toEqual([{prop:'section.width',value:'1.5'},{prop:'section.tuber',value:String(1/3)}]);
  await input('helix_ribbon').selectOption('true');
  await expect(input('helix_ribbon')).toHaveValue('true');
  const flag=await page.evaluate(()=>window.__shellCalls.filter(call=>call.command==='inspector_request' && call.args.args.prop==='helix_ribbon').at(-1).args.args.value);
  expect(flag).toBe('true');
  await page.getByRole('tab',{name:'Generic',exact:true}).click();
  await expect(input('section.tuber')).toHaveValue(String(1/3));
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


test('Tauri workspace selection actions apply the shared parser result', async ({page}) => {
  const errors=await openShell(page,{}, {rewrite:true,selectionExpression:'not chain A or not chain B'});
  const row=()=>page.locator('#scene-tree button').filter({hasText:'Probe'}).first();
  const select=async(label, output)=>{
    await page.evaluate(value=>{window.__rewriteResult=value;}, output);
    await row().click({button:'right'});
    await page.locator('.workspace-context-menu').getByRole('button',{name:/^Selection/}).hover();
    await page.locator('.workspace-context-menu').getByRole('menuitem',{name:label,exact:true}).click();
  };
  await select('Invert','not (not chain A or not chain B)');
  await expect.poll(()=>page.evaluate(()=>window.__shellCalls.filter(c=>c.command==='molecule_selection_action').length)).toBe(1);
  await select('Toggle Sidechain','bysidech (not (not chain A or not chain B))');
  await expect.poll(()=>page.evaluate(()=>window.__shellCalls.filter(c=>c.command==='molecule_selection_action').length)).toBe(2);
  const calls=await page.evaluate(()=>window.__shellCalls.filter(c=>c.command==='inspector_request'&&c.args.tool==='selection_info'&&c.args.args.action==='rewrite').map(c=>c.args.args));
  expect(calls).toEqual([
    {moleculeId:3,action:'rewrite',current:'not chain A or not chain B',operation:'invert',distance:5},
    {moleculeId:3,action:'rewrite',current:'not (not chain A or not chain B)',operation:'sidechain',distance:5},
  ]);
  await page.evaluate(()=>{window.__rewriteError=true;});
  await select('Invert','unused');
  await expect(page.locator('#error-alert-dialog')).toContainText('Rewrite failed');
  expect(await page.evaluate(()=>window.__shellCalls.filter(c=>c.command==='molecule_selection_action').length)).toBe(2);
  expect(errors).toEqual([]);
});


test('Tauri Focus submits the selected objects together', async ({page}) => {
  const errors = await openShell(page, {}, {selectionSecond:true});
  await page.locator('.tree-object').filter({hasText:'Probe'}).click();
  await page.locator('.tree-object').filter({hasText:'Other'}).click({modifiers:['ControlOrMeta']});
  await expect(page.locator('#workspace-zoom')).toBeEnabled();
  await page.locator('#workspace-zoom').click();
  await expect.poll(() => page.evaluate(() => window.__shellCalls.filter(call => call.command === 'fit_workspace_items').length)).toBe(1);
  const args = await page.evaluate(() => window.__shellCalls.find(call => call.command === 'fit_workspace_items').args);
  expect(args.targets.map(target => target.objectUid)).toEqual([3,8]);
  expect(args.targets.every(target => target.kind === 'object')).toBe(true);
  expect(args.width).toBeGreaterThan(0);
  expect(args.height).toBeGreaterThan(0);
  expect(errors).toEqual([]);
});


test('Tauri Get PDB searches, pages and downloads the selected EMDB map',async({page})=>{
  const errors=await openShell(page,{}, {pdbSearch:true});
  await page.getByRole('button',{name:'Get PDB',exact:true}).click();
  const dialog=page.locator('#reference-dialog');
  const input=dialog.getByLabel('PDB or EMDB ID, or keywords',{exact:true});
  await input.fill('ribosome');await dialog.getByRole('button',{name:'Search PDB',exact:true}).click();
  await expect(dialog.getByRole('listbox').getByRole('option')).toHaveCount(2);
  await dialog.getByRole('button',{name:'More',exact:true}).click();
  await expect(dialog.getByRole('listbox').getByRole('option')).toHaveCount(3);
  expect(await page.evaluate(()=>window.__shellCalls.filter(c=>c.command==='get_pdb_info'&&c.args.query.kind==='search').map(c=>c.args.query.start))).toEqual([0,25]);
  await dialog.getByRole('option',{name:/7A4M/}).click();
  await expect(input).toHaveValue('ribosome');
  await expect(dialog.getByText('Recommended level 20',{exact:false})).toBeVisible();
  await expect(dialog.getByLabel('Coordinate format')).toHaveValue('RCSB_CIF');
  await expect(dialog.locator('option[value="RCSB_PDB"]')).toHaveJSProperty('disabled',true);
  await dialog.getByLabel('Fetch coordinates',{exact:true}).uncheck();
  await dialog.getByRole('button',{name:'Download and Open',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>window.__shellCalls.filter(c=>c.command==='open_pdb').length)).toBe(1);
  const args=await page.evaluate(()=>window.__shellCalls.find(c=>c.command==='open_pdb').args);
  expect(args.pdbId).toBe('7a4m');expect(args.fetchCoordinates).toBe(false);
  expect(args.emMaps).toEqual([{emdbId:'EMD-11638',source:'WWPDB',level:20}]);
  expect(errors).toEqual([]);
});

test('Tauri Get PDB handles filters, stale searches, missing IDs and metadata errors',async({page})=>{
  const errors=await openShell(page,{}, {pdbSearch:true});
  await page.getByRole('button',{name:'Get PDB',exact:true}).click();
  const dialog=page.locator('#reference-dialog');
  const input=dialog.getByLabel('PDB or EMDB ID, or keywords',{exact:true});
  await input.fill('kinase');await input.press('Enter');
  await expect(dialog.getByRole('listbox').getByRole('option')).toHaveCount(2);
  await dialog.getByRole('combobox',{name:'Method',exact:true}).selectOption('em');
  await dialog.getByRole('combobox',{name:'Sort',exact:true}).selectOption('resolution');
  await expect.poll(()=>page.evaluate(()=>window.__shellCalls.filter(c=>c.command==='get_pdb_info'&&c.args.query.kind==='search').at(-1)?.args.query)).toMatchObject({method:'em',sort:'resolution',start:0});
  await input.fill('delayed');await input.press('Enter');
  await expect.poll(()=>page.evaluate(()=>typeof window.__releasePdbSearch)).toBe('function');
  await input.fill('EMD-11638');await page.evaluate(()=>window.__releasePdbSearch());
  await expect(dialog.getByText('Cryo-EM map, no coordinates',{exact:false})).toBeVisible();
  await expect(dialog.getByRole('listbox')).toBeHidden();
  await expect(dialog.getByLabel('Fetch coordinates',{exact:true})).toBeHidden();
  await input.fill('9zzz');await expect(dialog.getByText('No entry was found.',{exact:true})).toBeVisible();
  await expect(dialog.getByRole('button',{name:'Download and Open',exact:true})).toBeDisabled();
  await input.fill('1err');await expect(dialog.getByText('The files can still be downloaded.',{exact:false})).toBeVisible();
  await expect(dialog.getByLabel('2Fo-Fc',{exact:true})).toBeVisible();
  await expect(dialog.getByRole('button',{name:'Download and Open',exact:true})).toBeEnabled();
  await input.fill('delayed');await input.press('Enter');
  await dialog.getByRole('button',{name:'Cancel',exact:true}).click();
  await page.evaluate(()=>window.__releasePdbSearch());
  await expect(dialog).not.toBeVisible();
  expect(await page.evaluate(()=>window.__shellCalls.filter(c=>c.command==='open_pdb'))).toHaveLength(0);
  expect(errors).toEqual([]);
});


test('Tauri About displays and copies runtime rows and handles clipboard failure', async ({page}) => {
  const errors = await openShell(page);
  await page.evaluate(() => Object.defineProperty(navigator, 'clipboard', {configurable: true, value: {
    writeText: async text => { window.__aboutCopied = text; },
  }}));
  await page.getByRole('banner').getByText('Help', {exact: true}).click();
  await page.getByText('About CueMol3', {exact: true}).click();
  const dialog = page.locator('#reference-dialog');
  await expect(dialog).toContainText('test-wgpu');
  await expect(dialog).toContainText('test-tauri');
  await dialog.getByRole('button', {name: 'Copy', exact: true}).click();
  await expect(dialog.getByRole('button', {name: 'Copied', exact: true})).toBeVisible();
  const copied = await page.evaluate(() => window.__aboutCopied);
  expect(copied).toMatch(/^CueMol3\nVersion: test-version\nCommit: abc123\nDate: 2026-11-01T00:00:00Z\nRust: rustc test\nwgpu: test-wgpu\nTauri: test-tauri\nOS: test-os\nWebView: Mozilla\//);
  await page.evaluate(() => { navigator.clipboard.writeText = async () => { throw new Error('Clipboard unavailable'); }; });
  await dialog.getByRole('button', {name: 'Copied', exact: true}).click();
  await expect(page.locator('#error-alert-dialog')).toContainText('Clipboard unavailable');
  expect(errors).toEqual([]);
});


test('Tauri numeric dragging coalesces previews and cancels an in-flight edit', async ({page}) => {
  const errors=await openShell(page,{}, {inspectorPages:true});
  await page.locator('.tree-renderer').filter({hasText:'Atoms'}).click();
  const input=page.getByLabel('Inspected alpha',{exact:true});
  await expect(input).toHaveValue('1');
  const writes=()=>page.evaluate(()=>window.__shellCalls.filter(call=>call.command==='inspector_request' && call.args.tool==='edit_node_property').map(call=>call.args.args));
  const drag=async()=>{
    const box=await input.boundingBox();
    await page.mouse.move(box.x+box.width/2,box.y+box.height/2);
    await page.mouse.down();
    await page.mouse.move(box.x+box.width/2-4,box.y+box.height/2,{steps:4});
  };
  await page.evaluate(()=>{ window.__holdNumericPreview=new Promise(resolve=>{window.__releaseNumericPreview=()=>{window.__holdNumericPreview=null;resolve();};}); });
  await drag();
  await expect.poll(async()=>(await writes()).length).toBe(1);
  await page.mouse.move((await input.boundingBox()).x+10,(await input.boundingBox()).y+10,{steps:8});
  expect((await writes()).length).toBe(1);
  await page.keyboard.press('Escape');
  await page.mouse.up();
  await page.evaluate(()=>window.__releaseNumericPreview());
  await expect.poll(async()=>(await writes()).map(edit=>edit.mode)).toEqual(['preview','abort']);
  await expect(input).toHaveValue('1');
  await expect(page.getByRole('button',{name:'Reset alpha',exact:true})).toBeDisabled();
  await drag();
  await expect.poll(async()=>Number(await input.inputValue())).toBeLessThan(1);
  await page.mouse.up();
  await expect.poll(async()=>(await writes()).at(-1).mode).toBe('commit');
  expect((await writes()).filter(edit=>edit.mode==='commit')).toHaveLength(1);
  await expect(page.getByRole('button',{name:'Reset alpha',exact:true})).toBeEnabled();
  expect(errors).toEqual([]);
});


test('Tauri density pane edits units and coalesces opacity previews independently of the inspector', async({page})=>{
  const errors=await openShell(page,{}, {density:'xtal'});
  await page.getByRole('button',{name:'Crystal',exact:true}).click();
  const pane=page.locator('#density-content');
  const level=pane.getByLabel('Density level',{exact:true});
  await expect(level).toHaveValue('1.1');
  await level.fill('2'); await level.press('Enter');
  const writes=()=>page.evaluate(()=>window.__shellCalls.filter(call=>call.command==='inspector_request'&&call.args.tool!=='list_node_props').map(call=>({tool:call.args.tool,...call.args.args})));
  await expect.poll(async()=>(await writes()).filter(edit=>edit.prop==='siglevel').at(-1)?.value).toBe('2');
  await pane.getByLabel('Density map options').click();
  await pane.getByRole('button',{name:'Use absolute contour level',exact:true}).click();
  await expect(pane.getByLabel('Density level',{exact:true})).toHaveAttribute('max','1');
  const alpha=pane.getByLabel('Density opacity',{exact:true});
  await expect(alpha).toBeEnabled();
  await page.evaluate(()=>{window.__holdNumericPreview=new Promise(resolve=>{window.__releaseNumericPreview=()=>{window.__holdNumericPreview=null;resolve();};});});
  const box=await alpha.boundingBox();
  await page.mouse.move(box.x+box.width/2,box.y+box.height/2);await page.mouse.down();
  await page.mouse.move(box.x+box.width/2-4,box.y+box.height/2,{steps:4});
  await expect.poll(async()=>(await writes()).filter(edit=>edit.mode==='preview').length).toBe(1);
  await page.mouse.move(box.x+8,box.y+box.height/2,{steps:8});
  expect((await writes()).filter(edit=>edit.mode==='preview')).toHaveLength(1);
  await page.keyboard.press('Escape');await page.mouse.up();
  await page.evaluate(()=>window.__releaseNumericPreview());
  await expect(alpha).toHaveValue('1');
  expect((await writes()).filter(edit=>edit.mode).map(edit=>edit.mode)).toEqual(['preview','abort']);
  expect((await writes()).find(edit=>edit.mode==='abort')).toMatchObject({nodeId:11,originalWasDefault:true});
  await page.mouse.move(box.x+box.width/2,box.y+box.height/2);await page.mouse.down();
  await page.mouse.move(box.x+box.width/2-4,box.y+box.height/2,{steps:4});await page.mouse.up();
  await expect.poll(async()=>(await writes()).filter(edit=>edit.mode==='commit').length).toBe(1);
  await pane.getByLabel('Density renderer').selectOption('10:12');
  await expect(alpha).toHaveValue('0.5');
  await pane.getByRole('button',{name:'Cell',exact:true}).click();
  await expect(pane.getByRole('button',{name:'Cell',exact:true})).toBeDisabled();
  expect(errors).toEqual([]);
});

test('Tauri EM density pane uses top-percent limits and disables full-region extent', async({page})=>{
  const errors=await openShell(page,{}, {density:'em'});
  await page.getByRole('button',{name:'Crystal',exact:true}).click();
  const pane=page.locator('#density-content');
  await expect(pane.getByLabel('Density level',{exact:true})).toHaveValue('20');
  await expect(pane.getByLabel('Density level',{exact:true})).toHaveAttribute('min','0');
  await expect(pane.getByLabel('Density level',{exact:true})).toHaveAttribute('max','100');
  await expect(pane.getByLabel('Density extent',{exact:true})).toBeDisabled();
  await pane.getByLabel('Density map options').click();
  await expect(pane.getByRole('button',{name:/Use top-percent contour level$/})).toBeVisible();
  expect(errors).toEqual([]);
});


test('Tauri animation reports unresolved chains without hiding the editable row', async ({page}) => {
  const errors = await openShell(page, {}, {brokenAnimation:true});
  await page.locator('[data-bottom-select="animation"]').click();
  await expect(page.locator('#animation-error')).toContainText('missing');
  await expect(page.locator('#animation-list')).toContainText('child');
  await expect(page.locator('#animation-list')).toContainText('unresolved');
  await expect(page.locator('#animation-play')).toBeDisabled();
  await page.locator('.animation-row').click();
  await expect(page.locator('#animation-change')).toBeEnabled();
  expect(errors).toEqual([]);
});

test('Tauri animation strips move resize scrub and keep selection through reorder',async({page},testInfo)=>{
  const errors=await openShell(page,{}, {animationTimeline:true});
  await page.locator('[data-bottom-select="animation"]').click();
  const state=()=>page.evaluate(()=>window.__animationState());
  const writes=()=>page.evaluate(()=>window.__shellCalls.filter(call=>call.command==='inspector_request'&&call.args.tool==='set_anim_time'));
  let strip=page.getByRole('button',{name:'Animation strip base',exact:true}),box=await strip.boundingBox();
  await page.mouse.move(box.x+box.width/2,box.y+box.height/2);await page.mouse.down();await page.mouse.move(box.x+box.width/2+40,box.y+box.height/2,{steps:5});
  expect(await writes()).toHaveLength(0);await page.mouse.up();
  await expect.poll(async()=>(await state())[0].relativeStart).toBeCloseTo(0.4,6);
  expect((await writes())[0].args.args.element).toBe('#20');
  const right=page.locator('[data-animation-uid="20"] .animation-strip-edge.right');box=await right.boundingBox();
  await page.mouse.move(box.x+box.width/2,box.y+box.height/2);await page.mouse.down();await page.mouse.move(box.x+box.width/2+25,box.y+box.height/2,{steps:5});await page.mouse.up();
  await expect.poll(async()=>(await state())[0].duration).toBeCloseTo(1.25,6);
  box=await strip.boundingBox();await page.mouse.move(box.x+box.width/2,box.y+box.height/2);await page.mouse.down();await page.mouse.move(box.x+box.width/2+35,box.y+box.height/2,{steps:5});await page.keyboard.press('Escape');await page.mouse.up();
  expect(await writes()).toHaveLength(2);
  const ruler=page.getByLabel('Animation time ruler');box=await ruler.boundingBox();
  await page.mouse.move(box.x+20,box.y+box.height/2);await page.mouse.down();await page.mouse.move(box.x+70,box.y+box.height/2,{steps:4});await page.mouse.up();
  await expect(page.getByLabel('Animation current time',{exact:true})).toHaveValue('0:00.700');
  await page.locator('#animation-down').click();
  await expect(page.locator('[data-animation-uid="20"]')).toHaveClass(/selected/);
  expect((await state()).map(object=>object.uid)).toEqual([21,20,22]);
  await page.evaluate(async()=>{window.__reverseAnimations();snapshot=await invoke('get_snapshot');renderShell();renderAnimation();});
  await expect(page.locator('[data-animation-uid="20"]')).toHaveClass(/selected/);
  await page.getByLabel('Animation zoom fit',{exact:true}).click();await page.getByLabel('Animation zoom in',{exact:true}).click();await page.getByLabel('Animation zoom out',{exact:true}).click();
  const summary=await page.locator('.animation-summary').first().boundingBox(),rulerBox=await ruler.boundingBox(),ribbonBox=await page.locator('.animation-ribbon-panel').boundingBox();
  expect(summary.y).toBeGreaterThanOrEqual(ribbonBox.y+ribbonBox.height-1);expect(rulerBox.y).toBeGreaterThanOrEqual(summary.y+summary.height-1);
  await expect(page.locator('#object-stats')).not.toBeVisible();
  await page.screenshot({path:testInfo.outputPath('tauri-animation-strips.png')});
  await page.locator('#animation-add').click();await page.getByText('Simple spin',{exact:true}).click();
  await expect.poll(async()=>(await state()).length).toBe(4);
  await expect(page.locator('.animation-lane.selected')).toContainText('SimpleSpin0');

  expect(errors).toEqual([]);
});

test('Tauri animation Inspector previews timing and aborts queued numeric edits',async({page})=>{
  const errors=await openShell(page,{}, {animationTimeline:true});
  await page.locator('[data-bottom-select="animation"]').click();
  await page.getByRole('button',{name:'Animation element base',exact:true}).click();
  const duration=page.getByLabel('Animation property durationMs',{exact:true});
  await expect(duration).toHaveValue('0:01.000');
  const state=()=>page.evaluate(()=>window.__animationState());
  const edits=()=>page.evaluate(()=>window.__shellCalls.filter(call=>call.command==='inspector_request'&&call.args.tool==='edit_animation_property').map(call=>call.args.args));
  let box=await duration.boundingBox();
  await page.mouse.move(box.x+box.width/2,box.y+box.height/2);await page.mouse.down();await page.mouse.move(box.x+box.width/2+20,box.y+box.height/2,{steps:5});
  await expect.poll(async()=>(await state())[0].duration).toBeGreaterThan(1);expect((await edits()).some(edit=>edit.mode==='commit')).toBe(false);
  await page.mouse.up();await expect.poll(async()=>(await edits()).filter(edit=>edit.mode==='commit').length).toBe(1);
  await page.getByLabel('Animation Generic tab',{exact:true}).click();
  const angle=page.getByLabel('Animation property angle',{exact:true});await expect(angle).toHaveValue('360');
  await page.evaluate(()=>{window.__holdAnimationPreview=new Promise(resolve=>window.__releaseAnimationPreview=resolve);});
  box=await angle.boundingBox();await page.mouse.move(box.x+box.width/2,box.y+box.height/2);await page.mouse.down();await page.mouse.move(box.x+box.width/2-25,box.y+box.height/2,{steps:3});
  await expect.poll(async()=>(await edits()).filter(edit=>edit.prop==='angle'&&edit.mode==='preview').length).toBeGreaterThan(0);
  await page.keyboard.press('Escape');await page.mouse.up();await page.evaluate(()=>window.__releaseAnimationPreview());
  await expect.poll(async()=>(await edits()).filter(edit=>edit.prop==='angle'&&edit.mode==='abort').length).toBe(1);
  await expect(angle).toHaveValue('360');
  expect((await edits()).filter(edit=>edit.prop==='angle'&&edit.mode==='commit')).toHaveLength(0);
  await page.getByLabel('Animation Properties tab',{exact:true}).click();await page.getByLabel('Animation axis preset',{exact:true}).selectOption('x');
  await expect.poll(async()=>(await state())[0].axis).toEqual([1,0,0]);
  await page.getByLabel('Animation axis preset',{exact:true}).selectOption('cart');await page.getByLabel('Animation axis component y',{exact:true}).fill('2');await page.getByLabel('Animation axis component y',{exact:true}).press('Tab');
  await expect.poll(async()=>{const axis=(await state())[0].axis;return axis[1]/axis[0];}).toBeCloseTo(2,6);
  expect(errors).toEqual([]);
});

test('Tauri animation resets only declared defaults and keeps generic field types',async({page})=>{
  const errors=await openShell(page,{}, {animationTimeline:true,animationRealProperty:true});
  await page.locator('[data-bottom-select="animation"]').click();
  await page.getByRole('button',{name:'Animation element base',exact:true}).click();
  const name=page.getByLabel('Animation property name',{exact:true});
  await name.fill('true');await name.press('Tab');
  expect(await name.evaluate(input=>input.type)).toBe('text');
  await expect(name).toHaveValue('true');
  await expect(page.getByRole('button',{name:'Reset all to default',exact:true})).toBeDisabled();
  await expect(page.getByLabel('Animation reset startValue',{exact:true})).toHaveCount(0);
  const start=page.getByLabel('Animation property startValue',{exact:true});
  await expect(start).toHaveValue('-2');await start.fill('-5');await start.press('Tab');
  await expect(start).toHaveValue('-5');
  await page.getByLabel('Animation Generic tab',{exact:true}).click();
  await expect(page.getByLabel('Animation property absStart',{exact:true})).toBeDisabled();
  await expect(page.getByLabel('Animation reset rend',{exact:true})).toHaveCount(0);
  const quadric=page.getByLabel('Animation property quadric',{exact:true});
  const reset=page.getByLabel('Animation reset quadric',{exact:true});
  await expect(reset).toBeDisabled();
  const box=await quadric.boundingBox();
  await page.mouse.move(box.x+box.width/2,box.y+box.height/2);await page.mouse.down();
  await page.mouse.move(box.x+box.width/2+15,box.y+box.height/2,{steps:4});
  await expect.poll(()=>page.evaluate(()=>window.__animationState()[0].quadric)).toBeGreaterThan(0);
  await page.keyboard.press('Escape');await page.mouse.up();
  await expect(quadric).toHaveValue('0');await expect(reset).toBeDisabled();
  await quadric.fill('0.2');await quadric.press('Tab');await expect(reset).toBeEnabled();
  await page.getByRole('button',{name:'Reset all to default',exact:true}).click();await expect(reset).toBeDisabled();
  await expect(page.getByLabel('Animation property name',{exact:true})).toHaveValue('true');
  await expect(page.getByLabel('Animation property startValue',{exact:true})).toHaveValue('-5');
  await expect(page.getByLabel('Animation property rend',{exact:true})).toHaveValue('target');
  await page.evaluate(async()=>{snapshot=await invoke('undo',{index:0});renderShell();});
  await expect(quadric).toHaveValue('0.2');await expect(reset).toBeEnabled();
  expect(errors).toEqual([]);
});

test('Tauri symmetry uses reference cell defaults, constraints and unchanged-edit handling',async({page},testInfo)=>{
  const errors=await openShell(page,{}, {symmetry:true,density:'xtal'});
  await page.getByRole('button',{name:'Crystal',exact:true}).click();
  const pane=page.locator('#symmetry-content');
  const toggle=page.locator('[data-panel-name="Symmetry"] [data-panel-toggle]');
  await expect(toggle).toHaveAttribute('aria-expanded','true');
  await toggle.click();await expect(pane).not.toBeVisible();
  await toggle.click();await expect(pane).toBeVisible();
  expect((await pane.locator('summary').boundingBox()).height).toBeLessThanOrEqual(26);
  await expect(pane.getByRole('button',{name:'Unit cell',exact:true})).toBeDisabled();
  await pane.getByRole('button',{name:'Change ...',exact:true}).click();
  await page.getByRole('button',{name:'OK',exact:true}).click();
  const dialog=page.locator('#reference-dialog');
  await expect(dialog.getByLabel('a (Å)',{exact:true})).toHaveValue('1');
  await dialog.getByLabel('a (Å)',{exact:true}).fill('42');
  await dialog.getByLabel('b (Å)',{exact:true}).fill('25');
  await dialog.getByLabel('Crystal system',{exact:true}).selectOption('CUBIC');
  await expect(dialog.getByLabel('Space Group Number',{exact:true})).toHaveValue('195');
  await dialog.getByLabel('Restrict by symmetry',{exact:true}).check();
  await expect(dialog.getByLabel('b (Å)',{exact:true})).toHaveValue('42');
  await expect(dialog.getByLabel('c (Å)',{exact:true})).toBeDisabled();
  await dialog.getByLabel('Restrict by symmetry',{exact:true}).uncheck();
  await expect(dialog.getByLabel('b (Å)',{exact:true})).toHaveValue('25');
  await dialog.getByLabel('Restrict by symmetry',{exact:true}).check();
  await dialog.getByRole('button',{name:'OK',exact:true}).click();
  await expect(dialog).not.toBeVisible();
  const writes=()=>page.evaluate(()=>window.__shellCalls.filter(item=>item.command==='change_crystal_info'));
  expect((await writes()).map(item=>item.args)).toEqual([{objectUid:3,a:42,b:42,c:42,alpha:90,beta:90,gamma:90,spaceGroupNumber:195}]);
  await expect(pane).toContainText('Cubic');
  await pane.getByRole('button',{name:'Change ...',exact:true}).click();
  await dialog.getByRole('button',{name:'OK',exact:true}).click();
  expect(await writes()).toHaveLength(1);
  await pane.locator('summary').click();
  await pane.getByRole('button',{name:'50 Å',exact:true}).click();
  expect(await page.evaluate(()=>window.__shellCalls.find(item=>item.command==='show_symmetry_molecules').args)).toEqual({objectUid:3,extent:50});
  await pane.locator('.symmetry-target').selectOption('density:10');
  await expect(pane.getByRole('button',{name:'Change ...',exact:true})).toBeDisabled();
  await expect(pane.locator('summary')).toHaveAttribute('aria-disabled','true');
  await expect(pane.getByRole('button',{name:'Unit cell',exact:true})).toBeEnabled();
  await pane.getByRole('button',{name:'Unit cell',exact:true}).click();
  expect(await page.evaluate(()=>window.__shellCalls.filter(item=>item.command==='show_unit_cell').at(-1).args)).toEqual({targetKind:'density',objectUid:10});
  await page.screenshot({path:testInfo.outputPath('tauri-symmetry.png')});
  expect(errors).toEqual([]);
});

test('Tauri symmetry ignores stale space-group responses',async({page})=>{
  const errors=await openShell(page,{}, {symmetry:true,symmetryDelayed:true});
  await page.getByRole('button',{name:'Crystal',exact:true}).click();
  await page.locator('#symmetry-content').getByRole('button',{name:'Change ...',exact:true}).click();
  await page.getByRole('button',{name:'OK',exact:true}).click();
  const dialog=page.locator('#reference-dialog');
  const lattice=dialog.getByLabel('Crystal system',{exact:true});
  await lattice.selectOption('MONOCLINIC');
  await expect.poll(()=>page.evaluate(()=>typeof window.__releaseSpaceGroups)).toBe('function');
  await lattice.selectOption('CUBIC');
  await expect(dialog.getByLabel('Space Group Number',{exact:true})).toHaveValue('195');
  await page.evaluate(()=>window.__releaseSpaceGroups());
  await expect(dialog.getByLabel('Space Group Number',{exact:true})).toHaveValue('195');
  await dialog.getByRole('button',{name:'Cancel',exact:true}).click();
  expect(await page.evaluate(()=>window.__shellCalls.filter(item=>item.command==='change_crystal_info'))).toEqual([]);
  expect(errors).toEqual([]);
});

test('Tauri saved camera pane selects, applies, renames and reorders',async({page},testInfo)=>{
  const errors=await openShell(page,{'cuemol3.shell.v1':JSON.stringify({activity:'view'})},{cameras:true});
  const list=page.getByRole('listbox',{name:'Saved cameras',exact:true});
  const rows=list.getByRole('option');
  await expect(rows).toHaveCount(3);await expect(rows.first()).toHaveAttribute('data-camera-name','__current');
  const front=rows.filter({hasText:'front'});
  await front.click();await expect(front).toHaveAttribute('aria-selected','true');
  expect(await page.evaluate(()=>window.__shellCalls.filter(call=>call.args?.tool==='apply_camera'))).toHaveLength(0);
  await page.getByRole('button',{name:'Apply to view',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>window.__shellCalls.filter(call=>call.args?.tool==='apply_camera').at(-1)?.args.args.withVisibility)).toBe(false);
  await front.dblclick();
  await expect.poll(()=>page.evaluate(()=>window.__shellCalls.filter(call=>call.args?.tool==='apply_camera').at(-1)?.args.args.withVisibility)).toBe(true);
  await page.getByRole('button',{name:'Save from view',exact:true}).click();
  await expect(front.locator('[title="Has saved show/hide state"]')).toHaveCount(0);
  await page.getByRole('button',{name:'Save from view with show/hide',exact:true}).click();
  await expect(front.locator('[title="Has saved show/hide state"]')).toHaveCount(1);
  await front.click();await page.keyboard.press('ArrowDown');await page.keyboard.press('F2');
  await page.getByRole('textbox',{name:'Camera name',exact:true}).fill('renamed');await page.keyboard.press('Enter');
  await expect(rows.filter({hasText:'renamed'})).toHaveCount(1);
  await rows.filter({hasText:'renamed'}).dragTo(rows.first(),{targetPosition:{x:40,y:2}});
  await expect(rows.nth(1)).toHaveAttribute('data-camera-name','renamed');
  await rows.filter({hasText:'front'}).click({button:'right'});
  await expect(page.getByRole('menuitem',{name:'Rename...',exact:true})).toBeVisible();
  await page.keyboard.press('Escape');
  await page.screenshot({path:testInfo.outputPath('tauri-camera-pane.png')});
  expect(errors).toEqual([]);
});


test('View pane supports drags, relative arrow steps and chained absolute coordinate edits',async({page},testInfo)=>{
  const errors=await openShell(page,{'cuemol3.shell.v1':JSON.stringify({activity:'view'})});
  const panel=page.locator('#view-content');
  const x=panel.getByRole('textbox',{name:'TraX',exact:true});
  await expect(x).toHaveValue('0.00');
  const box=await x.boundingBox();
  await page.mouse.move(box.x+box.width/2,box.y+box.height/2);await page.mouse.down();
  await page.mouse.move(box.x+box.width/2+20,box.y+box.height/2+10,{steps:10});await page.mouse.up();
  await expect(x).toHaveValue('20.00');
  await x.click();await x.fill('12.25');await x.press('Enter');
  const y=panel.getByRole('textbox',{name:'TraY',exact:true});await expect(y).toBeFocused();await y.fill('-8.5');await y.press('Enter');
  const z=panel.getByRole('textbox',{name:'TraZ',exact:true});await expect(z).toBeFocused();await z.fill('3');await z.press('Enter');await expect(z).toHaveValue('3.00');
  await y.click();await y.press('ArrowUp');await expect(y).toHaveValue('-7.50');await y.press('Escape');
  const rotation=panel.getByRole('textbox',{name:'RotZ',exact:true});await rotation.click();await rotation.fill('90');await rotation.press('Enter');await expect(rotation).toHaveValue('0');
  const distance=panel.getByRole('textbox',{name:'Dist',exact:true});await distance.click();await distance.fill('0');await distance.press('Enter');await expect(distance).toHaveValue('0');
  await x.click();await x.fill('999');await x.press('Escape');await expect(x).toHaveValue('12.25');
  await panel.locator('#view-perspective').check();await panel.locator('#view-center-mark').selectOption('axis');
  const calls=await page.evaluate(()=>window.__shellCalls.filter(call=>call.command==='update_camera').map(call=>call.args));
  expect(calls.filter(call=>call.action==='translate-x').reduce((total,call)=>total+call.x,0)).toBe(20);
  expect(calls).toEqual(expect.arrayContaining([expect.objectContaining({action:'set-center-x',x:12.25}),expect.objectContaining({action:'set-center-y',x:-8.5}),expect.objectContaining({action:'set-center-z',x:3}),expect.objectContaining({action:'translate-y',x:1}),expect.objectContaining({action:'rotate-z',x:90}),expect.objectContaining({action:'set-distance',x:0}),expect.objectContaining({action:'center-mark-axis'})]));
  expect(calls.some(call=>call.x===999)).toBe(false);
  expect(errors).toEqual([]);await page.screenshot({path:testInfo.outputPath('view-pane-tauri.png')});
});


test('View input discards queued movement after changing the active view',async({page})=>{
  const errors=await openShell(page,{'cuemol3.shell.v1':JSON.stringify({activity:'view'})},{delayCamera:true});
  const field=page.locator('#view-content').getByRole('textbox',{name:'RotX',exact:true});
  const box=await field.boundingBox();
  await page.mouse.move(box.x+box.width/2,box.y+box.height/2);await page.mouse.down();
  await page.mouse.move(box.x+box.width/2+10,box.y+box.height/2);
  await expect.poll(()=>page.evaluate(()=>Boolean(window.__releaseCamera))).toBe(true);
  await page.mouse.move(box.x+box.width/2+30,box.y+box.height/2);await page.mouse.up();
  await page.evaluate(async()=>{window.__switchView();await mutate('get_snapshot',{},()=>{},false);window.__releaseCamera();});
  await expect.poll(()=>page.evaluate(()=>cameraRequest===null)).toBe(true);
  const calls=await page.evaluate(()=>window.__shellCalls.filter(call=>call.command==='update_camera'));
  expect(calls).toHaveLength(1);expect(calls[0].args.x).toBe(10);expect(errors).toEqual([]);
});


test('Tauri native file drops pass one destination through the batch',async({page})=>{
  const errors=await openShell(page,{}, {files:true});
  await page.evaluate(()=>window.__nativeFileDrop({payload:{type:'drop',paths:['first.pdb','second.pdb']}}));
  await expect.poll(()=>page.evaluate(()=>window.__shellCalls.filter(call=>call.command==='open_paths').length)).toBe(2);
  const calls=await page.evaluate(()=>window.__shellCalls.filter(call=>call.command==='open_paths'));
  expect(calls[0].args.openContext).toEqual({target:'new'});
  expect(calls[1].args.openContext).toEqual({target:'new',sceneId:7});
  expect(calls.map(call=>call.args.paths)).toEqual([['first.pdb'],['second.pdb']]);
  expect(errors).toEqual([]);
});


test('Tauri picking preferences preserve cancellation and stop hover requests', async ({page},testInfo) => {
  const errors=await openShell(page,{}, {picking:true});
  const dialog=page.locator('#reference-dialog');
  const open=async()=>{await page.getByRole('button',{name:'Settings',exact:true}).click();await dialog.locator('[data-options-tab="Mouse & Navigation"]').click();};
  await open();
  for(const name of ['GPU picking','Hover info','Hover highlight'])await expect(dialog.getByLabel(name,{exact:true})).toBeChecked();
  await dialog.getByLabel('GPU picking',{exact:true}).uncheck();
  await dialog.getByRole('button',{name:'Cancel',exact:true}).click();
  await open();await expect(dialog.getByLabel('GPU picking',{exact:true})).toBeChecked();
  for(const name of ['GPU picking','Hover info','Hover highlight'])await dialog.getByLabel(name,{exact:true}).uncheck();
  await dialog.getByRole('button',{name:'OK',exact:true}).click();
  const saved=await page.evaluate(()=>window.__shellCalls.filter(call=>call.command==='set_preferences').at(-1).args.preferences);
  expect([saved.gpuPicking,saved.hoverInfo,saved.hoverHighlight]).toEqual([false,false,false]);
  await page.mouse.move(1,1);
  const before=await page.evaluate(()=>window.__shellCalls.filter(call=>call.command==='hover_at_screen').length);
  await page.locator('#viewport').hover();await page.waitForTimeout(150);
  expect(await page.evaluate(()=>window.__shellCalls.filter(call=>call.command==='hover_at_screen').length)).toBe(before);
  await open();
  for(const name of ['GPU picking','Hover info','Hover highlight'])await expect(dialog.getByLabel(name,{exact:true})).not.toBeChecked();
  await page.screenshot({path:testInfo.outputPath('picking-preferences.png')});
  await dialog.getByLabel('Hover info',{exact:true}).check();
  await dialog.getByRole('button',{name:'OK',exact:true}).click();
  await page.locator('#viewport').hover();
  await expect.poll(()=>page.evaluate(()=>window.__shellCalls.filter(call=>call.command==='hover_at_screen').length)).toBeGreaterThan(before);
  expect(errors).toEqual([]);
});

test('Tauri plugin preferences cancel, restore tabs, and retain dialect choices',async({page},testInfo)=>{
  const errors=await openShell(page);
  await expect(page.locator('[data-bottom-select="console"]')).toBeVisible();
  await expect(page.locator('[data-bottom-select="output"]')).toBeVisible();
  await expect(page.locator('[data-bottom-select="trajectory"]')).toBeHidden();
  const open=async()=>{
    await page.getByRole('button',{name:'Settings',exact:true}).click();
    await page.locator('[data-options-tab="Installed"]').click();
  };
  await open();
  const dialog=page.locator('#reference-dialog');
  await dialog.getByLabel('MD Tools',{exact:true}).check();
  await dialog.getByRole('button',{name:'Cancel',exact:true}).click();
  await expect(page.locator('[data-bottom-select="trajectory"]')).toBeHidden();
  await open();
  await dialog.getByLabel('MD Tools',{exact:true}).check();
  await dialog.getByLabel('Component Catalog',{exact:true}).check();
  await dialog.getByLabel('PyMOL console',{exact:true}).uncheck();
  await dialog.getByRole('button',{name:'OK',exact:true}).click();
  await expect(page.locator('[data-bottom-select="trajectory"]')).toBeVisible();
  await expect(page.locator('[data-console-dialect="PyMOL"]')).toBeHidden();
  await page.locator('[data-activity="catalog"]').click();
  await expect(page.locator('[data-activity-panel="catalog"]')).toBeVisible();
  await page.screenshot({path:testInfo.outputPath('plugins-catalog.png')});
  await page.locator('[data-bottom-select="trajectory"]').click();
  await open();
  await dialog.getByLabel('MD Tools',{exact:true}).uncheck();
  await dialog.getByLabel('Component Catalog',{exact:true}).uncheck();
  await dialog.getByLabel('Console',{exact:true}).uncheck();
  await dialog.getByRole('button',{name:'OK',exact:true}).click();
  await expect(page.locator('[data-bottom-select="output"]')).toHaveClass('active');
  await expect(page.locator('[data-bottom-select="console"]')).toBeHidden();
  await expect(page.locator('[data-activity="catalog"]')).toBeHidden();
  await open();await dialog.getByLabel('Console',{exact:true}).check();
  await dialog.getByRole('button',{name:'OK',exact:true}).click();
  await page.locator('[data-bottom-select="console"]').click();
  await expect(page.locator('[data-console-dialect="PyMOL"]')).toBeHidden();
  await expect(page.locator('[data-console-dialect="ChimeraX"]')).toBeVisible();
  expect(errors).toEqual([]);
});


test('Tauri Agent sends with explicit settings, retains chat across panels, and stops turns',async({page},testInfo)=>{
  const errors=await openShell(page,{}, {plugins:{agent:true},agent:true});
  await page.getByRole('button',{name:'AI Agent',exact:true}).click();
  const panel=page.locator('[data-activity-panel="agent"]');
  await expect(panel).toBeVisible();
  await panel.getByText('Agent settings',{exact:true}).click();
  await page.locator('#agent-model').fill('openai:second-model');
  await page.locator('#agent-key').fill('dummy-ui-key');
  await panel.getByRole('button',{name:'Refresh available models'}).click();
  await expect(page.locator('#agent-model-list option')).toHaveCount(2);
  await expect(page.locator('#agent-key')).toHaveValue('');
  await page.locator('#agent-prompt').fill('Make the molecule blue');
  await page.locator('#agent-prompt').press('Control+Enter');
  await expect(page.locator('#agent-status')).toHaveText('Thinking…');
  await expect(page.locator('#agent-send')).toBeDisabled();
  await expect(page.locator('#agent-prompt')).toHaveValue('');
  await page.getByRole('button',{name:'Explorer',exact:true}).click();
  await page.getByRole('button',{name:'AI Agent',exact:true}).click();
  await expect(page.locator('#agent-transcript')).toContainText('Make the molecule blue');
  await page.locator('#agent-cancel').click();
  await expect(page.locator('#agent-status')).toContainText('Cancelled');
  await page.locator('#agent-prompt').press('ArrowUp');
  await expect(page.locator('#agent-prompt')).toHaveValue('Make the molecule blue');
  await page.locator('#agent-enter-sends').check();
  await page.locator('#agent-prompt').press('Enter');
  await expect(page.locator('#agent-status')).toHaveText('Thinking…');
  await page.evaluate(()=>window.__finishAgent());
  await expect(page.locator('#agent-transcript')).toContainText('kept as text');
  await expect(page.locator('#agent-transcript img')).toHaveCount(0);
  await expect(page.locator('#agent-status')).toHaveText('Complete');
  await expect(page.locator('#agent-transcript details')).not.toHaveAttribute('open');
  await page.locator('#agent-transcript summary').click();
  await expect(page.locator('#agent-transcript details')).toHaveAttribute('open','');
  await expect(page.locator('#agent-transcript pre').last()).toHaveText('{"objects":[]}');
  await expect(page.locator('#agent-status')).toBeInViewport({ratio:1});
  await expect(page.locator('#agent-send')).toBeInViewport({ratio:1});
  await page.screenshot({path:testInfo.outputPath('tauri-agent.png')});
  const stored=await page.evaluate(()=>JSON.stringify({...localStorage}));
  expect(stored).not.toContain('dummy-ui-key');
  const calls=await page.evaluate(()=>window.__shellCalls.filter(call=>call.command==='agent_request'));
  expect(calls.filter(call=>call.args.operation==='start')).toHaveLength(2);
  expect(calls.find(call=>call.args.operation==='configure').args.args.settings.model).toBe('openai:second-model');
  await page.locator('#agent-clear').click();
  await expect(page.locator('#agent-transcript article')).toHaveCount(0);
  expect(errors).toEqual([]);
});

test('Tauri Agent respects composition and multiline prompt history boundaries',async({page})=>{
  const errors=await openShell(page,{'cuemol.agent.promptHistory':JSON.stringify(['recent','older'])},{plugins:{agent:true},agent:true});
  await page.getByRole('button',{name:'AI Agent',exact:true}).click();
  const prompt=page.locator('#agent-prompt');
  await prompt.fill('draft');
  await prompt.press('ArrowUp');await expect(prompt).toHaveValue('recent');
  await prompt.press('ArrowUp');await expect(prompt).toHaveValue('older');
  await prompt.press('ArrowDown');await expect(prompt).toHaveValue('recent');
  await prompt.press('ArrowDown');await expect(prompt).toHaveValue('draft');
  await prompt.fill('first\nsecond');
  await prompt.press('ArrowUp');await expect(prompt).toHaveValue('first\nsecond');
  await prompt.evaluate(element=>element.dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',ctrlKey:true,isComposing:true,bubbles:true})));
  expect(await page.evaluate(()=>window.__shellCalls.some(call=>call.command==='agent_request'&&call.args.operation==='start'))).toBe(false);
  await page.getByRole('button',{name:'Settings',exact:true}).click();
  await page.locator('[data-options-tab="Installed"]').click();
  await page.locator('#reference-dialog').getByLabel('AI Agent',{exact:true}).uncheck();
  await page.locator('#reference-dialog').getByRole('button',{name:'OK',exact:true}).click();
  await expect(page.locator('[data-activity="agent"]')).toBeHidden();
  await expect(page.locator('[data-activity-panel="agent"]')).toBeHidden();
  expect(errors).toEqual([]);
});


test('Tauri settings categories search, cancel, persist tools, and check updates',async({page},testInfo)=>{
  const errors=await openShell(page,{}, {updates:true,plugins:{console:true}});
  const dialog=page.locator('#reference-dialog');
  const category=name=>dialog.locator(`[data-options-tab="${name}"]`);
  const open=async()=>page.getByRole('button',{name:'Settings',exact:true}).click();
  await open();
  await expect(category('MCP Server')).toBeHidden();
  await dialog.getByLabel('Dark Mode',{exact:true}).uncheck();
  await category('Rendering').click();
  await dialog.getByLabel('POV-Ray Executable',{exact:true}).fill('tools/renderer');
  await dialog.getByRole('button',{name:'Cancel',exact:true}).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme','dark');
  await open();
  await expect(dialog.getByLabel('POV-Ray Executable',{exact:true})).toHaveValue('');
  await dialog.getByLabel('Search settings').fill('ffmpeg');
  await expect(category('Rendering')).toBeVisible();await expect(category('Theme')).toBeHidden();
  await category('Rendering').click();
  await dialog.getByLabel('POV-Ray Executable',{exact:true}).fill('tools/renderer');
  await dialog.getByLabel('ffmpeg Executable',{exact:true}).fill('tools/movie');
  await category('APBS / PDB2PQR').click();
  await dialog.getByRole('combobox',{name:'pdb2pqr Force Field',exact:true}).selectOption('amber');
  await category('Console').click();
  await dialog.getByLabel('Command line access',{exact:true}).check();
  await category('Updates').click();
  await dialog.getByLabel('Check for Updates at Startup',{exact:true}).uncheck();
  await dialog.getByRole('button',{name:'Check for Updates',exact:true}).click();
  const updates=page.getByRole('dialog',{name:'CueMol updates',exact:true});
  await expect(updates.getByRole('status')).toHaveText('CueMol 0.2.0 is available.');
  await expect(updates.getByRole('button',{name:'Install update',exact:true})).toBeEnabled();
  await updates.getByRole('button',{name:'Close',exact:true}).click();
  await category('Theme').click();await dialog.getByLabel('Dark Mode',{exact:true}).uncheck();
  await dialog.getByRole('button',{name:'OK',exact:true}).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme','light');
  const saved=await page.evaluate(()=>window.__shellCalls.filter(call=>call.command==='set_preferences').at(-1).args.preferences);
  expect(saved).toMatchObject({darkMode:false,consoleRemote:true,updateCheck:false,tools:{povray:'tools/renderer',ffmpeg:'tools/movie',forceField:'amber'}});
  await open();await category('Rendering').click();
  await expect(dialog.getByLabel('ffmpeg Executable',{exact:true})).toHaveValue('tools/movie');
  await page.screenshot({path:testInfo.outputPath('settings-categories.png')});
  expect(errors).toEqual([]);
});


test('Tauri catalog keeps property flags, mock resets, and sample edits local',async({page},testInfo)=>{
  const errors=await openShell(page,{}, {plugins:{catalog:true}});
  await page.locator('[data-activity="catalog"]').click();
  const catalog=page.locator('[data-activity-panel="catalog"]');
  await expect(catalog).toBeVisible();
  for(const name of ['Catalog 1','Catalog 2','Catalog 3'])await expect(catalog.getByRole('button',{name,exact:true})).toBeVisible();
  const before=await page.evaluate(()=>window.__shellCalls.length);
  const label=catalog.getByRole('textbox',{name:'Property Label',exact:true});
  await label.fill('chain A');
  await expect(catalog.locator('[data-property="Property Label"]')).toHaveClass(/modified/);
  await catalog.getByRole('button',{name:'Reset Property Label',exact:true}).click();
  await expect(catalog.locator('[data-property="Property Label"]')).not.toHaveClass(/modified/);
  await expect(label).toHaveValue('chain A');
  await catalog.getByRole('checkbox',{name:'Property Visible',exact:true}).check();
  await expect(catalog.locator('[data-property="Property Visible"]')).toHaveClass(/modified/);
  await expect(catalog.getByRole('checkbox',{name:'Mock Visible',exact:true})).not.toBeChecked();
  await catalog.getByRole('button',{name:'Reset all to default',exact:true}).click();
  await expect(catalog.getByRole('checkbox',{name:'Mock Visible',exact:true})).toBeChecked();
  await expect(catalog.getByRole('spinbutton',{name:'Mock Width',exact:true})).toHaveValue('1');
  await expect(catalog.locator('[data-property="Property Visible"]')).toHaveClass(/modified/);
  await expect(catalog.getByRole('button',{name:'Reset all to default',exact:true})).toBeDisabled();
  await catalog.getByRole('checkbox',{name:'GatedControl (checkbox + control, one property)',exact:true}).uncheck();
  await expect(catalog.getByRole('spinbutton',{name:'Gated value',exact:true})).toBeDisabled();
  await catalog.getByRole('checkbox',{name:'GatedControl (checkbox + control, one property)',exact:true}).check();
  const drag=catalog.getByRole('spinbutton',{name:'Gated value',exact:true});
  await drag.scrollIntoViewIfNeeded();const rect=await drag.boundingBox();
  await page.mouse.move(rect.x+rect.width/2,rect.y+rect.height/2);await page.mouse.down();await page.mouse.move(rect.x+rect.width/2+15,rect.y+rect.height/2,{steps:5});await page.mouse.up();
  await expect(drag).not.toHaveValue('1');
  await catalog.getByRole('option',{name:'Water',exact:true}).click();
  await expect(catalog.getByRole('option',{name:'Water',exact:true})).toHaveAttribute('aria-selected','true');
  const mutation=await page.evaluate(before=>window.__shellCalls.slice(before).filter(call=>call.command==='inspector_request'||call.command==='set_preferences'||call.command==='update_renderer'),before);
  expect(mutation).toEqual([]);
  await page.locator('[data-activity="explorer"]').click();await expect(catalog).toBeHidden();
  await page.locator('[data-activity="catalog"]').click();await expect(label).toHaveValue('chain A');
  for(const name of ['Catalog 1','Catalog 2','Catalog 3'])await expect(catalog.getByRole('button',{name,exact:true})).toBeInViewport();
  await catalog.getByRole('button',{name:'Catalog 1',exact:true}).click();
  await expect(catalog.getByRole('button',{name:'Catalog 1',exact:true})).toHaveAttribute('aria-expanded','false');
  await catalog.getByRole('button',{name:'Catalog 1',exact:true}).click();
  await page.screenshot({path:testInfo.outputPath('catalog.png')});
  expect(errors).toEqual([]);
});

test('Tauri file options retain AMBER coordinates and map kind before submission',async({page},testInfo)=>{
  const errors=await openShell(page);
  const dialog=page.locator('#reference-dialog');
  const show=async setup=>page.evaluate(setup=>{window.__readerResult=undefined;void showRendererSetup(setup).then(result=>{window.__readerResult=result;});},setup);
  const common={objectName:'sample',rendererName:'atoms',rendererType:'ballstick',recenter:true,options:[{name:'ballstick',description:'Ball and stick',preset:false}]};
  await show({...common,targetKind:'molecule',amberPrmtopOptions:{coordinatePath:''}});
  await dialog.getByRole('tab',{name:'AMBER topology options',exact:true}).click();
  await expect(dialog.getByLabel('Coordinate file:',{exact:true})).toHaveValue('');
  await dialog.getByLabel('Coordinate file:',{exact:true}).fill('coordinates/sample.rst7');
  await dialog.getByRole('button',{name:'OK',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>window.__readerResult?.amberPrmtopOptions)).toEqual({coordinatePath:'coordinates/sample.rst7'});
  await show({...common,targetKind:'densityMap',mapCenterPolicy:'auto',rendererType:'isosurf',options:[{name:'isosurf',description:'Isosurface',preset:false}],mapHeader:{counts:[1025,1025,257],voxels:1025*1025*257},ccp4MapOptions:{normalize:false,truncateMin:false,min:0,truncateMax:false,max:5,subsample:1,maxVoxels:0,useHeaderStats:true,mapType:'auto'}});
  await expect(dialog.getByRole('radio',{name:'Auto (from map type)',exact:true})).toBeChecked();
  await dialog.getByRole('radio',{name:'Move view center (fit the map)',exact:true}).check();
  await dialog.getByRole('tab',{name:'CCP4Map options',exact:true}).click();
  await expect(dialog).toContainText('Grid 1025 × 1025 × 257');
  await expect(dialog).toContainText('suggested subsample: 2');
  await dialog.getByLabel('Grid sampling stride:',{exact:true}).fill('2');
  await expect(dialog).not.toContainText('Large map');
  await dialog.getByLabel('Map type:',{exact:true}).selectOption('em');
  await page.screenshot({path:testInfo.outputPath('map-reader.png')});
  await dialog.getByRole('button',{name:'OK',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>window.__readerResult?.ccp4MapOptions)).toMatchObject({mapType:'em',subsample:2});
  await expect.poll(()=>page.evaluate(()=>window.__readerResult?.rendererSetup)).toMatchObject({mapCenterPolicy:'moveViewCenter',recenter:false});
  expect(errors).toEqual([]);
});


test('Tauri interaction analysis validates inputs, remembers molecules and enables the second selection', async ({page},testInfo) => {
  const errors=await openShell(page,{}, {interactions:true});
  const open=async()=>{
    await page.locator('summary').filter({hasText:/^Tools$/}).click();
    await page.locator('[data-dialog="Interaction"]').click();
  };
  await open();
  const dialog=page.locator('#reference-dialog');
  await expect(dialog.getByLabel('Min distance (Å)',{exact:true})).toHaveValue('2.5');
  await expect(dialog.getByLabel('Max labels',{exact:true})).toHaveValue('30');
  await expect(dialog.getByLabel('Hydrogen bond (N, O) only',{exact:true})).not.toBeChecked();
  await expect(dialog.getByLabel('Selection 1',{exact:true})).toHaveValue('');
  await expect(dialog.getByLabel('Selection 2',{exact:true})).toBeDisabled();
  await dialog.getByRole('button',{name:'Choose Selection 1',exact:true}).click();
  await dialog.getByRole('button',{name:'Selection choice *',exact:true}).click();
  await expect(dialog.getByLabel('Selection 1',{exact:true})).toHaveValue('*');
  await dialog.getByLabel('Second molecule',{exact:true}).check();
  await expect(dialog.getByLabel('Selection 2',{exact:true})).toBeEnabled();
  await dialog.getByLabel('Selection 2',{exact:true}).fill('name CA');
  await dialog.getByLabel('Min distance (Å)',{exact:true}).fill('3.5');
  await expect(dialog.getByRole('button',{name:'OK',exact:true})).toBeDisabled();
  await dialog.getByLabel('Min distance (Å)',{exact:true}).fill('2.5');
  await dialog.getByLabel('Max labels',{exact:true}).fill('0');
  await expect(dialog.getByRole('button',{name:'OK',exact:true})).toBeDisabled();
  await dialog.getByLabel('Max labels',{exact:true}).fill('30');
  await dialog.getByLabel('Selection 1',{exact:true}).fill('none');
  await dialog.getByRole('button',{name:'OK',exact:true}).click();
  await expect(dialog.getByRole('alert')).toHaveText('No interaction was found');
  await dialog.getByLabel('Selection 1',{exact:true}).fill('invalid (((');
  await dialog.getByRole('button',{name:'OK',exact:true}).click();
  await expect(dialog.getByRole('alert')).toContainText('Invalid selection');
  await page.screenshot({path:testInfo.outputPath('tauri-interaction.png')});
  await dialog.getByLabel('Selection 1',{exact:true}).fill('name N');
  await dialog.getByRole('button',{name:'OK',exact:true}).click();
  await expect(dialog).not.toBeVisible();
  const args=await page.evaluate(()=>window.__shellCalls.filter(c=>c.command==='calculate_contacts').at(-1).args);
  expect(args).toEqual({objectUid:3,selection1:'name N',object2Uid:5,selection2:'name CA',rMin:2.5,rMax:3.5,hbond:false,maxCount:30,rendererName:'measure'});
  await open();
  await expect(dialog.getByLabel('Selection 1',{exact:true})).toHaveValue('');
  await expect(dialog.getByLabel('Second molecule',{exact:true})).not.toBeChecked();
  await dialog.getByLabel('Second molecule',{exact:true}).check();
  await dialog.getByLabel('Molecule 2',{exact:true}).selectOption('3');
  await dialog.getByLabel('Selection 2',{exact:true}).fill('invalid (((');
  await dialog.getByRole('button',{name:'OK',exact:true}).click();
  const same=await page.evaluate(()=>window.__shellCalls.filter(c=>c.command==='calculate_contacts').at(-1).args);
  expect(same.object2Uid).toBeNull();expect(same.selection2).toBeNull();
  await open();
  await expect(dialog.getByLabel('Molecule 2',{exact:true})).toHaveValue('3');
  await page.evaluate(async()=>{window.__interactionSceneChanged();await mutate('get_snapshot');});
  await dialog.getByLabel('Max labels',{exact:true}).fill('29');
  await expect(dialog.getByRole('button',{name:'OK',exact:true})).toBeDisabled();
  await expect(dialog.getByRole('alert')).toContainText('active scene changed');
  expect(errors).toEqual([]);
});


test('Tauri molecule edit dialogs validate, confirm and retain failed requests', async ({page}, testInfo) => {
  const errors = await openShell(page, {}, {interactions: true, moleculeEdits: true});
  const dialog = page.locator('#reference-dialog');
  const open = async title => {
    await page.locator('summary').filter({hasText: /^Edit$/}).click();
    await page.locator(`[data-dialog="${title}"]`).click();
  };
  const last = command => page.evaluate(command => window.__shellCalls.filter(c => c.command === command).at(-1)?.args, command);
  await open('Delete mol atoms');
  await expect(dialog.getByLabel('Selection', {exact:true})).toHaveValue('');
  await expect(dialog.getByRole('button', {name:'Delete', exact:true})).toBeDisabled();
  await dialog.getByRole('button', {name:'Choose Selection', exact:true}).click();
  await dialog.getByRole('button', {name:'Selection choice *', exact:true}).click();
  await expect(dialog.getByRole('button', {name:'Delete', exact:true})).toBeEnabled();
  await dialog.getByLabel('Selection', {exact:true}).fill('none');
  await dialog.getByRole('button', {name:'Delete', exact:true}).click();
  await expect(dialog.getByRole('alert')).toHaveText('No atoms matched the selection');
  await dialog.getByLabel('Selection', {exact:true}).fill('invalid (((');
  await dialog.getByRole('button', {name:'Delete', exact:true}).click();
  await expect(dialog.getByRole('alert')).toContainText('Invalid selection');
  await expect(dialog.getByLabel('Selection', {exact:true})).toHaveValue('invalid (((');
  await dialog.getByLabel('Molecule', {exact:true}).selectOption('5');
  await dialog.getByLabel('Selection', {exact:true}).fill('*');
  await dialog.getByRole('button', {name:'Delete', exact:true}).click();
  await expect(dialog).not.toBeVisible();
  expect(await last('delete_atoms')).toEqual({objectUid:5, selection:'*'});
  await open('Delete mol atoms');
  await expect(dialog.getByLabel('Molecule', {exact:true})).toHaveValue('5');
  await expect(dialog.getByLabel('Selection', {exact:true})).toHaveValue('');
  await dialog.getByRole('button', {name:'Cancel', exact:true}).click();

  await open('Change chain ID');
  await expect(dialog.getByLabel('New chain ID', {exact:true})).toHaveValue('');
  await dialog.getByLabel('New chain ID', {exact:true}).fill(' ');
  await dialog.getByRole('button', {name:'OK', exact:true}).click();
  await expect(dialog.getByText('Chain ID < > will be converted to <_>. Change the chain ID?', {exact:true})).toBeVisible();
  expect(await last('change_chain')).toBeUndefined();
  await dialog.getByRole('button', {name:'No', exact:true}).click();
  await dialog.getByLabel('New chain ID', {exact:true}).fill(' AB ');
  await dialog.getByRole('button', {name:'OK', exact:true}).click();
  await expect(dialog.getByLabel('New chain ID', {exact:true})).toBeDisabled();
  await page.screenshot({path:testInfo.outputPath('tauri-chain-confirm.png')});
  await dialog.getByRole('button', {name:'Yes', exact:true}).click();
  await expect(dialog).not.toBeVisible();
  expect(await last('change_chain')).toEqual({objectUid:3, selection:'', chainName:'AB'});
  await open('Change chain ID');
  await dialog.getByLabel('New chain ID', {exact:true}).fill(' ');
  await dialog.getByRole('button', {name:'OK', exact:true}).click();
  await dialog.getByRole('button', {name:'Yes', exact:true}).click();
  await expect(dialog).not.toBeVisible();
  expect((await last('change_chain')).chainName).toBe('_');

  await open('Change residue number');
  await expect(dialog.getByLabel('Mode', {exact:true})).toHaveValue('shift');
  await expect(dialog.getByLabel('Residue index', {exact:true})).toHaveValue('1');
  await expect(dialog.getByLabel('Renumber', {exact:true})).not.toBeChecked();
  await dialog.getByLabel('Mode', {exact:true}).selectOption('start');
  await dialog.getByRole('button', {name:'OK', exact:true}).click();
  await expect(dialog.getByRole('alert')).toHaveText('Residue index change was rejected');
  await dialog.getByLabel('Mode', {exact:true}).selectOption('shift');
  await dialog.getByLabel('Residue index', {exact:true}).fill('0');
  await dialog.getByRole('button', {name:'OK', exact:true}).click();
  await expect(dialog.getByRole('alert')).toContainText('must not be zero');
  await dialog.getByLabel('Residue index', {exact:true}).fill('2147483648');
  await dialog.getByRole('button', {name:'OK', exact:true}).click();
  await expect(dialog.getByRole('alert')).toContainText('integer range');
  expect((await last('renumber_residues')).shiftMode).toBe(false);
  await dialog.getByLabel('Residue index', {exact:true}).fill('1.5');
  await dialog.getByRole('button', {name:'OK', exact:true}).click();
  await expect(dialog).not.toBeVisible();
  expect(await last('renumber_residues')).toEqual({objectUid:3, selection:'', shiftMode:true, value:1, renumber:false});
  await open('Change residue number');
  await dialog.getByLabel('Mode', {exact:true}).selectOption('start');
  await dialog.getByLabel('Residue index', {exact:true}).fill('10000');
  await dialog.getByLabel('Renumber', {exact:true}).check();
  await dialog.getByRole('button', {name:'OK', exact:true}).click();
  await dialog.getByRole('button', {name:'No', exact:true}).click();
  expect((await last('renumber_residues')).value).toBe(1);
  await dialog.getByRole('button', {name:'OK', exact:true}).click();
  await dialog.getByRole('button', {name:'Yes', exact:true}).click();
  await expect(dialog).not.toBeVisible();
  expect(await last('renumber_residues')).toEqual({objectUid:3, selection:'', shiftMode:false, value:10000, renumber:true});

  await open('Merge molecule');
  await dialog.getByLabel('From molecule', {exact:true}).selectOption('3');
  await dialog.getByLabel('To molecule', {exact:true}).selectOption('3');
  await expect(dialog.getByRole('button', {name:'OK', exact:true})).toBeDisabled();
  await dialog.getByLabel('To molecule', {exact:true}).selectOption('5');
  await dialog.getByLabel('Copy (keep source atoms)', {exact:true}).uncheck();
  await dialog.getByLabel('Selection', {exact:true}).fill('invalid (((');
  await dialog.getByRole('button', {name:'OK', exact:true}).click();
  await expect(dialog.getByRole('alert')).toContainText('Invalid selection');
  await dialog.getByLabel('Selection', {exact:true}).fill('');
  await dialog.getByRole('button', {name:'OK', exact:true}).click();
  await expect(dialog).not.toBeVisible();
  expect(await last('merge_molecules')).toEqual({sourceObjectUid:3,targetObjectUid:5,selection:'',copy:false});
  await open('Merge molecule');
  await expect(dialog.getByLabel('From molecule', {exact:true})).toHaveValue('3');
  await expect(dialog.getByLabel('To molecule', {exact:true})).toHaveValue('5');
  await expect(dialog.getByLabel('Copy (keep source atoms)', {exact:true})).toBeChecked();
  await page.evaluate(async () => { window.__interactionSceneChanged(); await mutate('get_snapshot'); });
  await dialog.getByRole('button', {name:'OK', exact:true}).click();
  await expect(dialog.getByRole('alert')).toContainText('active scene changed');
  expect((await last('merge_molecules')).copy).toBe(false);
  expect(errors).toEqual([]);
});


test('secondary dialog gates modes, preserves errors, remembers molecule and resets options', async ({page}, testInfo) => {
  const errors = await openShell(page, {}, {secondary:true, interactions:true});
  const dialog = page.locator('#reference-dialog');
  const open = async () => {
    await page.locator('summary').filter({hasText:/^Tools$/}).click();
    await page.locator('[data-dialog="Reassign secondary str"]').click();
    await expect(dialog).toBeVisible();
  };
  const last = () => page.evaluate(() => window.__shellCalls.filter(c => c.command === 'assign_secondary_structure').at(-1)?.args);
  await open();
  await expect(dialog.locator('#dialog-title')).toHaveText('Reassign secondary structure');
  await expect(dialog.getByLabel('Mode', {exact:true})).toHaveValue('recalc');
  await expect(dialog.getByLabel('Angle', {exact:true})).toHaveValue('120');
  await expect(dialog.getByLabel('Angle', {exact:true})).toBeDisabled();
  await expect(dialog.getByLabel('Selection', {exact:true})).toBeDisabled();
  await dialog.getByLabel('Helix gap-fill angle (°)', {exact:true}).check();
  await dialog.getByLabel('Angle', {exact:true}).fill('181');
  await expect(dialog.getByRole('button', {name:'OK', exact:true})).toBeDisabled();
  await expect(dialog.getByRole('alert')).toContainText('180');
  await dialog.getByLabel('Mode', {exact:true}).selectOption('assign');
  await expect(dialog.getByLabel('Angle', {exact:true})).toBeDisabled();
  await expect(dialog.getByRole('button', {name:'OK', exact:true})).toBeEnabled();
  await dialog.getByRole('button', {name:'Choose Selection', exact:true}).click();
  await dialog.getByRole('button', {name:'Selection choice *', exact:true}).click();
  await expect(dialog.getByLabel('Selection', {exact:true})).toHaveValue('*');
  await dialog.getByLabel('Selection', {exact:true}).fill('invalid (((');
  await dialog.getByRole('button', {name:'OK', exact:true}).click();
  await expect(dialog.getByRole('alert')).toContainText('Invalid selection');
  await expect(dialog.getByLabel('Selection', {exact:true})).toHaveValue('invalid (((');
  await dialog.getByLabel('Molecule', {exact:true}).selectOption('5');
  await dialog.getByLabel('Selection', {exact:true}).fill('resi 1');
  await dialog.getByLabel('Type', {exact:true}).selectOption('4');
  await page.screenshot({path:testInfo.outputPath('tauri-secondary.png')});
  await dialog.getByRole('button', {name:'OK', exact:true}).click();
  await expect(dialog).not.toBeVisible();
  expect(await last()).toEqual({objectUid:5,mode:'assign',selection:'resi 1',secondaryType:4,ignoreBulge:false,angle:0});
  await open();
  await expect(dialog.getByLabel('Molecule', {exact:true})).toHaveValue('5');
  await expect(dialog.getByLabel('Mode', {exact:true})).toHaveValue('recalc');
  await expect(dialog.getByLabel('Selection', {exact:true})).toHaveValue('');
  await expect(dialog.getByLabel('Type', {exact:true})).toHaveValue('0');
  await expect(dialog.getByLabel('Angle', {exact:true})).toHaveValue('120');
  await expect(dialog.getByLabel('Helix gap-fill angle (°)', {exact:true})).not.toBeChecked();
  await dialog.getByLabel('Ignore β bulge', {exact:true}).check();
  await dialog.getByLabel('Helix gap-fill angle (°)', {exact:true}).check();
  await dialog.getByRole('button', {name:'OK', exact:true}).click();
  await expect(dialog).not.toBeVisible();
  expect(await last()).toEqual({objectUid:5,mode:'recalc',selection:'',secondaryType:0,ignoreBulge:true,angle:120});
  await open();
  await page.evaluate(async () => { window.__interactionSceneChanged(); await mutate('get_snapshot'); });
  await dialog.getByRole('button', {name:'OK', exact:true}).click();
  await expect(dialog.getByRole('alert')).toContainText('active scene changed');
  expect((await last()).ignoreBulge).toBe(true);
  await dialog.getByRole('button', {name:'Cancel', exact:true}).click();
  expect(errors).toEqual([]);
});


test('Tauri startup files wait for per-file options and retain shell destination context',async({page})=>{
  const errors=await openShell(page,{}, {files:true,fileDefaults:true,empty:true,startupPaths:['first.pdb','second.pdb','third.pdb']});
  const dialog=page.locator('#reference-dialog');
  await expect(dialog.getByLabel('Object:',{exact:true})).toHaveValue('first');
  expect(await page.evaluate(()=>window.__shellCalls.filter(c=>['open_paths','open_initial_paths','open_path_with_renderer_setup'].includes(c.command)))).toEqual([]);
  await dialog.getByRole('button',{name:'Cancel',exact:true}).click();
  await expect(dialog.getByLabel('Object:',{exact:true})).toHaveValue('second');
  await dialog.getByRole('button',{name:'OK',exact:true}).click();
  await expect(dialog.getByLabel('Object:',{exact:true})).toHaveValue('third');
  await dialog.getByRole('button',{name:'OK',exact:true}).click();
  await expect(dialog).not.toBeVisible();
  const calls=await page.evaluate(()=>window.__shellCalls.filter(c=>c.command==='open_path_with_renderer_setup'));
  expect(calls.map(call=>call.args.path)).toEqual(['second.pdb','third.pdb']);
  expect(calls[0].args.openContext.shell).toBe(true);
  expect(calls[0].args.openContext.sceneId).toBeUndefined();
  expect(calls[1].args.openContext.shell).toBe(true);
  expect(calls[1].args.openContext.sceneId).toBe(1);
  expect(await page.evaluate(()=>window.__shellCalls.filter(c=>c.command==='take_initial_paths').length)).toBe(1);
  expect(errors).toEqual([]);
});

test('cancelling every Tauri startup file clears the loading status without opening files',async({page})=>{
  const errors=await openShell(page,{}, {files:true,fileDefaults:true,empty:true,startupPaths:['cancelled.pdb']});
  const dialog=page.locator('#reference-dialog');
  await expect(dialog.getByLabel('Object:',{exact:true})).toHaveValue('cancelled');
  await dialog.getByRole('button',{name:'Cancel',exact:true}).click();
  await expect(dialog).not.toBeVisible();
  await expect(page.locator('#status-text')).toHaveText('Ready');
  expect(await page.evaluate(()=>window.__shellCalls.filter(c=>['open_paths','open_initial_paths','open_path_with_renderer_setup'].includes(c.command)))).toEqual([]);
  expect(errors).toEqual([]);
});

test('Tauri file opens show options for each file and preserve automatic representations',async({page},testInfo)=>{
  const errors=await openShell(page,{}, {files:true,fileDefaults:true});
  await page.evaluate(()=>{void openSelectedPaths(['first.pdb','second.pdb'],true);});
  const dialog=page.locator('#reference-dialog');
  await expect(dialog.getByLabel('Object:',{exact:true})).toHaveValue('first');
  await expect(dialog.getByLabel('Renderer type:',{exact:true})).toHaveValue('auto');
  await expect(dialog.getByLabel('Renderer name:',{exact:true})).toBeDisabled();
  expect(await page.evaluate(()=>window.__shellCalls.filter(c=>c.command==='open_path_with_renderer_setup').length)).toBe(0);
  await dialog.getByLabel('Renderer type:',{exact:true}).selectOption('cpk');
  await expect(dialog.getByLabel('Renderer name:',{exact:true})).toBeEnabled();
  await dialog.getByLabel('Renderer type:',{exact:true}).selectOption('auto');
  await page.screenshot({path:testInfo.outputPath('tauri-file-defaults.png')});
  await dialog.getByRole('button',{name:'OK',exact:true}).click();
  await expect(dialog.getByLabel('Object:',{exact:true})).toHaveValue('second');
  const calls=await page.evaluate(()=>window.__shellCalls.filter(c=>c.command==='open_path_with_renderer_setup'));
  expect(calls).toHaveLength(1);expect(calls[0].args.fileSetup.rendererSetup.rendererType).toBe('auto');
  await dialog.getByRole('button',{name:'Cancel',exact:true}).click();
  await expect(dialog).not.toBeVisible();
  expect(await page.evaluate(()=>window.__shellCalls.filter(c=>c.command==='open_path_with_renderer_setup').length)).toBe(1);
  expect(errors).toEqual([]);
});

test('superposition dialog retains failed input and saves only successful settings', async ({page}, testInfo) => {
  const key = 'cuemol2.ui.histories.mol_superpose';
  const errors = await openShell(page, {[key]: JSON.stringify({algo:'invalid',pivot:'invalid',refObjId:-1,movObjId:999})}, {superposition:true, interactions:true});
  const dialog = page.locator('#reference-dialog');
  const open = async () => {
    await page.locator('summary').filter({hasText:/^Tools$/}).click();
    await page.locator('[data-dialog="Molecular superposition"]').click();
    await expect(dialog).toBeVisible();
  };
  const last = () => page.evaluate(() => window.__shellCalls.filter(c => c.command === 'superpose_objects').at(-1)?.args);
  const stored = () => page.evaluate(key => JSON.parse(localStorage.getItem(key)), key);
  await open();
  await expect(dialog.getByLabel('Method', {exact:true})).toHaveValue('USalign');
  await expect(dialog.getByLabel('Pivot atom', {exact:true})).toHaveValue('auto');
  await expect(dialog.getByLabel('Reference molecule', {exact:true})).toHaveValue('3');
  await expect(dialog.getByLabel('Moving molecule', {exact:true})).toHaveValue('5');
  await expect(dialog.getByLabel('Reference selection', {exact:true})).toHaveValue('');
  await expect(dialog.getByLabel('Moving selection', {exact:true})).toHaveValue('');
  await expect(dialog.getByLabel('Write RMSD info file', {exact:true})).toBeDisabled();
  await dialog.getByRole('button', {name:'Choose Reference selection', exact:true}).click();
  await dialog.getByRole('button', {name:'Selection choice *', exact:true}).click();
  await dialog.getByLabel('Reference selection', {exact:true}).fill('invalid (((');
  await dialog.getByRole('button', {name:'OK', exact:true}).click();
  await expect(dialog.getByRole('alert')).toContainText('Invalid selection');
  await expect(dialog.getByLabel('Reference selection', {exact:true})).toHaveValue('invalid (((');
  expect((await stored()).algo).toBe('invalid');
  await dialog.getByLabel('Reference molecule', {exact:true}).selectOption('5');
  await dialog.getByLabel('Reference selection', {exact:true}).fill('chain A');
  await dialog.getByLabel('Moving selection', {exact:true}).fill('chain B');
  await dialog.getByLabel('Pivot atom', {exact:true}).selectOption('P');
  await dialog.getByLabel('Use xformMat property', {exact:true}).check();
  await dialog.getByLabel('Auto recenter', {exact:true}).uncheck();
  await page.screenshot({path:testInfo.outputPath('tauri-superposition.png')});
  await dialog.getByRole('button', {name:'OK', exact:true}).click();
  await expect(dialog).not.toBeVisible();
  expect(await last()).toEqual({algorithm:'USalign',pivot:'P',referenceObjectUid:5,referenceSelection:'chain A',movingObjectUid:5,movingSelection:'chain B',autoRecenter:false,useProperty:true,rmsdPath:null});
  const success = {algo:'USalign',pivot:'P',refObjId:5,movObjId:5,autoRecenter:false,useprop:true};
  expect(await stored()).toEqual(success);
  await open();
  await expect(dialog.getByLabel('Reference molecule', {exact:true})).toHaveValue('5');
  await expect(dialog.getByLabel('Moving molecule', {exact:true})).toHaveValue('5');
  await expect(dialog.getByLabel('Reference selection', {exact:true})).toHaveValue('');
  await expect(dialog.getByLabel('Moving selection', {exact:true})).toHaveValue('');
  await expect(dialog.getByLabel('Pivot atom', {exact:true})).toHaveValue('P');
  await expect(dialog.getByLabel('Use xformMat property', {exact:true})).toBeChecked();
  await dialog.getByLabel('Method', {exact:true}).selectOption('LSQ');
  await expect(dialog.getByLabel('Pivot atom', {exact:true})).toBeDisabled();
  await expect(dialog.getByLabel('Write RMSD info file', {exact:true})).toBeEnabled();
  await dialog.getByRole('button', {name:'Cancel', exact:true}).click();
  expect(await stored()).toEqual(success);
  await open();
  await expect(dialog.getByLabel('Method', {exact:true})).toHaveValue('USalign');
  await page.evaluate(async () => { window.__interactionSceneChanged(); await mutate('get_snapshot'); });
  await dialog.getByRole('button', {name:'OK', exact:true}).click();
  await expect(dialog.getByRole('alert')).toContainText('active scene changed');
  expect(await stored()).toEqual(success);
  await dialog.getByRole('button', {name:'Cancel', exact:true}).click();
  expect(errors).toEqual([]);
});


test('surface dialogs match defaults, reset drafts and keep failed input', async ({page}, testInfo) => {
  const errors = await openShell(page, {}, {surfaceDialogs:true, interactions:true});
  const dialog = page.locator('#reference-dialog');
  const open = async name => {
    await page.locator('summary').filter({hasText:/^Tools$/}).click();
    await page.locator(`[data-dialog="Mol surface ${name}"]`).click();
    await expect(dialog).toBeVisible();
  };
  const last = command => page.evaluate(command => window.__shellCalls.filter(call => call.command === command).at(-1)?.args, command);
  await open('generation');
  await expect(dialog.getByLabel('Use selection', {exact:true})).not.toBeChecked();
  await expect(dialog.getByLabel('Selection', {exact:true})).toBeDisabled();
  await expect(dialog.getByLabel('Object name', {exact:true})).toHaveValue('sf_Probe(1)');
  await expect(dialog.getByLabel('Point density (/Å)', {exact:true})).toHaveValue('2');
  await expect(dialog.getByLabel('Probe radius (Å)', {exact:true})).toHaveValue('1.4');
  await dialog.getByLabel('Use selection', {exact:true}).check();
  await dialog.getByRole('button', {name:'Choose Selection', exact:true}).click();
  await dialog.getByRole('button', {name:'Selection choice *', exact:true}).click();
  await dialog.getByLabel('Selection', {exact:true}).fill('invalid (((');
  await dialog.getByRole('button', {name:'OK', exact:true}).click();
  await expect(dialog.getByRole('alert')).toContainText('Invalid selection');
  await expect(dialog.getByLabel('Selection', {exact:true})).toHaveValue('invalid (((');
  await dialog.getByLabel('Selection', {exact:true}).fill('chain A');
  await dialog.getByLabel('Object name', {exact:true}).fill('custom');
  await dialog.getByLabel('Molecule', {exact:true}).selectOption('5');
  await expect(dialog.getByLabel('Object name', {exact:true})).toHaveValue('sf_Other');
  await expect(dialog.getByLabel('Selection', {exact:true})).toHaveValue('chain A');
  await dialog.getByLabel('Point density (/Å)', {exact:true}).fill('3');
  await dialog.getByLabel('Probe radius (Å)', {exact:true}).fill('1.8');
  await dialog.getByLabel('Algorithm', {exact:true}).selectOption('rust');
  await page.screenshot({path:testInfo.outputPath('tauri-surface-generation.png')});
  await dialog.getByRole('button', {name:'OK', exact:true}).click();
  await expect(dialog).not.toBeVisible();
  expect(await last('create_molecular_surface')).toEqual({objectUid:5,selection:'chain A',name:'sf_Other',density:3,probeRadius:1.8,backend:'rust'});
  await open('generation');
  await expect(dialog.getByLabel('Molecule', {exact:true})).toHaveValue('5');
  await expect(dialog.getByLabel('Use selection', {exact:true})).not.toBeChecked();
  await expect(dialog.getByLabel('Selection', {exact:true})).toHaveValue('');
  await expect(dialog.getByLabel('Point density (/Å)', {exact:true})).toHaveValue('2');
  await expect(dialog.getByLabel('Probe radius (Å)', {exact:true})).toHaveValue('1.4');
  await expect(dialog.getByLabel('Algorithm', {exact:true})).toHaveValue('rust');
  await dialog.getByRole('button', {name:'Cancel', exact:true}).click();
  await page.evaluate(() => showSurfaceDialog('regenerate', snapshot.surfaces[0]));
  await expect(dialog.locator('#dialog-title')).toHaveText('Regenerate surface');
  await expect(dialog.getByLabel('Selection', {exact:true})).toHaveValue('chain A');
  await expect(dialog.getByLabel('Selection', {exact:true})).toHaveAttribute('readonly', '');
  await expect(dialog.getByLabel('Point density (/Å)', {exact:true})).toHaveValue('10');
  await expect(dialog.getByLabel('Algorithm', {exact:true})).toHaveValue('auto');
  await dialog.getByLabel('Point density (/Å)', {exact:true}).fill('3');
  await page.evaluate(() => { window.__surfaceFailure = true; });
  await dialog.getByRole('button', {name:'OK', exact:true}).click();
  await expect(dialog.getByRole('alert')).toContainText('Surface generation failed');
  await expect(dialog.getByLabel('Point density (/Å)', {exact:true})).toHaveValue('3');
  await dialog.getByLabel('Algorithm', {exact:true}).selectOption('ball');
  await page.evaluate(() => { window.__surfaceFailure = false; });
  await dialog.getByRole('button', {name:'OK', exact:true}).click();
  await expect(dialog).not.toBeVisible();
  expect(await last('regenerate_surface')).toEqual({surfaceUid:10,density:3,backend:'ball'});
  await open('cutter');
  await expect(dialog.getByLabel('Cross section type', {exact:true})).toHaveValue('full');
  await expect(dialog.getByLabel('Section mesh density (/Å)', {exact:true})).toHaveValue('5');
  await dialog.getByLabel('Cross section type', {exact:true}).selectOption('separate');
  await dialog.getByRole('button', {name:'OK', exact:true}).click();
  await expect(dialog).not.toBeVisible();
  expect(await last('cut_surface')).toEqual({surfaceUid:10,density:5,mode:'separate'});
  await open('cutter');
  await expect(dialog.getByLabel('Cross section type', {exact:true})).toHaveValue('full');
  await dialog.getByLabel('Section mesh density (/Å)', {exact:true}).fill('51');
  await expect(dialog.getByRole('button', {name:'OK', exact:true})).toBeDisabled();
  await dialog.getByRole('button', {name:'Cancel', exact:true}).click();
  await open('generation');
  await page.evaluate(async () => { window.__interactionSceneChanged(); await mutate('get_snapshot'); });
  await dialog.getByRole('button', {name:'OK', exact:true}).click();
  await expect(dialog.getByRole('alert')).toContainText('active scene changed');
  expect(errors).toEqual([]);
});


test('Tauri new renderer uses the selected object history and follows only default names', async ({page}, testInfo) => {
  const errors = await openShell(page, {}, {rendererHistory:true});
  const open = () => page.evaluate(() => showNewWorkspaceRendererDialog(snapshot.scenes[0], 'renderer', snapshot.objects[0]));
  await page.locator("#inspector-add-renderer").click();
  const dialog = page.locator('#reference-dialog');
  await expect(dialog.getByLabel('Renderer type', {exact:true})).toHaveValue('ribbon');
  await expect(dialog.getByLabel('Renderer name', {exact:true})).toHaveValue('ribbon1');
  await dialog.getByLabel('Renderer type', {exact:true}).selectOption('cpk');
  await expect(dialog.getByLabel('Renderer name', {exact:true})).toHaveValue('cpk1');
  await dialog.getByLabel('Renderer name', {exact:true}).fill('custom');
  await dialog.getByLabel('Renderer type', {exact:true}).selectOption('simple');
  await expect(dialog.getByLabel('Renderer name', {exact:true})).toHaveValue('custom');
  await dialog.getByLabel('Renderer name', {exact:true}).fill('');
  await expect(dialog.getByLabel('Renderer name', {exact:true})).toHaveValue('simple1');
  await dialog.getByLabel('Renderer type', {exact:true}).selectOption('Default1RendPreset');
  await expect(dialog.getByLabel('Renderer name', {exact:true})).toHaveValue('default1_1');
  await expect(dialog.getByLabel('Use selection', {exact:true})).toBeDisabled();
  await expect(dialog.getByLabel('Selection', {exact:true})).toBeDisabled();
  await page.screenshot({path:testInfo.outputPath('tauri-renderer-history.png')});
  await dialog.getByRole('button', {name:'Cancel', exact:true}).click();
  await open();
  await expect(dialog.getByLabel('Renderer type', {exact:true})).toHaveValue('ribbon');
  await dialog.getByRole('button', {name:'Cancel', exact:true}).click();
  expect(await page.evaluate(() => window.__shellCalls.filter(call=>call.command==='create_workspace_renderer').length)).toBe(0);
  expect(errors).toEqual([]);
});


test('Tauri new renderer keeps failed edits, gates presets in groups and passes map centering', async ({page}, testInfo) => {
  const errors = await openShell(page, {}, {rendererSetup:true,density:true,interactions:true});
  const dialog = page.locator('#reference-dialog');
  const open = () => page.evaluate(() => showNewWorkspaceRendererDialog(snapshot.scenes[0], 'renderer', snapshot.objects[0]));
  await open();
  await expect(dialog.getByLabel('Recenter view', {exact:true})).toBeChecked();
  await dialog.getByLabel('Object name', {exact:true}).fill('Ignored object rename');
  await dialog.getByRole('button', {name:'Choose Selection', exact:true}).click();
  await dialog.getByRole('button', {name:'Selection choice *', exact:true}).click();
  await dialog.getByLabel('Selection', {exact:true}).fill('invalid (((');
  await dialog.getByLabel('Renderer name', {exact:true}).fill('custom');
  await dialog.getByRole('button', {name:'Create', exact:true}).click();
  await expect(dialog.getByRole('alert')).toContainText('Invalid selection');
  await expect(dialog.getByLabel('Selection', {exact:true})).toHaveValue('invalid (((');
  await expect(page.locator('#error-alert-dialog')).not.toBeVisible();
  await dialog.getByLabel('Renderer type', {exact:true}).selectOption('Default1RendPreset');
  await dialog.getByRole('button', {name:'Create', exact:true}).click();
  await expect(dialog).not.toBeVisible();
  let input = await page.evaluate(() => window.__shellCalls.filter(call=>call.command==='create_workspace_renderer').at(-1).args.input);
  expect(input).toMatchObject({sceneUid:1,objectUid:3,rendererType:'Default1RendPreset',rendererName:'custom',selection:null,recenter:true});
  expect(await page.evaluate(()=>snapshot.objects[0].name)).toBe('Probe');
  await page.evaluate(() => {
    snapshot.objects[0].renderers.push({uid:99,name:'bundle',rendererType:'*group'});
    return showNewWorkspaceRendererDialog(snapshot.scenes[0], 'renderer', snapshot.objects[0], 'bundle');
  });
  await expect(page.locator('#dialog-title')).toHaveText('New Renderer (group: bundle)');
  await expect(dialog.getByLabel('Renderer type', {exact:true}).locator('option[value="Default1RendPreset"]')).toHaveCount(0);
  await page.screenshot({path:testInfo.outputPath('tauri-new-renderer-group.png')});
  await dialog.getByRole('button', {name:'Cancel', exact:true}).click();
  await page.evaluate(() => showNewWorkspaceRendererDialog(snapshot.scenes[0], 'densityRenderer', snapshot.densityMaps[0]));
  await expect(dialog.getByLabel('Renderer type', {exact:true})).toHaveValue('isosurf');
  await expect(dialog.getByLabel('View after loading', {exact:true})).toHaveValue('auto');
  await dialog.getByLabel('View after loading', {exact:true}).selectOption('view');
  const fitCalls = await page.evaluate(() => window.__shellCalls.filter(call=>call.command==='fit_workspace_nodes').length);
  await dialog.getByRole('button', {name:'Create', exact:true}).click();
  await expect(dialog).not.toBeVisible();
  input = await page.evaluate(() => window.__shellCalls.filter(call=>call.command==='create_workspace_renderer').at(-1).args.input);
  expect(input).toMatchObject({sceneUid:1,objectUid:10,densityCentering:'view',recenter:null});
  expect(await page.evaluate(() => window.__shellCalls.filter(call=>call.command==='fit_workspace_nodes').length)).toBe(fitCalls);
  await open();
  const count = await page.evaluate(() => window.__shellCalls.filter(call=>call.command==='create_workspace_renderer').length);
  await page.evaluate(async () => { window.__interactionSceneChanged(); await mutate('get_snapshot'); });
  await dialog.getByRole('button', {name:'Create', exact:true}).click();
  await expect(dialog.getByRole('alert')).toContainText('active scene changed');
  expect(await page.evaluate(() => window.__shellCalls.filter(call=>call.command==='create_workspace_renderer').length)).toBe(count);
  expect(errors).toEqual([]);
});

test('Tauri new renderer discards late options after cancel or another dialog', async ({page}) => {
  const errors = await openShell(page, {}, {rendererSetup:true});
  const start = () => page.evaluate(() => {
    window.__holdRendererOptions = true;
    void showNewWorkspaceRendererDialog(snapshot.scenes[0], 'renderer', snapshot.objects[0]);
  });
  await start();
  await expect.poll(() => page.evaluate(() => typeof window.__releaseRendererOptions)).toBe('function');
  await page.locator('#reference-dialog').getByRole('button', {name:'Cancel', exact:true}).click();
  await page.evaluate(() => window.__releaseRendererOptions());
  await expect(page.locator('#reference-dialog')).not.toBeVisible();
  await page.evaluate(() => { window.__releaseRendererOptions = null; });
  await start();
  await expect.poll(() => page.evaluate(() => typeof window.__releaseRendererOptions)).toBe('function');
  await page.evaluate(() => {
    referenceDialog.close(); dialogContent.replaceChildren(); dialogTitle.textContent='Another dialog'; referenceDialog.showModal();
    window.__releaseRendererOptions();
  });
  await expect(page.locator('#dialog-title')).toHaveText('Another dialog');
  await expect(page.locator('#dialog-content')).toBeEmpty();
  expect(errors).toEqual([]);
});

test('Tauri renderer preset names share file and object collision rules', async ({page}) => {
  const errors = await openShell(page, {}, {rendererSetup:true});
  await page.evaluate(() => {
    snapshot.lwObjects = [{sceneUid:1,renderers:[{name:'default1_1'}],rendererGroups:[{name:'customshort_1'}]}];
    void showRendererSetup({targetKind:'molecule',objectName:'file',rendererType:'cpk',rendererName:'cpk1',recenter:true,
      options:[{name:'cpk',description:'CPK',preset:false},{name:'Default1RendPreset',description:'Default',preset:true},
        {name:'CustomShort',description:'Custom',preset:true}]});
  });
  const dialog = page.locator('#reference-dialog');
  await dialog.getByLabel('Renderer type:', {exact:true}).selectOption('CustomShort');
  await expect(dialog.getByLabel('Renderer name:', {exact:true})).toHaveValue('customshort_2');
  await dialog.getByLabel('Renderer type:', {exact:true}).selectOption('Default1RendPreset');
  await expect(dialog.getByLabel('Renderer name:', {exact:true})).toHaveValue('default1_2');
  await dialog.getByRole('button', {name:'Cancel', exact:true}).click();
  expect(errors).toEqual([]);
});


test('Tauri morph dialog converts explicitly, protects the base and filters source molecules', async ({page}, testInfo) => {
  const errors=await openShell(page, {}, {morphDialogs:true});
  const dialog=page.locator('#reference-dialog');
  const open=()=>page.evaluate(()=>showReferenceDialog('Mol morphing animation'));
  await open();
  await expect(dialog.getByLabel('Target molecule',{exact:true})).toHaveValue('3');
  expect(await page.evaluate(()=>window.__shellCalls.filter(c=>c.command==='initialize_morph_object').length)).toBe(0);
  await dialog.getByRole('button',{name:'Close',exact:true}).last().click();
  await open();
  await dialog.getByRole('button',{name:'Convert to MorphMol',exact:true}).click();
  await expect(dialog.locator('.morph-frame-row')).toHaveCount(1);
  await dialog.locator('.morph-frame-row').click();
  await expect(dialog.getByRole('button',{name:'Delete',exact:true})).toBeDisabled();
  await dialog.getByLabel('Add morph frame',{exact:true}).click();
  await dialog.getByRole('button',{name:'Add mol...',exact:true}).click();
  await expect(dialog.getByLabel('Source molecule',{exact:true}).locator('option')).toHaveCount(1);
  await expect(dialog.getByLabel('Source molecule',{exact:true})).toHaveValue('5');
  await dialog.getByRole('button',{name:'Add',exact:true}).click();
  await expect(dialog.locator('.morph-frame-row')).toHaveCount(2);
  await dialog.locator('.morph-frame-row').first().click();
  await page.evaluate(()=>{window.__morphPaths=['first.pdb','second.ent.gz'];});
  await dialog.getByLabel('Add morph frame',{exact:true}).click();
  await dialog.getByRole('button',{name:'Add PDB file...',exact:true}).click();
  await expect(dialog.locator('.morph-frame-row')).toHaveCount(4);
  expect(await page.evaluate(()=>window.__morphPickerOptions.multiple)).toBe(true);
  expect(await page.evaluate(()=>window.__shellCalls.filter(c=>c.command==='insert_morph_frame_from_file').map(c=>[c.args.sceneUid,c.args.targetObjectUid,c.args.insertBefore]))).toEqual([[1,3,1],[1,3,2]]);
  await expect(dialog.locator('.morph-frame-row.active')).toContainText('second.ent.gz');
  await dialog.getByRole('separator',{name:'Name column width'}).focus();
  await page.keyboard.press('ArrowRight');
  const resizedWidth=await dialog.getByRole('separator',{name:'Name column width'}).getAttribute('aria-valuenow');
  expect(Number(resizedWidth)).toBeGreaterThan(180);
  await page.screenshot({path:testInfo.outputPath('tauri-morph-frames.png')});
  await dialog.getByRole('button',{name:'Delete',exact:true}).click();
  await expect(dialog.locator('.morph-frame-row')).toHaveCount(3);
  await expect(dialog.getByRole('button',{name:'Delete',exact:true})).toBeDisabled();
  await dialog.getByLabel('Target molecule',{exact:true}).selectOption('5');
  await expect(dialog.getByRole('button',{name:'Convert to MorphMol',exact:true})).toBeVisible();
  await dialog.getByRole('button',{name:'Close',exact:true}).last().click();
  await open();
  await expect(dialog.getByLabel('Target molecule',{exact:true})).toHaveValue('3');
  await expect(dialog.locator('.morph-frame-row')).toHaveCount(3);
  await expect(dialog.getByRole('separator',{name:'Name column width'})).toHaveAttribute('aria-valuenow',resizedWidth);
  expect(errors).toEqual([]);
});

test('Tauri morph keeps successful files after an error and discards a late file picker', async ({page}) => {
  const errors=await openShell(page, {}, {morphDialogs:true});
  const dialog=page.locator('#reference-dialog');
  const open=()=>page.evaluate(()=>showReferenceDialog('Mol morphing animation'));
  await open();
  await dialog.getByRole('button',{name:'Convert to MorphMol',exact:true}).click();
  await page.evaluate(()=>{window.__morphPaths=['good.pdb','bad.pdb','skipped.pdb'];});
  await dialog.getByLabel('Add morph frame',{exact:true}).click();
  await dialog.getByRole('button',{name:'Add PDB file...',exact:true}).click();
  await expect(dialog.getByRole('alert')).toContainText('Invalid PDB frame');
  await expect(page.locator('#error-alert-dialog')).not.toBeVisible();
  await expect(dialog.locator('.morph-frame-row')).toHaveCount(2);
  await expect(dialog.locator('.morph-frame-row.active')).toHaveCount(0);
  expect(await page.evaluate(()=>window.__shellCalls.filter(c=>c.command==='insert_morph_frame_from_file').map(c=>c.args.path))).toEqual(['good.pdb','bad.pdb']);
  await page.evaluate(()=>{window.__delayMorphPicker=true;});
  await dialog.getByLabel('Add morph frame',{exact:true}).click();
  await dialog.getByRole('button',{name:'Add PDB file...',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>typeof window.__releaseMorphPicker)).toBe('function');
  await dialog.getByRole('button',{name:'Close',exact:true}).last().click(); await open();
  await page.evaluate(()=>window.__releaseMorphPicker(['late.pdb']));
  await expect(dialog.locator('.morph-frame-row')).toHaveCount(2);
  expect(await page.evaluate(()=>window.__shellCalls.filter(c=>c.command==='insert_morph_frame_from_file').length)).toBe(2);
  await page.evaluate(async()=>{window.__morphSceneChanged();await mutate('get_snapshot');});
  await dialog.getByLabel('Add morph frame',{exact:true}).click();
  expect(await page.evaluate(()=>window.__shellCalls.filter(c=>c.command==='insert_morph_frame_from_file').length)).toBe(2);
  expect(errors).toEqual([]);
});

test('Tauri APBS resets defaults, validates inline, and retains success history only', async ({page}, testInfo) => {
  const errors = await openShell(page, {}, {apbsDialogs:true});
  const dialog = page.locator('#reference-dialog');
  const open = () => page.evaluate(() => showReferenceDialog('APBS elepot calculation'));
  const starts = () => page.evaluate(() => window.__shellCalls.filter(c=>c.command==='start_apbs_calculation'));
  await open();
  await expect(dialog.locator('#dialog-title')).toHaveText('APBS electrostatic potential');
  await expect(dialog.getByLabel('Target molecule',{exact:true})).toHaveValue('3');
  await expect(dialog.getByLabel('Selection',{exact:true})).not.toBeChecked();
  await expect(dialog.getByLabel('Selection code',{exact:true})).toHaveValue('');
  await expect(dialog.getByLabel('Force field',{exact:true})).toHaveValue('amber');
  await expect(dialog.getByLabel('Temperature (K)',{exact:true})).toHaveValue('298.15');
  await expect(dialog.getByLabel('Temperature (K)',{exact:true})).toHaveAttribute('step','0.01');
  await expect(dialog.getByLabel('APBS exe path',{exact:true})).toHaveCount(0);
  await expect(dialog.getByLabel('Use hydrogen atoms',{exact:true})).toBeHidden();
  await dialog.getByLabel('Target molecule',{exact:true}).selectOption('5');
  await expect(dialog.getByLabel('Potential name',{exact:true})).toHaveValue('pot_Second');
  await dialog.getByLabel('Potential name',{exact:true}).fill('custom');
  await dialog.getByLabel('Target molecule',{exact:true}).selectOption('3');
  await expect(dialog.getByLabel('Potential name',{exact:true})).toHaveValue('pot_Probe');
  await dialog.getByLabel('Max grid size (Å)',{exact:true}).fill('11');
  await expect(dialog.getByRole('button',{name:'Start',exact:true})).toBeDisabled();
  await expect(dialog.getByRole('alert')).toContainText('between 0.1 and 10');
  await dialog.getByLabel('Max grid size (Å)',{exact:true}).fill('1');
  await dialog.getByLabel('Selection',{exact:true}).check();
  await dialog.getByLabel('Selection code',{exact:true}).fill('invalid (((');
  await dialog.getByRole('button',{name:'Start',exact:true}).click();
  await expect(dialog.getByRole('alert')).toContainText('Invalid selection');
  expect((await starts())).toHaveLength(1);
  expect(await page.evaluate(()=>window.__shellCalls.filter(c=>c.command==='inspector_request'&&c.args.tool==='selection_history'&&c.args.args.action==='remember'&&JSON.stringify(c.args).includes('invalid')).length)).toBe(0);
  await dialog.getByLabel('Selection code',{exact:true}).fill('name CA');
  await dialog.getByLabel('Potential name',{exact:true}).fill('');
  await dialog.getByRole('button',{name:'Start',exact:true}).click();
  await expect(dialog.getByRole('button',{name:'Stop',exact:true})).toBeEnabled();
  await expect(dialog.getByRole('button',{name:'Close',exact:true}).last()).toBeDisabled();
  await expect(dialog.getByLabel('Target molecule',{exact:true})).toBeDisabled();
  await expect(dialog.getByRole('status')).toContainText('Running PDB2PQR');
  await page.evaluate(()=>{window.__apbsJob.status='Running APBS...';window.__apbsJob.log='PDB2PQR complete\nSolving potential';});
  await expect(dialog.getByRole('status')).toContainText('Running APBS');
  await dialog.locator('summary').filter({hasText:'Calculation log'}).click();
  await expect(dialog.locator('pre')).toContainText('Solving potential');
  await page.screenshot({path:testInfo.outputPath('apbs-progress.png')});
  const request = (await starts())[1].args.input;
  expect(request).toMatchObject({sceneUid:1,objectUid:3,selection:'name CA',potentialName:'',forceField:'amber',temperature:298.15});
  expect(request.requestId).toBeTruthy();
  await dialog.getByRole('button',{name:'Stop',exact:true}).click();
  await expect(dialog.getByRole('button',{name:'Start',exact:true})).toBeEnabled();
  await expect(dialog.getByRole('status')).toContainText('stopped');
  await expect(dialog.getByRole('alert')).toHaveText('');
  await dialog.getByRole('button',{name:'Start',exact:true}).click();
  await expect(dialog.getByRole('button',{name:'Stop',exact:true})).toBeEnabled();
  await page.evaluate(()=>{window.__apbsResult='error:APBS solver failed';});
  await expect(dialog.getByRole('alert')).toContainText('APBS solver failed');
  await expect(dialog.getByRole('button',{name:'Start',exact:true})).toBeEnabled();
  await dialog.getByRole('button',{name:'Start',exact:true}).click();
  await expect(dialog.getByRole('button',{name:'Stop',exact:true})).toBeEnabled();
  await page.evaluate(()=>{window.__apbsResult='complete:10';});
  await expect(dialog).not.toBeVisible();
  await expect.poll(()=>page.evaluate(()=>window.__shellCalls.filter(c=>c.command==='inspector_request'&&c.args.tool==='selection_history'&&c.args.args.action==='remember'&&c.args.args.expression==='name CA').length)).toBe(1);
  await open();
  await expect(dialog.getByLabel('Selection',{exact:true})).not.toBeChecked();
  await expect(dialog.getByLabel('Selection code',{exact:true})).toHaveValue('');
  await expect(dialog.getByLabel('Potential name',{exact:true})).toHaveValue('pot_Probe');
  await expect(dialog.getByLabel('Method',{exact:true})).toHaveValue('pdb2pqr');
  const count = (await starts()).length;
  await page.evaluate(async()=>{window.__apbsSceneChanged();await mutate('get_snapshot');});
  await dialog.getByRole('button',{name:'Start',exact:true}).click();
  await expect(dialog.getByRole('alert')).toContainText('active scene changed');
  expect((await starts())).toHaveLength(count);
  expect(errors).toEqual([]);
});

test('Tauri APBS gates missing tools and cancels a detached pending start by request ID', async ({page}) => {
  const errors=await openShell(page, {}, {apbsDialogs:true});
  const dialog=page.locator('#reference-dialog');
  const open=()=>page.evaluate(()=>showReferenceDialog('APBS elepot calculation'));
  await page.evaluate(()=>{window.__apbsSettings.pdb2pqrExecutable='';});
  await open();
  await expect(dialog.getByRole('button',{name:'Start',exact:true})).toBeDisabled();
  await expect(dialog.getByRole('alert')).toContainText('PDB2PQR executable');
  await dialog.getByLabel('Method',{exact:true}).selectOption('internal');
  await expect(dialog.getByRole('button',{name:'Start',exact:true})).toBeEnabled();
  await expect(dialog.getByLabel('Force field',{exact:true})).toBeHidden();
  await expect(dialog.getByLabel('Use hydrogen atoms',{exact:true})).toBeVisible();
  await page.evaluate(()=>{window.__apbsHoldStart=true;});
  await dialog.getByRole('button',{name:'Start',exact:true}).click();
  await expect(dialog.getByRole('button',{name:'Stop',exact:true})).toBeDisabled();
  await expect.poll(()=>page.evaluate(()=>typeof window.__apbsReleaseStart)).toBe('function');
  await page.evaluate(()=>showReferenceDialog('About config'));
  await page.evaluate(()=>window.__apbsReleaseStart());
  await expect.poll(()=>page.evaluate(()=>window.__shellCalls.filter(c=>c.command==='cancel_apbs_calculation').length)).toBeGreaterThan(0);
  await expect.poll(()=>page.evaluate(()=>window.__apbsJob)).toBeNull();
  await expect(dialog).toBeVisible();
  expect(await page.evaluate(()=>{const calls=window.__shellCalls;return calls.find(c=>c.command==='start_apbs_calculation').args.input.requestId===calls.find(c=>c.command==='cancel_apbs_calculation').args.requestId;})).toBe(true);
  expect(errors).toEqual([]);
});


test('Tauri geometry export fixes the format and preserves the scene on picker cancellation', async ({page}) => {
  const errors = await openShell(page);
  for (const [format, label, menu] of [['stl','StereoLithography','STL…'],['mqo','Metasequoia object','Metasequoia (MQO)…'],['pov','POV-Ray SDL','POV-Ray SDL…']]) {
    await page.locator('summary').filter({hasText:/^Rendering$/}).click();
    await page.getByRole('button', {name:'Export scene',exact:true}).hover();
    await page.getByRole('button', {name:menu,exact:true}).click();
    await expect.poll(() => page.evaluate(() => window.__shellCalls.filter(call => call.command === 'export_scene_geometry').at(-1)?.args)).toEqual({path:`Test scene.${format}`,format});
    expect(await page.evaluate(() => window.__saveDialogOptions)).toEqual({defaultPath:`Test scene.${format}`,filters:[{name:label,extensions:[format]}]});
  }
  const before = await page.evaluate(() => window.__shellCalls.filter(call => call.command === 'export_scene_geometry').length);
  await page.evaluate(() => { window.__cancelSave = true; });
  await page.locator('summary').filter({hasText:/^Rendering$/}).click();
  await page.getByRole('button', {name:'Export scene',exact:true}).hover();
  await page.getByRole('button', {name:'STL…',exact:true}).click();
  await expect(page.locator('#status-text')).toHaveText('Ready');
  expect(await page.evaluate(() => window.__shellCalls.filter(call => call.command === 'export_scene_geometry').length)).toBe(before);
  expect(errors).toEqual([]);
});


test('Tauri image export chooses the path before options and releases every cancelled session', async ({page}) => {
  const errors = await openShell(page);
  const calls = operation => page.evaluate(operation => window.__shellCalls.filter(c => c.command==='image_export_request' && c.args.operation===operation), operation);
  await page.evaluate(()=>{window.__cancelSave=true;void exportPng();});
  await expect.poll(async()=>(await calls('release')).length).toBe(1);
  expect(await calls('start')).toHaveLength(0);
  await expect(page.locator('#reference-dialog')).toBeHidden();
  await page.evaluate(()=>{window.__cancelSave=false;window.__holdSave=true;void exportPng('umbreon');});
  await expect.poll(()=>page.evaluate(()=>typeof window.__resolveSave)).toBe('function');
  expect(await page.evaluate(()=>window.__optionsVisibleAtSave)).toBe(false);
  await expect(page.locator('#reference-dialog')).toBeHidden();
  await page.evaluate(()=>{window.__holdSave=false;window.__resolveSave('target.png');});
  const options=page.locator('#reference-dialog');
  await expect(options).toBeVisible();
  await expect(options.getByLabel('Resolution (DPI)',{exact:true})).toHaveValue('150');
  await expect(options.getByLabel('Retain aspect ratio',{exact:true})).toBeChecked();
  await expect(options.getByLabel('Height',{exact:true})).toBeDisabled();
  await options.getByRole('button',{name:'Cancel',exact:true}).click();
  await expect.poll(async()=>(await calls('release')).length).toBe(2);
  expect(await calls('start')).toHaveLength(0);
  expect(errors).toEqual([]);
});

test('Tauri image export validates dimensions, fixes the backend, and cancels its worker', async ({page}) => {
  const errors=await openShell(page);
  const dialog=page.locator('#reference-dialog');
  const starts=()=>page.evaluate(()=>window.__shellCalls.filter(c=>c.command==='image_export_request'&&c.args.operation==='start'));
  for(const [kind,label] of [['png','PNG image…'],['umbreon','Umbreon ray-traced image…']]) {
    await page.locator('summary').filter({hasText:/^Rendering$/}).click();
    await page.getByRole('button',{name:'Export scene',exact:true}).hover();
    await page.getByRole('button',{name:label,exact:true}).click();
    await expect(dialog).toBeVisible();
    await dialog.getByLabel('Retain aspect ratio',{exact:true}).uncheck();
    await dialog.getByLabel('Width',{exact:true}).fill('9000');
    await dialog.getByLabel('Height',{exact:true}).fill('48');
    await dialog.getByRole('button',{name:'OK',exact:true}).click();
    await expect(dialog).toContainText('at most 8192');
    await dialog.getByLabel('Width',{exact:true}).fill('64');
    await dialog.getByLabel('Transparent PNG',{exact:true}).check();
    await page.evaluate(()=>{window.__holdImage=true;});
    await dialog.getByRole('button',{name:'OK',exact:true}).click();
    const progress=page.locator('#stream-progress-dialog');
    await expect(progress).toBeVisible();
    const start=(await starts()).at(-1).args.args;
    expect(start).toMatchObject({kind,path:'cuemol.png',width:64,height:48,resolutionDpi:150,transparent:true});
    if(kind==='png') {
      await progress.getByRole('button',{name:'Cancel',exact:true}).click();
      await expect(page.locator('#status-text')).toHaveText('Image export cancelled');
    } else {
      await page.evaluate(()=>{window.__holdImage=false;});
      await expect(page.locator('#status-text')).toHaveText('Exported image: cuemol.png');
    }
    await expect(progress).toBeHidden();
    await expect.poll(()=>page.evaluate(()=>imageExportActive)).toBe(false);
  }
  await page.evaluate(()=>{window.__imageError='output is not writable';});
  await page.locator('summary').filter({hasText:/^Rendering$/}).click();
  await page.getByRole('button',{name:'Export scene',exact:true}).hover();
  await page.getByRole('button',{name:'PNG image…',exact:true}).click();
  await expect(dialog).toBeVisible();
  await dialog.getByRole('button',{name:'OK',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>imageExportActive)).toBe(false);
  await expect(page.locator('#error-alert-dialog')).toContainText('output is not writable');
  expect(errors).toEqual([]);
});


test('Tauri menus use the current groups and focus Rendering without a mode preset', async ({page}) => {
  const errors = await openShell(page);
  expect(await page.locator('.application-menu > .menu > summary').allTextContents()).toEqual([
    'File','Edit','Rendering','Scene','View','Tools','Window','Help',
  ]);
  await page.evaluate(() => {
    window.__focusedWindows = [];
    window.__renderFocusRequests = [];
    window.__TAURI__.webviewWindow = {getCurrentWebviewWindow:() => ({setFocus:async() => __focusedWindows.push('main')})};
    CueMolRendering.open = async (...args) => __renderFocusRequests.push(args);
  });
  const file = page.locator('.menu').filter({has:page.locator('summary', {hasText:/^File$/})});
  await file.locator('summary').click();
  await expect(file.getByRole('button', {name:'New Window', exact:true})).toHaveCount(0);
  await expect(file.getByRole('button', {name:'Quit', exact:true})).toBeVisible();
  const windowMenu = page.locator('.menu').filter({has:page.locator('summary', {hasText:/^Window$/})});
  await windowMenu.locator('summary').click();
  await windowMenu.getByRole('button', {name:'Rendering Window', exact:true}).click();
  await expect.poll(() => page.evaluate(() => __renderFocusRequests)).toEqual([[]]);
  await windowMenu.locator('summary').click();
  await windowMenu.getByRole('button', {name:'Main Window', exact:true}).click();
  await expect.poll(() => page.evaluate(() => __focusedWindows)).toEqual(['main']);
  await expect(page.locator('#viewport')).toBeFocused();
  expect(errors).toEqual([]);
});


test('Tauri native menus request the separate Rendering window with explicit mode only for render actions', async ({page}) => {
  const errors=await openShell(page,{}, {nativeRenderingWindow:true});
  const windowMenu=page.locator('.menu').filter({has:page.locator('summary',{hasText:/^Window$/})});
  await windowMenu.locator('summary').click();
  await windowMenu.getByRole('button',{name:'Rendering Window',exact:true}).click();
  const rendering=page.locator('.menu').filter({has:page.locator('summary',{hasText:/^Rendering$/})});
  await rendering.locator('summary').click();
  await rendering.getByRole('button',{name:'Movie rendering…',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>__shellCalls.filter(call=>call.command==='render_window_request').map(call=>call.args))).toEqual([
    {operation:'open',args:{}},{operation:'open',args:{movie:true}},
  ]);
  await expect(page.getByRole('dialog',{name:'Rendering',exact:true})).toHaveCount(0);
  expect(errors).toEqual([]);
});


test('Tauri application close flushes remote settings and keeps errors or cancelled exits open', async ({page}) => {
  const errors=await openShell(page,{}, {closeGuard:true});
  await page.evaluate(()=>{window.__closePreparationError=true;void __requestApplicationClose();});
  const alert=page.locator('#error-alert-dialog');
  await expect(alert).toBeVisible();
  await expect(alert).toContainText('Pending render settings could not be saved');
  expect(await page.evaluate(()=>__closedWindows)).toBe(0);
  await alert.getByRole('button',{name:'OK',exact:true}).click();
  await page.evaluate(()=>{window.__closePreparationError=false;void __requestApplicationClose();});
  const confirmation=page.locator('#reference-dialog.confirm-close-dialog');
  await expect(confirmation).toBeVisible();
  await confirmation.getByRole('button',{name:'Cancel',exact:true}).click();
  expect(await page.evaluate(()=>__closedWindows)).toBe(0);
  await page.evaluate(()=>{void __requestApplicationClose();});
  await expect(confirmation).toBeVisible();
  await confirmation.getByRole('button',{name:"Don't Save",exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>__closedWindows)).toBe(1);
  expect(errors).toEqual([]);
});
