import { createServer } from "vite";
import { ViteNodeServer } from "vite-node/server";
import { ViteNodeRunner } from "vite-node/client";
import { installSourcemapsSupport } from "vite-node/source-map";
import {
  createHotContext,
  handleMessage,
  viteNodeHmrPlugin,
} from "vite-node/hmr";

const files = ["./example.ts", "./other.ts"];

// create vite server
const server = await createServer({
  logLevel: 'error',
  optimizeDeps: {
    // It's recommended to disable deps optimization
    disabled: true,
  },
  server: {
    hmr: true,
  },
  plugins: [
    viteNodeHmrPlugin(),
  ],
});
// this is need to initialize the plugins
await server.pluginContainer.buildStart({});

// create vite-node server
const node = new ViteNodeServer(server);

// fixes stacktraces in Errors
installSourcemapsSupport({
  getSourceMap: (source) => node.getSourceMap(source),
});

// create vite-node runner
const runner = new ViteNodeRunner({
  root: server.config.root,
  base: server.config.base,

  createHotContext(runner, url) {
    return createHotContext(runner, server.emitter, files, url);
  },

  // when having the server and runner in a different context,
  // you will need to handle the communication between them
  // and pass to this function
  fetchModule(id) {
    return node.fetchModule(id).then((fr) => {
      return fr;
    });
  },
  resolveId(id, importer) {
    return node.resolveId(id, importer).then((rid) => {
      return rid;
    });
  },
});

for (const file of files) await runner.executeFile(file);

server.emitter?.on("message", (payload: any) => {
  console.log(payload)
  handleMessage(runner, server.emitter, files, payload);
});
