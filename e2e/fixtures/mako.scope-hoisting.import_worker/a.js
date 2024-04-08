import foo from "./worker_dep.js"


new Worker(new URL("./worker", import.meta.url))


console.log("foo", foo);

