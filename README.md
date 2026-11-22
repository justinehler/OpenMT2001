<p align="center">
  <h1 align="center">OpenMT2001</h1>
</p>

<h3 align="center">An open-source re-implementation of <em>Monopoly Tycoon</em> (2001), a business simulation game about building and managing a city-scale commercial empire.</h3>

---

# 1. Introduction

**OpenMT2001** is an open-source re-implementation of *Monopoly Tycoon* (2001). The gameplay revolves around buying city blocks, constructing businesses and apartments, setting prices, managing utilities and inventory, and competing against rival tycoons for control of the city. Players build their fortunes by balancing rent, customer demand, property ownership, auctions, and monopolies across a living urban economy.

*Monopoly Tycoon* was developed by Deep Red Games and published by Infogrames. OpenMT2001 aims to preserve and extend the original game by providing a modern, portable engine, documented file formats, improved tooling, and support for current platforms while remaining compatible with legally owned copies of the original game data.

The Rust engine source is available here. It is an incomplete reimplementation:
rendering, menus, business management, opponents, audio and save continuation
are implemented, but complete campaign, tutorial and audiovisual fidelity are
still being verified. There is no finished release or guarantee of whole-game
equivalence.

---

# 2. Playing the game

OpenMT2001 requires player-supplied Monopoly Tycoon files, preferably v1.4.
No original executable, assets, scenarios, saves, compiler or decoder is
distributed here. The engine reads the supplied executable as data; it does
not launch it.

Build the native game and run it with explicit paths:

```sh
cargo build --locked --release --features native --bin game
cargo run --locked --release --features native --bin game -- \
  --menu \
  --assets "/path/to/extracted/installer" \
  --scene-root "/path/to/game/max" \
  --executable "/path/to/v1.4/mc.exe" \
  --intro-map "/path/to/v1.4/scripts/MAPS/intromap.lua" \
  --language "/path/to/v1.4/language/usa" \
  --save-dir "/path/to/original/save/imports" \
  --profile-dir "/path/to/writable/openmt2001-data"
```

These paths refer to separate parts of the supplied game data. The assets
directory contains Standard/, scripts/ and parameters/; the scene directory
contains archive.DIR, archive.bin and BuildEditorModels/BuildFile.dat.
Preserve original files and use a separate writable profile directory.
Modern saves include engine-specific runtime extensions; compatibility with
loading those saves in the original executable is not established.

Install FFmpeg on PATH, or specify --ffmpeg /path/to/ffmpeg, for original
movie and compressed audio decoding. --menu skips the opening presentation.
Run the game with --help for the complete argument list.

Windows, Linux and macOS are the target native platforms. Fresh platform
verification is in progress; this source import is not a tested binary release.
Browser execution is not currently supported.

Tutorials 5 and 8 are currently withheld from the live selector while their
construction-rights paths are investigated. The Tutorial 5 failure matches a
[reported original v1.4 tutorial limitation](https://www.gamepressure.com/download/monopoly-tycoon-14-patch/z3135e5).
Passing initialization or selected tutorial flows does not establish complete
campaign playability.

---

# 3. Building the project

Use Rust 1.99 or later and a C compiler for the optional Lua runtime. The
native frontend uses wgpu, winit and the platform audio system. On Debian/Ubuntu,
install the native development dependencies:

```sh
sudo apt-get install build-essential pkg-config libasound2-dev libxkbcommon-dev
```

The core and Lua tests need no original game files:

```sh
cargo test --locked --lib
cargo test --locked --features lua4 --lib
```

Original-input comparisons and GPU checks are explicitly ignored by default.
A passing synthetic test suite does not establish complete game fidelity.
Native builds need graphics, audio and window-system support. The core library
keeps simulation and asset decoding independent of the native host.

---

# 4. Maintainer model

OpenMT2001 is a public, MIT-licensed project maintained as a personal development effort.

The source is published so that others can study, build, modify, and fork it under the terms of the MIT License. However, this repository is not operated as a community-led open-source project. Pull requests, design proposals, roadmap requests, and general support requests should not be expected to receive review or acceptance.

Issues and Discussions are open for bug reports, compatibility notes, research findings, and useful project context. Reports may be reviewed and acted on at the maintainer's discretion, but there is no guarantee of response, fix, release timeline, or ongoing support.

Development direction, scope, and release timing are decided by the maintainer.

---

# 5. Contributing

OpenMT2001 is not currently accepting outside code contributions. You are welcome to fork the project, modify it, and distribute your own work under the terms of the MIT License.

Please see [CONTRIBUTING.md](CONTRIBUTING.md) for the repository contribution policy.

---

# 6. Licence

OpenMT2001’s project-owned implementation is licensed under the MIT License. See [LICENSE](LICENSE) and [third-party notices](THIRD_PARTY_NOTICES.md), including the separate Lua 4.0 terms.

---

# 7. Disclaimer

OpenMT2001 is an independent open-source project. It is not affiliated with, endorsed by, or sponsored by Hasbro, Deep Red Games, Infogrames, Atari, or any other rights holder.
