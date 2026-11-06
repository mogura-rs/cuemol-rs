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
Tauri and WebGL/WebGPU checks. The current operation inventory is complete; newly added operation behavior has focused coverage. The reference checkout is read-only.

| Requirement | Existing implementation and next verification |
| --- | --- |
| Complete feature and appearance compatibility | Open. Reconcile the older feature checklist with the current reference, then execute behavior checks for each added or changed interface. |
| Rust implementation and wgpu | Shared Rust scene, command, GUI and rendering crates exist. Native and browser viewports use wgpu. Vendored numerical kernels and bundled external executables retain their provenance. |
| Molecular structures, maps, surfaces and editing | Readers, representations, map controls, selections, labels, measurements, editing and undo exist. Current QIF method/property coverage passes; the new selection and fit-box numerical oracles pass. Continue the remaining behavior audit. |
| Console and scripting | CueMol, PyMOL, ChimeraX, JavaScript and persistent native Python exist. ChimeraX shares the command runtime, supports `.cxc` scripts and has 59 registered commands. Parser, palette and automatic-style comparisons and native, Tauri and WebGL/WebGPU execution checks pass. |
| Unified command and MCP | `cuemol` owns GUI launch, scripts, MCP, tools, web builds, installation diagnostics and updates. All 129 current registrations, 127 MCP operations, 444 parameters and 159 console names/aliases pass the read-only inventory. Added focus/orientation, clipboard, scene encoding and EMDB behavior passes focused native and browser checks; broader behavior remains under audit. |
| Native appearance and input | The current Linux OS-input suite passes, including the new ChimeraX console, camera-preserving history and multiple-object Zoom. Screenshots were inspected. Local macOS screen/input permissions remain unavailable; hardware and other-platform window checks still need current binaries. |
| MD loading, seeking and playback | Indexed DCD, XTC, TRR, NetCDF and MDCRD are implemented. MDCRD decoding and shared frame application avoid intermediate copies; malformed frames and oversized averaging windows have regressions. Coordinate/RSS benchmarks, affected native tests, and rebuilt-browser File seeking, agent undo and multi-object trajectory export/reload pass. |
| Performance and memory | Measure changed operations with identical inputs and output checks. Record at least three fresh-process samples; do not infer application-wide speedups from one stage. |
| Bug fixes | Require a reproducer and relevant regression checks for confirmed defects. Keep failed operations transactional. |
| Built-in rendering and external tools | Rust GI/NPR and bundled APBS, PDB2PQR, FFmpeg, POV-Ray, OIDN and numerical kernels exist. Recheck packaged execution after relevant changes. |
| Native binary and app distributions | Packaging for Linux x86-64/ARM64, macOS Intel/Apple silicon and Windows exists. Latest baseline Release build passed all five targets; it did not publish a new version. |
| WASM and browser interaction | Parser worker, WebGPU/WebGL2 viewer and Pages deployment exist. Build and run relevant browser checks after shared behavior changes. |
| Installer and self-upgrade | Raw GitHub installers, checksums, rollback and binary/app self-upgrade pass current Linux and macOS local-fixture checks. Current source changes have not been published as a release. |
| Configuration and startup | Shared configuration, preferences and startup scripts are implemented. CLI regressions for persistence and read-only commands that must not execute startup scripts pass in the current workspace run. |
| Default representations and navigation | Protein ribbons, nucleotide cartoons, other-atom ball-and-stick, mouse presets, slab and centering exist. Recheck current reference defaults and selection-sensitive behavior. |
| Build time and Actions cost | Headless default builds, split crates, debug-free dev/test profiles, Cargo/tool caches and one shared browser artifact exist. Baseline CI took about 12 minutes; the five-target Release build took about 36 minutes including browser validation. Investigate the critical path before adding jobs. |
| Incremental commits | Commit coherent implementations after focused verification; preserve unrelated working-tree changes. |

Hardware quad-buffer presentation remains unimplemented and needs a compatible
OS, GPU and display for integration validation. Live third-party AI provider
checks require credentials; loopback tests cover adapters without paid requests.
These boundaries do not establish complete parity.
