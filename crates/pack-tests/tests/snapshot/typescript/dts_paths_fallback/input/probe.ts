import { getMessage as exact } from "dts-exact";
import { getMessage as wildcard } from "dts-wild/feature";
import { getMessage as main } from "dts-main";
import { getMessage as mixed } from "dts-mixed";
import * as empty from "empty-path";

export function probe() {
  return {
    exact: exact(),
    wildcard: wildcard(),
    main: main(),
    mixed: mixed(),
    empty: typeof empty,
  };
}
