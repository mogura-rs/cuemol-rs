let cueMolSelectionPicker = null;

function attachCueMolSelectionPicker(input, controls, target, commit) {
  const identity = `${snapshot?.activeSceneUid}:${target.nodeType}:${target.nodeId}:${input.id}`;
  const button = document.createElement('button');
  button.type = 'button'; button.textContent = '▾'; button.disabled = input.disabled;
  button.setAttribute('aria-label', `Build ${input.getAttribute('aria-label').replace('Inspected ', '')}`);
  button.addEventListener('click', () => {
    if (cueMolSelectionPicker?.identity === identity) { void cueMolSelectionPicker.finish(false); return; }
    cueMolSelectionPicker?.cancel();
    cueMolSelectionPicker = createCueMolSelectionPicker(input, target, identity, commit);
  });
  controls.append(button);
  if (cueMolSelectionPicker?.identity === identity) {
    if (input.value !== cueMolSelectionPicker.original) cueMolSelectionPicker.cancel();
    else cueMolSelectionPicker.attach(input);
  }
  input.addEventListener('input', () => {
    if (cueMolSelectionPicker?.identity === identity) cueMolSelectionPicker.setValue(input.value);
  });
}

function createCueMolSelectionPicker(input, target, identity, commit, options = {}) {
  const inline = Boolean(options.container);
  const inspectorKey = cueMolPropertyInspector.key;
  const original = input.value;
  let value = original;
  let tab = 'Named';
  let context = null;
  const draft = options.draft || cueMolPropertyInspector.selectionDrafts.get(input.id) || {keyword:'hierarchical',fields:{},picked:'',distance:'5'};
  if (!inline) cueMolPropertyInspector.selectionDrafts.set(input.id, draft);
  const candidates = new Map();
  let candidateRevision = snapshot?.structureKey;
  let disposed = false;
  let composeTail = Promise.resolve();
  let valueGeneration = 0;
  let countGeneration = 0;
  let timer = null;
  let closing = false;
  const popup = document.createElement('div');
  popup.className = inline ? 'selection-builder-inline' : 'selection-builder-popup';
  popup.setAttribute('role', inline ? 'group' : 'dialog'); popup.setAttribute('aria-label', 'Build selection');
  (options.container || document.body).append(popup);
  const live = () => !disposed && (inline ? options.isLive() : cueMolPropertyInspector.key === inspectorKey && snapshot?.activeSceneUid === Number(identity.split(':')[0]));
  const request = args => invoke('inspector_request', {tool:'selection_info',args:{...target,...args}});
  const currentDraft = () => ({...draft,fields:{...draft.fields}});
  const rect = () => {
    if (inline || !input.isConnected) return;
    const anchor = input.getBoundingClientRect();
    const below = innerHeight-anchor.bottom-12;
    const above = anchor.top-12;
    popup.style.width = `${Math.max(160,Math.min(310,innerWidth-24))}px`;
    popup.style.maxHeight = `${Math.max(80,Math.max(below,above))}px`;
    popup.style.left = `${Math.max(12,Math.min(anchor.right-310,innerWidth-popup.offsetWidth-12))}px`;
    popup.style.top = `${below >= popup.offsetHeight || below >= above ? anchor.bottom+4 : Math.max(12,anchor.top-popup.offsetHeight-4)}px`;
  };
  const close = cancel => {
    if (disposed) return;
    disposed = true; clearTimeout(timer); countGeneration++;
    document.removeEventListener('pointerdown', outside, true);
    document.removeEventListener('keydown', escape, true);
    window.removeEventListener('resize', rect);
    popup.remove();
    if (!inline && cancel && input.isConnected && cueMolPropertyInspector.key === inspectorKey) input.value = original;
    if (cueMolSelectionPicker?.identity === identity) cueMolSelectionPicker = null;
  };
  const finish = async cancel => {
    if (cancel || !live()) { close(true); return; }
    if (closing) return;
    closing = true;
    try {
      await composeTail;
      if (!live()) { close(true); return; }
      const chosen = value;
      const result = await request({action:'evaluate',expression:chosen});
      if (!live()) { close(true); return; }
      if (chosen !== value) { closing = false; return; }
      if (!result.valid) { showError(result.error || 'Invalid selection expression'); closing = false; return; }
      input.value = chosen;
      close(false);
      if (chosen !== original) commit(chosen);
    } catch (error) { if (live()) showError(String(error)); closing = false; }
  };
  const outside = event => {
    if (!popup.contains(event.target) && !input.parentElement?.contains(event.target)) void finish(false);
  };
  const escape = event => {
    if (event.key === 'Escape' && !event.isComposing) { event.preventDefault(); event.stopPropagation(); close(true); }
  };
  const showError = text => { if(options.onError)options.onError(text);const status=popup.querySelector('.selection-builder-status'); if (status) {status.textContent=text; status.classList.add('error');} };
  const button = (label, action, aria=label) => {
    const element=document.createElement('button'); element.type='button'; element.textContent=label; element.setAttribute('aria-label',aria); element.addEventListener('click',action); return element;
  };
  const setValue = (next, external=true) => {
    if (external) valueGeneration++;
    value=next; if(!inline)input.value=next; scheduleCounts();
  };
  const apply = async (expression, generation) => {
    if(!live()||generation!==valueGeneration)return false;
    if(inline && !await commit(expression))return false;
    if(!live()||generation!==valueGeneration)return false;
    setValue(expression,false);
    return true;
  };
  const quick = expression => {
    if(!inline){setValue(expression);void finish(false);return;}
    const generation=valueGeneration;
    composeTail=composeTail.then(()=>apply(expression,generation)).catch(error=>{if(live())showError(String(error));return false;});
  };
  const field = (key, placeholder) => {
    const element=document.createElement('input'); element.type='text'; element.placeholder=placeholder; element.value=draft.fields[key] || '';
    element.setAttribute('aria-label',`Selection ${key}`);
    element.addEventListener('input',()=>{draft.fields[key]=element.value;scheduleCounts();});
    element.addEventListener('keydown',event=>{if(event.key==='Enter'&&!event.isComposing&&!value.trim()){event.preventDefault();void compose('set',true);}});
    return element;
  };
  const populateCandidates = (input, fields, kind) => {
    const revision=snapshot?.structureKey;
    if(candidateRevision!==revision){candidateRevision=revision;candidates.clear();}
    if(!candidates.has(kind))candidates.set(kind,request({action:'values',kind}));
    void candidates.get(kind).then(values=>{
      if(!live()||!input.isConnected||snapshot?.structureKey!==revision)return;
      let select=fields.querySelector('[aria-label="Selection candidates"]');
      if(!values.length){select?.remove();return;}
      if(!select){
        select=document.createElement('select');select.setAttribute('aria-label','Selection candidates');
        select.addEventListener('change',()=>{input.value=select.value;draft.fields.value=select.value;scheduleCounts();});fields.append(select);
      }
      const key=JSON.stringify(values);
      if(select.dataset.values===key)return;
      const previous=select.value;select.dataset.values=key;select.replaceChildren();
      const empty=document.createElement('option');empty.value='';empty.textContent='▾';select.append(empty);
      for(const value of values){const option=document.createElement('option');option.value=value;option.textContent=value;select.append(option);}
      select.value=values.includes(previous)?previous:'';
    }).catch(error=>{if(live())showError(String(error));});
  };
  const compose = (operation, immediate=false) => {
    if (!live() || closing) return;
    const generation=valueGeneration, term=currentDraft();
    composeTail=composeTail.then(async()=>{
      if(!live()||generation!==valueGeneration)return false;
      const result=await request({action:'compose',current:value,draft:term,operation});
      if(!live()||generation!==valueGeneration||result.expression===null)return false;
      return apply(result.expression,generation);
    }).catch(error=>{if(live()&&generation===valueGeneration)showError(String(error));return false;});
    if(immediate&&!inline)void composeTail.then(applied=>{if(applied&&live()&&generation===valueGeneration)return finish(false);});
    return composeTail;
  };
  const scheduleCounts = () => {
    clearTimeout(timer); const generation=++countGeneration;
    timer=setTimeout(async()=>{
      if(!live())return;
      const current=value, term=currentDraft();
      const buttons=[...popup.querySelectorAll('[data-selection-op]')];
      try {
        if(!inline){
          const evaluated=await request({action:'evaluate',expression:current});
          if(!live()||generation!==countGeneration)return;
          const status=popup.querySelector('.selection-builder-status');
          if(status){status.classList.toggle('error',!evaluated.valid);status.textContent=evaluated.valid?(evaluated.count===null?'':`${evaluated.count} atoms`):(evaluated.error||'Invalid selection expression');}
        }
        for(const control of buttons){
          const result=await request({action:'compose',current,draft:term,operation:control.dataset.selectionOp});
          if(!live()||generation!==countGeneration)return;
          control.disabled=result.expression===null;
          let count=null;
          if(result.expression!==null) count=(await request({action:'evaluate',expression:result.expression})).count;
          if(!live()||generation!==countGeneration)return;
          control.textContent=control.dataset.label+(count===null?'':`  ${count}`);
        }
      }catch(error){if(live()&&generation===countGeneration)showError(String(error));}
    },250);
  };
  const render = () => {
    if(!live())return;
    popup.replaceChildren();
    const tabs=document.createElement('div');tabs.className='selection-builder-tabs';
    for(const name of ['Named','History','Term','Mod']){
      const control=button(name,()=>{tab=name;render();},`Selection tab ${name}`);control.classList.toggle('active',name===tab);tabs.append(control);
    }
    popup.append(tabs);
    const body=document.createElement('div');body.className='selection-builder-body';popup.append(body);
    const status=document.createElement('div');status.className='selection-builder-status';popup.append(status);
    if(!context){status.textContent='Loading selections...';rect();return;}
    const choice=(label,expression)=>{const control=button(label,()=>quick(expression),`Selection choice ${expression}`);control.title=expression;control.classList.toggle('active',expression===value.trim());body.append(control);};
    if(tab==='Named'){
      choice('all (*)','*');
      for(const [label,values] of [['Selected',!inline&&context.current?[context.current]:[]],['Scene',context.scene],['Global',context.global]]){
        if(!values.length)continue;const heading=document.createElement('div');heading.className='selection-builder-heading';heading.textContent=label;body.append(heading);
        for(const expression of values)choice(expression,expression);
      }
    }else if(tab==='History'){
      if(!context.history.length)body.textContent='No history';
      for(const expression of context.history)choice(expression,expression);
    }else if(tab==='Term'){
      const keywords=document.createElement('select');keywords.setAttribute('aria-label','Selection keyword');
      for(const definition of context.keywords){const option=document.createElement('option');option.value=definition.key;option.textContent=definition.label;keywords.append(option);}
      keywords.value=draft.keyword;
      keywords.addEventListener('change',()=>{draft.keyword=keywords.value;draft.fields=draft.keyword==='bfac'?{op:'<'}:{};draft.picked='';render();});
      body.append(keywords);
      const definition=context.keywords.find(item=>item.key===draft.keyword);
      const fields=document.createElement('div');fields.className='selection-builder-fields';body.append(fields);
      if(definition.kind==='hierarchical')for(const key of ['chain','resid','aname'])fields.append(field(key,key==='aname'?'atom':key));
      else if(['named','history'].includes(definition.kind)){
        const select=document.createElement('select');select.setAttribute('aria-label','Selection candidates');
        const empty=document.createElement('option');empty.value='';empty.textContent='Select...';empty.disabled=true;select.append(empty);
        const values=definition.kind==='named'?[...context.scene,...context.global]:context.history;
        for(const value of values){const option=document.createElement('option');option.value=value;option.textContent=value;select.append(option);}
        select.value=draft.picked;select.addEventListener('change',()=>{draft.picked=select.value;scheduleCounts();});fields.append(select);
      }else if(definition.kind!=='none'){
        if(definition.kind==='compare'){
          const select=document.createElement('select');select.setAttribute('aria-label','Selection comparison');
          for(const value of ['<','>','<=','>=','=']){const option=document.createElement('option');option.value=value;option.textContent=value;select.append(option);}
          select.value=draft.fields.op||'<';select.addEventListener('change',()=>{draft.fields.op=select.value;scheduleCounts();});fields.append(select);
        }else if(definition.kind==='nameValue')fields.append(field('name','property'));
        const input=field('value','value');fields.append(input);
        if(definition.candidates)populateCandidates(input,fields,definition.candidates);
      }
    }else{
      const select=document.createElement('select');select.setAttribute('aria-label','Selection distance');
      for(const value of ['3','4','5','6','8','10']){const option=document.createElement('option');option.value=value;option.textContent=`${value} Å`;select.append(option);}
      select.value=draft.distance;select.addEventListener('change',()=>{draft.distance=select.value;scheduleCounts();});body.append(select);
    }
    if(tab==='Term'||tab==='Mod'){
      const operations=tab==='Term'?[['set','Set'],['add','Add'],['sub','Sub'],['intersect','Intsec']]:[['not','Invert'],['byres','Byres'],['sidechain','Sidech'],['mainchain','Mainch'],['around','Arnd'],['expand','Expn']];
      const grid=document.createElement('div');grid.className='selection-builder-operations';popup.append(grid);
      for(const [operation,label]of operations){const control=button(label,()=>{void compose(operation);},`Selection ${label}`);control.dataset.selectionOp=operation;control.dataset.label=label;control.disabled=operation!=='set'&&!value.trim();grid.append(control);}
    }
    scheduleCounts();rect();
  };
  if(!inline){
    document.addEventListener('pointerdown',outside,true);
    document.addEventListener('keydown',escape,true);
    window.addEventListener('resize',rect);
  }
  render();
  const refresh = async () => {
    try {
      if(!await updateCueMolSelectionHistory() || !live())return;
      const result=await request({action:'context'});
      if(!live())return;
      const changed=JSON.stringify(context)!==JSON.stringify(result);
      const initial=context===null;
      context=result;
      if(JSON.stringify(selectionHistoryEntries)!==JSON.stringify(context.history))syncCueMolSelectionHistory(context);
      if(initial || changed && (tab==='Named'||tab==='History'||['named','history'].includes(draft.keyword)))render();
      else {
        const kind=context.keywords.find(item=>item.key===draft.keyword)?.candidates;
        const fields=popup.querySelector('.selection-builder-fields');
        const input=fields?.querySelector('[aria-label="Selection value"]');
        if(tab==='Term'&&kind&&input)populateCandidates(input,fields,kind);
        scheduleCounts();
      }
    } catch(error){if(live())showError(String(error));}
  };
  void refresh();
  return {identity,original,finish,cancel:()=>close(true),setValue,refresh,
    invalidate:()=>{valueGeneration++;},
    syncValue:(next,external=true)=>{if(next!==value)setValue(next,external);},
    owns:element=>element===input,attach:element=>{input=element;input.value=value;requestAnimationFrame(rect);}};
}

let cueMolSelectionPane = null;
let cueMolSelectionHistoryReady = false;
let cueMolSelectionHistoryTail = Promise.resolve();

function syncCueMolSelectionHistory(result) {
  if (Array.isArray(result?.history)) {
    selectionHistoryEntries = result.history;
    saveSelectionHistory();
  }
  if (result?.warning) showError(result.warning);
}

function updateCueMolSelectionHistory(expression) {
  if (!invoke) return Promise.resolve(false);
  const legacy=[...selectionHistoryEntries];
  cueMolSelectionHistoryTail = cueMolSelectionHistoryTail.then(async () => {
    if (!cueMolSelectionHistoryReady) {
      syncCueMolSelectionHistory(await invoke('inspector_request', {tool:'selection_history',args:{action:'import',entries:legacy}}));
      cueMolSelectionHistoryReady = true;
    }
    if (expression !== undefined) {
      syncCueMolSelectionHistory(await invoke('inspector_request', {tool:'selection_history',args:{action:'remember',expression}}));
    }
    return true;
  }).catch(error => {showError(String(error));return false;});
  return cueMolSelectionHistoryTail;
}

async function refreshCueMolSelectionSnapshot() {
  const previous = snapshot;
  try {
    const current = await invoke('get_snapshot');
    if (snapshot !== previous || current.activeSceneUid !== previous?.activeSceneUid) return;
    snapshot = current;
    renderShell();
    void renderViewport();
  } catch (error) { showError(String(error)); }
}

function renderCueMolSelectionPane() {
  const scene = snapshot?.activeSceneUid;
  const objects = activeMoleculeObjects();
  if (cueMolSelectionPane && cueMolSelectionPane.scene !== scene) selectionObjectUid = null;
  if (!objects.some(object => object.uid === selectionObjectUid)) selectionObjectUid = objects[0]?.uid ?? null;
  replaceOptions(selectionObject, objects, selectionObjectUid);
  const object = objects.find(object => object.uid === selectionObjectUid);
  const identity = `${scene}:${selectionObjectUid}`;
  if (cueMolSelectionPane?.identity !== identity) {
    cueMolSelectionPane?.dispose();
    cueMolSelectionPane = createCueMolSelectionPane(scene, object, identity);
  }
  cueMolSelectionPane.sync(object);
}

function createCueMolSelectionPane(scene, object, identity) {
  const molecule = object?.uid;
  const byId = id => document.getElementById(`selection-${id}`);
  const applyButton=byId('apply'), center=byId('center'), clear=byId('clear'), define=byId('define');
  const definition=byId('definition'), name=byId('definition-name');
  const validation=byId('validation'), error=byId('error'), container=byId('builder');
  let current=object?.selection || '';
  let pendingExpression=null;
  let draftGeneration=0, pendingDraftGeneration=null;
  let disposed=false, timer=null, refreshTimer=null, validationGeneration=0;
  let editTail=Promise.resolve();
  let validationKey='', refreshKey='';
  const live=()=>!disposed && snapshot?.activeSceneUid===scene && selectionObjectUid===molecule;
  const visible=()=>live() && sidePanelIsExpanded('Selection');
  const showError=text=>{if(live())error.textContent=text;};
  selectionInput.value=current;
  selectionInput.disabled=!molecule;
  name.value=''; definition.hidden=true; define.setAttribute('aria-expanded','false');
  validation.textContent=''; error.textContent=''; container.replaceChildren();
  const controls=()=>{
    applyButton.disabled=!molecule || selectionInput.value===current;
    center.disabled=!molecule || current==='';
    clear.disabled=!molecule || selectionInput.value==='' && current==='';
    define.disabled=!molecule || !selectionInput.value.trim();
    definition.querySelector('button').disabled=define.disabled || !name.value.trim();
  };
  const validate=()=>{
    clearTimeout(timer); const generation=++validationGeneration;
    validation.textContent=''; validation.classList.remove('error');
    if(!molecule || !selectionInput.value.trim())return;
    timer=setTimeout(async()=>{
      if(!visible())return;
      try {
        const result=await invoke('inspector_request',{tool:'selection_info',args:{moleculeId:molecule,action:'evaluate',expression:selectionInput.value}});
        if(!live() || generation!==validationGeneration)return;
        validation.classList.toggle('error',!result.valid);
        validation.textContent=result.valid ? result.count===null ? '' : `${result.count} atoms` : result.error || 'Invalid selection expression';
      } catch(error){if(live()&&generation===validationGeneration)showError(String(error));}
    },250);
  };
  const edit=(action,expression,remember=false,selectionName)=>{
    const requestedDraftGeneration=draftGeneration;
    editTail=editTail.then(async()=>{
      if(!live())return false;
      pendingExpression=action==='apply' ? expression : null;
      pendingDraftGeneration=requestedDraftGeneration;
      try {
        const result=await invoke('inspector_request',{tool:'selection_edit',args:{sceneId:scene,moleculeId:molecule,action,expression,remember,name:selectionName}});
        syncCueMolSelectionHistory(result);
        if(!live()){await refreshCueMolSelectionSnapshot();return false;}
        error.textContent='';
        snapshot=result.snapshot;
        sync(snapshot.objects.find(object=>object.uid===molecule));
        renderShell();
        void renderViewport();
        if(action==='define'){
          definition.hidden=true; name.value=''; define.setAttribute('aria-expanded','false');
        }
        void builder.refresh();
        return true;
      } catch(error){showError(String(error));return false;}
      finally {pendingExpression=null;pendingDraftGeneration=null;if(live())controls();}
    });
    return editTail;
  };
  const builder=createCueMolSelectionPicker(selectionInput,{moleculeId:molecule},identity,expression=>edit('apply',expression),{
    container,draft:{keyword:'hierarchical',fields:{},picked:'',distance:'5'},isLive:live,onError:showError,
  });
  container.hidden=!molecule;
  selectionInput.oninput=()=>{draftGeneration++;builder.invalidate();error.textContent='';controls();validate();};
  selectionInput.onkeydown=event=>{
    if(event.key==='Enter'&&!event.isComposing){event.preventDefault();if(!applyButton.disabled)void edit('apply',selectionInput.value,true);}
  };
  applyButton.onclick=()=>{builder.invalidate();void edit('apply',selectionInput.value,true);};
  center.onclick=()=>{void edit('center',current);};
  clear.onclick=()=>{draftGeneration++;builder.invalidate();selectionInput.value='';controls();validate();void edit('apply','');};
  define.onclick=()=>{
    definition.hidden=!definition.hidden; name.value='';
    define.setAttribute('aria-expanded',String(!definition.hidden)); controls();
    if(!definition.hidden)name.focus();
  };
  name.oninput=controls;
  name.onkeydown=event=>{
    if(event.key==='Enter'&&event.isComposing){event.preventDefault();}
    else if(event.key==='Escape'&&!event.isComposing){event.preventDefault();definition.hidden=true;define.setAttribute('aria-expanded','false');}
  };
  definition.onsubmit=event=>{
    event.preventDefault();
    if(name.value.trim()&&selectionInput.value.trim())void edit('define',selectionInput.value,false,name.value.trim());
  };
  controls();
  function sync(object) {
    const applied=object?.selection || '';
    if(current!==applied){
      const keepDraft=applied===pendingExpression && draftGeneration!==pendingDraftGeneration;
      current=applied;
      if(!keepDraft)selectionInput.value=applied;
      error.textContent='';
      builder.syncValue(applied,applied!==pendingExpression);
    }
    const key=JSON.stringify([selectionInput.value,snapshot?.structureKey,snapshot?.actionHistoryKey]);
    if(validationKey!==key){validationKey=key;validate();}
    const next=JSON.stringify([snapshot?.structureKey,snapshot?.actionHistoryKey,snapshot?.styles,selectionHistoryEntries]);
    if(refreshKey!==next){
      refreshKey=next;clearTimeout(refreshTimer);
      refreshTimer=setTimeout(()=>{if(visible())void builder.refresh();},30);
    }
    controls();
  }
  return {identity,scene,builder,sync,
    dispose:()=>{disposed=true;clearTimeout(timer);clearTimeout(refreshTimer);validationGeneration++;builder.cancel();},
  };
}
