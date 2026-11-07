# User guide

[Back to README](../README.md) · [Development](development.md) · [Validation](validation.md)

Use this guide for viewing, scripting and rendering. For installation and the
first launch, start with the [README](../README.md#installation).

- [Molecular viewing](#molecular-viewing)
- [Commands, MCP and configuration](#commands-mcp-and-configuration)
- [AI Agent](#ai-agent)
- [Browser](#browser)
- [Compatibility boundaries](#compatibility-boundaries)

## Molecular viewing

The native and browser interfaces use Tritium's activity bar, Explorer,
scene tabs, right-hand Inspector, viewport tools, and bottom panels for console,
output, animation, sequence and trajectories. Both support light and dark themes.
When the central area is narrow, bottom-panel tabs use icons; hover to see their
names. If the icons still exceed the available width, scroll the tab strip
horizontally. The panel's collapse/expand button remains at the right edge, and
opening the panel restores its previous height.
The View menu provides parallel and cross stereo, eye swapping, and angular
separation. Each eye has its own projection, picking, labels and post-processing;
CPU image exports and offscreen depth captures use the same eye cameras.
Hardware quad-buffer stereo is unavailable on the wgpu frontends.
Center marks invert the displayed colors so they remain visible over molecular
geometry. Crosses, axes and labels retain their logical size on HiDPI displays.

Drag an Inspector numeric field horizontally to adjust it. Appearance controls
such as opacity, sphere radii and line widths update the viewport during the
drag and create one undo step when released. Escape cancels the drag and
restores the original value and default state. Hold Shift for finer adjustments
or Ctrl/Cmd for larger steps. Tauri also accepts dragging the field label.

Use **Help → About cuemol-rs** to inspect the version, source commit, build time,
compiler, target and the libraries used by the selected frontend. **Copy** puts
these rows on the clipboard for bug reports. Native builds include the OS
release; browser and Tauri builds also include the browser or WebView user agent.
Builds without Git metadata display `unavailable` for the commit. Reproducible
builds can set `SOURCE_DATE_EPOCH` to control the reported UTC build time.

### File destinations

The default destination for dropped object files is the current scene. Native
Options and browser Settings expose **Drag and drop opens into**; choosing
**New scene** creates one destination for each batch, while reusing an untouched
empty scene. Later files in that batch keep the first successful destination,
including when a later scene file opens another tab. Invalid files do not create
empty destination tabs or consume redo history.

Native Options also provides **File manager opens into** for startup file paths.
Tauri places both choices in its Files options tab. The portable configuration
keys are `files:drop-target` and `files:shell-target`, each accepting `active`
or `new`. File > Open and recent-file actions continue to use the active scene.
QSC and PyMOL scene files use the scene-opening rule independently of these
object-file preferences. Browser file access uses the selected `File` objects;
it does not require filesystem paths.

### Reader options

Native egui, the browser and Tauri show options before loading each supported
object file, including ordinary molecule opens, drops and file-manager inputs.
Tauri startup arguments use this same form and retain the shell-open destination
preference. Cancelling one file continues with the remaining startup paths;
cancelling all of them leaves the initial scene untouched.
The dialog provides reader settings, object/renderer names, renderer types or
presets, selection and view centering. Without compatible history, molecules default to Automatic
representations, which creates protein ribbons, nucleotide cartoons and
ball-and-stick for the remaining atoms. The renderer name is disabled in this
mode because each representation receives its own name. Object names remove the
last filename extension and gain a suffix when already used. **File > Open with
options…** remains an alias for the same flow. Scene, camera, profile and
trajectory files retain their dedicated loading flows.

Native and browser reader sections can be collapsed and reset; action buttons
remain outside the scrolling content. The dialogs accept optional AMBER
coordinates when opening a topology. An empty coordinate choice loads topology
only. NAMD coordinates require a PSF topology; MSMS faces/vertices require the
matching companion. The browser uses a second file chooser for companions.
Each batch waits for the current file's options. Accepting the first valid file
establishes the batch destination; cancelling a file leaves it unloaded and
creates no history. Switching scenes cancels the pending native/browser dialog
and its remaining batch.

Renderer choices are remembered separately for each object class. Molecular,
density-map, electrostatic-potential and surface choices do not overwrite one
another. File options and the New Renderer dialogs share this
history. A removed preset or unsupported stored type falls back to the default.
Confirming a file dialog remembers the offered choice; cancelling does not.
Undo preserves these preferences, and the next application/browser session
restores them. Renderer names follow type changes until edited by hand; clearing
the name restores automatic suggestions. Presets use short lowercase names, such
as `default1_1`, with collision checks across all object and group types.

Choose **New Renderer…** from an object, renderer or group context menu. The
browser also has a button below the scene tree, and Tauri has one in the Inspector. The form offers compatible types,
presets, a scene-wide unique name, a selection builder and view centering.
Opening it from a group or grouped renderer inherits that group and hides presets,
since presets create their own group. A preset disables the selection input;
its children retain the selections defined by the preset. The object-name field
does not rename an existing object. Errors retain the draft, Cancel leaves the
scene and preferences unchanged, and Create adds one undo entry. Reopening starts
from the object's current selection and enables recentering again. Stale targets
are rejected, and cancelled Tauri dialogs discard late renderer-option replies.

Map forms offer Auto, keep the current view, or fit the map. Explicit centering
also works for electrostatic potential maps; Auto keeps their current view.
The policy affects the newly added renderer and leaves existing renderer centers
unchanged.

Linked and embedded native QSC saves retain loaded AMBER coordinates; linked
sources preserve the companion path and compression setting. Browser scene
exports embed the loaded coordinates or surface so they remain portable.

CCP4/MRC dialogs display the probed grid size and estimated quantized storage at
the selected stride. They suggest a stride for large maps and offer Auto,
Crystallographic and Cryo-EM map kinds before renderer creation. Map-kind choices
survive QSC save/reload. Map centering defaults to Auto: EM maps fit the whole
volume, while crystallographic maps keep the current view and place the display
region at its center. Explicit keep-view and fit-map choices override that
default. Fitting uses the loaded map bounds and viewport aspect ratio.
These controls also apply to browser imports. Map inspection reads only the
CCP4/MRC header; uncompressed MTZ inspection reads the prefix and header tail.
Compressed MTZ inspection decodes the stream twice while retaining only metadata.
It still reads the compressed input file. CIF inspection scans through long
preambles without retaining the decoded body. The parser worker is cancelled
when a request is superseded and releases its memory after idle retirement.

### Saved cameras

The View activity contains the Camera pane. Select a row, then use the down-arrow
buttons to save the current view or the up-arrow buttons to apply it. Filled
arrows include object/renderer show/hide flags. A plain save replaces any stored
flags with an empty set, while a plain apply leaves the scene's visibility alone.
Double-click applies the view and its saved flags. View-only navigation preserves
undo/redo; visibility changes are ordinary scene edits.

Use New to capture a named view, F2 to rename, and Delete to remove the selected
camera. Creating a name that already exists overwrites that camera while keeping
its file link and display position. Up/Down and Home/End select rows; drag rows to
reorder them. The scene-opening camera, `__current`, remains at the top. Its row
can still be selected, edited or deleted. Camera properties and show/hide editing
are available from the context menu, together with Cut, Copy and Paste.

The Camera file submenu loads, reloads and saves `.cam` documents. Native reload
reads the linked file; browser reload uses the File already opened in that tab.
Browser Save and Save as both create downloads and do not overwrite local files.
To read changed file contents in a browser, open the file again. Camera names,
links, order and show/hide flags survive scene saves and ordinary camera edits.

### View transforms

Below Cameras, the View pane provides rotation, translation, zoom, slab depth,
camera distance, projection and center marks. Native and browser windows share
these controls; Tauri provides the same interactions. Drag a field horizontally
for live updates. Shift slows dragging by a factor of ten and uses finer steps;
Ctrl/Cmd snaps to ten-unit steps. Rotation inputs apply relative angles and
return to zero when the interaction ends.

Translation drags and arrow-key steps pan along the current viewing axes. The
fields display absolute world coordinates, and typing a coordinate sets that
axis directly. Enter moves from TraX to TraY to TraZ for consecutive input;
Escape discards an unfinished text edit. These view changes preserve scene
undo/redo. Changing camera distance leaves the slab unchanged. Editing the
slab itself clamps it to twice the current distance and at most 10,000 Å,
matching the reference.
The separator above View resizes the Camera pane, and View scrolls when its
controls exceed the available height.

### Morph frame editing

Tools → Mol morphing animation opens the frame editor in each frontend. Choose a
molecule, then press Convert to MorphMol. Opening the editor does not change the
scene. Conversion retains the object's identity and renderers, and stores its
current coordinates as the protected `(this)` frame.

Add PDB file accepts multiple `.pdb` or `.ent` files, including gzip-compressed
files. Add mol selects an ordinary molecule from the active scene; existing
MorphMol objects are excluded. New frames follow the selected row. Without a
selected row, they append to the list. Delete removes an ordinary frame and
clears the row selection. The column divider resizes the Name column.

Each successful edit has its own Undo step. Close retains those edits. A failed
file stops the remaining batch and keeps previously added frames. Browser parsing
runs in the file worker; closing or replacing the editor discards pending results.
Use a Mol morphing element in the Animation panel to play the frames.

### Animation references

The native, Tauri and browser windows show animation elements as strips in the
bottom Animation tab. Drag a strip to move it, or drag an edge to resize it.
The ruler and current-time field scrub the playhead. These gestures preview
locally and commit once on release; Escape cancels them. Fit and the zoom
buttons change the time scale. Select a channel or strip to open its Inspector,
which includes common and type-specific properties, target choices and a
Generic tab. Add inserts after the selection; Delete retains the next available
selection, and the arrow buttons reorder the selected element.

Adding an element creates `__current` from the live view only when that saved
camera is absent. An empty start-camera choice adopts it; an explicit choice is
preserved. Undoing the added element leaves this initialization in place. Moving
the view afterwards does not change the saved animation start. Tauri keeps live
view synchronization separate from camera-file/export persistence.

Start and duration fields preview the resolved timeline while dragging. Numeric
fields in the Generic tab also preview their values. Release records one undo
step; Escape, focus loss or changing the target restores the original value.
Spin axes support X/Y/Z presets and Cartesian components, and renderer slide
directions support cardinal presets. Structured controls retain their type's
ranges; generic real-property values are not restricted to morph fractions.

Reset buttons follow each property's declared default. Reset all restores only
modified properties that have a default and retains names, timing and targets.
An explicit value equal to the default remains an explicit setting until reset;
this distinction survives undo, preview cancellation and scene reload. Generic
fields use their declared types and show resolved absolute times as read-only.

`set_anim_prop` accepts boolean aliases such as `on`, `yes`, `1` and their
opposites, without case sensitivity. Numeric text accepts decimal, hexadecimal,
octal and binary notation. The generic command first interprets numeric and
lowercase boolean text as typed values; incompatible property types fail.
For spin axes, use `"0 1 0"` with the structured command and `"(0,1,0)"` with
`set_anim_generic_prop`. Axes shorter than 0.0001 retain the previous direction.
Renderer target lists normalize spaces around comma-separated names and `/`.

Animation elements can start relative to another element's end. Renaming the
reference updates its dependent elements, and deleting it keeps their current
absolute times. Changing a reference preserves the span unless it must move
forward to the new reference's end. The property dialog uses the same rules as
`set_anim_prop` and `set_anim_generic_prop`; use `set_anim_time` to edit both time
bounds together.

A scene with missing or cyclic references still loads. The Animation panel and
`list_anims` report the unresolved elements so their references can be repaired.
Playback and seeking resume once the chain resolves. A rejected edit preserves
current playback and undo history; an accepted element edit stops playback and
restores the underlying scene before applying the change.

### Representations and rendering

Open files on the command line, through the file dialog, or by drag and drop.
Browser file parsing runs in a worker so navigation and edits remain available.
Use **Cancel loading** or Escape to stop the current input and discard queued
files. **Get PDB** uses the same queue and also cancels its network request.
In the native and browser dialogs, enter a PDB ID, an EMDB ID such as
`EMD-11638`, or keywords. Keyword results can be filtered by experimental method,
sorted by relevance, resolution or release date, and expanded with **More**.
Selecting a result keeps the search text and list available. Entry metadata
determines which coordinate and map choices are shown; entries without the legacy
PDB format use mmCIF. Associated EMDB maps use the archive's recommended contour
level when available. A metadata-service error still allows a direct download,
while a confirmed missing entry disables it. Recent IDs and keyword queries
share the history menu. Closing the dialog discards late metadata results.
The shared console also accepts `fetch_emdb EMD-1010` or a PDB identifier with
an associated EM map. It uses the archive's recommended contour when available;
an explicit `level` overrides it. `mapCenterPolicy` controls whether loading or
creating a map renderer centers the view or centers the new contour on the view.
A failed load keeps existing edits and redo history. Changing the active scene
while an input is queued, downloading or being parsed prevents its late result
from entering the new scene. Loading waits for an active image render, and
cancelling queued inputs leaves that render running and cancellable.
The initial representation uses a protein ribbon, the nucleic-acid (`nucl`) renderer, and
ball-and-stick geometry for other atoms. The renderer controls remain available
for adding or changing representations, selections, coloring, and materials.
New molecules use CueMol3's Okabe-Ito palette in molecule load order. Helices and
sheets share the molecule's base color, nucleic acids shift its hue, and coils
use a lighter, less saturated color. The default ribbon and nucleotide geometry
also follows CueMol3. A global user style or scene style can change the palette
with `<palette id="default_paint" value="tolbright"/>`. Other bundled themes are
`okabeito`, `tolvibrant`, `tolmuted`, `tollight`, `brewerset2`, `brewerdark2` and
`tableau10`. The `classic` extension keeps the earlier secondary-structure colors.
Palette changes affect subsequently loaded molecules. Used colors are stored in
the scene, so a later global edit does not recolor saved molecules. Renderer
`$molcol` colors inherit the object's coloring scheme, including hue, saturation,
brightness and alpha modifiers.
The browser accepts Unicode text and IME composition in its text fields.
Its UI includes a Japanese fallback font; Console input and object names use
the same text-input path.
Saved and direct molecular surfaces use their renderer's coloring scheme,
including paint rules and imported atom colors.
Spline selections retain the whole chain's interpolation coefficients, so a
selected pair of residues follows the same curve as the complete backbone.
Native and browser viewports share GTAO, edge-aware AO filtering, FXAA, SMAA,
and 2–32 temporal samples. Spatial AA works independently of AO. Camera,
geometry, or setting changes reset temporal accumulation; unchanged frames reuse
the completed image. Optional half-resolution AO refines at full resolution when
the view stops changing. Scene properties include the SMAA edge threshold, and
these settings survive QSC saves. Camera PNG exports apply full-resolution AO
and supersampling while preserving transparent backgrounds.
The Rendering menu offers built-in GI and hatched PNG output. Animation
rendering also defaults to the bundled Rust renderer. Maps, molecular surfaces,
saved cameras, measurements, labels, and animation
share the same application state and command API.
Atom-based measurements retain their object and atom references across scene
saves, so their geometry and displayed values follow coordinate and transform
changes, including measurements between different molecules.
Atom labels support per-renderer and per-atom formats, horizontal and vertical
alignment, pixel offsets and optional display above molecular geometry.
The shared GPU path clips label pixels against scene depth in native, browser
and Tauri viewports. Label text and settings survive embedded scene saves.
Native labels use installed font families, with bundled proportional and
monospaced fallbacks. Browser labels use CSS family lists through OffscreenCanvas.
Font styles and weights accept either letter case. Glyph shape and hinting can
vary with the fonts installed on each platform.

### Mouse and keyboard

| Input | Action |
| --- | --- |
| Left drag | Rotate with the reference virtual trackball |
| Right drag | Pan |
| Wheel | Zoom without changing camera distance |
| Shift + left drag, horizontal | Adjust slab thickness |
| Shift + left drag, vertical | Zoom |
| Left + right drag, horizontal | Move the view depth |
| Left + right drag, vertical | Roll the view |
| Click an atom | Toggle its name label |
| Double-click an atom | Toggle its residue selection |
| Shift + double-click | Extend selection from the previous residue in the same chain |
| Atom context menu | Center, select, or expand the selection |
| `C` | Center the selection |
| `F` | Fit the selection |
| `Home` | Fit the scene |

Use Ctrl on Windows/Linux or Command on macOS for application shortcuts:
`O` opens files, `Shift+O` opens a scene, `S` saves the scene, `Shift+S` saves it
under another name, `R` reloads, `T` creates a tab and `W` closes it. Undo uses
`Z`; Redo accepts `Y` or `Shift+Z`. The toolbar arrows list available steps.
`Shift+P` folds the viewport palette. In a browser, use `Alt+T` or `Alt+W` with
the same primary modifier when the browser reserves its own tab shortcuts.
Browser scene saves create downloads, and closing a scene with content or
settings offers save, discard and cancel choices.

The table describes the Default mouse preset. Settings also provide Coot, PyMOL,
iPhone and Trackpad presets, automatic device detection, trackball sensitivity
and optional momentum. Trackpad mode uses two-finger scrolling to pan and pinching
to zoom. Preset bindings and custom `UserViewConf` styles use the same resolver as
scripted `ViewInputConfig` changes. Dragging starts at two logical pixels; mixed
operations lock to the dominant axis until release. The native and browser
viewports share this input calculation.

To select the PyMOL preset before launching the application, run:

```sh
cuemol config set cuemol2.ui.viewinconf PymolViewInConf
```

The native egui and browser palettes and `N`, `B`, `L`, `D`, `A`, `T`, `E` keys select navigation,
rectangle selection, lasso selection, distance, angle, torsion, and bond editing.
Rectangle and lasso drags replace the selection; hold Shift to add atoms. A click
without a drag retains the atom-label behavior. Measurement tools collect two,
three, or four atoms, and Escape cancels a pending set. Bond editing requires two
different atoms in one molecule. The options buttons choose a measurement label
set or remove non-standard bonds. Each completed edit has one undo entry.
The palette cap and **View > Tool palette** share a persisted collapse setting.
Hover information and highlighting are configurable in Settings.

### Molecule editing

The **Edit** menu provides dedicated Merge molecule, Delete mol atoms,
Change chain ID and Change residue number dialogs in each frontend. Their
selection builders edit the draft without replacing the molecule's current
selection. Reopening clears the selection and restores the options while
retaining valid molecule choices.

- **Merge molecule** copies selected atoms into a different molecule. Copy is
  initially on; turn it off to move the atoms. An empty selection includes all
  source atoms. Copy and source deletion share one undo entry.
- **Delete mol atoms** requires an explicit, nonempty selection. Use `*` to delete
  every atom. Invalid expressions or selections with no atoms stay in the form.
- **Change chain ID** changes all selected residues; an empty selection includes
  the entire molecule. The name is trimmed. A whitespace-only name becomes `_`
  after confirmation. Names longer than one UTF-16 code unit are accepted after
  a PDB-format warning. An existing destination residue rejects the edit.
- **Change residue number** starts in Shift by mode with value `1` and Renumber
  off. Start from sets the first selected residue's number. Renumber assigns
  consecutive numbers and removes insertion codes; otherwise gaps and insertion
  codes are retained. Empty selection includes all residues. Numeric text follows
  the reference decimal-prefix rule (`1.5` and `1e2` both mean `1`). A zero shift
  is rejected. Start numbers below `-999` or above `9999` require confirmation.
  Values and final residue numbers must fit a signed 32-bit integer.

Each successful change has one undo entry. Errors and rejected residue edits
retain the draft and preserve the molecule. Changing a chain with no matching
residues closes the form without adding history. Cancel discards the draft;
switching scenes cancels or rejects it. Successful selection expressions are
available in selection history.

### Molecular superposition

**Tools > Molecular superposition** opens the dedicated form in every frontend.
Choose reference and moving molecules, their selections, and US-align, SSM or
LSQ. Both roles may use the same molecule, for example to align different
chains. Empty selections include all atoms. Choose opens the selection builder.
US-align also accepts Auto (CA / C3'), CA, C3', C4' or P as the pivot atom.
The pivot control is inactive for SSM and LSQ.

Auto recenter frames the moving selection after alignment. Use xformMat property
stores the transformation on the object; otherwise its atomic coordinates are
transformed. Native egui and Tauri retain the optional LSQ RMSD-report export.
A failed operation keeps the form and leaves the scene/history unchanged.
Successful changes create one undo entry. Reopening clears both selections and
restores only the last successful molecule, method, pivot and switch choices.
Cancel does not save a draft, and a changed scene invalidates it.

### Molecular surfaces

**Tools → Mol surface generation** creates a solvent-excluded surface. Reopening
keeps the last valid molecule and algorithm, but resets the selection gate and
expression, point density to 2 and probe radius to 1.4 Å. Density uses integers
from 1 to 10; probe radius ranges from 0.1 to 10 Å. Changing the molecule proposes
a fresh unique `sf_<molecule>` name. A blank name accepts that suggestion.
The selection builder is available when **Use selection** is enabled.
MeshMS, BALL and Auto follow the reference choices; Rust retains the existing
grid-based backend. New surfaces use CPK element coloring and a unique `molsurf`
renderer name.

Use the surface object's **Regenerate surface** context menu to rebuild it.
The original molecule, selection and probe radius remain fixed; density and
algorithm are editable. Auto keeps the surface's stored backend. Regeneration
remembers its algorithm separately from generation, and requires the original
molecule to remain in the scene.

**Tools → Mol surface cutter** uses the current front clipping plane. Each open
starts with Complete mode and section density 5; the allowed density range is
0.1–50. Separately creates a distinct `sect_<surface>` object, while Section only
and No section retain just the corresponding geometry. Successful operations
close the dialog and use one undo step. Errors keep the draft available for
correction, and Cancel leaves the scene unchanged. Changing scenes invalidates
a draft. Native, browser and Tauri expose these controls.

### Secondary structure

**Tools > Reassign secondary str** opens a shared form in native egui and the
browser, with matching controls in Tauri. Recalculate uses molecular geometry,
with optional Ignore beta bulge and a helix gap-fill angle from 0 to 180 degrees.
Assign uses an atom selection and one of Coil, beta strand, alpha helix,
3-10 helix or pi helix. An empty selection means the entire molecule; use
Choose to build or reuse a selection. Options from the inactive mode are ignored.

Reopening resets the mode, selection and parameters while retaining a valid
molecule choice. Errors stay in the form for correction. Successful changes
close the form and create one undo entry; a successful operation with no changes
closes without adding history. Cancel discards the draft, and a changed active
scene invalidates its target.

### Interaction analysis

**Tools > Interaction** opens the interaction form in each frontend. Choose a
molecule and selection; Choose opens the selection builder. Defaults are
2.5–3.5 Å, 30 labels, all atom types and the `measure` label set. Hydrogen-bond
filtering restricts candidates to N and O. A distinct second molecule uses
Selection 2 automatically. Without it, enable Second selection to restrict both
ends within the first molecule. Choosing the first molecule in both pickers
has the same behavior as using only the first picker.

OK adds the found pairs to the named interaction renderer under one undo step.
An empty result or invalid expression stays in the form without changing the
scene. Cancel discards the draft. Reopening restores the defaults and keeps
valid molecule choices. Switching scenes cancels or rejects the old draft.

### Picking and hover feedback

Native, WebGL, WebGPU and Tauri views share geometry-based GPU picking and a
translucent hover fill with a two-tone outline. Picking follows visible sphere,
bond and mesh fragments, including ribbon and nucleic-base atom ownership.
**GPU picking** in native/Tauri Options and browser Settings switches between
rendered-geometry picking and CPU atom-center picking. **Hover info** disables
pointer-movement hit tests while retaining clicks; **Hover highlight** controls
the overlay and requires both other switches. Changes apply immediately after
accepting native/Tauri Options, or when toggled in the browser. They persist
across restarts and scene undo/redo. The corresponding config keys are
`view_input:gpu-pick`, `view_input:hover-info` and `view_input:hover-highlight`.
For scripts, `ViewInputConfig.gpu_pick` controls the current runtime and
`hover_hl_color` sets the fill color. `View.setHoverHit` and `clearHoverHit` update
transient highlighting without adding scene geometry or undo entries. Locked
objects/renderers and GPU sections with opacity at or below 0.6 do not
participate. Symmetry and other renderers without GPU hit names retain their
CPU picking fallback. The GPU ID buffer uses half the viewport resolution;
cached geometry and asynchronous readback avoid rebuilding it on pointer motion.

### Crystal symmetry

The Crystal activity's Symmetry pane selects a molecule or density map and
shows its crystal system, space group and cell parameters. Change opens the
molecule's crystal editor; a molecule without crystal data first asks to create
it. New cells start at 1 Å with right angles. Restrict by symmetry derives the
dependent cell parameters from the selected crystal system and space group.
Turning the restriction off restores the unconstrained input values.
Cancel leaves the molecule unchanged, and accepting an unchanged edit keeps
redo history. Switching scenes or removing the target closes the shared editor.

Symm mol displays neighboring molecules within 20, 50, 100 or 200 Å of the view
center, or packs the home unit cell. Finite extents follow later view-center
changes. Unit cell adds the cell box for either a molecule or map and reuses an
existing cell renderer. These actions preserve the camera. Maps expose the
readout and Unit cell action; molecular symmetry edits remain disabled for maps.
The native and browser frontends share the pane and editor.

### Density maps and volumes

CCP4/MRC density maps support header inspection, strict grid subsampling and
voxel limits through `CCP4MapReader` and all three frontend file dialogs. Native reads
use a row buffer and quantized voxel storage; compressed files spool to a
temporary seekable file. MRC origins and crystallographic/EM map-kind evidence
are preserved. QDF/QSC saves retain the reader's quantization range.
OpenDX and QDF electrostatic potentials retain shared 32-bit floating-point
samples and trilinear sampling; embedded scenes use `qdfpot` for these objects.
Scripts expose the quantization step, map origin, top-fraction density levels,
and an editable `map_type` with its resolved crystallographic or EM kind.
EM contours use the occupied top fraction, while crystallographic contours use
standard deviations. Default EM levels are capped by occupied voxels; explicitly
setting the same numeric level overrides that cap. Reset and embedded scene
storage preserve this distinction, and both desktop panels expose the matching
absolute or native level range.
The Crystal activity's Density map pane selects any contour, isosurface, map
mesh or volume renderer. Its Options menu switches between native and absolute
levels; the hint below Level shows the other unit. Extent is disabled for a
full-map region. Transp edits opacity (0 is transparent and 1 is opaque), with
live dragging, Escape cancellation and one undo step on release. Level and
Extent commit after dragging or typing. The color field accepts named colors
and is enabled in Solid color mode; other coloring modes remain in Coloring.
Redraw moves the map's extraction center to the current view. Cell adds a unit
cell without moving that view.

`gpu_mapvol` uses interpolated 3D density textures and the reference rainbow
opacity table. Native, WebGL and WebGPU views share the same volume pipeline;
CPU rendering also composites volume samples with molecular geometry. Volume
opacity, transforms and scene storage remain editable through the shared API.
Map regions default to the whole block for EM and a centered box for other
scalar maps. `region_mode`, `lod`, `lod_budget` and `zoom_refine` control the
shared extraction. Automatic full-region sampling uses a cell budget, and view
refinement retains existing geometry while it covers the visible area. Native,
Tauri and browser navigation debounce region updates by 150 ms.
Isosurface `cap_mode` controls boundary faces independently of that region:
`auto` caps full regions and generated surface objects, `on` always caps, and
`off` leaves them open. Molecular coloring uses the nearest selected atom in
`target`, with the renderer's coloring scheme and the molecule's inherited
colors. Both selection and coloring survive embedded scene storage.

### Molecular dynamics

Load an MD topology and trajectory through the MD dialog or `load_trajectory`.
GRO, PDB, mmCIF, PSF, AMBER topology, SDF, and MOL2 topologies are accepted.
DCD, XTC, TRR, AMBER NetCDF and ASCII MDCRD trajectories retain a frame index and decode the
requested frame. Coordinates and periodic cells update together. Undo history
shares trajectory sources and map voxels, and frame playback does not add an
undo snapshot per frame. Compressed trajectories spool to a temporary seekable
file; this uses disk space proportional to the decompressed input. MDCRD reads
standard coordinate rows and detects a three-length periodic box row. For
ambiguous one-atom files, the diagnostic command accepts `--box` or `--no-box`.

```sh
cuemol trajectory-info trajectory.xtc
cuemol trajectory-info trajectory.dcd 10
cuemol trajectory-info trajectory.mdcrd 10 --atoms 20000 --box
```

The optional second argument is the frame stride. Legacy `dcd-info`, `xtc-info`,
`trr-info`, and `ambernetcdf-info` commands retain their diagnostic reader behavior.

## Commands, MCP and configuration

### Console and reusable actions

The Console offers CueMol, PyMOL, ChimeraX and JavaScript dialects. Enter runs a command,
Shift-Enter adds a line, Tab completes a command or argument, and Up/Down recall
previous submissions at the ends of the input. A multiline submission creates
one undo step. Stop keeps completed edits and prevents pending results from
changing the scene. The Help button lists the selected dialect's commands.

The Actions sidebar shows undo steps and their operations. Select several steps
with Shift or Ctrl/Cmd, then copy their commands, export QAX or save a reusable
macro. Include renderer creation when the macro edits that renderer. Select a
target molecule before running a macro from the library. The library includes
the reference presets and supports importing, exporting, renaming and deleting
user files. Native files are stored in the configuration directory's `actions`
subdirectory; browser files remain in local storage until exported or deleted.

In the native CueMol dialect, `open_log actions.qax` records subsequent edits,
`close_log` closes the log, and `run actions.qax` replays it. A QAX file can
contain separate undo steps. Commands that open or save a scene must run alone.

### ChimeraX commands

Select **ChimeraX** in the Console, or run the same dialect from the CLI:

```sh
cuemol run commands.cxc
cuemol console --dialect chimerax -c 'open structure.pdb; color bychain; view'
cuemol console --dialect chimerax
cuemol run commands.pml
```

The last console command reads one submission per line from standard input and
keeps the scene, aliases and model numbers until the process exits. `run` detects
`.cxc` and `.pml`; `--dialect cuemol|pymol|chimerax|javascript` explicitly selects
the language. JavaScript retains its existing positional arguments.

The dialect implements the current reference console's 59 registered commands.
Use `help` or `usage surface` to inspect them. Model numbers such as `#1` are
assigned in scene order on first use and remain stable after closing or undoing
objects. Atom specifications include chains, insertion codes, names, attributes,
boolean operators, zones and scene selections. `sel` reads the current selection.
Aliases can contain several commands; `.cxc` files use newlines or semicolons,
and `#` starts a comment only where a command name would appear.

`show`, `hide`, `style`, `cartoon`, `color`, `rainbow`, `surface`, `label` and
`transparency` edit the live `chx:` renderers. Manual renderer edits are visible
to subsequent console commands. Surfaces retain their calculation selection
while patch visibility changes. Residue labels attach to CA, then C4', then the
first atom. Transparency applies to whole representations, so partial-model
transparency is rejected. Opening a molecule applies size-dependent automatic
styling. `distance` measures two atoms, `align` fits ordered atom pairs, and
`matchmaker` uses CueMol's US-align structural fit with the reference warning.

`save` supports PNG, QSC and one PDB model; `format pdb` also works with a
filename whose extension differs. QSC open/save and undo/redo require a standalone
submission. Native `.cxc` scripts read local files; browser file access uses the
existing picker and download paths. Unsupported ChimeraX options retain the
reference's explicit errors or warnings. This is the CueMol console dialect,
whose supported command set is smaller than the ChimeraX application.

### Agent skill

`cuemol skill` prints the bundled [agent skill](../skills/cuemol/SKILL.md) as
Markdown without network access or startup-script execution. Use
`cuemol skill > SKILL.md` to save it for an agent. The same document covers
native GUI sessions, persistent MCP scenes and headless notebook commands.
It directs agents to `cuemol mcp tools` for the installed command schemas.
The [Colab notebook](https://colab.research.google.com/github/mogura-rs/cuemol-rs/blob/main/notebooks/cuemol.ipynb)
provides the same skill alongside release installation and headless examples.

### Connect to the application with MCP

All installed entry points use `cuemol`; no separate JavaScript MCP launcher is
needed. Enable **MCP Server** and **MCP: Accept connections** under
**Options > Plugins > Installed** in either native frontend. The GUI then exposes an
authenticated loopback endpoint; disabling either switch stops it. The default
port is 27182; choose 0 to allocate an available port automatically. Before
launching the GUI, the same plugin preference can be set with
`cuemol config set plugins:mcp:enabled true`. The browser does not host an MCP
listener.

Property commands edit molecular, map, surface and lightweight objects through
shared state. They preserve renderer identities even when names are empty or
duplicated, and resets restore the effective values used for rendering. View
properties update the active view and camera; unknown view properties fail.
Browser file loading accepts OBJ/PLY surfaces, QDF molecules/maps/potentials/
surfaces/lightweight objects, and PyMOL sessions in addition to the molecular and
map formats. QDF dispatch uses its content rather than assuming a molecule.

```sh
cuemol mcp config
cuemol mcp status
cuemol mcp connect
cuemol mcp serve
cuemol mcp serve --http 127.0.0.1:8081
cuemol mcp tools
cuemol mcp call get_scene_state '{}'
cuemol mcp connect --upstream
```

`connect` bridges stdio to the running GUI. `serve` owns an independent headless
scene, and `config --headless` prints the corresponding client configuration.
`connect --upstream` bridges to CueMol2's existing local API. The native catalog
covers the current reference's 129 registrations, including its 127 MCP
operations, plus convenience operations. This is command coverage, not a claim that every
parameter combination or rendering algorithm is identical; see the boundaries
below. `mcp tools` is the authoritative schema for the installed build.

### Run scripts and export files

```sh
cuemol exec get_scene_state '{}'
cuemol run commands.js first-argument second-argument
cuemol -c 'print(21 * 2);'
cuemol render example.pdb image.png --width 1200 --height 900
cuemol render example.qsc illustration.png --style npr
cuemol export-scene example.pdb model.obj
cuemol render-qsc example.qsc scene.pov
cuemol tools list
cuemol tools run pdb2pqr --help
cuemol tools run ffmpeg -version
```

`exec` starts a new headless scene; use a persistent MCP connection for a series
of edits. Relative output filenames are resolved from the process's working
directory. JavaScript runs in the embedded Boa engine and CueMol bindings, with
no Node.js runtime requirement. `getCmdArgs(-1)` returns the number of script
arguments; `getCmdArgs(0)` returns the first argument. Both `run` and `-c` preserve
quoted spaces and empty arguments. Read-only scripts leave Undo and Redo intact.

Native `PythonBridge` uses a persistent `uv`-managed interpreter per application.
Install `uv` to use it. Python variables survive `runString` and `runFile` calls;
`import cuemol` provides live scene objects, services, properties and callbacks.
For example, the JavaScript console accepts:

```javascript
var py = getService('PythonBridge');
py.runString('import cuemol\nscene = cuemol.scene()\nprint(scene.name)');
py.runString('print(scene.uid)');
```

Python execution requires a native process and is unavailable in the browser.
The embedded helpers include `fileio`, `mol_util`, `renderer`, `scene_util`,
`cmd`, `net_fetch`, `logging`, `UndoTxn`, `povrender`, `umbreon_render` and the wrapper compatibility modules.
They operate on the live scene, including object copies, selections, transforms,
compressed input and scene exporters such as Umbreon. Third-party scientific
Python libraries are not bundled. Native `povrender.PovRender` supports still
images, animation frames, transparent-layer blending and the reference render
options. The legacy `povrender.render(scene, output, ...)` function is also
available. Both use `cuemol tools render-pov` and the packaged POV-Ray executable;
no Python image library is required. Pixel-label export uses the GUI-enabled
native build shipped in releases. A failed render leaves an existing output
file intact. For example, run the following through `PythonBridge`:

```python
from cuemol import povrender

povrender.PovRender(1200, 900).render_simple(scene, "molecule")
povrender.PovRender(1200, 900, fps=25).render_anim(scene, "frame", 0, -1)
```

`umbreon_render.render` uses the scene's saved render settings, including the
Umbreon/NPR backend, projection and physical image size. Explicit width and
height override the output size without changing the scene settings.
`render_file` loads a QSC and returns its scene after rendering. Both require
the named saved camera, defaulting to `__current`. Without stored render settings,
the helper uses RenderSettings defaults and the saved camera's projection;
it does not create scene app data. Run these helpers through the same native
PythonBridge:

```python
from cuemol import umbreon_render

umbreon_render.render(scene, "molecule.png", width=1200, height=900)
loaded_scene = umbreon_render.render_file("molecule.qsc", "saved-settings.png")
```

`cuemol tools render-pov request.json` accepts `input_path`, `output_path`,
optional `executable` and `include_dir`, and an `options` object. Pass `-` to read
the JSON request from standard input. Relative paths use the current directory.
The requested PNG replaces its destination only after complete PNG decoding and
dimension checks succeed.

`export-scene` and `render-qsc` use the same readers, default representations and
scene geometry as the GUI. They support STL, OBJ, POV-Ray, PNG, MQO, LuxRender,
LuxCore and Warabi output. PNG output uses the shared render settings, including
saved dimensions. Triangle formats export geometry; volume data remains available
to image renderers rather than being converted into an unrelated isosurface.
Scene Undo, Redo and Rollback update script queries immediately. Transactions
preserve edits made before they start, changes in other scenes and later changes
to unrelated properties. An uncaught script error discards that evaluation's
scene edits and history. Atom IDs belong to their molecule and remain stable
through merge, coordinate edits and deletion history. New copied or appended
atoms receive unused IDs during that molecule's session, including after Undo.
Deleting an atom without an alternate-conformation identifier also removes its
alternate conformations; Undo restores the complete set and its original order.

### Preferences and startup scripts

Preferences are stored in `$XDG_CONFIG_HOME/cuemol`, falling back to
`~/.config/cuemol`. `CUEMOL_CONFIG_DIR` overrides the directory, and
`CUEMOL_CONFIG_FILE` overrides the TSV preference file. GUI preferences,
`config.tsv`, `startup.js`, and the local API descriptor use this directory.
Put startup JavaScript in `startup.js`; it runs before command-line files open.
`cuemol config path`, `get KEY`, and `set KEY VALUE` inspect or edit preferences.
Configuration inspection does not execute startup scripts. Download caches use
`$XDG_CACHE_HOME/cuemol` or `~/.cache/cuemol`, with `CUEMOL_CACHE_DIR` as an override.

Saved TSV files begin with `# cuemol-config-v2`. Keys and values preserve empty
values and whitespace; backslashes, tabs, line endings and hash marks use `\\`,
`\t`, `\n`, `\r` and `\#` escapes. Keys beginning with `#` remain readable.
Older files without the header remain readable with their original
`\n` escape convention. Invalid files leave existing preferences unchanged.

For example, save the following as `startup.js` in the directory printed by
`cuemol config path`:

```javascript
scene.setName('My session');
print('Startup commands loaded');
```

### Settings categories

Options in the native frontends and Settings in the browser use the same
Display, Input, General, Tools and Plugins groups. Search shows matching
categories across groups. Display includes the theme and default atom-label
font, size, color, bold and italic values. Input retains the mouse presets,
pointing-device override, picking controls and keyboard bindings. The native
Options dialogs apply with OK and discard drafts with Cancel; browser controls
save immediately. Scene Undo/Redo leaves host preferences unchanged.

Native Rendering and APBS/PDB2PQR settings select external executables, the
POV-Ray include directory and the pdb2pqr force field. Empty executable paths use
the bundled tool or PATH. Layer composition is built in, so it does not require
a blendpng executable. Browser rendering uses its own Render panel; native
executables cannot run in a browser.

General > Updates controls the delayed startup check in native applications.
It is enabled by default and only announces a newer release. Check for Updates
runs a manual check; installing a release requires the explicit Install update
action. The browser offers Reload viewer. Native and browser configuration and
startup script editors remain under Configuration & Startup; Tauri retains its
configuration command interfaces.

The enabled Console plugin contributes **Plugins > Console > Command line
access** in native applications. It is off by default. Enable it to send a
submission to the running window:

```sh
cuemol console --connect -c 'pwd'
cuemol console --connect --dialect chimerax -c 'color bychain; view'
```

The authenticated loopback server exposes Console and MCP independently. A
Console-only connection cannot call MCP. The client sets its working directory
for relative paths. Submissions use the GUI console and its history; one
submission creates one scene-undo entry. The native editor keeps its unfinished
input. Disabling access stops pending commands while retaining completed edits.
The command also accepts a script path or lines from standard input.

### Built-in plugins

Open **Options > Plugins > Installed** in a native frontend or **Settings > Plugins > Installed** in the
browser. Native dialogs apply the complete draft with OK and discard it with
Cancel. Browser checkboxes save immediately in that browser. These switches
control the plugins compiled into the application; they do not install packages.

| Plugin | Initial state |
| --- | --- |
| Get PDB and Sequence | Always enabled |
| Console, PyMOL console, ChimeraX console | Enabled |
| MD Tools, AI Agent, Component Catalog | Disabled |
| MCP Server | Disabled; native application only |

Disabling Console hides its tab and stops its pending commands. The individual
PyMOL and ChimeraX choices remain stored while Console is disabled. Action
Library remains available. Disabling MD Tools stops playback and hides its tab
without deleting loaded trajectories. A hidden active tab returns to Output,
and a hidden sidebar activity returns to Explorer. Scene Undo/Redo retains these
preferences. The Component Catalog contains three independently scrolling, collapsible panes.
Its form, list and property examples edit local sample values. Numeric drags,
time/vector fields, selection builders, disabled controls and property reset
flags can be inspected without changing a scene. The isolated property rows and
the mock Inspector retain separate values; only Reset clears a modified flag.

### Image rendering

In the native egui, browser and Tauri frontends, open Render to preview an image
before saving it. Tauri also exposes Rendering > Image Rendering. The Image tab sets size, physical units, DPI and transparency. The
Render tab contains independent supersampling, lighting, AO quality, GI lighting
and shadow choices, with grouped advanced controls. NPR offers raytracing or AO;
its Detail tab edits hatch layers and tone specifications. Selecting a quality
step changes its own settings, while manual edits are reflected as Custom.
Switching the rendering backend retains each backend's scene settings.

NPR Detail includes primary Strength/Curve, editable line/dot/stipple layers,
advanced variation controls and shading/ink settings. Controls become available
when their prerequisites apply, such as AO for Contact/Shape and color fill for
albedo quantization. Spacing and mark-width hints show the effective values.
Reset restores the selected style and clears its Edited marker. At least one
layer is retained because an empty specification restores template layers;
up to 64 layers are supported. Opening Detail does not clamp saved values to the
controls' input ranges, and changing away from NPR returns to the Render tab.

The native egui and Tauri windows also offer POV-Ray, including radiosity, lighting, shadows,
stereo, clipping and post-blending. The POV-Ray menu opens this same window.
It uses the executable and include directory selected in Settings > Tools,
falling back to the bundled executable. Missing tools appear beside the disabled
Start button. The browser can edit and save these settings, but POV-Ray execution
requires the native application. Shared image commands also use a scene's selected
POV-Ray backend; an explicit `style` argument selects the built-in renderer.

Settings edits are stored after a short quiet period and grouped into one Undo
entry. Closing the window, switching scenes or starting a render flushes pending
edits to their original scene. Undo/Redo and other scene-setting edits reload the
open editor. Target selects a scene and view without activating it. Its settings,
geometry, background and saved camera are used for rendering; the active view
uses its live camera. Changing the main window's active view also changes the
render target, while ordinary scene updates retain a manual target choice.
With the Rendering window focused, Ctrl/Command-Z and its redo shortcut act on
the selected target scene. Later edits in other scenes and their redo entries
remain intact. Text fields keep their own text-editing shortcuts. An operation
that changes multiple scenes remains one history step; later edits to any linked
scene must be undone before that step. The main workspace history controls retain
their ordinary chronological behavior.
Start Render uses these target values. Stop and failures
retain completed results. Current view and fixed image-size presets are available;
when dimensions use inches, centimeters or millimeters, changing DPI preserves
physical size and updates the output pixel count. The command APIs retain their
direct image/download behavior. `capture_view`, `get_screenshot` and `render_image`
also accept optional `sceneId` and `viewId` targets. A view from a different
scene or an invalid ID fails without falling back to the active scene.

Previous and Next browse up to 50 completed results without changing the scene.
Save exports the displayed image with its original DPI metadata; Copy puts that
image on the system clipboard. Settings shows the captured values, and Use
settings restores the common settings and the captured backend to the selected
target scene in one Undo step.
Other backend settings remain unchanged. Clear asks before discarding the image
history and leaves scenes untouched. Native images use owned temporary files;
the browser also limits compressed history to 128 MiB, retaining the latest
result even when it exceeds that budget. Only the displayed image is decoded.

Fit, 100% and the zoom buttons control image framing. Drag or scroll to pan;
pinch or Ctrl/Command-wheel to zoom around the pointer. Switching results keeps
framing, with the zoom adjusted for a different image size. Clipboard errors
appear in the bounded, scrollable log. POV-Ray also streams its process output
there. Large previews use multiple GPU textures to preserve full-resolution
viewing on devices with smaller texture limits; Save and Copy retain the original
PNG. Tauri uses its webview image element for the same full-resolution preview.
Its modeless Rendering window retains history when closed, and its target selector
waits for the new scene settings before accepting edits.

Native egui, Tauri and the browser handle Movie mode. The Animation rendering menu opens
that mode with the QVGA preset. Still/Movie requests reset the size preset while
preserving backend settings and movie preferences. Movies use pixel dimensions,
the complete target animation, a positive fractional frame rate, and an optional
last frame. Output can use the managed temporary session or a custom folder.
The base name selects the numbered PNGs and an optional encoded movie. Native
applications support all seven listed formats.
Custom output replacement and Cleanup require confirmation. Cleanup removes only
the matching frames and movies; clearing result history keeps those output files.

Frames render sequentially through the same settings and backend as still images.
The window displays a throttled preview and keeps running when closed. Completed
and cancelled runs retain a result with on-demand frame browsing, selected-frame
Save/Copy, Open movie and Save movie as. Native applications also offer Reveal movie. Re-encode uses existing
contiguous PNGs and does not require an animation. A failed re-encode preserves
the previous movie. Replacing or cleaning frame files disables browsing those
frames in older results while preserving their cached preview. A replaced movie
file is no longer offered by older results.

Managed temporary sessions survive shutdown. On the next session creation,
inactive owned sessions lose frames after one day and movies after 30 days;
at most ten inactive sessions with movies are retained. Live sessions, unknown
content and custom folders are not automatically cleaned.

Browser movies use the same Rust scene capture and frame preparation. PNGs and
encoded movies are written incrementally to browser storage; only the current
frame and bounded history previews stay in memory. Frame browsing serializes
reads and gives priority to the latest slider request. Choose a custom folder
with the folder button when the browser supports directory access. Save movie as
uses the browser's file picker or a download, and Open movie opens a media tab.
Browser temporary output follows the retention policy above, but clearing site
storage or browser eviction can also remove it. Custom folders are never swept.

MP4 and QuickTime H.264/H.265 encoding use WebCodecs and a pinned local muxer.
Codec support is checked for the requested size before replacing any output.
H.265 availability depends on the browser and hardware. Raw QuickTime, Windows
Media and animated GIF currently require the native application. Disable Encode
movie to produce numbered PNGs without an encoder. Browser POV-Ray execution
continues to require the native application.

Animation renders capture the target scene and display settings at Start.
Rendering does not move the live camera or animation timeline, and edits made
while frames render remain after completion, cancellation or an error. Each job
uses its captured scene for every frame. Re-encoding existing frames also leaves
the current scene untouched.

Umbreon is implemented in Rust and uses a BVH for primary, shadow, AO, and GI
rays. Its script exporter applies typed scene render settings, including physical
image size, DPI, lighting, adaptive antialiasing, reduced-resolution AO and NPR
styles. `getRenderLog()` drains actual render diagnostics; `getHatchStyleSpec()`
returns an editable preset. Adaptive sampling preserves silhouette detail, and
AO interpolation rejects depth or normal discontinuities. Auxiliary sampling
buffers are bounded to strips. Native file exporters support asynchronous
rendering; browser image commands use the same renderer and browser downloads.
Script file-export threads require a native filesystem and report an error in
the browser. The Rust integrator does not promise pixel identity with Embree.

### Bundled tools

USalign is compiled from the vendored reference implementation; its source
and license are retained. Release tool bundles contain APBS, PDB2PQR, FFmpeg,
POV-Ray and the Intel OIDN runtime, including a private PDB2PQR runtime. They do
not require a system Python install.
Tool lookup checks configured paths, `CUEMOL_TOOLS_DIR`, the binary/application
bundle, the cache, and `PATH`. The POV-Ray console build includes its standard
include files, native runtime libraries, licenses and pinned source archive.
`cuemol tools run povray` and both desktop rendering dialogs use that bundle.

## AI Agent

Enable **AI Agent** under **Options > Plugins > Installed** in either native frontend or
**Settings > Plugins > Installed** in the browser. Then open **Tools > AI Agent** or the
**AI Agent** sidebar, choose `openai:model`, `anthropic:model`, or `google:model`,
and enter that provider's API key. The model field accepts arbitrary model IDs;
**Refresh available models** queries the provider's catalog. Settings are saved
in `agent.json` beside the TSV configuration file in native egui and Tauri;
the browser uses local storage. Native keys can be stored
in the OS credential store, with `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, or
`GEMINI_API_KEY` as a fallback. Browser keys remain in the current tab's memory.
On Linux, saved keys require a running Secret Service, such as GNOME Keyring,
in the desktop session. Environment variables or `--api-key-stdin` remain
available when that service is absent.

Use Ctrl+Enter to send, or Cmd+Enter on macOS. The **Enter sends** preference
uses Enter to send and Shift+Enter for a new line. IME conversion keys do not
submit a message. Up and Down recall earlier prompts at the first and last line
of the composer; returning past the newest prompt restores the unsent draft.
The latest 50 distinct prompts survive restarts. Native egui stores them in
`agent-prompts.json` beside its configuration; browser and Tauri shells use
local storage. **Clear chat** resets the conversation without deleting prompt
recall. Tool rows expand to show their input and result, while image thumbnails
remain visible. Tauri applies edited settings when sending or with **Apply settings**.

Sending a message shares the prompt, scene description, tool results, and any
captured images with the selected provider. Replies stream into the panel.
Specialized tool groups are enabled as needed; the full MCP catalog remains
available through those groups. Each turn is limited to 16 model rounds and
creates one undo entry for its successful scene edits. Cancel keeps edits
already shown. A manual scene edit cancels the pending turn, and late download
or APBS results are discarded. Network requests, image rendering, and native
APBS calculation run without blocking the panel. Browser filesystem and external
process restrictions still apply.

```sh
cuemol agent run example.pdb --prompt 'Show chain A as a ribbon' --save result.qsc
cuemol agent models --model openai:MODEL_ID
cuemol agent key set openai < key.txt
cuemol agent key delete openai
```

`key set` reads standard input. `run` and `models` also accept `--api-key-stdin`
for a session key, `--endpoint URL` for a compatible API server, and
`--reasoning default|low|medium|high`. `default` leaves reasoning settings to the
provider. Model availability, reasoning support, and API charges depend on the
provider and account. Conversation history stays in memory; **New conversation**
clears it.

The Rust adapters use [OpenAI Responses streaming](https://developers.openai.com/api/docs/guides/streaming-responses),
[Anthropic Messages streaming](https://platform.claude.com/docs/en/build-with-claude/streaming),
and [Google streaming generation](https://ai.google.dev/api/generate-content).
Provider-specific reasoning data and signatures are preserved between tool
rounds. Switching providers carries over only the user and assistant text.

## Browser

Release binaries embed the browser application:

```sh
cuemol web serve
cuemol web serve --port 8080 --no-open
cuemol web serve --dir web
```

Alternatively, serve the extracted `cuemol-web.zip` from an HTTPS host. WebGPU
requires a secure context; localhost is supported. `?renderer=webgl` selects the
WebGL2 fallback. Local file data stays in the browser unless shared through the optional AI Agent.
File selection, drag and drop,
representations, properties, sequence, saved cameras, commands, undo, animation,
PNG export, and embedded QSC download use the shared Rust core. Image rendering
yields between ray-tracing and denoising-guide batches so the viewport remains
interactive. Stop in the Render window or Cancel in the status bar cancels it.
Browser settings and startup scripts are stored in localStorage for that origin.

### APBS electrostatic potential

Open Tools > APBS elepot calculation to choose a molecule and optionally build a
selection. Each opening resets the calculation options. PDB2PQR uses the force
field from Settings; switching to the internal method exposes the hydrogen
option. Set executable paths in Settings > Tools, or install the tool bundle.
The application also checks PATH. Missing required tools disable Start.

The potential name is suggested again when changing the target; leaving it blank
uses the same automatic naming rule. Start runs the calculation asynchronously.
The form shows the current stage and a bounded calculation log, while Stop cancels
the job. Errors leave the draft open. Success adds an ElePotMap with its unit-cell
renderer, records one undo step and closes the form. Failed or cancelled jobs do
not add selection history. Generated map data is embedded by ordinary scene saves
and remains available after temporary calculation files are removed.

The browser displays the same options and explains its execution boundary.
Run APBS in the native application, then import the resulting DX map into the
browser.

### Trajectories and browser storage

Browser MD playback keeps `File` handles and reads selected DCD/XTC/TRR/NetCDF/MDCRD
frames with `File.slice`, without first loading whole trajectories into JS
memory. Additional files append as ordered blocks for the selected molecule;
multiple molecules retain independent frame positions. Loading a topology and
several trajectories together preserves their input order. Undo shares the file
handles, and scene downloads interleave them with QSC fragments as browser Blobs.
### Native and browser differences

The browser cannot access arbitrary native paths or spawn external
tools. USalign uses the same vendored calculation kernel as the native application,
compiled into a companion WebAssembly module. LSQ and SSM remain shared Rust code.
ICC soft proofing uses the embedded LittleCMS engine on both targets. Open an
ICC/ICM file in the browser to activate it; the profile is retained in browser
storage. OIDN runs through WebGPU with a bundled neural-network model; it requires
WebGPU even when the interactive viewport uses WebGL2. Bilateral and A-trous
remain available without WebGPU. APBS runs in the native application; the browser can import APBS DX maps.
Scenes with external
file references must be saved with embedded data before transfer to the browser.

## Compatibility boundaries

The ribbon, cartoon and tube paths use the reference cross-section profiles,
natural and smoothing splines, junctions, helix-width modes and end caps.
Their geometry, triangle winding, partial selections and color boundaries are
checked against an optional C++ oracle. Object transforms apply to molecular
coordinates before glyph construction, preserving atom radii and thermal tensor
axes. Renderer transforms apply to the resulting geometry. Uniform scales keep
sphere/cylinder primitives; nonuniform scales and shear tessellate them while
retaining picking identities, section materials and transparency. Mesh normals
use the inverse transpose, and reflections preserve outward face orientation.
Secondary-structure assignment excludes proline donors and uses sparse
hydrogen-bond/bridge searches with bounded per-residue bond storage.

The native and browser wgpu frontends implement Tritium's layout with Rust egui
controls. The optional Tauri shell provides the same activity bar, inspector,
themes and bottom docks through its webview. All show the `cuemol-rs` application
name and the repository's PNG logo. Direct molecular
surfaces support distance fields, EDTSurf and MeshMS on native and browser
targets, with reference detail and surface-type controls. Saved molecular
surfaces support MeshMS and BALL, with `auto` selecting MeshMS and BALL as its
fallback; the `rust` backend retains grid-based SES. The generation backend,
density and probe radius survive embedded scene saves. Scripted
`MolSurfObj.sesbackend` selects `auto`, `meshms` or `ball` for both creation and
regeneration, including after scene reload. Render settings use typed
scene application data, including nested backend settings, script access and undo. NPR rendering includes the nine reference styles and editable line, dot, and
stipple layers, with shared native/browser controls for ink, paper, and tone.
Hatch and tone specifications use the reference syntax. The Rust ray tracer
uses bounded diffuse path tracing, metallic reflection, camera-relative lights,
and a sky/ground environment. AO supports multiple distance scales and bent
normals; compatible supersamples reuse lighting within each output pixel.
Contact/crease contours and both section-alpha blend modes are configurable. OIDN denoises
floating-point indirect irradiance with albedo and normal guides, then restores
the direct-light contribution and original alpha. Full-frame OIDN is also
available through `denoiser=2`. Native builds load the bundled CPU runtime,
while browser builds use a packaged WebGPU model. Bilateral and color-guided
A-trous filters are also selectable. `CUEMOL_OIDN_LIBRARY` overrides the native
library location. Missing runtimes or unsupported browser devices report errors.
The A-trous implementation uses increasing B3-spline sample spacing, following
the [wavelet filtering approach](https://doi.org/10.2312/EGGH/HPG10/067-075),
with color and alpha guidance rather than the paper's additional geometry data.
Unsupported render settings return errors. The legacy PNG exporter's `interlace`
and `compresslev` properties retain the C++ behavior: they are accepted but do
not change PNG encoding. Native MCP ray tracing and APBS run in
background workers; the MCP call returns when its result is available.
These boundaries remain relevant when comparing complex reference scenes.

The workspace consists of foundation crates (`core`, `gfx`, `qsys`), molecular
and domain crates, shared `desktop-core` and `command` crates, and the `desktop`,
`web`, and `cli` applications. The Node.js crate provides a binding facade; native Python connects to live
application state through a persistent worker. See [CLAUDE.md](../CLAUDE.md) for code organization and
[validation.md](validation.md) for measured checks and their scope.
