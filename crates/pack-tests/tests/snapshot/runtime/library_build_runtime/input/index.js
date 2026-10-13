console.log('Hello, world!')

export {
  dependencyUrl,
  dependencyEnv,
  readDependencyUrl,
  answer,
} from "./meta space % 中文.mjs";

const meta = import.meta;

export const entryUrl = meta.url;
export const entryEnv = meta.env;
export function readEntryUrl() {
  return meta.url;
}
