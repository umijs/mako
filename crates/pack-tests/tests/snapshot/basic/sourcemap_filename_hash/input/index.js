import "./initial.css";

globalThis.loadFirst = () => import("./lazy-one.js");
globalThis.loadSecond = () => import("./lazy-two.js");
