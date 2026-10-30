---
name: cuemol
description: View, edit, analyze and render molecular structures, density maps and trajectories with cuemol-rs through its CLI or MCP interface. Use for CueMol scenes and molecular visualization workflows.
---

# CueMol

Use the installed `cuemol` application. Inspect its version with
`cuemol --version` and its commands with `cuemol --help`. This skill is also
available offline from `cuemol skill`.

## Choose the session

- `cuemol structure.pdb` opens the native GUI. With no arguments, `cuemol`
  opens an empty window. Proteins use ribbons, nucleic acids use nucleotide
  cartoons, and other atoms use ball-and-stick representations on initial load.
- `cuemol mcp connect` exposes the running native GUI over MCP stdio.
  `cuemol mcp status` checks that session; `cuemol mcp config` prints its client
  configuration. Use this session when editing what the user is viewing.
- `cuemol mcp serve` runs an independent headless scene over MCP stdio.
  `cuemol mcp config --headless` prints its client configuration.
- `cuemol exec TOOL JSON` runs one shared command in a fresh headless process.
  Separate invocations do not retain a scene. Use one script or an MCP session
  for a sequence of edits.

Inspect `cuemol mcp tools` for the installed tool names and JSON schemas before
calling unfamiliar operations. `cuemol mcp call TOOL JSON` calls the running
GUI session directly. Read the active scene and object identifiers before
editing or exporting an existing session.

## Files, scripts and rendering

```sh
cuemol pdb-info structure.pdb
cuemol trajectory-info trajectory.xtc
cuemol run commands.js
cuemol -c 'print(21 * 2);'
cuemol render structure.pdb image.png --width 1200 --height 900
cuemol render scene.qsc illustration.png --style npr
cuemol export-scene structure.pdb model.obj
```

JavaScript runs in the embedded engine with `cuemol`, `scene` and `view`.
`cuemol run commands.js arg1 arg2` exposes arguments through `getCmdArgs(index)`;
`getCmdArgs(-1)` returns their count.
Use the current tool schemas for renderer, selection, camera and export
parameters. Loading coordinates creates default representations; loading a
saved QSC restores its stored renderer settings. Prefer an embedded QSC when
the scene must be portable.

Verify the requested output, including atom/frame counts, camera, renderer
selection and image dimensions. A successful process exit alone does not
establish that the expected scene was rendered.

## Interactive viewing

The Default mouse preset uses left-drag rotation, right-drag pan and wheel zoom.
Shift-left-drag adjusts slab thickness horizontally and zoom vertically.
Use `C` to center the selection, `F` to fit the selection and `Home` to fit the
scene. An atom's context menu also provides centering. Settings include a PyMOL
mouse preset; its bindings differ from Default.
Set it before launch with
`cuemol config set cuemol2.ui.viewinconf PymolViewInConf`.

## Configuration and installation

`cuemol config path` reports the active configuration directory. By default it
is `$XDG_CONFIG_HOME/cuemol` or `~/.config/cuemol`. `CUEMOL_CONFIG_DIR` overrides
the directory. Preferences are stored in `config.tsv`; `startup.js` runs before
command-line structures open. Read or write individual preferences with
`cuemol config get KEY` and `cuemol config set KEY VALUE`.

`cuemol self-upgrade --check` checks the latest release, and
`cuemol self-upgrade` installs it. Release packages include native external tools.
For source installations, `cuemol tools list` reports availability and
`cuemol tools bundle` prepares the optional tool bundle. The browser build and
external tools are separate build steps for Git/pip source installations.

For hosted notebooks without a desktop, use headless commands, image export or
an MCP headless session. `cuemol web serve` serves a built browser application;
prebuilt releases include it. Browser file access and native subprocess tools
have different limits. Hardware quad-buffer stereo is unavailable.

Consult the [user guide](https://github.com/mogura-rs/cuemol-rs/blob/main/docs/usage.md)
for renderer settings, trajectories, native Python, browser use and tool setup.
