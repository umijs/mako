const meta = import.meta;

export const dependencyUrl = meta.url;
export const dependencyEnv = meta.env;
export const answer = 42;
export function readDependencyUrl() {
  return meta.url;
}
