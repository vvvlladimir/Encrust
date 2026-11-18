// The connector spike's server (docs/design/ai-integration.md, step 18): a thin MCP server over
// Streamable HTTP for ChatGPT. It computes nothing; its page runs the engine in the browser.
// PUBLIC_URL=https://<tunnel> node server.mjs — see ../README.md. Throwaway code.
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { extname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StreamableHTTPServerTransport } from "@modelcontextprotocol/sdk/server/streamableHttp.js";
import { z } from "zod";

const PORT = Number(process.env.PORT ?? 8787);
const PUBLIC_URL = (process.env.PUBLIC_URL ?? `http://localhost:${PORT}`).replace(/\/$/, "");
const HERE = fileURLToPath(new URL(".", import.meta.url));
const WIDGET_DIR = join(HERE, "../widget");
const WIDGET_URI = "ui://encrust/connector.html";

/// The files the page may fetch from this server, by the path it asks.
const SERVED = {
  "/pkg/connector_engine.js": "pkg/connector_engine.js",
  "/pkg/connector_engine_bg.wasm": "pkg/connector_engine_bg.wasm",
  "/profiles/printer.toml": "printer.toml",
  "/profiles/resin.toml": "resin.toml",
};
const TYPES = { ".js": "text/javascript", ".wasm": "application/wasm", ".toml": "text/plain" };

function mcpServer(widget) {
  const server = new McpServer({ name: "encrust-connector-spike", version: "0.0.0" });
  server.registerResource("connector-page", WIDGET_URI, {}, async () => ({
    contents: [{
      uri: WIDGET_URI,
      mimeType: "text/html+skybridge",
      text: widget,
      _meta: {
        "openai/widgetCSP": {
          connect_domains: [PUBLIC_URL],
          resource_domains: [PUBLIC_URL],
        },
      },
    }],
  }));
  server.registerTool(
    "prepare_model",
    {
      title: "Prepare a model for a resin printer",
      description:
        "Prepares the 3D model the user attached (STL) for their resin printer: orientation and " +
        "supports, checked as it would print. The page shown runs it in the user's browser and " +
        "sends its report back.",
      inputSchema: {
        model: z.object({ download_url: z.string(), file_id: z.string() }),
        printer: z.string().describe("The printer the user named"),
      },
      _meta: {
        "openai/outputTemplate": WIDGET_URI,
        "openai/fileParams": ["model"],
        "openai/widgetAccessible": true,
        "openai/toolInvocation/invoking": "Preparing the model in your browser",
        "openai/toolInvocation/invoked": "The page is preparing the model",
      },
    },
    async ({ model, printer }) => {
      console.log(`prepare_model: file ${model.file_id} for ${printer}`);
      return {
        structuredContent: {
          download_url: model.download_url,
          file_id: model.file_id,
          engine: `${PUBLIC_URL}/pkg/connector_engine.js`,
          printer_toml: `${PUBLIC_URL}/profiles/printer.toml`,
          resin_toml: `${PUBLIC_URL}/profiles/resin.toml`,
        },
        content: [{
          type: "text",
          text: "The page below prepares the model in the user's browser and calls " +
            "report_preparation with what it found; wait for that before answering.",
        }],
      };
    },
  );
  server.registerTool(
    "report_preparation",
    {
      title: "The page's report",
      description: "Called by the page with the prepared model's check; not for the model to call.",
      inputSchema: { report: z.record(z.string(), z.unknown()) },
      _meta: { "openai/widgetAccessible": true },
    },
    async ({ report }) => {
      console.log(`report_preparation: ${JSON.stringify(report)}`);
      return { content: [{ type: "text", text: JSON.stringify(report) }] };
    },
  );
  return server;
}

async function serveFile(path, response) {
  const file = SERVED[path];
  if (!file) {
    response.writeHead(404).end();
    return;
  }
  const body = await readFile(join(WIDGET_DIR, file));
  response.writeHead(200, {
    "content-type": TYPES[extname(file)] ?? "application/octet-stream",
    // The page runs on the host's sandbox origin, not this one.
    "access-control-allow-origin": "*",
  });
  response.end(body);
}

const widget = await readFile(join(WIDGET_DIR, "connector.html"), "utf8");
createServer(async (request, response) => {
  const path = new URL(request.url, PUBLIC_URL).pathname;
  try {
    if (path === "/log" && request.method === "POST") {
      // What the page saw, printed here, since the spike's answers are read off this log.
      let body = "";
      for await (const chunk of request) body += chunk;
      console.log(`page: ${body}`);
      response.writeHead(204, { "access-control-allow-origin": "*" }).end();
    } else if (path === "/log" && request.method === "OPTIONS") {
      response.writeHead(204, {
        "access-control-allow-origin": "*",
        "access-control-allow-headers": "content-type",
      }).end();
    } else if (path === "/mcp") {
      // Stateless: a server and a transport per request, as MCP 2026-07-28 asks nothing more.
      const transport = new StreamableHTTPServerTransport({ sessionIdGenerator: undefined });
      response.on("close", () => transport.close());
      await mcpServer(widget).connect(transport);
      await transport.handleRequest(request, response);
    } else {
      await serveFile(path, response);
    }
  } catch (error) {
    console.error(error);
    if (!response.headersSent) response.writeHead(500).end();
  }
}).listen(PORT, () => console.log(`MCP at ${PUBLIC_URL}/mcp`));
