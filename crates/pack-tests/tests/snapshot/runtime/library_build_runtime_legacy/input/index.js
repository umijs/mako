import asset from "./asset.svg";

export { asset };
export const read = (value) => value?.answer ?? 42;
export const flag = 0b001;
export const load = () => import("./lazy").then((module) => module.answer);
export function last(node) {
  do {
    node = node.next;
  } while (node.next);
  return node.value;
}
