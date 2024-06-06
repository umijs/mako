export default (v: any) => console.log(v);

import("./workerHelper").then((w) => w.startWorkers());
