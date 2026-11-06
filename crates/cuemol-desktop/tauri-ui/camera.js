const cueMolCameraPane = {scene:null, fingerprint:null, editing:null, drag:null};

async function editSavedCamera(tool, args, scene=snapshot.activeSceneUid) {
  await cancelCueMolNumericDrag();
  try {
    const result = await invoke('inspector_request', {tool, args:{...args, sceneId:scene}});
    if (snapshot.activeSceneUid === scene && result.snapshot) {
      snapshot = result.snapshot;
      if (tool === 'rename_camera') selectedCameraName = args.newName;
      renderShell();
      void renderViewport();
    }
    return true;
  } catch(error) { showError(error); return false; }
}

function selectSavedCamera(name) {
  selectedCameraName = name;
  workspaceSelectionKind = name === null ? 'cameraRoot' : 'camera';
  renderCameraPane();
  document.getElementById('camera-list').focus();
}

function renameSavedCamera(camera) {
  const scene = snapshot.activeSceneUid;
  const row = [...document.querySelectorAll('#camera-list [data-camera-name]')].find(row=>row.dataset.cameraName===camera.name);
  if (!row) return;
  const input = document.createElement('input');
  input.className = 'workspace-inline-rename';
  input.setAttribute('aria-label','Camera name');
  input.value = camera.name;
  row.replaceChildren(input);
  row.draggable = false;
  const editor = {scene,name:camera.name,input};
  cueMolCameraPane.editing = editor;
  let done = false;
  const finish = async commit => {
    if(done) return;
    done = true;
    const name = input.value.trim();
    cueMolCameraPane.editing = null;
    if(commit && name && name!==camera.name && snapshot.activeSceneUid===scene) {
      await editSavedCamera('rename_camera',{name:camera.name,newName:name},scene);
    }
    cueMolCameraPane.fingerprint = null;
    renderCameraPane();
    document.getElementById('camera-list').focus();
  };
  input.addEventListener('keydown',event=>{
    event.stopPropagation();
    if(event.key==='Enter' || event.key==='Escape') {
      event.preventDefault();
      void finish(event.key==='Enter');
    }
  });
  input.addEventListener('click',event=>event.stopPropagation());
  input.addEventListener('dblclick',event=>event.stopPropagation());
  input.addEventListener('blur',()=>void finish(true));
  input.focus(); input.select();
}

function savedCameraContext(event,camera) {
  event.preventDefault();
  event.stopPropagation();
  selectedCameraName = camera?.name ?? null;
  workspaceSelectionKind = camera ? 'camera' : 'cameraRoot';
  const scene = snapshot.scenes.find(scene=>scene.active);
  const items = workspaceCameraContextItems(scene,camera).map(item=>{
    if(item?.label==='Rename...') return {...item,action:()=>renameSavedCamera(camera)};
    if(item?.label==='Delete') return {...item,action:()=>editSavedCamera('delete_camera',{name:camera.name},scene.uid)};
    return item;
  });
  if(camera)items.splice(2,0,{label:'Cut',action:()=>{clipboardScope='tree';return dispatchClipboard('cut');}});
  renderCameraPane();
  showWorkspaceContextMenu(event,items);
}

function renderCameraPane() {
  const state=cueMolCameraPane, body=document.getElementById('camera-list');
  if (!body) return;
  const scene=snapshot?.activeSceneUid ?? null;
  if(state.scene!==scene) {
    state.scene=scene;state.fingerprint=null;state.editing=null;state.drag=null;
    selectedCameraName=null;
  }
  const cameras=activeCameras().slice().sort((a,b)=>Number(b.name==='__current')-Number(a.name==='__current'));
  if(!cameras.some(camera=>camera.name===selectedCameraName))selectedCameraName=null;
  const selected=cameras.find(camera=>camera.name===selectedCameraName);
  const toolbar=document.getElementById('camera-toolbar');
  toolbar.replaceChildren();
  for(const [label,icon,tool,visibility] of [
    ['New camera from view','new',null,false],
    ['Save from view','cameraSave','save_camera',false],
    ['Save from view with show/hide','cameraSaveVis','save_camera',true],
    ['Apply to view','cameraApply','apply_camera',false],
    ['Apply to view with show/hide','cameraApplyVis','apply_camera',true],
    ['Delete camera','delete','delete_camera',false],
  ]) {
    const action=button('', 'camera-action', ()=>{
      if(!tool)void openNewCameraFlow();
      else if(selected)void editSavedCamera(tool,tool==='delete_camera'?{name:selected.name}:{name:selected.name,withVisibility:visibility},scene);
    });
    action.innerHTML=cueMolIcon(icon);action.title=label;action.setAttribute('aria-label',label);
    action.disabled=scene===null || (tool!==null && !selected);
    toolbar.append(action);
  }
  if(state.editing && state.editing.scene===scene && cameras.some(camera=>camera.name===state.editing.name))return;
  state.editing=null;
  for(const row of body.querySelectorAll('[data-camera-name]')) row.setAttribute('aria-selected',String(row.dataset.cameraName===selectedCameraName));
  const fingerprint=JSON.stringify([scene,cameras]);
  if(state.fingerprint===fingerprint)return;
  const focused=body.contains(document.activeElement);
  state.fingerprint=fingerprint;body.replaceChildren();
  const clearDrop=()=>body.querySelectorAll('.camera-drop-before,.camera-drop-after').forEach(row=>row.classList.remove('camera-drop-before','camera-drop-after'));
  const drop=(event,target,after)=>{
    event.preventDefault();event.stopPropagation();clearDrop();
    const drag=state.drag;state.drag=null;
    if(!drag || drag.scene!==scene)return;
    const names=cameras.map(camera=>camera.name),from=names.indexOf(drag.name);
    if(from<0 || drag.name==='__current')return;
    const insertion=target===null ? names.length : names.indexOf(target)+Number(after);
    const to=Math.max(Number(names[0]==='__current'),insertion-Number(from<insertion));
    if(to!==from)void editSavedCamera('move_camera',{name:drag.name,to:to+1},scene);
  };
  for(const camera of cameras) {
    const row=document.createElement('div');
    row.className='camera-item';row.dataset.cameraName=camera.name;row.setAttribute('role','option');
    row.setAttribute('aria-selected',String(camera.name===selectedCameraName));
    row.draggable=camera.name!=='__current';
    if(camera.name==='__current')row.classList.add('camera-pinned');
    const icon=document.createElement('span');icon.className='camera-row-icon';icon.innerHTML=cueMolIcon('camera');
    const label=document.createElement('span');label.className='camera-row-name';label.textContent=camera.name;
    row.append(icon,label);
    for(const [enabled,title,iconName] of [[camera.visibilityCount>0,'Has saved show/hide state','view'],[Boolean(camera.source),camera.source,'link']]) {
      if(enabled){const badge=document.createElement('span');badge.className='camera-badge';badge.title=title;badge.innerHTML=cueMolIcon(iconName);row.append(badge);}
    }
    row.addEventListener('click',()=>selectSavedCamera(camera.name));
    row.addEventListener('dblclick',()=>void editSavedCamera('apply_camera',{name:camera.name,withVisibility:true},scene));
    row.addEventListener('contextmenu',event=>savedCameraContext(event,camera));
    row.addEventListener('dragstart',event=>{state.drag={scene,name:camera.name};event.dataTransfer.setData('application/x-cuemol-camera',camera.name);event.dataTransfer.effectAllowed='move';});
    row.addEventListener('dragover',event=>{if(!state.drag || state.drag.scene!==scene)return;event.preventDefault();event.stopPropagation();clearDrop();row.classList.add(event.clientY>row.getBoundingClientRect().top+row.clientHeight/2?'camera-drop-after':'camera-drop-before');});
    row.addEventListener('drop',event=>drop(event,camera.name,event.clientY>row.getBoundingClientRect().top+row.clientHeight/2));
    row.addEventListener('dragend',()=>{state.drag=null;clearDrop();});
    body.append(row);
  }
  if(!cameras.length){const empty=document.createElement('p');empty.textContent='(no cameras)';body.append(empty);}
  body.onkeydown=event=>{
    if(event.target instanceof HTMLInputElement)return;
    const selected=cameras.find(camera=>camera.name===selectedCameraName);
    const index=cameras.findIndex(camera=>camera.name===selectedCameraName);
    const next={ArrowDown:Math.min(index+1,cameras.length-1),ArrowUp:Math.max(index-1,0),Home:0,End:cameras.length-1}[event.key];
    if(next!==undefined && cameras[next]) {
      event.preventDefault();event.stopPropagation();selectSavedCamera(cameras[next].name);
      [...body.children].find(row=>row.dataset.cameraName===cameras[next].name)?.scrollIntoView({block:'nearest'});
    } else if(event.key==='F2' && selected) {
      event.preventDefault();event.stopPropagation();renameSavedCamera(selected);
    } else if(['Delete','Backspace'].includes(event.key) && selected) {
      event.preventDefault();event.stopPropagation();void editSavedCamera('delete_camera',{name:selected.name},scene);
    }
  };
  body.oncontextmenu=event=>savedCameraContext(event,null);
  body.ondragover=event=>{if(state.drag?.scene===scene)event.preventDefault();};
  body.ondrop=event=>drop(event,null,true);
  if(focused)body.focus();
}
