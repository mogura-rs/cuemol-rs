function setupCueMolCatalog() {
  const host = document.querySelector('[data-activity-panel="catalog"]');
  if (!host) return;
  const forms = {text:'aname CA',kind:'ribbon',filter:'',secret:'',combo:'gpt-5.6',multiline:'',number:50,number2:8,drag:1,enabled:true,gated:true};
  const lists = {selection:'*',opacity:80,angle:90,color:'#3b82f6',selected:'3J3Q',segment:'All',radio:'Temporary'};
  const syncers = [];
  const sync = () => syncers.forEach(update => update());
  const element = (tag, text, className) => {const node=document.createElement(tag);if(text)node.textContent=text;if(className)node.className=className;return node;};
  const pane = title => {
    const section=element('section',null,'catalog-pane');
    const toggle=element('button',title,'catalog-pane-toggle');toggle.type='button';toggle.setAttribute('aria-label',title);toggle.setAttribute('aria-expanded','true');
    const body=element('div',null,'catalog-scroll');
    toggle.addEventListener('click',()=>{body.hidden=!body.hidden;toggle.setAttribute('aria-expanded',String(!body.hidden));section.classList.toggle('is-collapsed',body.hidden);});
    section.append(toggle,body);host.append(section);return body;
  };
  const group = (parent,title) => {
    const box=element('fieldset',null,'catalog-group');box.append(element('legend',title));parent.append(box);return box;
  };
  const field = (parent,label) => {
    const box=element('div',null,'catalog-field');const caption=element('label',label);box.append(caption);parent.append(box);return {box,caption};
  };
  let fieldId=0;
  const labelled = (input,caption,label) => {input.id=`catalog-input-${fieldId++}`;input.setAttribute('aria-label',label);caption.htmlFor=input.id;};
  const text = (parent,label,state,key,options={}) => {
    const {box,caption}=field(parent,label);const input=element(options.multiline?'textarea':'input');
    if(!options.multiline)input.type=options.password?'password':'text';
    if(options.multiline)input.rows=1;
    labelled(input,caption,label);input.value=state[key];input.placeholder=options.placeholder||'';input.disabled=!!options.disabled;
    if(options.invalid)input.setAttribute('aria-invalid','true');
    input.addEventListener('input',()=>{if(!options.invalid)state[key]=input.value;sync();});
    if(options.multiline){
      const resize=()=>{input.style.height='auto';input.style.height=`${Math.min(132,Math.max(22,input.scrollHeight))}px`;};
      input.addEventListener('input',resize);input.addEventListener('focus',resize);
      input.addEventListener('keydown',event=>{if(event.key==='Enter'&&(event.ctrlKey||event.metaKey)&&!event.isComposing)event.preventDefault();});
    }
    syncers.push(()=>{if(document.activeElement!==input)input.value=state[key];});box.append(input);return input;
  };
  const select = (parent,label,state,key,options) => {
    const {box,caption}=field(parent,label),input=element('select');labelled(input,caption,label);
    for(const [value,title] of options){const option=element('option',title);option.value=value;input.append(option);}
    input.value=state[key];input.addEventListener('change',()=>{state[key]=input.value;sync();});
    syncers.push(()=>{input.value=state[key];});box.append(input);return input;
  };
  const checkbox = (parent,label,state,key) => {
    const box=element('label',null,'catalog-check'),input=element('input');input.type='checkbox';input.setAttribute('aria-label',label);
    input.checked=state[key];input.addEventListener('change',()=>{state[key]=input.checked;sync();});
    syncers.push(()=>{input.checked=state[key];});box.append(input,document.createTextNode(label));parent.append(box);return input;
  };
  const number = (parent,label,state,key,{min=0,max=100,step=1,unit='',slider=false,disabled=()=>false,onChange=()=>{}}={}) => {
    const {box,caption}=field(parent,label),row=element('div',null,'catalog-number');
    const input=element('input');input.type='number';input.min=String(min);input.max=String(max);input.step=String(step);labelled(input,caption,label);input.value=state[key];
    const write=raw=>{const value=Number(raw);if(!Number.isFinite(value)||raw==='')return;state[key]=Math.max(min,Math.min(max,value));onChange();sync();};
    input.addEventListener('change',()=>write(input.value));
    const owner={key:input.id,target:{},onFinish:sync};
    attachCueMolNumericDrag(input,caption,{key,get value(){return String(state[key]);},type:'real',schema:{min,max,step}},write,owner);
    syncers.push(()=>{input.disabled=disabled();if(document.activeElement!==input)input.value=state[key];});
    row.append(input);if(unit)row.append(element('span',unit));box.append(row);
    if(slider){const range=element('input');range.type='range';range.min=min;range.max=max;range.step=step;range.setAttribute('aria-label',`${label} slider`);range.value=state[key];range.addEventListener('input',()=>write(range.value));syncers.push(()=>{range.value=state[key];range.disabled=disabled();});box.append(range);}
    return input;
  };
  const selection = (parent,label,state,key,onChange=()=>{}) => {
    const input=text(parent,label,state,key);
    input.addEventListener('change',onChange);
    const controls=element('span',null,'catalog-selection-controls');input.parentElement.append(controls);
    const open=element('button','▾');open.type='button';open.setAttribute('aria-label',`Build ${label}`);controls.append(open);
    open.addEventListener('click',()=>{
      cueMolSelectionPicker?.cancel();
      const identity=`${snapshot?.activeSceneUid}:catalog:${input.id}`;
      cueMolSelectionPicker=createCueMolSelectionPicker(input,{},identity,value=>{state[key]=value;onChange();sync();});
    });
    return input;
  };
  const first=pane('Catalog 1');
  const hierarchy=group(first,'Label hierarchy (FieldSection vs Field)');
  select(hierarchy,'Molecule',forms,'kind',[['ribbon','1CRN'],['cpk','3J3Q']]);hierarchy.append(element('strong','Term'));text(hierarchy,'Dist',forms,'text');
  const texts=group(first,'Text & Select');
  text(texts,'TextField',forms,'text');text(texts,'TextField (invalid)',{text:'bogus('},'text',{invalid:true});
  text(texts,'TextField (disabled)',{text:'locked'},'text',{disabled:true});
  text(texts,'TextField (filter / leftIcon)',forms,'filter',{placeholder:'⌕ Filter…'});
  text(texts,'TextField (password)',forms,'secret',{password:true,placeholder:'API key'});
  const combo=text(texts,'ComboBoxField (labelled suggestions)',forms,'combo',{placeholder:'Pick one, or type anything'});
  const suggestions=element('datalist');suggestions.id='catalog-suggestions';
  for(const [value,label] of [['gpt-6-astra','Most capable'],['gpt-5.6','Flagship']]){const option=element('option');option.value=value;option.label=label;suggestions.append(option);}combo.setAttribute('list',suggestions.id);texts.append(suggestions);
  text(texts,'TextAreaField',forms,'multiline',{multiline:true,placeholder:'Grows from one row to six. Cmd/Ctrl+Enter submits'});
  select(texts,'SelectField',forms,'kind',[['ribbon','Ribbon'],['cpk','CPK'],['stick','Stick']]);
  const numeric=group(first,'Numeric & Switch');
  number(numeric,'NumericField (slider + unit)',forms,'number',{unit:'Å',slider:true});
  number(numeric,'NumericField (no slider)',forms,'number2');
  number(numeric,'DragNumericField (drag-snap / click-edit)',forms,'drag',{max:10,step:0.1,unit:'Å'});
  const time=dialogTimeField('TimeField',1.5,numeric);
  time.inputs.forEach((input,index)=>{
    const axis=element('label',null,'catalog-time-axis');input.replaceWith(axis);
    const caption=element('span',['h','m','s','ms'][index]);axis.append(caption,input);
    const owner={key:`catalog-time-${index}`,target:{},onFinish:()=>{time.value=time.value;}};
    attachCueMolNumericDrag(input,caption,{key:String(index),get value(){return String(Number(input.value));},type:'integer',schema:{min:0,max:[99,59,59,999][index],step:1}},value=>{input.value=value;time.value=time.value;},owner);
    input.addEventListener('wheel',event=>{if(!event.ctrlKey&&!event.metaKey)return;event.preventDefault();const scale=[3600,60,1,0.001][index];time.value=formatAnimationObjectTime(Math.max(0,(parseAnimationObjectTime(time.value)||0)+Math.sign(-event.deltaY)*scale));},{passive:false});
  });
  const vector=group(numeric,'VectorField (x/y/z cells; qlib::Vector4D text)'),coords={x:1,y:2,z:3,text:'(1,2,3)'};
  for(const key of ['x','y','z'])number(vector,`Vector ${key}`,coords,key,{min:-1e10,max:1e10,step:0.1,onChange:()=>{coords.text=`(${coords.x},${coords.y},${coords.z})`;}});
  const vectorText=text(vector,'Vector text',coords,'text');vectorText.addEventListener('change',()=>{
    const source=coords.text.trim(),values=source.slice(1,-1).split(',').map(Number);
    const valid=source.startsWith('(')&&source.endsWith(')')&&values.length===3&&values.every(Number.isFinite);vectorText.setAttribute('aria-invalid',String(!valid));
    if(valid){[coords.x,coords.y,coords.z]=values;sync();}
  });
  checkbox(numeric,'SwitchField',forms,'enabled');checkbox(numeric,'CheckboxField (inline controlFirst)',forms,'enabled');
  checkbox(numeric,'GatedControl (checkbox + control, one property)',forms,'gated');
  number(numeric,'Gated value',forms,'drag',{max:10,step:0.1,unit:'Å',disabled:()=>!forms.gated});
  const second=pane('Catalog 2');
  const grid=group(second,'FieldGrid (aligned label column)'),location={x:-0.52,y:-0.7,z:0.54612};grid.classList.add('catalog-aligned');
  for(const [key,label] of [['x','Location X'],['y','Y'],['z','Z']])number(grid,label,location,key,{min:-1e10,max:1e10,step:0.1,unit:'m'});
  const shared=group(second,'Shared widgets');selection(shared,'MolSelList (selection picker)',lists,'selection');
  const color=text(shared,'ColorField',lists,'color');const palette=element('input');palette.type='color';palette.value=lists.color;palette.setAttribute('aria-label','Color swatch');color.parentElement.append(palette);palette.addEventListener('input',()=>{lists.color=palette.value;sync();});syncers.push(()=>{if(/^#[0-9a-f]{6}$/i.test(lists.color))palette.value=lists.color;});
  number(shared,'Opacity',lists,'opacity',{slider:true,unit:'%'});number(shared,'Stride (slider=false)',lists,'opacity',{min:1,max:9999});number(shared,'Angle (hideLabel, in a Field)',lists,'angle',{max:360,unit:'°'});
  const listbox=group(second,'Listbox (list / tree row)'),list=element('div',null,'catalog-list');list.setAttribute('role','listbox');list.setAttribute('aria-label','Catalog structures');listbox.append(list);
  for(const name of ['1CRN','3J3Q','Water','Ligand']){const row=element('button',name);row.type='button';row.setAttribute('role','option');row.addEventListener('click',()=>{lists.selected=name;sync();});row.addEventListener('keydown',event=>{const index=['1CRN','3J3Q','Water','Ligand'].indexOf(name);let next=index;if(event.key==='ArrowDown')next=Math.min(3,index+1);else if(event.key==='ArrowUp')next=Math.max(0,index-1);else return;event.preventDefault();list.children[next].click();list.children[next].focus();});syncers.push(()=>row.setAttribute('aria-selected',String(lists.selected===name)));list.append(row);}
  const tree=group(second,'Tree (ListboxTree) / disclosure caret'),root=element('details');root.open=true;root.append(element('summary','1ggg (MolCoord)'),element('div','ribbon1 (ribbon)'),element('div','simple1 (simple)'));tree.append(root,element('p','▾  ▸  ·  expanded / collapsed / leaf'));
  const segments=group(second,'Segmented control');
  for(const name of ['All','Backbone','Sidechain']){const button=element('button',name);button.type='button';button.addEventListener('click',()=>{lists.segment=name;sync();});syncers.push(()=>button.setAttribute('aria-pressed',String(lists.segment===name)));segments.append(button);}
  const radios=group(second,'Radio group');radios.append(element('span','Location'));
  for(const name of ['Temporary','Custom']){const label=element('label',null,'catalog-check'),input=element('input');input.type='radio';input.name='catalog-location';input.value=name;input.addEventListener('change',()=>{lists.radio=name;sync();});syncers.push(()=>{input.checked=lists.radio===name;});label.append(input,document.createTextNode(name));radios.append(label);}
  const buttons=group(second,'Buttons');
  for(const name of ['Default','Primary','Icon','Minimal','Disabled']){const item=element('button',name==='Icon'?'✓ Icon':name,name==='Primary'?'dialog-primary':name==='Minimal'?'catalog-minimal':'');item.type='button';item.disabled=name==='Disabled';buttons.append(item);}
  const third=pane('Catalog 3');
  const makeProperty=(value,fallback,modified=true)=>({value,fallback,modified});
  const rows={label:makeProperty('backbone','chain A'),visible:makeProperty(false,true),opacity:makeProperty(0.4,1),width:makeProperty(1,1,false),selection:makeProperty('resn ALA','*')};
  const mock={visible:makeProperty(false,true),opacity:makeProperty(0.4,1),width:makeProperty(2.5,1)};
  const property=(parent,label,key,p,type,options={})=>{
    const row=element('div',null,'catalog-property');row.dataset.property=key;
    const header=element('div',null,'catalog-property-heading'),caption=element('span',label),reset=element('button','↶');reset.type='button';reset.setAttribute('aria-label',`Reset ${key}`);reset.title=`Default: ${p.fallback}`;header.append(caption,reset);row.append(header);parent.append(row);
    const changed=()=>{p.modified=true;sync();};
    if(type==='number')number(row,key,p,'value',{...options,onChange:changed});
    else if(type==='boolean'){const input=checkbox(row,key,p,'value');input.addEventListener('change',changed);}
    else if(type==='selection')selection(row,key,p,'value',changed).addEventListener('input',changed);
    else text(row,key,p,'value').addEventListener('input',changed);
    reset.addEventListener('click',()=>{p.value=p.fallback;p.modified=false;sync();});
    syncers.push(()=>{row.classList.toggle('modified',p.modified);reset.disabled=!p.modified;});
  };
  const isolated=group(third,'Property rows (PropertyField)');
  property(isolated,'Label','Property Label',rows.label,'text');property(isolated,'Visible','Property Visible',rows.visible,'boolean');property(isolated,'Opacity','Property Opacity',rows.opacity,'number',{max:1,step:0.1});property(isolated,'Line width','Property Line width',rows.width,'number',{max:10,step:0.2,unit:'px'});property(isolated,'Selection','Property Selection',rows.selection,'selection');
  const inspector=group(third,'Inspector property pane (mockup)'),modes=element('div',null,'catalog-mode');inspector.append(modes);
  let mode='Properties';for(const name of ['Properties','Generic']){const button=element('button',name);button.type='button';button.addEventListener('click',()=>{mode=name;sync();});syncers.push(()=>button.setAttribute('aria-pressed',String(mode===name)));modes.append(button);}
  const reset=element('button','Reset all to default');reset.type='button';reset.addEventListener('click',()=>{Object.values(mock).forEach(p=>{p.value=p.fallback;p.modified=false;});sync();});syncers.push(()=>{reset.disabled=!Object.values(mock).some(p=>p.modified);});inspector.append(reset);
  const basic=element('details');basic.open=true;basic.append(element('summary','Basic settings'));inspector.append(basic);property(basic,'Visible','Mock Visible',mock.visible,'boolean');property(basic,'Opacity','Mock Opacity',mock.opacity,'number',{max:1,step:0.1});
  const edge=element('details');edge.open=true;edge.append(element('summary','Edge lines'));inspector.append(edge);property(edge,'Width','Mock Width',mock.width,'number',{max:10,step:0.2,unit:'px'});
  sync();
}
window.addEventListener('DOMContentLoaded',setupCueMolCatalog);
