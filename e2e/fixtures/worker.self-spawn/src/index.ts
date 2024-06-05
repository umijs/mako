import {
  ForceAtlas2Layout,
  initThreads,
  supportsThreads,
} from "@antv/layout-wasm";
import { Graph, register, ExtensionCategory } from "@antv/g6";

register(ExtensionCategory.LAYOUT, "forceatlas2-wasm", ForceAtlas2Layout);

const supported = await supportsThreads();
const threads = await initThreads(supported);

const graph = new Graph({
  layout: {
    type: "forceatlas2-wasm",
    threads,
  },
});

graph.render();
