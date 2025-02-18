import { SyncHook } from "./SyncHook";
import {
  AsArray,
  UnsetAdditionalOptions,
  SyncBailHook as _SyncBailHook,
} from "./tapable";

export class SyncBailHook<
  T,
  R = void,
  AdditionalOptions = UnsetAdditionalOptions,
> extends SyncHook<T, R, AdditionalOptions> {
  call(...args: AsArray<T>): R {
    throw new Error("Method not implemented.");
  }
}
