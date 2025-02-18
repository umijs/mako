import {
  Append,
  AsArray,
  Callback,
  FullTap,
  Hook as _Hook,
  HookInterceptor,
  IfSet,
  TapOptions,
  UnsetAdditionalOptions,
} from "./tapable";

export abstract class BaseHook<T, R, AdditionalOptions = UnsetAdditionalOptions>
  implements _Hook<T, R, AdditionalOptions>
{
  public name: string | undefined;
  public taps: (FullTap & IfSet<AdditionalOptions>)[];

  private interceptors: HookInterceptor<T, R, AdditionalOptions>[] = [];

  public intercept(
    interceptor: HookInterceptor<T, R, AdditionalOptions>,
  ): void {
    this.interceptors.push(Object.assign({}, interceptor));
    if (interceptor.register) {
      for (let i = 0; i < this.taps.length; i++) {
        this.taps[i] = interceptor.register(this.taps[i]);
      }
    }
  }

  public isUsed(): boolean {
    return this.taps.length > 0 || this.interceptors.length > 0;
  }

  public callAsync(...args: Append<AsArray<T>, Callback<Error, R>>): void {}

  public promise(...args: AsArray<T>): Promise<R> {
    throw new Error("Method not implemented.");
  }

  public tapAsync(
    opt: string | (TapOptions & { name: string } & IfSet<AdditionalOptions>),
    fn: (...args: AsArray<T>) => R,
  ) {
    throw new Error("Method not implemented.");
  }

  public tapPromise(
    opt: string | (TapOptions & { name: string } & IfSet<AdditionalOptions>),
    fn: (...args: AsArray<T>) => R,
  ) {
    throw new Error("Method not implemented.");
  }

  public tap(
    options:
      | string
      | (TapOptions & { name: string } & IfSet<AdditionalOptions>),
    fn: (...args: AsArray<T>) => R,
  ): void {
    this._tap("sync", options, fn);
  }

  public withOptions(
    options: TapOptions & IfSet<AdditionalOptions>,
  ): Omit<this, "call" | "callAsync" | "promise"> {
    const mergeOptions = <O>(opt: O) =>
      Object.assign({}, options, typeof opt === "string" ? { name: opt } : opt);

    return {
      taps: this.taps,
      tap: (
        opt:
          | string
          | (TapOptions & { name: string } & IfSet<AdditionalOptions>),
        fn: (...args: AsArray<T>) => R,
      ) => this.tap(mergeOptions(opt), fn),
      tapAsync: (
        opt:
          | string
          | (TapOptions & { name: string } & IfSet<AdditionalOptions>),
        fn: (...args: AsArray<T>) => R,
      ) => this.tapAsync(mergeOptions(opt), fn),
      tapPromise: (
        opt:
          | string
          | (TapOptions & { name: string } & IfSet<AdditionalOptions>),
        fn: (...args: AsArray<T>) => R,
      ) => this.tapPromise(mergeOptions(opt), fn),
      intercept: (interceptor) => this.intercept(interceptor),
      isUsed: () => this.isUsed(),
      withOptions: (
        opt: TapOptions & { name: string } & IfSet<AdditionalOptions>,
      ) => this.withOptions(mergeOptions(opt)),
    } as unknown as Omit<this, "call" | "callAsync" | "promise">;
  }

  private _tap(
    type: FullTap["type"],
    options:
      | string
      | (TapOptions & { name: string } & IfSet<AdditionalOptions>),
    fn: (...args: AsArray<T>) => R,
  ) {
    let fullOptions = {} as unknown as FullTap & IfSet<AdditionalOptions>;

    if (typeof options === "string") {
      fullOptions.name = options;
    } else if (typeof options !== "object" || options === null) {
      throw new Error("Invalid tap options");
    } else if (typeof options.name !== "string" || options.name === "") {
      throw new Error("Missing name for tap");
    }

    fullOptions = this._runRegisterInterceptors(
      Object.assign(fullOptions, { type, fn }, options),
    );

    this._insert(fullOptions);
  }

  private _runRegisterInterceptors(
    options: FullTap & IfSet<AdditionalOptions>,
  ) {
    for (const interceptor of this.interceptors) {
      if (interceptor.register) {
        const newOptions = interceptor.register(options);
        if (newOptions !== undefined) {
          options = newOptions;
        }
      }
    }
    return options;
  }

  private _insert(item: FullTap & IfSet<AdditionalOptions>) {
    let before: Set<string> | undefined;
    if (typeof item.before === "string") {
      before = new Set([item.before]);
    } else if (Array.isArray(item.before)) {
      before = new Set(item.before);
    }
    let stage = 0;
    if (typeof item.stage === "number") {
      stage = item.stage;
    }
    let i = this.taps.length;
    while (i > 0) {
      i--;
      const x = this.taps[i];
      this.taps[i + 1] = x;
      const xStage = x.stage || 0;
      if (before) {
        if (before.has(x.name)) {
          before.delete(x.name);
          continue;
        }
        if (before.size > 0) {
          continue;
        }
      }
      if (xStage > stage) {
        continue;
      }
      i++;
      break;
    }
    this.taps[i] = item;
  }
}
