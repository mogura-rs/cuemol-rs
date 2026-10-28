# Local patch

Source: winit 0.30.13, the crates.io source archive.
Archive SHA-256: `a6755fa58a9f8350bd1e472d4c3fcc25f824ec358933bba33306d0b63df5978d`.
Upstream commit: `e9809ef54b18499bb4f2cac945719ecc2a61061b`.
Repository: https://github.com/rust-windowing/winit
License: Apache-2.0; see `LICENSE`.

The XInput2 button path ignores releases of wheel buttons 4 through 7.
The published implementation emits a `MouseWheel` event for both states,
which doubles each discrete scroll step with XTest and other button-based
X11 input. Valuator-based scrolling is unchanged. The native viewport is
checked with actual X11 wheel events and the reference camera formula.

The unused nested Cargo lockfile is omitted; the workspace lockfile selects
all dependencies. This directory is excluded from workspace test discovery.
