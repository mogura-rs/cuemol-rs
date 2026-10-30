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

function createCueMolSelectionPicker(input, target, identity, commit) {
  const inspectorKey = cueMolPropertyInspector.key;
  const original = input.value;
  let value = original;
  let tab = 'Named';
  let context = null;
  const draft = cueMolPropertyInspector.selectionDrafts.get(input.id) || {keyword:'hierarchical',fields:{},picked:'',distance:'5'};
  cueMolPropertyInspector.selectionDrafts.set(input.id, draft);
  const candidates = new Map();
  let disposed = false;
  let composeTail = Promise.resolve();
  let valueGeneration = 0;
  let countGeneration = 0;
  let timer = null;
  let closing = false;
  const popup = document.createElement('div');
  popup.className = 'selection-builder-popup';
  popup.setAttribute('role', 'dialog'); popup.setAttribute('aria-label', 'Build selection');
  document.body.append(popup);
  const live = () => !disposed && cueMolPropertyInspector.key === inspectorKey && snapshot?.activeSceneUid === Number(identity.split(':')[0]);
  const request = args => invoke('inspector_request', {tool:'selection_info',args:{...target,...args}});
  const currentDraft = () => ({...draft,fields:{...draft.fields}});
  const rect = () => {
    if (!input.isConnected) return;
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
    if (cancel && input.isConnected && cueMolPropertyInspector.key === inspectorKey) input.value = original;
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
  const showError = text => { const status=popup.querySelector('.selection-builder-status'); if (status) {status.textContent=text; status.classList.add('error');} };
  const button = (label, action, aria=label) => {
    const element=document.createElement('button'); element.type='button'; element.textContent=label; element.setAttribute('aria-label',aria); element.addEventListener('click',action); return element;
  };
  const setValue = (next, external=true) => {
    if (external) valueGeneration++;
    value=next; input.value=next; scheduleCounts();
  };
  const quick = expression => { setValue(expression); void finish(false); };
  const field = (key, placeholder) => {
    const element=document.createElement('input'); element.type='text'; element.placeholder=placeholder; element.value=draft.fields[key] || '';
    element.setAttribute('aria-label',`Selection ${key}`);
    element.addEventListener('input',()=>{draft.fields[key]=element.value;scheduleCounts();});
    element.addEventListener('keydown',event=>{if(event.key==='Enter'&&!event.isComposing&&!value.trim()){event.preventDefault();void compose('set',true);}});
    return element;
  };
  const compose = (operation, immediate=false) => {
    if (!live() || closing) return;
    const generation=valueGeneration, term=currentDraft();
    composeTail=composeTail.then(async()=>{
      if(!live()||generation!==valueGeneration)return;
      const result=await request({action:'compose',current:value,draft:term,operation});
      if(!live()||generation!==valueGeneration)return;
      if(result.expression!==null)setValue(result.expression,false);
    }).catch(error=>{if(live()&&generation===valueGeneration)showError(String(error));});
    if(immediate)void composeTail.then(()=>finish(false));
    return composeTail;
  };
  const scheduleCounts = () => {
    clearTimeout(timer); const generation=++countGeneration;
    timer=setTimeout(async()=>{
      if(!live())return;
      const current=value, term=currentDraft();
      const buttons=[...popup.querySelectorAll('[data-selection-op]')];
      try {
        const evaluated=await request({action:'evaluate',expression:current});
        if(!live()||generation!==countGeneration)return;
        const status=popup.querySelector('.selection-builder-status');
        if(status){status.classList.toggle('error',!evaluated.valid);status.textContent=evaluated.valid?(evaluated.count===null?'':`${evaluated.count} atoms`):(evaluated.error||'Invalid selection expression');}
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
      for(const [label,values] of [['Selected',context.current?[context.current]:[]],['Scene',context.scene],['Global',context.global]]){
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
        if(definition.candidates){
          const kind=definition.candidates;
          if(!candidates.has(kind))candidates.set(kind,request({action:'values',kind}));
          void candidates.get(kind).then(values=>{
            if(!live()||!input.isConnected||!values.length)return;
            const select=document.createElement('select');select.setAttribute('aria-label','Selection candidates');
            const empty=document.createElement('option');empty.value='';empty.textContent='▾';select.append(empty);
            for(const value of values){const option=document.createElement('option');option.value=value;option.textContent=value;select.append(option);}
            select.addEventListener('change',()=>{input.value=select.value;draft.fields.value=select.value;scheduleCounts();});fields.append(select);
          }).catch(error=>{if(live())showError(String(error));});
        }
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
  document.addEventListener('pointerdown',outside,true);
  document.addEventListener('keydown',escape,true);
  window.addEventListener('resize',rect);
  render();
  void request({action:'context'}).then(result=>{if(live()){context=result;render();}}).catch(error=>{if(live())showError(String(error));});
  return {identity,original,finish,cancel:()=>close(true),setValue,owns:element=>element===input,attach:element=>{input=element;input.value=value;requestAnimationFrame(rect);}};
}
