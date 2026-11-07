# Implementation and validation status

This checklist tracks the requested complete Rust/wgpu replacement. Existing
implementation, passing finite comparisons, and complete feature parity are
separate states. A row remains open until its behavior and applicable frontends
have been checked. Generated fixtures, profiles and screenshots belong in the
ignored `.cache` directory.

The current reference is the sibling CueMol checkout at `794fe4ac` (3.0.0.554).
Earlier comparisons in [parity-audit.md](parity-audit.md) used 2.4.1.547.
The newer reference adds a ChimeraX console, selection simplification, combined
node framing and additional command options. Selection rewriting, fit-box APIs and combined node framing are now implemented
and checked across the shared command, native and browser frontends. Camera command
rotation, slab fitting and undo separation also pass focused cross-frontend
checks. Object export selection, format and source-link options also pass native and
browser checks. The current ChimeraX console command set is implemented and passes native,
Tauri and WebGL/WebGPU checks. The current operation inventory is complete; newly added operation behavior has focused coverage. Current CueMol3 style overrides, molecule-order palettes and named Paint colors now pass reference and cross-frontend checks. The Get PDB keyword search, entry lookup and EMDB-aware download choices are now shared by the native egui and browser frontends and exposed in Tauri. This worker-service feature was outside the command-catalog inventory. The current About runtime information and clipboard action now pass shared, native-window, Tauri and WebGL/WebGPU checks. Structured renderer numeric controls now preview without adding history, restore defaults on cancellation, and commit one undo step; native-window, Tauri and WebGL/WebGPU checks pass. The Density map pane now shares native/browser controls and Tauri behavior, including unit-aware levels and one-step opacity drags; focused checks pass. The reference checkout is read-only.

| Requirement | Existing implementation and next verification |
| --- | --- |
| Complete feature and appearance compatibility | Open. Reconcile the older feature checklist with the current reference, then execute behavior checks for each added or changed interface. |
| Rust implementation and wgpu | Shared Rust scene, command, GUI and rendering crates exist. Native and browser viewports use wgpu. Vendored numerical kernels and bundled external executables retain their provenance. |
| Molecular structures, maps, surfaces and editing | Readers, representations, map controls, selections, labels, measurements, editing and undo exist. Current QIF method/property coverage passes; the new selection and fit-box numerical oracles pass. Partial-residue deletion, alternate conformations, merge identity and coordinate/history composition now pass low-level, shared-script and browser checks. Continue the remaining behavior audit. |
| Console and scripting | CueMol, PyMOL, ChimeraX, JavaScript and persistent native Python exist. ChimeraX shares the command runtime, supports `.cxc` scripts and has 59 registered commands. Parser, palette and automatic-style comparisons and native, Tauri and WebGL/WebGPU execution checks pass. |
| Unified command and MCP | `cuemol` owns GUI launch, scripts, MCP, tools, web builds, installation diagnostics and updates. All 129 current registrations, 127 MCP operations, 444 parameters and 159 console names/aliases pass the read-only inventory. Added focus/orientation, clipboard, scene encoding and EMDB behavior passes focused native and browser checks; broader behavior remains under audit. |
| Native appearance and input | The current Linux OS-input suite passes, including the new ChimeraX console, camera-preserving history and multiple-object Zoom. Screenshots were inspected. On the authorized Apple-silicon host, normal and software-stereo wgpu images pass, including exact left/right eye exchange. OS screen/input permission grants remain pending; other-platform window checks still need current binaries. |
| Animation timelines | Reference chains, transactional edits and broken-chain repair pass shared/native/Tauri/browser checks. All three frontends provide UID selection, strip move/resize, scrubbing, transport, axis/direction presets and an element Inspector. Timing and generic numeric drags preview without history and commit one undo step. Property types and default flags match the current C++ oracle; resets, cancellation, history and serialization have focused checks. Command conversions now match 98 current TypeScript/C++ cases, including boolean aliases, numeric notation, native generic types and near-zero axes. Start-camera initialization, explicit show defaults and first-add undo now pass shared/native/Tauri/browser checks. Broader appearance checks remain under audit. |
| File destinations and OS drops | Native and Tauri accept OS file-drop events. Native and browser imports share batch destinations, reuse untouched scenes and retain the first successful destination. Tauri applies the same destination after accepting reader options. Menu opens remain in the active scene; drop and shell preferences are independent. Focused model, browser and Linux XDND checks are recorded in the validation log. |
| Built-in plugin preferences and MCP lifecycle | Defaults, required contributions, tab/activity/dialect visibility, cancellation and persistence are shared. Native GUI MCP and Tauri MCP use authenticated loopback transport and GUI history; APBS and image workers remain asynchronous. Focused tests cover listener shutdown during an outstanding request and reconnection, late APBS cancellation, original-scene output and numeric-preview/history isolation. The full settings category tree, catalog examples and Tauri Agent interface remain open. |
| Picking preferences | Native, Tauri and browser controls persist GPU picking, hover information and highlighting. Shared runtime/config checks, scene-history regressions, Metal pixel comparisons, WebGL/WebGPU and Linux OS input cover switching to CPU atom picking, stopping hover requests, cancellation and persistence. |
| Saved camera controls | Native and browser frontends share named selection, toolbar actions, rename, reordering, properties, show/hide editing and file controls. Tauri exposes the same actions. Focused command, model and actual-window/browser checks cover overwrite metadata, pinned order, visibility, navigation preserving redo and camera-file round trips. |
| View transforms | Native and browser share relative rotation, viewing-axis pan, absolute coordinate entry, chained keyboard input, zoom/slab/distance, projection and center-mark controls. Tauri uses the shared transform math and matching numeric interactions. C++ comparisons, shared/model tests, WebGL/WebGPU and Linux OS input verify distance/slab separation and navigation preserving redo. |
| Crystal symmetry controls | Native and browser frontends share molecule/map selection, crystal readouts, constrained cell editing, symmetry extents and unit-cell actions. Default cell values, cancellation, scene changes, repeated unit-cell actions and history pass focused checks. Tauri also uses the current defaults, ignores stale space-group replies and offers explicit cancellation. |
| MD loading, seeking and playback | Indexed DCD, XTC, TRR, NetCDF and MDCRD are implemented. MDCRD decoding and shared frame application avoid intermediate copies; malformed frames and oversized averaging windows have regressions. Coordinate/RSS benchmarks, affected native tests, and rebuilt-browser File seeking, agent undo and multi-object trajectory export/reload pass. |
| Performance and memory | Measure changed operations with identical inputs and output checks. Record at least three fresh-process samples; do not infer application-wide speedups from one stage. |
| Bug fixes | Require a reproducer and relevant regression checks for confirmed defects. Keep failed operations transactional. |
| Built-in rendering and external tools | Rust GI/NPR and bundled APBS, PDB2PQR, FFmpeg, POV-Ray, OIDN and numerical kernels exist. Current native tool checks pass. The Python Umbreon helper now passes stored-settings, NPR, size-override and scene-reload pixel comparisons. Python POV-Ray stills, name/measurement labels, post-blending, animation, Unicode/space paths and failure preservation pass using the bundled renderer. Non-interlaced PNG composition now uses bounded row buffers and passes pixel/RSS comparisons. |
| Native binary and app distributions | Packaging for Linux x86-64/ARM64, macOS Intel/Apple silicon and Windows exists. Latest baseline Release build passed all five targets; it did not publish a new version. |
| WASM and browser interaction | Parser worker, WebGPU/WebGL2 viewer and Pages deployment exist. The 214-check current baseline exposed seven failures; all pass after updating obsolete appearance assertions and bounding Paint-row widths. The 62 affected browser checks and 190 native GUI tests pass. The subsequent Get PDB changes pass all 62 affected loading, search and Tauri shell checks; shared core/command/UI and native model checks also pass. Continue browser checks after shared behavior changes. |
| Installer and self-upgrade | Raw GitHub installers, checksums, rollback and binary/app self-upgrade pass current Linux and macOS local-fixture checks. Current source changes have not been published as a release. |
| Configuration and startup | Shared configuration, preferences and startup scripts are implemented. CLI regressions for persistence and read-only commands that must not execute startup scripts pass in the current workspace run. |
| Default representations and navigation | Protein ribbons, nucleotide cartoons, other-atom ball-and-stick, mouse presets, slab and centering exist. Current CueMol3 ribbon/nucleotide dimensions and palette themes pass reference comparisons and WebGL/WebGPU rendering and scene reload; native OS input also passes with the new style rows. |
| Build time and Actions cost | Headless default builds, split crates, debug-free dev/test profiles, Cargo/tool caches and one shared browser artifact exist. Baseline CI took about 12 minutes; the five-target Release build took about 36 minutes including browser validation. Investigate the critical path before adding jobs. |
| Incremental commits | Commit coherent implementations after focused verification; preserve unrelated working-tree changes. |

Hardware quad-buffer presentation remains unimplemented and needs a compatible
OS, GPU and display for integration validation. The authorized Apple M4/macOS
setup rejects stereo pixel-format selection, although software stereo passes. Live third-party AI provider
checks require credentials; loopback tests cover adapters without paid requests.
These boundaries do not establish complete parity.

## Current interface inventory

The current shell declares ten sidebar panes: Scene, Color, Camera, View,
Molecular Structure, Selection, Symmetry, Density Map, Action History and
Action Library. Each has a corresponding native, browser and Tauri interface;
its focused evidence is recorded above and in [validation.md](validation.md).
The built-in bottom tabs are Output and Animation. The compiled plugin registry
adds Console, Sequence and Trajectory tabs, the Agent activity and Get PDB.
Their user workflows are implemented, but this does not imply support for every
plugin setting or contribution rule.

The settings audit found the two file-destination preferences and disconnected
native file-drop handling. Those paths are now implemented. GPU-picking and hover controls now pass persistence and interaction checks across
all frontends. Plugin enable/disable behavior now has focused native, Tauri and browser checks.
The remaining settings comparison includes the current category tree, the full
component catalog and the Tauri Agent interface. The current reference
ships its plugin registry at build time; runtime installation is a future
reference feature, not an implemented interface to port.

The old [parity audit](parity-audit.md) retains measurements against 2.4.1.547.
Its historical open rows and publication notes do not supersede this checklist.
The remaining dialog audit covers reader options, molecular editing, surface
operations, interaction analysis and rendering controls against the current
reference. A registered command alone does not close a dialog or appearance row.
