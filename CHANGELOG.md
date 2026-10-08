# Changelog

## [0.2.0](https://github.com/vvvlladimir/Encrust/compare/v0.1.0...v0.2.0) (2026-10-08)


### ✨ New

* **app:** a progress bar runs, a message is cut, and an opened file ([3546370](https://github.com/vvvlladimir/Encrust/commit/354637038b7b3e40ccaf14f2028862f1e32f5aa9))
* **app:** a resin taken off a printer waits in the pool, and Esc leaves the Settings screen ([#117](https://github.com/vvvlladimir/Encrust/issues/117)) ([d593eb1](https://github.com/vvvlladimir/Encrust/commit/d593eb183b862eb9f12c02e827eb3b2fd3ce1f92))
* **app:** add bug report sheet and markdown generator ([0555784](https://github.com/vvvlladimir/Encrust/commit/055578446d9fe677b3dc665b23f680f46e256d17))
* **app:** add cli and web versions of app ([#6](https://github.com/vvvlladimir/Encrust/issues/6)) ([37c1c7f](https://github.com/vvvlladimir/Encrust/commit/37c1c7f705bafa2f346197aaf47fa040c683b61f))
* **app:** tool values are remembered between runs and undone on their ([bc01e94](https://github.com/vvvlladimir/Encrust/commit/bc01e94c70df2bdf7c28ac66b4f735990d3156e9))
* **core-geometry,app:** a hole is closed when it is asked for, and the ([#115](https://github.com/vvvlladimir/Encrust/issues/115)) ([6644462](https://github.com/vvvlladimir/Encrust/commit/66444623c72da83f0311868acb795da07f65923e))
* **core-volume,app,cli:** resin drains through a hole, and the hollow is seen in the viewport ([#112](https://github.com/vvvlladimir/Encrust/issues/112)) ([c8fcb9f](https://github.com/vvvlladimir/Encrust/commit/c8fcb9fdf7cac5d543b5e30236877cd83ce678a1))
* **geometry:** drop unbalanced faces and define closure by winding ([a64e356](https://github.com/vvvlladimir/Encrust/commit/a64e3562dc9a10d9bea398ec391caf8b4bcfe676))
* **project:** store plate state in projects to skip rebuilding on open ([#114](https://github.com/vvvlladimir/Encrust/issues/114)) ([1c09939](https://github.com/vvvlladimir/Encrust/commit/1c099399982849c4eb0d401efb5ef6b4d547e3e2))
* **render:** draw cut surfaces using stencil and depth passes ([71fe869](https://github.com/vvvlladimir/Encrust/commit/71fe8693cb44ffe2b306cf65c26faed236ee57a9))
* **report:** scrub network addresses and hostnames from bug reports ([dd6507a](https://github.com/vvvlladimir/Encrust/commit/dd6507aebb306201f67a2bd30e7caa8c1ffbce3e))


### 🐛 Fixed

* **app:** the cut plane is traced on the model and placed from its ([bb4fb17](https://github.com/vvvlladimir/Encrust/commit/bb4fb17a7de8820904209357a8767275bef95567))
* **app:** the window reads its own keys and the pointer belongs to the ([2b1d7a4](https://github.com/vvvlladimir/Encrust/commit/2b1d7a484649f5542a098686c1462beecb93096b))
* **cli,core-pipeline,core-slicer,format-chitu,format-anycubic:** a layer ([74e611f](https://github.com/vvvlladimir/Encrust/commit/74e611ff722eaa1b4c2ce63c08c429946fc680d3))
* **cli,core-volume,core-engine:** a stop lands inside a cavity, and a ([99c244a](https://github.com/vvvlladimir/Encrust/commit/99c244ad1b963a82ebedf9574843d6defa558a54))
* **core-analysis,core-slicer,core-plate,cli:** a report counts what is ([ba70d2b](https://github.com/vvvlladimir/Encrust/commit/ba70d2ba284244ecb2dd6c8ac4c2a1aae9564f40))
* **core-geometry:** a cut closes over the vertices and edges the plane ([47ef22f](https://github.com/vvvlladimir/Encrust/commit/47ef22f1b9644203904426a8f8f95240af03a93c))
* **core-pipeline,format-svgx,encrust-app:** a machine is offered only ([868074a](https://github.com/vvvlladimir/Encrust/commit/868074a6b46adf6e4d0fc111dbe7d76f95132d1e))
* **core-slicer,core-volume:** nothing under the plate is cut, and a ([e0a9a07](https://github.com/vvvlladimir/Encrust/commit/e0a9a070d92ff05782cbc16bfaea897ed129ecf0))
* **core-volume,app,engine:** a stretched model hollows with the wall and ([792c68d](https://github.com/vvvlladimir/Encrust/commit/792c68d5c69e6fa9abe9e572e0a01e402847d8fa))
* **core-volume,core-slicer,app:** a hollowed cavity is closed and slices ([7cdade6](https://github.com/vvvlladimir/Encrust/commit/7cdade6148b16d20116311bb38647d2a87cb4600))
* **net-sdcp,printer-link,app,cli:** a sent file can be started, and a ([#113](https://github.com/vvvlladimir/Encrust/issues/113)) ([f772046](https://github.com/vvvlladimir/Encrust/commit/f772046d95d7f47815fbeac74c4cdc1246f810ea))
* **printer-profiles,app,cli:** a resin is measured, not shipped, and a copy says where it came from ([#116](https://github.com/vvvlladimir/Encrust/issues/116)) ([6b27403](https://github.com/vvvlladimir/Encrust/commit/6b2740389c72a4ddb969fb8b278e672b3b292657))

## [0.1.0](https://github.com/vvvlladimir/Encrust/compare/v0.0.1...v0.1.0) (2026-10-02)


### 🐛 Fixed

* **ci:** pass on Windows and on every pull request ([857f698](https://github.com/vvvlladimir/Encrust/commit/857f698886574b3406abf4ca52d1da73a0e106fc))

## Changelog

Written by `release-please` from the Conventional Commit subjects on `main`; see
[CONTRIBUTING.md](CONTRIBUTING.md#commit-messages).
