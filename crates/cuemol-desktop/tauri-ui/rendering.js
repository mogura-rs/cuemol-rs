"use strict";

const CueMolRendering = (() => {
  let dialogElement, state, catalog, controlsKey = '', queue = Promise.resolve(), pending = 0, switchingTargets = 0, timer;
  let tab = 'Image', scale = 1, center = [0.5, 0.5], fitted = false, previousSize = [1, 1];
  const nodes = {};
  const fieldNodes = new Map();
  const element = (tag, text, className) => {
    const node = document.createElement(tag);
    if (text !== undefined) node.textContent = text;
    if (className) node.className = className;
    return node;
  };
  const button = (label, action) => {
    const node = element('button', label); node.type = 'button';
    node.addEventListener('click', () => { Promise.resolve().then(action).catch(error); }); return node;
  };
  const error = value => { nodes.message.textContent = String(value); };
  function request(operation, args = {}) {
    const source = {sceneId: state?.sceneId, targetViewId: state?.viewId, imageId: state?.current?.id, imageKey: state?.imageKey, ...(typeof args === 'function' ? {} : args)};
    pending++;
    if(operation === 'target'){switchingTargets++;if(state)render();}
    const task = queue.then(async () => {
      const value = await invoke('rendering_request', {operation, args: typeof args === 'function' ? {...source, ...args()} : source});
      if (value.catalog) catalog = value.catalog;
      state = value; render(); return value;
    });
    queue = task.catch(error).finally(() => { pending--;if(operation === 'target'){switchingTargets--;render();} });
    return task;
  }
  const send = (operation, args) => { void request(operation, args).catch(() => {}); };
  function field(label, type, value, changed, options = {}) {
    const row = element('label', undefined, 'rendering-field');
    row.append(element('span', label));
    let input;
    if (type === 'enum') {
      input = element('select');
      for (const [id, title] of options.choices || []) {
        const option = element('option', title); option.value = JSON.stringify(id); input.append(option);
      }
      input.value = JSON.stringify(value);
    } else if (type === 'text') {
      input = element('textarea'); input.value = value ?? ''; input.rows = 4;
    } else {
      input = element('input'); input.type = type === 'boolean' ? 'checkbox' : type === 'color' ? 'color' : type === 'string' ? 'text' : 'number';
      if (type === 'boolean') input.checked = !!value;
      else input.value = value ?? '';
      if (input.type === 'number') {
        input.step = type === 'integer' ? 1 : options.step ?? 'any';
        if (options.min !== undefined) input.min = options.min;
        if (options.max !== undefined) input.max = options.max;
      }
    }
    input.setAttribute('aria-label', label);
    input.addEventListener(type === 'text' || type === 'string' || type === 'enum' || type === 'boolean' ? 'change' : 'input', () => {
      if (!input.checkValidity()) return;
      const value = type === 'enum' ? JSON.parse(input.value) : type === 'boolean' ? input.checked : ['text','string','color'].includes(type) ? input.value : Number(input.value);
      changed(value);
    });
    row.append(input); return {row, input, type};
  }
  function property(row, local, parent) {
    const key = local ? `${state.values.backend}.${row.key}` : row.key;
    const choices = (row.options || []).map((label,index) => [(row.enumValues || row.options)[index],label]);
    const control = field(row.label, row.type, state.values[key], value => send('edit', {key, value}), {...row, choices});
    control.input.dataset.renderProperty = key;
    fieldNodes.set(key, control); parent.append(control.row);
  }
  function section(title, parent, open = true) {
    const details = element('details'); details.open = open;
    details.append(element('summary', title)); parent.append(details); return details;
  }
  function movieProperty(parent,key,label,type,options={}) {
    const control=field(label,type,state.movieOptions[key],value=>send('movieEdit',{key,value}),options);
    fieldNodes.set(`movie:${key}`,control);parent.append(control.row);return control;
  }
  function moviePreset(parent,key,label,values) {
    const control=field(label,'enum','',value=>{if(value!=='')send('movieEdit',{key,value});},{choices:[['','Custom'],...values.map(value=>[value,String(value)])]});
    control.input.addEventListener('change',()=>{control.input.value=JSON.stringify('');});parent.append(control.row);
  }
  function buildMovie(parent) {
    const box=section('Movie',parent),location=element('div',undefined,'rendering-row');
    location.setAttribute('role','radiogroup');location.setAttribute('aria-label','Movie output location');
    for(const [value,label]of [[true,'Temporary'],[false,'Custom']]){
      const row=element('label'),input=element('input');input.type='radio';input.name='rendering-movie-location';input.setAttribute('aria-label',label);input.dataset.temporary=String(value);
      input.addEventListener('change',()=>{if(input.checked)send('movieEdit',{key:'useTempDir',value});});row.append(input,document.createTextNode(label));location.append(row);
    }
    box.append(location);
    movieProperty(box,'outputDir','Folder','string');
    box.append(button('Browse…',async()=>{
      const movieOptions=structuredClone(state.movieOptions),sceneId=state.sceneId;
      const selected=await dialog.open({directory:true,multiple:false,defaultPath:movieOptions.outputDir||undefined});
      if(selected)send('movieEdit',{key:'outputDir',value:selected,movieOptions,sceneId});
    }));
    movieProperty(box,'baseName','Base name','string');
    movieProperty(box,'fps','Frame rate','real',{min:0.001});moviePreset(box,'fps','Frame rate preset',[24,30,60]);
    movieProperty(box,'dupLastFrame','Duplicate last frame','boolean');
    movieProperty(box,'makeMovie','Encode movie','boolean');
    movieProperty(box,'movieFormat','Movie format','enum',{choices:state.movieFormats});
    movieProperty(box,'bitrateKbps','Bitrate (kbps)','integer',{min:1,max:4294967295});moviePreset(box,'bitrateKbps','Bitrate preset',[256,1024,10240]);
  }
  function buildImage(parent) {
    const choices=state.movieMode?(state.movieSizes||[]).map(([label],index)=>[index,label]):[[0,'Current view'],[1,'100 × 100 (72 dpi)'],[2,'300 × 300 (300 dpi)'],[3,'600 × 600 (300 dpi)'],[4,'1200 × 1200 (600 dpi)']];
    const presets=field('Size preset','enum','',index=>{if(index!=='')send('preset',{index});},{choices:[['','Custom'],...choices]});
    presets.input.addEventListener('change',()=>{presets.input.value=JSON.stringify('');});parent.append(presets.row);
    for(const key of ['width','height'])property({key,label:key==='width'?'Width':'Height',type:state.movieMode?'integer':'real',min:state.movieMode?1:0.001,max:10000},false,parent);
    if(!state.movieMode){
      property({key:'unit',label:'Size unit',type:'enum',options:['px','in','mm','cm']},false,parent);
      property({key:'dpi',label:'DPI',type:'real',min:1,max:4800},false,parent);
      const dpi=field('DPI preset','enum','',value=>{if(value!=='')send('edit',{key:'dpi',value});},{choices:[['','Custom'],...[72,150,300,600,1200,2400].map(value=>[value,String(value)])]});parent.append(dpi.row);
    }
    nodes.pixels=element('p');parent.append(nodes.pixels);
    for(const row of catalog.common.filter(row=>row.group==='Image'&&row.type==='boolean'&&(state.values.backend==='povray'||row.key==='transparentBg')))property(row,false,parent);
    if(state.movieMode)buildMovie(parent);
  }
  function buildRender(parent) {
    const backend = state.values.backend;
    if (backend !== 'povray') {
      const quality = section('Quality',parent);
      const choices = [['none','Raytrace only'],['ao','Ambient Occlusion'],...(backend === 'umbreon_npr'?[]:[['gi','Global Illumination']])];
      const lighting = field('Lighting','enum',state.lighting,value=>send('lighting',{value}),{choices});
      fieldNodes.set('@lighting',lighting); quality.append(lighting.row);
      for (const [axis,label] of [['aa','Supersampling'],['ao','AO quality'],['giLighting','GI lighting'],['shadows','Shadows']]) {
        if ((axis === 'ao' && state.lighting !== 'ao') || (axis === 'giLighting' && state.lighting !== 'gi')) continue;
        const control = field(label,'enum',state.quality[axis].selected,step=>{ if(step!=='custom')send('quality',{axis,step}); },{choices:[['custom','Custom'],...state.quality[axis].steps.map(step=>[step.id,step.label])]});
        fieldNodes.set(`@${axis}`,control); quality.append(control.row);
      }
    }
    for (const group of ['Camera','Quality','Hatching','Antialiasing','Lights','Ambient Occlusion','Shadows','Edges','Transparency','Global Illumination','POV-Ray']) {
      if ((group === 'Quality' && backend !== 'povray') || (group === 'Ambient Occlusion' && state.lighting !== 'ao') || (group === 'Global Illumination' && (backend === 'umbreon_npr' || state.lighting !== 'gi'))) continue;
      const rows = [...catalog.common.map(row=>[row,false]),...(catalog[backend]||[]).map(row=>[row,true])].filter(([row])=>row.group === group && !['aoEnabled','useGI'].includes(row.key) && (backend === 'povray' || !['stereoMode','stereoDepth','numThreads'].includes(row.key)));
      if (!rows.length) continue;
      const box = section(group,parent,['Camera','Hatching','POV-Ray'].includes(group));
      rows.forEach(([row,local])=>property(row,local,box));
    }
  }
  function parseSpec(spec) {
    return (spec || '').trim().split('\n').filter(Boolean).map(line => {
      const [kind,...rest] = line.split(':');
      return {kind, values:Object.fromEntries(rest.join(':').split(',').map(pair => {const i=pair.indexOf('=');return [pair.slice(0,i),pair.slice(i+1)];}))};
    });
  }
  const specLine = part => `${part.kind}:${Object.entries(part.values).map(([key,value])=>`${key}=${value}`).join(',')}`;
  function changeHatch(change) {
    send('hatch', () => {
      const parts = parseSpec(state.hatchSpec);
      change(parts);
      return {layersSpec:parts.filter(part=>part.kind==='layer').map(specLine).join('\n'),toneSpec:parts.filter(part=>part.kind!=='layer').map(specLine).join('\n')};
    });
  }
  function hatchField(parent,index,definition) {
    const {key,label,type,min,max,step,hint}=definition;
    const control = field(label,type==='bool'?'boolean':type==='int'?'integer':'real',definition.value,
      value=>changeHatch(parts=>{parts[index].values[key]=String(value);}),{min,max,step});
    fieldNodes.set(`hatch:${index}:${key}`,control); parent.append(control.row);
    if(['spacing','width','dotscale'].includes(key)){const node=element('p',hint||'','rendering-hatch-hint');node.hidden=!hint;node.dataset.hatchHint=`${index}:${key}`;parent.append(node);}
  }
  function buildHatch(parent) {
    if (state.values.backend !== 'umbreon_npr') return;
    parent.append(element('p',`Style template: ${state.values['umbreon_npr.hatchStyle']}`));
    const density=state.values['umbreon_npr.hatchDensity']??1, width=state.values['umbreon_npr.hatchWidthScale']??1;
    if(density!==1||width!==1)parent.append(element('p',`Render tab multipliers: Mark density ×${density}, Mark width ×${width}`));
    if (state.hatchError) parent.append(element('p',state.hatchError));
    const reset=button('Reset to style',()=>send('hatch',{layersSpec:'',toneSpec:''}));
    reset.dataset.hatchReset='true';parent.append(reset);
    const edited=element('span','Edited');edited.dataset.hatchEdited='true';parent.append(edited);
    const parts = parseSpec(state.hatchSpec);
    const layers = section('Layers',parent);
    let layerIndex=0;
    parts.forEach((part,index)=>{
      if (part.kind !== 'layer') return;
      const kind = part.values.kind, definitions=state.hatchEditor?.layers[layerIndex++]||[];
      const box = section(`Layer ${layerIndex} · ${kind}`,layers,layerIndex===1);
      const kindField=field('Layer kind','enum',kind,value=>changeHatch(parts=>{parts[index].values.kind=value;}),{choices:[['line','Line'],['dot','Dot screen'],['stipple','Stipple']]});
      fieldNodes.set(`hatch:${index}:kind`,kindField);box.append(kindField.row);
      const actions = element('div',undefined,'rendering-row');
      const duplicate=button('Duplicate',()=>changeHatch(parts=>parts.splice(index+1,0,structuredClone(parts[index]))));duplicate.dataset.hatchDuplicate='true';
      const remove=button('Remove',()=>changeHatch(parts=>{if(parts.filter(part=>part.kind==='layer').length>1)parts.splice(index,1);}));remove.dataset.hatchRemove='true';
      actions.append(duplicate,remove); box.append(actions);
      for(const definition of definitions.filter(field=>field.place==='primary'))hatchField(box,index,definition);
      const advanced=section('Randomness / Advanced',box,false);
      for(const definition of definitions.filter(field=>field.place==='advanced'))hatchField(advanced,index,definition);
    });
    const add = element('div',undefined,'rendering-row');
    for (const kind of ['line','dot','stipple']) {
      const node=button(`Add ${kind}`,()=>changeHatch(parts=>{const layer=parseSpec(state.defaultLayer)[0];layer.values.kind=kind;parts.splice(parts.filter(part=>part.kind==='layer').length,0,layer);}));
      node.dataset.hatchAdd='true';add.append(node);
    }
    layers.append(add);
    const shading = section('Shading',parent);
    const tone = parts.findIndex(part=>part.kind==='tone'), ink=parts.findIndex(part=>part.kind==='ink');
    if(tone>=0)for(const definition of state.hatchEditor?.tone.filter(field=>field.place==='primary')||[])hatchField(shading,tone,definition);
    const advanced=section('Advanced',shading,false);
    if(tone>=0)for(const definition of state.hatchEditor?.tone.filter(field=>field.place==='advanced')||[])hatchField(advanced,tone,definition);
    if(ink>=0)for(const definition of state.hatchEditor?.ink||[])hatchField(advanced,ink,definition);
    const text = section('Specification text',parent,false);
    for(const [key,label]of [['hatchLayersSpec','Layers specification'],['hatchToneSpec','Tone and ink specification']])property({key,label,type:'text'},true,text);
  }
  function hatchDefinition(index,key) {
    const parts=parseSpec(state.hatchSpec),part=parts[index];
    if(!part)return;
    const fields=part.kind==='layer'?state.hatchEditor?.layers[parts.slice(0,index).filter(part=>part.kind==='layer').length]:state.hatchEditor?.[part.kind];
    return fields?.find(field=>field.key===key);
  }
  function renderControls() {
    if(tab==='NPR Detail'&&state.values.backend!=='umbreon_npr')tab='Render';
    nodes.tabs.querySelectorAll('button').forEach(node=>{
      node.hidden=node.textContent==='NPR Detail'&&state.values.backend!=='umbreon_npr';
      node.setAttribute('aria-selected',String(node.textContent===tab));
    });
    const structure = parseSpec(state.hatchSpec).map(part=>part.kind==='layer'?part.values.kind:part.kind).join(',');
    const key = `${state.sceneId}/${state.viewId}|${state.movieMode}|${tab}|${state.values.backend}|${state.lighting}|${structure}|${state.values['umbreon_npr.hatchDensity']}|${state.values['umbreon_npr.hatchWidthScale']}`;
    if(key!==controlsKey){
      controlsKey=key; fieldNodes.clear(); nodes.controls.replaceChildren();
      ({Image:buildImage,Render:buildRender,'NPR Detail':buildHatch})[tab](nodes.controls);
    }
    const parts=parseSpec(state.hatchSpec);
    const busy=state.running||switchingTargets>0;
    nodes.controls.querySelectorAll('input,select,textarea,button').forEach(node=>{node.disabled=busy;});
    const layerCount=parts.filter(part=>part.kind==='layer').length;
    nodes.controls.querySelectorAll('[data-hatch-reset]').forEach(node=>{node.disabled=busy||(!state.hatchEditor?.dirty&&!state.hatchError);});
    nodes.controls.querySelectorAll('[data-hatch-edited]').forEach(node=>{node.hidden=!state.hatchEditor?.dirty;});
    nodes.controls.querySelectorAll('[data-hatch-remove]').forEach(node=>{node.disabled=busy||layerCount<2;});
    nodes.controls.querySelectorAll('[data-hatch-duplicate],[data-hatch-add]').forEach(node=>{node.disabled=busy||layerCount>=64;});

    for(const [key,control]of fieldNodes){
      let value=state.values[key];
      if(key==='@lighting')value=state.lighting;
      else if(key.startsWith('movie:'))value=state.movieOptions[key.slice(6)];
      else if(key.startsWith('@'))value=state.quality[key.slice(1)].selected;
      else if(key.startsWith('hatch:')){const [,index,name]=key.split(':');value=hatchDefinition(Number(index),name)?.value??parts[Number(index)]?.values[name];if(control.type==='boolean')value=value===true||value==='true'||value==='on';else if(control.type!=='enum')value=Number(value);}
      control.input.disabled=busy || (key==='movie:outputDir'&&state.movieOptions.useTempDir) || (['movie:movieFormat','movie:bitrateKbps'].includes(key)&&!state.movieOptions.makeMovie) || (key.endsWith('.hatchInkColor')&&!state.values[`${state.values.backend}.hatchCustomInk`]) || (key.endsWith('.hatchPaperColor')&&!state.values[`${state.values.backend}.hatchCustomPaper`]);
      if(key.startsWith('hatch:')){
        const [,index,name]=key.split(':'), definition=hatchDefinition(Number(index),name);
        control.input.disabled ||= definition?.enabled===false;
        const hint=nodes.controls.querySelector(`[data-hatch-hint="${index}:${name}"]`);
        if(hint){hint.textContent=definition?.hint||'';hint.hidden=!definition?.hint;}
      }
      if(document.activeElement===control.input)continue;
      if(control.type==='boolean')control.input.checked=!!value;
      else control.input.value=control.type==='enum'?JSON.stringify(value):value??'';
    }
    for(const key of ['width','height']){
      const input=fieldNodes.get(key)?.input;if(!input)continue;
      const [min,max]=({px:[1,8192],in:[0.1,60],mm:[1,1500],cm:[0.1,150]})[state.values.unit]||[1,8192];
      input.min=min;input.max=max;input.step=state.values.unit==='px'?1:'any';
    }
    nodes.controls.querySelectorAll('[data-temporary]').forEach(node=>{node.checked=state.movieOptions?.useTempDir===(node.dataset.temporary==='true');});
    if(nodes.pixels)nodes.pixels.textContent=`${state.pixels[0]} × ${state.pixels[1]} pixels`;
  }
  function fitScale(size=(state?.preview||state?.current)?.size||[1,1]) {
    return Math.max(0.05,Math.min(8,nodes.preview.clientWidth/size[0],nodes.preview.clientHeight/size[1]));
  }
  function frame() {
    const size=(state?.preview||state?.current)?.size||[1,1];
    const view=[nodes.preview.clientWidth,nodes.preview.clientHeight];
    if((state?.preview||state?.current)&&!fitted&&view[0]>0&&view[1]>0){scale=fitScale();center=[0.5,0.5];fitted=true;}
    for(let i=0;i<2;i++){const half=Math.min(0.5,view[i]/(size[i]*scale)*0.5);center[i]=Math.max(half,Math.min(1-half,center[i]));}
    nodes.image.style.width=`${size[0]*scale}px`;nodes.image.style.height=`${size[1]*scale}px`;
    nodes.image.style.left=`${view[0]/2-center[0]*size[0]*scale}px`;nodes.image.style.top=`${view[1]/2-center[1]*size[1]*scale}px`;
    nodes.zoom.textContent=`${Math.round(scale*100)}%`;
  }
  function zoom(next,anchor=[0,0]) {
    const size=(state?.preview||state?.current)?.size||[1,1];
    const at=center.map((value,i)=>value+anchor[i]/(size[i]*scale));
    scale=Math.max(0.05,Math.min(8,next));center=at.map((value,i)=>value-anchor[i]/(size[i]*scale));frame();
  }
  function render() {
    if(!dialogElement)return;
    nodes.start.textContent=state.running?'Stop':'Start Render';
    for(const [name,value]of [['stillMode',false],['movieMode',true]]){nodes[name].setAttribute('aria-pressed',String(!!state.movieMode===value));nodes[name].disabled=state.running||switchingTargets>0;}
    nodes.reencode.hidden=nodes.cleanup.hidden=!state.movieMode;
    nodes.reencode.disabled=state.running||switchingTargets>0||!state.availableFrames||!state.movieOptions?.makeMovie||!!state.movieError;
    nodes.cleanup.disabled=state.running||switchingTargets>0||!state.hasMovieOutputs;
    nodes.start.disabled=!state.running&&(switchingTargets>0||!state.sceneId||!!state.povError||!!state.movieError);
    nodes.backend.value=state.values.backend;nodes.backend.disabled=state.running||switchingTargets>0;
    const targets=JSON.stringify(state.targets);
    if(nodes.target.dataset.targets!==targets){nodes.target.dataset.targets=targets;nodes.target.replaceChildren(...state.targets.map(target=>{const option=element('option',target.label);option.value=target.viewId;return option;}));}
    nodes.target.value=state.viewId;nodes.target.disabled=state.running||switchingTargets>0;
    nodes.message.textContent=[state.running && state.elapsed != null ? `${state.message} (${state.elapsed.toFixed(1)} s)` : state.message,state.povError,state.movieError,state.log].filter(Boolean).join('\n');
    nodes.message.setAttribute('role','status');
    const index=state.selected??-1;
    nodes.history.textContent=state.livePreview?`Frame ${(state.preview?.frame??0)+1} / ${state.progress?.total??'…'}`:state.current?`${index+1} / ${state.historyLength} · ${state.current.size.join(' × ')} · ${state.current.source.scene_name}`:'No render result';
    nodes.previous.disabled=state.livePreview||index<=0;nodes.next.disabled=state.livePreview||index<0||index>=state.historyLength-1;
    for(const node of [nodes.save,nodes.copy,nodes.useSettings])node.disabled=state.livePreview||!state.current||(node===nodes.useSettings&&state.running);
    nodes.clear.disabled=!state.historyLength;
    if(Object.hasOwn(state,'image')){
      if(state.image){
        const next=(state.preview||state.current).size;
        if(fitted)scale=Math.max(0.05,Math.min(8,scale*fitScale(next)/fitScale(previousSize)));
        previousSize=next;nodes.image.src=state.image;nodes.image.hidden=false;
      }else{nodes.image.removeAttribute('src');nodes.image.hidden=true;}
    }
    if(!state.preview&&!state.current){nodes.image.hidden=true;nodes.image.removeAttribute('src');fitted=false;scale=1;center=[0.5,0.5];}
    nodes.recordSettings.hidden=state.livePreview||!state.current;
    if(state.current){const settings=state.current.source.settings;nodes.recordValues.textContent=`Scene: ${state.current.source.scene_name}\n${Object.entries(settings).map(([key,value])=>`${key}: ${typeof value==='string'?value:JSON.stringify(value)}`).join('\n')}`;}
    nodes.empty.hidden=!!state.preview||!!state.current;
    const movie=!state.livePreview&&state.current?.movie;
    nodes.frames.hidden=!movie;
    nodes.frameSlider.disabled=!movie?.framesAvailable||state.running;
    nodes.frameSlider.max=Math.max(0,(movie?.frameCount||1)-1);nodes.frameSlider.value=state.frame??0;
    nodes.frameLabel.textContent=movie?.framesAvailable?`Frame ${(state.frame??0)+1} / ${movie.frameCount}`:'Frames were replaced or removed; the saved preview remains available.';
    nodes.movieActions.hidden=!movie?.moviePath;
    for(const name of ['saveMovie','openMovie','revealMovie'])nodes[name].disabled=state.running;

    renderControls();frame();
  }
  function build() {
    dialogElement=element('dialog',undefined,'rendering-dialog');dialogElement.setAttribute('aria-label','Rendering');
    const header=element('header',undefined,'rendering-header');header.append(element('strong','Rendering'),button('×',close));
    header.lastChild.setAttribute('aria-label','Close Rendering');dialogElement.append(header);
    const body=element('div',undefined,'rendering-body'),left=element('div',undefined,'rendering-left'),right=element('div',undefined,'rendering-right');body.append(left,right);dialogElement.append(body);
    nodes.preview=element('div',undefined,'rendering-preview');nodes.preview.tabIndex=0;nodes.preview.setAttribute('aria-label','Rendered image preview');
    nodes.image=element('img');nodes.image.alt='Rendered image';nodes.image.draggable=false;nodes.image.hidden=true;
    nodes.empty=element('span','No render result yet');nodes.preview.append(nodes.image,nodes.empty);left.append(nodes.preview);
    const framing=element('div',undefined,'rendering-row');nodes.zoom=element('span','100%');
    framing.append(button('−',()=>zoom(scale/1.25)),nodes.zoom,button('+',()=>zoom(scale*1.25)),button('Fit',()=>{scale=fitScale();center=[0.5,0.5];fitted=true;frame();}),button('100%',()=>zoom(1)));left.append(framing);
    const history=element('div',undefined,'rendering-row');nodes.previous=button('Previous',()=>send('select',{index:state.selected-1}));nodes.next=button('Next',()=>send('select',{index:state.selected+1}));nodes.history=element('span');history.append(nodes.previous,nodes.history,nodes.next);left.append(history);
    nodes.frames=element('div',undefined,'rendering-frames');nodes.frameLabel=element('span');nodes.frameSlider=element('input');nodes.frameSlider.type='range';nodes.frameSlider.min=0;nodes.frameSlider.step=1;nodes.frameSlider.setAttribute('aria-label','Movie frame');
    nodes.frameSlider.addEventListener('input',()=>send('frame',{index:Number(nodes.frameSlider.value)}));nodes.frames.append(nodes.frameLabel,nodes.frameSlider);left.append(nodes.frames);
    nodes.movieActions=element('div',undefined,'rendering-row');
    nodes.saveMovie=button('Save Movie As…',async()=>{const imageId=state.current?.id,imageKey=state.imageKey,path=state.current?.movie?.moviePath;const name=path?.split(/[\\/]/).pop()||'movie.mp4';const output=await chooseOutputPath(name,'Movie',[name.split('.').pop()]);if(output)send('saveMovie',{path:output,imageId,imageKey});});
    nodes.openMovie=button('Open Movie',()=>send('openMovie'));nodes.revealMovie=button('Reveal Movie',()=>send('revealMovie'));nodes.movieActions.append(nodes.saveMovie,nodes.openMovie,nodes.revealMovie);left.append(nodes.movieActions);
    const actions=element('div',undefined,'rendering-row');
    nodes.save=button('Save Image…',async()=>{const imageId=state.current?.id,imageKey=state.imageKey;const path=await chooseOutputPath(state.filename,'PNG image',['png']);if(path)send('save',{path,imageId,imageKey});});
    nodes.copy=button('Copy Image',()=>send('copy'));
    nodes.useSettings=button('Use Settings',()=>send('useSettings'));
    nodes.clear=button('Clear History',async()=>{if(await showConfirmationDialog({title:'Clear render history',message:'Clear this image history? Movie files will remain in their output folders.',confirmLabel:'Clear'}))send('clear');});
    actions.append(nodes.save,nodes.copy,nodes.useSettings,nodes.clear);left.append(actions);
    nodes.recordSettings=element('details',undefined,'rendering-record-settings');nodes.recordSettings.append(element('summary','Settings'));nodes.recordValues=element('pre');nodes.recordSettings.append(nodes.recordValues);left.append(nodes.recordSettings);
    const modes=element('div',undefined,'rendering-row');nodes.stillMode=button('Still',()=>send('mode',{movie:false}));nodes.movieMode=button('Movie',()=>send('mode',{movie:true}));modes.append(nodes.stillMode,nodes.movieMode);left.append(modes);
    const run=element('div',undefined,'rendering-row');nodes.start=button('Start Render',async()=>{
      if(state.running){send('stop');return;}
      const args={sceneId:state.sceneId,targetViewId:state.viewId,movieOptions:structuredClone(state.movieOptions)};
      if(state.movieMode&&!state.movieOptions.useTempDir&&state.hasMovieOutputs){
        if(!await showConfirmationDialog({title:'Replace movie outputs',message:`Replace frames and movies for ${state.movieOptions.baseName||'movie'} in ${state.movieOptions.outputDir}?`,confirmLabel:'Replace'}))return;
        args.overwrite=true;
      }
      send('start',args);
    });
    nodes.reencode=button('Reencode',()=>send('reencode',{movieOptions:structuredClone(state.movieOptions)}));
    nodes.cleanup=button('Cleanup…',async()=>{
      const movieOptions=structuredClone(state.movieOptions);
      if(await showConfirmationDialog({title:'Cleanup movie outputs',message:`Delete frames and movies for ${movieOptions.baseName||'movie'} in ${movieOptions.outputDir}?`,confirmLabel:'Delete'}))send('cleanup',{movieOptions,confirmed:true});
    });run.append(nodes.reencode,nodes.cleanup);
    nodes.backend=element('select');nodes.backend.setAttribute('aria-label','Render backend');
    for(const [value,label]of [['umbreon','Umbreon'],['umbreon_npr','Umbreon NPR'],['povray','POV-Ray']]){const option=element('option',label);option.value=value;nodes.backend.append(option);}
    nodes.backend.addEventListener('change',()=>send('edit',{key:'backend',value:nodes.backend.value}));run.prepend(nodes.start);run.append(nodes.backend);left.append(run);
    nodes.target=element('select');nodes.target.setAttribute('aria-label','Render target');nodes.target.addEventListener('change',()=>send('target',{viewId:Number(nodes.target.value)}));left.append(nodes.target);
    nodes.message=element('pre',undefined,'rendering-message');left.append(nodes.message);
    const tabs=element('div',undefined,'rendering-tabs');nodes.tabs=tabs;tabs.setAttribute('role','tablist');
    for(const label of ['Image','Render','NPR Detail']){const node=button(label,()=>{tab=label;tabs.querySelectorAll('button').forEach(item=>item.setAttribute('aria-selected',String(item===node)));renderControls();});node.setAttribute('role','tab');node.setAttribute('aria-selected',String(label===tab));tabs.append(node);}
    nodes.controls=element('div',undefined,'rendering-controls');right.append(tabs,nodes.controls);
    dialogElement.addEventListener('cancel',event=>{event.preventDefault();close();});
    dialogElement.addEventListener('keydown',event=>{
      event.stopPropagation();
      if(event.key==='Escape'){event.preventDefault();close();return;}
      if((event.ctrlKey||event.metaKey)&&!event.target.closest('input,textarea,[contenteditable="true"]')){
        const key=event.key.toLowerCase();
        if(key==='z'||(event.ctrlKey&&key==='y')){event.preventDefault();send(key==='y'||event.shiftKey?'redo':'undo');}
      }
    });
    nodes.preview.addEventListener('wheel',event=>{
      event.preventDefault();const rect=nodes.preview.getBoundingClientRect();
      if(event.ctrlKey||event.metaKey)zoom(scale*Math.exp(-event.deltaY*0.002),[event.clientX-rect.x-rect.width/2,event.clientY-rect.y-rect.height/2]);
      else {const size=(state?.preview||state?.current)?.size||[1,1];center[0]+=event.deltaX/(size[0]*scale);center[1]+=event.deltaY/(size[1]*scale);frame();}
    },{passive:false});
    const pointers=new Map();let gesture=null;
    const geometry=()=>{const points=[...pointers.values()];if(!points.length)return null;return {center:points.length===1?points[0]:points[0].map((value,i)=>(value+points[1][i])/2),distance:points.length>1?Math.hypot(points[0][0]-points[1][0],points[0][1]-points[1][1]):null};};
    nodes.preview.addEventListener('pointerdown',event=>{if(event.button!==0||!(state.preview||state.current))return;nodes.preview.setPointerCapture(event.pointerId);pointers.set(event.pointerId,[event.clientX,event.clientY]);gesture=geometry();});
    nodes.preview.addEventListener('pointermove',event=>{
      if(!pointers.has(event.pointerId)||!(state.preview||state.current))return;
      pointers.set(event.pointerId,[event.clientX,event.clientY]);const next=geometry(),previous=gesture;gesture=next;
      if(!previous||!next)return;
      if(previous.distance&&next.distance){const rect=nodes.preview.getBoundingClientRect();zoom(scale*next.distance/previous.distance,[previous.center[0]-rect.x-rect.width/2,previous.center[1]-rect.y-rect.height/2]);}
      const size=(state.preview||state.current).size;center=center.map((value,i)=>value-(next.center[i]-previous.center[i])/(size[i]*scale));frame();
    });
    for(const name of ['pointerup','pointercancel','lostpointercapture'])nodes.preview.addEventListener(name,event=>{pointers.delete(event.pointerId);gesture=geometry();});
    new ResizeObserver(frame).observe(nodes.preview);
    header.addEventListener('pointerdown',event=>{
      if(event.button!==0||event.target.closest('button'))return;
      const rect=dialogElement.getBoundingClientRect(),offset=[event.clientX-rect.x,event.clientY-rect.y];
      header.setPointerCapture(event.pointerId);dialogElement.style.transform='none';
      const move=event=>{dialogElement.style.left=`${Math.max(0,Math.min(innerWidth-dialogElement.offsetWidth,event.clientX-offset[0]))}px`;dialogElement.style.top=`${Math.max(0,Math.min(innerHeight-dialogElement.offsetHeight,event.clientY-offset[1]))}px`;};
      const done=()=>{header.removeEventListener('pointermove',move);header.removeEventListener('lostpointercapture',done);};
      move(event);header.addEventListener('pointermove',move);header.addEventListener('lostpointercapture',done);
    });
    document.body.append(dialogElement);
  }
  async function close() {
    try{await request('close');dialogElement.close();clearInterval(timer);}catch{}
  }
  async function open(backend,movie) {
    if(!dialogElement)build();
    await request('open',{backend,movie});
    if(!dialogElement.open)dialogElement.show();
    if(!dialogElement.contains(document.activeElement)){dialogElement.tabIndex=-1;dialogElement.focus({preventScroll:true});}
    clearInterval(timer);timer=setInterval(()=>{if(!pending&&dialogElement.open)send('poll');},250);
    frame();
  }
  return {open};
})();
