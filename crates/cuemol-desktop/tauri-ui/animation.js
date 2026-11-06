"use strict";

const cueMolAnimationTimeline = {scene:null, selected:new Set(), anchor:null, primary:null, scale:0.1, drag:null, rendered:'', scroll:null};
const cueMolAnimationInspector = {key:'', target:null, source:null, serial:0, tab:'Properties', data:null, error:'', axisMode:null};
const cueMolAnimationTypes = {
  SimpleSpin:['↻','#2f7d8a'], CamMotion:['▣','#3a63b0'], ShowHideAnim:['◉','#b07d2a'],
  SlideInOutAnim:['↔','#3f8a55'], MolAnim:['◇','#7355a8'], NoopAnimObj:['Ⅱ','#4a5060'],
  RealPropAnim:['ƒ','#a84f68'], RendXformAnim:['✥','#5a68b0'],
};
function animationClock(ms) {
  const v=Math.round(Math.abs(ms));
  return `${ms<0?'-':''}${Math.floor(v/60000)}:${String(Math.floor(v/1000)%60).padStart(2,'0')}.${String(v%1000).padStart(3,'0')}`;
}
function animationParseClock(text) {
  const trimmed=text.trim();
  if(!trimmed || !/^-?\d+(?:\.\d+)?(?::\d+(?:\.\d+)?)*$/.test(trimmed))return null;
  const negative=trimmed.startsWith('-');
  const result=trimmed.replace(/^-/,'').split(':').reduce((total,part)=>total*60+Number(part),0)*1000;
  return Number.isFinite(result)?(negative?-result:result):null;
}
function animationClampScale(value) {return Number.isFinite(value)?Math.max(0.004,Math.min(4,value)):0.1;}
function animationSpan(object) {return {start:object.relativeStart*1000,end:(object.relativeStart+object.duration)*1000,absStart:object.start*1000,absEnd:object.end*1000,reference:object.timeRefName||''};}
function animationFingerprint(object) {return JSON.stringify([object.uid,animationSpan(object),object.disabled,object.resolveError]);}
function reconcileAnimationSelection() {
  const state=cueMolAnimationTimeline;
  if(state.scene!==snapshot.activeSceneUid) {
    cancelAnimationStripDrag();
    state.scene=snapshot.activeSceneUid;state.selected.clear();state.primary=null;state.anchor=null;state.rendered='';
  }
  const available=new Set(snapshot.animationObjects.map(object=>object.uid));
  state.selected=new Set([...state.selected].filter(uid=>available.has(uid)));
  if(!state.selected.has(state.primary))state.primary=[...state.selected][0]??null;
  if(!available.has(state.anchor))state.anchor=null;
  selectedAnimationIndices=new Set(snapshot.animationObjects.filter(object=>state.selected.has(object.uid)).map(object=>object.index));
  selectedAnimationIndex=snapshot.animationObjects.find(object=>object.uid===state.primary)?.index??null;
  selectedAnimationAnchor=snapshot.animationObjects.find(object=>object.uid===state.anchor)?.index??null;
}
function selectAnimationObject(object,event={}) {
  const state=cueMolAnimationTimeline;
  const objects=snapshot.animationObjects;
  if(event.shiftKey && state.anchor!==null) {
    const anchor=objects.findIndex(item=>item.uid===state.anchor),index=objects.findIndex(item=>item.uid===object.uid);
    state.selected=new Set(objects.slice(Math.min(anchor,index),Math.max(anchor,index)+1).map(item=>item.uid));
  } else if(event.metaKey||event.ctrlKey) {
    if(state.selected.has(object.uid))state.selected.delete(object.uid);else state.selected.add(object.uid);
    state.anchor=object.uid;
  } else {state.selected=new Set([object.uid]);state.anchor=object.uid;}
  state.primary=state.selected.has(object.uid)?object.uid:[...state.selected][0]??null;
  reconcileAnimationSelection();
  workspaceSelectionKind='animation';cueMolInspectorClosed=false;
  renderAnimation();renderInspector();
}
function selectAnimationByIndex(index) {
  const object=snapshot.animationObjects.find(object=>object.index===index);
  if(object)selectAnimationObject(object);
}
function showAnimationInspector() {
  reconcileAnimationSelection();
  if(cueMolAnimationTimeline.primary===null)return;
  workspaceSelectionKind='animation';cueMolInspectorClosed=false;renderInspector();
}
async function editCueMolAnimation(tool,args,scene=snapshot.activeSceneUid) {
  await cancelCueMolNumericDrag();
  try {
    const result=await invoke('inspector_request',{tool,args:{...args,sceneId:scene}});
    if(result.snapshot && snapshot.activeSceneUid===scene) {
      snapshot=result.snapshot;cueMolAnimationInspector.source=null;renderShell();renderAnimation();void renderViewport();
    }
    return true;
  } catch(error) {showError(error);return false;}
}
function cancelAnimationStripDrag() {cueMolAnimationTimeline.drag?.finish(false);}
function startAnimationStripDrag(event,object,mode) {
  if(event.button!==0 || object.resolveError || snapshot.animationPlaying)return;
  event.preventDefault();event.stopPropagation();
  selectAnimationObject(object,event);
  const state=cueMolAnimationTimeline,original=animationSpan(object),scene=snapshot.activeSceneUid;
  const scroll=state.scroll,initialScroll=scroll.scrollLeft,scale=state.scale;
  const drag={uid:object.uid,fingerprint:animationFingerprint(object),span:original,moved:false,finish:null};
  state.drag=drag;
  const valid=()=>snapshot.activeSceneUid===scene && snapshot.animationObjects.some(item=>item.uid===object.uid && animationFingerprint(item)===drag.fingerprint);
  const move=next=>{
    if(next.pointerId!==event.pointerId)return;
    const pixels=next.clientX-event.clientX+scroll.scrollLeft-initialScroll;
    if(!drag.moved && Math.abs(pixels)<3)return;
    next.preventDefault();drag.moved=true;
    const delta=pixels/scale,minimum=-Math.min(original.absStart,original.reference?original.start:Infinity);
    const startDelta=mode==='right'?0:mode==='left'?Math.min(original.absEnd-original.absStart,Math.max(minimum,delta)):Math.max(minimum,delta);
    const endDelta=mode==='left'?0:mode==='right'?Math.max(original.absStart-original.absEnd,delta):startDelta;
    drag.span={...original,start:original.start+startDelta,end:original.end+endDelta,absStart:original.absStart+startDelta,absEnd:original.absEnd+endDelta};
    paintAnimationStrip(object.uid,drag.span);
  };
  const up=next=>{if(next.pointerId===event.pointerId)drag.finish(true);};
  const abort=()=>drag.finish(false);
  const escape=next=>{if(next.key==='Escape'){next.preventDefault();next.stopImmediatePropagation();abort();}};
  drag.finish=commit=>{
    if(state.drag!==drag)return;
    state.drag=null;
    document.removeEventListener('pointermove',move,true);document.removeEventListener('pointerup',up,true);
    document.removeEventListener('pointercancel',abort,true);document.removeEventListener('keydown',escape,true);window.removeEventListener('blur',abort);
    const span=drag.span;
    if(commit && drag.moved && valid() && (span.start!==original.start||span.end!==original.end)) {
      void editCueMolAnimation('set_anim_time',{element:`#${object.uid}`,startMs:span.start,endMs:span.end},scene);
    } else {state.rendered='';renderAnimation();}
  };
  document.addEventListener('pointermove',move,true);document.addEventListener('pointerup',up,true);
  document.addEventListener('pointercancel',abort,true);document.addEventListener('keydown',escape,true);window.addEventListener('blur',abort);
}
function paintAnimationStrip(uid,span) {
  const strip=animationList.querySelector(`[data-animation-uid="${uid}"] .animation-strip`);
  if(!strip)return;
  strip.style.left=`${Math.max(0,span.absStart)*cueMolAnimationTimeline.scale}px`;
  strip.style.width=`${Math.max(3,(Math.max(0,span.absEnd)-Math.max(0,span.absStart))*cueMolAnimationTimeline.scale)}px`;
  strip.title=`${animationClock(span.absStart)} – ${animationClock(span.absEnd)}`;
}
function startAnimationScrub(event) {
  if(event.button!==0 || snapshot.animationResolveError)return;
  event.preventDefault();
  cancelAnimationStripDrag();
  const state=cueMolAnimationTimeline,scene=snapshot.activeSceneUid;
  const ruler=event.currentTarget,origin=ruler.getBoundingClientRect().left,initialScroll=state.scroll.scrollLeft;
  let elapsed=snapshot.animationElapsed*1000;
  const drag={finish:null};state.drag=drag;
  const move=next=>{if(next.pointerId!==event.pointerId)return;elapsed=Math.max(0,Math.min(snapshot.animationDuration*1000,(next.clientX-origin+state.scroll.scrollLeft-initialScroll)/state.scale));paintAnimationPlayhead(elapsed);};
  const up=next=>{if(next.pointerId===event.pointerId)drag.finish(true);};
  const abort=()=>drag.finish(false);
  const escape=next=>{if(next.key==='Escape'){next.preventDefault();next.stopImmediatePropagation();abort();}};
  drag.finish=commit=>{
    if(state.drag!==drag)return;state.drag=null;
    document.removeEventListener('pointermove',move,true);document.removeEventListener('pointerup',up,true);document.removeEventListener('pointercancel',abort,true);document.removeEventListener('keydown',escape,true);window.removeEventListener('blur',abort);
    if(commit && snapshot.activeSceneUid===scene)void mutate('seek_animation',{elapsed:elapsed/1000});else paintAnimationPlayhead(snapshot.animationElapsed*1000);
  };
  document.addEventListener('pointermove',move,true);document.addEventListener('pointerup',up,true);document.addEventListener('pointercancel',abort,true);document.addEventListener('keydown',escape,true);window.addEventListener('blur',abort);move(event);
}
function paintAnimationPlayhead(elapsed) {
  animationList.querySelectorAll('.animation-playhead').forEach(line=>{line.style.left=`${140+Math.max(0,elapsed)*cueMolAnimationTimeline.scale}px`;});
  const input=document.getElementById('animation-current-time');
  if(input && document.activeElement!==input)input.value=animationClock(elapsed);
}
function renderAnimationStrips() {
  reconcileAnimationSelection();
  const state=cueMolAnimationTimeline,objects=snapshot.animationObjects;
  if(state.drag?.uid && !objects.some(object=>object.uid===state.drag.uid&&animationFingerprint(object)===state.drag.fingerprint))cancelAnimationStripDrag();
  const length=Math.max(1000,snapshot.animationDuration*1000,...objects.map(object=>object.end*1000));
  const key=JSON.stringify([state.scene,[...state.selected],objects.map(object=>[animationFingerprint(object),object.name,object.typeName]),state.scale,length]);
  if(key!==state.rendered && !state.drag) {
    state.rendered=key;
    const left=animationList.scrollLeft,top=animationList.scrollTop;
    animationList.replaceChildren();animationList.classList.add('animation-strip-list');state.scroll=animationList;
    const width=Math.max(320,Math.ceil(length*state.scale)+40);
    const grid=document.createElement('div');grid.className='animation-timeline-grid';grid.style.width=`${width+140}px`;
    const head=document.createElement('div');head.className='animation-timeline-head';
    const label=document.createElement('div');label.className='animation-lane-label';label.textContent='Elements';
    const ruler=document.createElement('div');ruler.className='animation-time-ruler';ruler.setAttribute('aria-label','Animation time ruler');ruler.addEventListener('pointerdown',startAnimationScrub);
    const step=[50,100,200,250,500,1000,2000,2500,5000,10000,15000,30000,60000,120000,300000,600000,1200000,3600000].find(value=>value>=72/state.scale)||3600000;
    for(let ms=0;ms<=length+40/state.scale;ms+=step) {
      const tick=document.createElement('span');tick.className='animation-time-tick';tick.style.left=`${ms*state.scale}px`;
      tick.textContent=step<1000?`${(ms/1000).toFixed(2)}s`:step<10000?`${(ms/1000).toFixed(1)}s`:ms>=60000?`${Math.floor(ms/60000)}:${String(Math.round(ms/1000)%60).padStart(2,'0')}`:`${Math.round(ms/1000)}s`;
      ruler.append(tick);
    }
    head.append(label,ruler);grid.append(head);
    for(const object of objects) {
      const [icon,color]=cueMolAnimationTypes[object.typeName]||['◇','#82909e'];
      const row=document.createElement('div');row.className=`animation-row animation-lane${state.selected.has(object.uid)?' selected':''}${object.disabled?' disabled':''}${object.resolveError?' unresolved':''}`;row.dataset.animationUid=object.uid;
      const name=button(`${icon} ${object.name||object.typeName}`,'animation-lane-label',event=>selectAnimationObject(object,event));name.title=object.resolveError||`${object.name} (${object.typeName})`;name.setAttribute('aria-label',`Animation element ${object.name||object.typeName}`);
      const lane=document.createElement('div');lane.className='animation-lane-track';
      const strip=document.createElement('div');strip.className='animation-strip';strip.style.backgroundColor=color;strip.tabIndex=0;strip.setAttribute('role','button');strip.setAttribute('aria-label',`Animation strip ${object.name||object.typeName}`);
      const leftEdge=document.createElement('span');leftEdge.className='animation-strip-edge left';const rightEdge=document.createElement('span');rightEdge.className='animation-strip-edge right';
      const title=document.createElement('span');title.className='animation-strip-caption';title.textContent=object.resolveError?'unresolved':object.name||object.typeName;
      strip.append(leftEdge,title,rightEdge);lane.append(strip);row.append(name,lane);grid.append(row);
      strip.addEventListener('pointerdown',event=>startAnimationStripDrag(event,object,event.target===leftEdge?'left':event.target===rightEdge?'right':'move'));
      strip.addEventListener('keydown',event=>{if(event.key==='Enter'||event.key===' '){event.preventDefault();selectAnimationObject(object);}});
      row.addEventListener('click',event=>{if(event.target===row||event.target===lane)selectAnimationObject(object,event);});
      row.addEventListener('dblclick',()=>{selectAnimationObject(object);showAnimationInspector();});
      row.addEventListener('contextmenu',event=>{if(!state.selected.has(object.uid))selectAnimationObject(object);showWorkspaceContextMenu(event,animationContextItems());});
    }
    if(!objects.length){const empty=document.createElement('p');empty.className='animation-empty';empty.textContent='No animation objects.';grid.append(empty);}
    const playhead=document.createElement('div');playhead.className='animation-playhead';grid.append(playhead);animationList.append(grid);
    for(const object of objects)paintAnimationStrip(object.uid,animationSpan(object));
    animationList.scrollLeft=left;animationList.scrollTop=top;
  }
  if(!state.drag)paintAnimationPlayhead(snapshot.animationElapsed*1000);
}
function setupAnimationTimeline() {
  const ribbon=document.querySelector('.animation-ribbon-panel');
  const first=button('⏮','',()=>void mutate('seek_animation',{elapsed:0}));first.title='Animation start';first.setAttribute('aria-label','Animation start');ribbon.prepend(first);
  const last=button('⏭','',()=>void mutate('seek_animation',{elapsed:snapshot.animationDuration}));last.title='Animation end';last.setAttribute('aria-label','Animation end');ribbon.insertBefore(last,animationLoop.parentElement);
  const current=document.createElement('input');current.id='animation-current-time';current.setAttribute('aria-label','Animation current time');current.value='0:00.000';
  current.addEventListener('change',()=>{const ms=animationParseClock(current.value);if(ms!==null)void mutate('seek_animation',{elapsed:Math.max(0,ms)/1000});else current.value=animationClock(snapshot.animationElapsed*1000);});
  current.addEventListener('keydown',event=>{if(event.key==='Enter')current.blur();if(event.key==='Escape'){current.value=animationClock(snapshot.animationElapsed*1000);current.blur();}});
  ribbon.insertBefore(current,animationTime);animationSlider.hidden=true;
  const zoom=document.createElement('div');zoom.className='animation-zoom-controls';
  const change=value=>{cancelAnimationStripDrag();cueMolAnimationTimeline.scale=animationClampScale(value);cueMolAnimationTimeline.rendered='';renderAnimation();};
  const fit=button('Fit','',()=>change((animationList.clientWidth-140-40)/Math.max(1000,snapshot.animationDuration*1000)));
  const less=button('−','',()=>change(cueMolAnimationTimeline.scale/1.4));less.setAttribute('aria-label','Animation zoom out');
  const more=button('+','',()=>change(cueMolAnimationTimeline.scale*1.4));more.setAttribute('aria-label','Animation zoom in');
  fit.setAttribute('aria-label','Animation zoom fit');zoom.append(document.createTextNode('30 fps'),fit,less,more);ribbon.append(zoom);
}

function clearAnimationInspector() {
  const editor=cueMolAnimationInspector;
  if(cueMolNumericDrag?.owner===editor)void cancelCueMolNumericDrag();
  if(editor.key){editor.key='';editor.target=null;editor.serial++;editor.source=null;}
}
function renderAnimationInspector() {
  reconcileAnimationSelection();
  const editor=cueMolAnimationInspector,uid=cueMolAnimationTimeline.primary;
  const object=snapshot.animationObjects.find(object=>object.uid===uid);
  const key=object?`${snapshot.activeSceneUid}:animation:${uid}`:'';
  if(cueMolNumericDrag?.owner===cueMolPropertyInspector)void cancelCueMolNumericDrag();
  if(key!==cueMolInspectorTarget){cueMolSelectionPicker?.cancel();cueMolInspectorTarget=key;cueMolInspectorClosed=false;}
  contentHolder.classList.toggle('has-inspector',Boolean(object)&&!cueMolInspectorClosed);
  inspectorTitle.textContent=object?.name||'Animation element';
  if(!object||cueMolInspectorClosed){clearAnimationInspector();document.getElementById('inspector-fields').replaceChildren();return;}
  if(cueMolNumericDrag?.owner===editor) {
    if(editor.key!==key)void cancelCueMolNumericDrag();else return;
  }
  if(editor.key===key&&editor.source===snapshot)return;
  if(editor.key!==key){editor.axisMode=null;editor.tab='Properties';editor.data=null;}
  editor.key=key;editor.target={sceneId:snapshot.activeSceneUid,uid};editor.source=snapshot;
  const serial=++editor.serial;
  void invoke('inspector_request',{tool:'animation_element_detail',args:{...editor.target}}).then(data=>{
    if(editor.key!==key||editor.serial!==serial)return;editor.data=data;editor.error='';paintAnimationInspector();
  }).catch(error=>{if(editor.key!==key||editor.serial!==serial)return;editor.error=String(error);paintAnimationInspector();});
}
function animationNumericSchema(key,type) {
  return key==='angle'?{step:5,unit:'°'}:key==='quadric'?{min:0,max:50,step:5,unit:'%'}:
    key==='direction'?{min:0,max:360,step:1,unit:'°'}:key==='distance'?{min:0,max:2,step:0.1}:
    key==='tgt_alpha'||type==='MolAnim'&&['startValue','endValue'].includes(key)?{min:0,max:1,step:0.1}:{};
}
function paintAnimationInspector() {
  const editor=cueMolAnimationInspector,data=editor.data,fields=document.getElementById('inspector-fields');
  if(!editor.key||workspaceSelectionKind!=='animation')return;
  fields.replaceChildren();
  if(editor.error){const error=document.createElement('p');error.className='inspector-error';error.textContent=editor.error;fields.append(error);}
  if(!data)return;
  const target={...editor.target},key=editor.key;
  const commit=(prop,value,tool='set_anim_generic_prop')=>editCueMolAnimation(tool,{element:`#${target.uid}`,prop,value:String(value)},target.sceneId);
  const tabs=document.createElement('div');tabs.className='inspector-tabs';
  for(const tab of ['Properties','Generic']){const control=button(tab,'',async()=>{await cancelCueMolNumericDrag();if(editor.key!==key)return;editor.tab=tab;paintAnimationInspector();});control.setAttribute('aria-label',`Animation ${tab} tab`);control.setAttribute('aria-selected',String(editor.tab===tab));tabs.append(control);}
  fields.append(tabs);
  if(data.resolveError){const warning=document.createElement('p');warning.className='inspector-error';warning.textContent=data.resolveError;fields.append(warning);}
  const resettable=prop=>data.metadata?.[prop]?.hasDefault===true;
  const modified=prop=>resettable(prop)&&data.metadata[prop].isDefault===false;
  const reset=props=>editCueMolAnimation('reset_anim_prop',{element:`#${target.uid}`,props},target.sceneId);
  const modifiedKeys=Object.keys(data.properties).filter(modified);
  const resetAll=button('Reset all to default','',()=>void reset(modifiedKeys.join(',')));resetAll.disabled=!modifiedKeys.length;fields.append(resetAll);
  const add=(prop,labelText,value,{readonly=false,options=null,schema=null,timing=false,percent=false}={})=>{
    const row=document.createElement('div');row.className='inspector-property';
    const label=document.createElement('label');label.textContent=labelText;
    const holder=document.createElement('div');holder.className='inspector-property-input';
    const input=document.createElement(options?'select':'input');input.id=`animation-inspector-${prop}`;label.htmlFor=input.id;input.setAttribute('aria-label',`Animation property ${prop}`);
    if(options){for(const option of options){const item=document.createElement('option');item.value=option.value;item.textContent=option.label;item.disabled=Boolean(option.error);if(option.error)item.title=option.error;input.append(item);}input.value=value;input.addEventListener('change',()=>void commit(prop,input.value));}
    else if(data.metadata?.[prop]?.type==='boolean'){input.type='checkbox';input.checked=value==='true';input.addEventListener('change',()=>void commit(prop,input.checked));}
    else if(timing || ['real','integer'].includes(data.metadata?.[prop]?.type)&&value!==''&&Number.isFinite(Number(value))) {
      input.type=timing?'text':'number';const original=String(value);const display=v=>timing?animationClock(Number(v)):String(v);
      input.value=display(original);if(!timing){if(schema?.min!==undefined)input.min=schema.min;if(schema?.max!==undefined)input.max=schema.max;input.step=schema?.step??0.01;}
      const write=value=>{
        let number=timing?animationParseClock(value):Number(value);if(number===null||value===''||!Number.isFinite(number))return;
        if(schema){number=Math.max(schema.min??-Infinity,Math.min(schema.max??Infinity,number));}
        if(editor.tab!=='Generic'&&prop==='angle'){const n=((number%360)+360)%360;number=n===0&&number>0?360:n;}
        if(timing){const start=prop==='startMs'?number:data.startMs,duration=prop==='durationMs'?number:data.endMs-data.startMs;void editCueMolAnimation('set_anim_time',{element:`#${target.uid}`,startMs:start,endMs:start+duration},target.sceneId);}
        else if(number!==Number(original))void commit(prop,percent?number/100:number);
      };
      input.addEventListener('change',()=>{if(!cueMolNumericDrag)write(input.value);});
      input.addEventListener('keydown',event=>{if(event.key==='Enter')input.blur();if(event.key==='Escape'){event.preventDefault();input.value=display(original);input.blur();}});
      if(!readonly)attachCueMolNumericDrag(input,label,{key:prop,value:original,type:'real',isDefault:data.metadata?.[prop]?.isDefault===true,schema:{...schema,realtime:timing||editor.tab==='Generic'},format:timing?display:null,parse:timing?animationParseClock:null,timing,span:{startMs:data.startMs,endMs:data.endMs}},value=>write(display(value)),editor);
    } else {input.value=value;input.addEventListener('change',()=>{if(input.value!==value)void commit(prop,input.value);});input.addEventListener('keydown',event=>{if(event.key==='Enter')input.blur();if(event.key==='Escape'){input.value=value;input.blur();}});}
    input.disabled=readonly;holder.append(input);row.append(label,holder);
    if(resettable(prop)&&!timing&&!readonly){const control=button('↺','',()=>void reset(prop));control.title='Reset to default';control.disabled=!modified(prop);control.setAttribute('aria-label',`Animation reset ${prop}`);row.append(control);}
    fields.append(row);return {row,input,holder};
  };
  editor.editRequest=(row,mode,value,args)=>{
    let final=value,original=row.value,prop=row.key;
    if(row.timing){prop='timing';const span=row.span;const start=row.key==='startMs'?Number(value):span.startMs;const duration=row.key==='durationMs'?Number(value):span.endMs-span.startMs;final={startMs:start,endMs:start+duration};original=span;}
    return {tool:'edit_animation_property',args:{...args,...target,prop,mode,value:final,originalValue:original,originalWasDefault:row.isDefault===true}};
  };
  editor.onPreview=()=>renderAnimation();
  if(editor.tab==='Generic') {
    for(const [prop,value] of Object.entries(data.properties))add(prop,prop,value,{readonly:data.metadata?.[prop]?.readonly===true});
    return;
  }
  add('name','Name',data.properties.name);add('disabled','Disabled',data.properties.disabled);
  const references=[{value:'',label:'(absolute)'},...data.references.map(item=>({value:item.name,label:item.error?`${item.name} (${item.error})`:item.name,error:item.error}))];
  const reference=data.properties.timeRefName;
  if(reference&&!references.some(item=>item.value===reference))references.unshift({value:reference,label:`(missing: ${reference})`,error:'Missing element'});
  add('timeRefName','Relative to',reference,{options:references});
  add('startMs','Start time',data.startMs,{timing:true,schema:{min:0,step:10}});
  if(data.startMs<0){const note=document.createElement('p');note.textContent=`Stored start: ${animationClock(data.startMs)}; preserved until edited.`;fields.append(note);}
  add('durationMs','Duration',data.endMs-data.startMs,{timing:true,schema:{min:0,step:10}});
  add('quadric','Quadric',Number(data.properties.quadric)*100,{schema:animationNumericSchema('quadric',data.type),percent:true});
  for(const [prop,value] of Object.entries(data.properties)) {
    if(['uid','name','disabled','timeRefName','start','end','absStart','absEnd','quadric'].includes(prop))continue;
    if(prop==='axis') {
      const values=value.replace(/[()]/g,'').trim().split(/[,\s]+/).map(Number).slice(0,3);
      const derived=values[0]===1&&values[1]===0&&values[2]===0?'x':values[0]===0&&values[1]===1&&values[2]===0?'y':values[0]===0&&values[1]===0&&values[2]===1?'z':'cart';
      const row=document.createElement('div');row.className='animation-axis-editor';
      const label=document.createElement('label');label.textContent='Spin axis';const select=document.createElement('select');select.setAttribute('aria-label','Animation axis preset');
      for(const [value,label]of[['x','X axis'],['y','Y axis'],['z','Z axis'],['cart','Cartesian']]){const option=document.createElement('option');option.value=value;option.textContent=label;select.append(option);}
      select.value=editor.axisMode??derived;
      select.addEventListener('change',()=>{editor.axisMode=select.value;if(select.value==='cart'){paintAnimationInspector();return;}void commit('axis',select.value==='x'?'1 0 0':select.value==='y'?'0 1 0':'0 0 1','set_anim_prop');});
      label.append(select);row.append(label);
      const xyz=document.createElement('div');xyz.className='animation-axis-components';
      for(const [index,axis]of ['x','y','z'].entries()){const box=document.createElement('input');box.type='number';box.step='0.0001';box.value=String(Math.round(values[index]*1e4)/1e4);box.disabled=select.value!=='cart';box.setAttribute('aria-label',`Animation axis component ${axis}`);box.addEventListener('change',()=>{const updated=values.slice();updated[index]=Number(box.value);if(box.value!==''&&updated.every(Number.isFinite)&&Math.hypot(...updated)>=1e-6)void commit('axis',updated.join(' '),'set_anim_prop');else box.value=String(values[index]);});xyz.append(box);}
      const resetAxis=button('↺','',()=>void reset('axis'));resetAxis.disabled=!modified('axis');resetAxis.setAttribute('aria-label','Animation reset axis');xyz.append(resetAxis);row.append(xyz);fields.append(row);continue;
    }
    if(prop==='rend') {
      const row=document.createElement('div');row.className='animation-renderer-choices';row.textContent='Target renderers';
      const names=new Set(value.split(',').map(s=>s.trim()).filter(Boolean));
      for(const renderer of data.renderers){const label=document.createElement('label'),input=document.createElement('input');input.type='checkbox';input.checked=names.has(renderer.name);input.addEventListener('change',()=>{if(input.checked)names.add(renderer.name);else names.delete(renderer.name);void commit(prop,[...names].join(','));});label.append(input,document.createTextNode(`${renderer.object}/${renderer.name}`));row.append(label);}
      if(!data.renderers.length)row.append(document.createTextNode(' (no renderers)'));fields.append(row);continue;
    }
    const labels={angle:'Rotation angle',endcam:'Target camera',ignorecenter:'Ignore center',ignorerotate:'Ignore rotation',ignorezoom:'Ignore zoom',ignoreslab:'Ignore slab',keepquatpositive:'Positive quaternion',mol:'Target MorphMol',hide:'Hide',fade:'Fade',tgt_alpha:'Target opacity',direction:'Direction angle',distance:'Distance',startValue:'Start value',endValue:'End value'};
    const options=['endcam','mol'].includes(prop)?[{value:'',label:'(none)'},...data[prop==='mol'?'molecules':'cameras'].map(value=>({value,label:value}))]:null;
    if(options&&value&&!options.some(item=>item.value===value))options.push({value,label:`(missing: ${value})`});
    const control=add(prop,labels[prop]||prop,value,{schema:animationNumericSchema(prop,data.type),options});
    if(prop==='direction') {
      const select=document.createElement('select');select.setAttribute('aria-label','Animation direction preset');
      for(const [value,label]of [['','Custom'],['0','Left'],['90','Up'],['180','Right'],['270','Down']]){const option=document.createElement('option');option.value=value;option.textContent=label;select.append(option);}select.value=['0','90','180','270'].includes(value)?value:'';
      select.addEventListener('change',()=>{if(select.value!=='')void commit(prop,select.value);});control.holder.append(select);
    }
  }
}
