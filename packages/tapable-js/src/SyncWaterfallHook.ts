import { SyncHook } from "./SyncHook";
import {
  AsArray,
  UnsetAdditionalOptions,
  SyncWaterfallHook as _SyncWaterfallHook,
} from "./tapable";

export class SyncWaterfallHook<
  T,
  AdditionalOptions = UnsetAdditionalOptions,
> extends SyncHook<T, AsArray<T>[0], AdditionalOptions> {
  call(...args: AsArray<T>) {
    throw new Error("Method not implemented.");
  }
}
