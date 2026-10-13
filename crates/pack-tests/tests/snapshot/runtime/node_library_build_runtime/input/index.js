export function answer() {
  return 42;
}

export {
  dependencyUrl,
  dependencyEnv,
  readDependencyUrl,
} from "./meta space % 中文.mjs";

export const entryUrl = import.meta.url;
export const entryEnv = import.meta.env;
export function readEntryUrl() {
  return import.meta.url;
}
