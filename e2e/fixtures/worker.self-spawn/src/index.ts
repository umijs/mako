export default (v: any) => console.log(v);

!!self.document && import("./workerHelper").then((w) => w.startWorkers());
