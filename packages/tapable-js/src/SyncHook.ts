import { BaseHook } from "./BaseHook";
import {
  AsArray,
  UnsetAdditionalOptions,
  SyncHook as _SyncHook,
} from "./tapable";

export class SyncHook<T, R = void, AdditionalOptions = UnsetAdditionalOptions>
  extends BaseHook<T, R, AdditionalOptions>
  implements _SyncHook<T, R, AdditionalOptions>
{
  call(...args: AsArray<T>): R {
    throw new Error("Method not implemented.");
  }
}
