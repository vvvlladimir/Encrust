# The ChatGPT connector spike

Step 18 of `docs/design/ai-integration.md`: whether a connector whose page computes can work
in ChatGPT before step 19 builds one. Throwaway code, outside the workspace: `engine/` is a
wasm module over the cores (an STL in, the orientation search with its finalists printed on
the automatic supports out, as `--prepare` begins), `server/` a thin MCP server over
Streamable HTTP that computes nothing, `widget/` the page ChatGPT shows, `measure/` the
runs in Chromium.

## The four questions

| | Question | How it is answered | Answer |
|---|---|---|---|
| 1 | The page can fetch the attached file's `download_url` | live, in ChatGPT | waiting |
| 2 | A prepare finishes within a minute with no cross-origin isolation | `measure/run.mjs` | **yes** |
| 3 | The report reaches the model in the same turn | live, in ChatGPT | waiting |
| 4 | The file the page offers downloads | live, in ChatGPT | waiting |

Question 2, measured on a Xeon at 2.1 GHz in headless Chromium, the page served with no
COOP/COEP so `crossOriginIsolated` is false and the module runs on one thread:

| Model | Faces | Poses printed | Prepare | Peak wasm memory | Module load |
|---|---|---|---|---|---|
| exposure check (`encrust test-print exposure`) | 3,160 | 16 | 8.4 s | 80 MB | 0.09 s |
| Crustling (`test-prints/crustling.stl`) | 174,942 | 11 | 19.7 s | 69 MB | 0.07 s |

The module is 7.5 MB before `wasm-opt`. A prepare here is the search and its proof; hollowing
and drains are not in it, and a hollowed figure would take longer.

## Running it

Build the engine and put the page's files beside it:

```bash
cd prototypes/connector/engine
CARGO_TARGET_DIR=../../../target/connector cargo build --release --target wasm32-unknown-unknown
wasm-bindgen --target web --out-dir ../widget/pkg \
  ../../../target/connector/wasm32-unknown-unknown/release/connector_engine.wasm
cp ../../../assets/profiles/printers/elegoo-mars-4-ultra.toml ../widget/printer.toml
cp ../../../crates/encrust-cli/tests/fixtures/test-resin.toml ../widget/resin.toml
```

Question 2 and the page's own code, here:

```bash
cd prototypes/connector/measure
node run.mjs ../widget exposure-check.stl crustling.stl   # the models copied into ../widget
node page.mjs ../widget exposure-check.stl                # with a stand-in for window.openai
```

Questions 1, 3 and 4 need ChatGPT with developer mode, on a plan that has it:

1. Start the server and give it a public HTTPS address, for instance a free quick tunnel:

   ```bash
   cd prototypes/connector/server && npm install
   ```

   ```bash
   cloudflared tunnel --url http://localhost:8787
   ```

   ```bash
   PUBLIC_URL=https://<the tunnel's address> node server.mjs
   ```

2. In ChatGPT's settings, turn developer mode on and add a custom MCP server at
   `https://<the tunnel's address>/mcp`, with no authentication.
3. In a new chat with it on, attach an STL and ask to prepare it for an Elegoo Mars 4 Ultra.
4. Read the answers on the page and in the server's log (`page: …` lines): which
   `window.openai` functions there are, whether the file came in, how long the prepare took,
   whether the model answered with the report, and whether the download arrived.

A failed answer to 1, 3 or 4 stops the connector (step 19), and ChatGPT users go through
Codex, which runs the local server today.
