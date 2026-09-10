/*! @license Library runtime target fixture */
import asset from "./asset.svg";

export { asset };
export const read = (value) => value?.answer ?? 42;
export function load() {
  return import("./lazy").then((module) => module.answer);
}

export function fail() {
  throw new Error("target-map");
}

/*! @license Library runtime target fixture */
Object.defineProperty(read, "fixture", { value: true });
