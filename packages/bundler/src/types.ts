import {
  HmrIdentifiers,
  NapiEntryOptions,
  NapiEntrypoints,
  NapiIssue,
  NapiUpdateMessage,
  NapiWrittenEndpoint,
  StackFrame,
} from "./binding";

export interface BaseUpdate {
  resource: {
    headers: unknown;
    path: string;
  };
  diagnostics: unknown[];
  issues: NapiIssue[];
}

export interface IssuesUpdate extends BaseUpdate {
  type: "issues";
}

export interface EcmascriptMergedUpdate {
  type: "EcmascriptMergedUpdate";
  chunks: { [moduleName: string]: { type: "partial" } };
  entries: { [moduleName: string]: { code: string; map: string; url: string } };
}

export interface PartialUpdate extends BaseUpdate {
  type: "partial";
  instruction: {
    type: "ChunkListUpdate";
    merged: EcmascriptMergedUpdate[] | undefined;
  };
}

export type Update = IssuesUpdate | PartialUpdate;

export type RustifiedEnv = { name: string; value: string }[];

export interface DefineEnv {
  client: RustifiedEnv;
  edge: RustifiedEnv;
  nodejs: RustifiedEnv;
}

export interface ExperimentalConfig {}

export type TurbopackRuleConfigItemOrShortcut = TurbopackRuleConfigItem;

export type TurbopackRuleConfigItem =
  | TurbopackRuleConfigItemOptions
  | { [condition: string]: TurbopackRuleConfigItem }
  | false;

export type TurbopackRuleConfigItemOptions = {
  as?: string;
};

export interface ModuleOptions {
  rules?: Record<string, TurbopackRuleConfigItemOrShortcut>;
}

export interface ResolveOptions {
  alias?: Record<string, string | string[] | Record<string, string | string[]>>;
  extensions?: string[];
}

export interface ConfigComplete {
  module?: ModuleOptions;
  resolve?: ResolveOptions;
  output?: {
    path?: string;
    type?: "standalone" | "export";
  };
  sourceMaps?: boolean;
  optimization?: {
    moduleIds?: "named" | "deterministic";
    minify?: boolean;
    treeShaking?: boolean;
    modularizeImports?: Record<
      string,
      {
        transform: string | Record<string, string>;
        preventFullImport?: boolean;
        skipDefaultConversion?: boolean;
      }
    >;
    packageImports?: string[];
    transpilePackages?: string[];
    image?: {
      inlineLimit?: number;
    };
  };
  defineEnv: Record<string, string | undefined>;
  sassOptions?: {
    implementation?: string;
    [key: string]: any;
  };
  lessOptions?: {
    implementation?: string;
    [key: string]: any;
  };
  styleOptions?: {
    [key: string]: any;
  };
  serverExternalPackages?: string[];
  compiler?: {
    removeConsole?:
      | boolean
      | {
          exclude?: string[];
        };
    styledComponents?: boolean | StyledComponentsConfig;
    emotion?: boolean | EmotionConfig;

    styledJsx?:
      | boolean
      | {
          useLightningcss?: boolean;
        };

    /**
     * Replaces variables in your code during compile time. Each key will be
     * replaced with the respective values.
     */
    define?: Record<string, string>;
  };
  experimental?: ExperimentalConfig;
  persistentCaching?: boolean;
  cacheHandler?: string;
}

export interface StyledComponentsConfig {
  /**
   * Enabled by default in development, disabled in production to reduce file size,
   * setting this will override the default for all environments.
   */
  displayName?: boolean;
  topLevelImportPaths?: string[];
  ssr?: boolean;
  fileName?: boolean;
  meaninglessFileNames?: string[];
  minify?: boolean;
  transpileTemplateLiterals?: boolean;
  namespace?: string;
  pure?: boolean;
  cssProp?: boolean;
}

export interface EmotionConfig {
  sourceMap?: boolean;
  autoLabel?: "dev-only" | "always" | "never";
  labelFormat?: string;
  importMap?: {
    [importName: string]: {
      [exportName: string]: {
        canonicalImport?: [string, string];
        styledBaseImport?: [string, string];
      };
    };
  };
}

export type JSONValue =
  | string
  | number
  | boolean
  | JSONValue[]
  | { [k: string]: JSONValue };

export type TurboLoaderItem =
  | string
  | {
      loader: string;
      // At the moment, Turbopack options must be JSON-serializable, so restrict values.
      options: Record<string, JSONValue>;
    };

export type TurboRuleConfigItemOrShortcut =
  | TurboLoaderItem[]
  | TurboRuleConfigItem;

export type TurboRuleConfigItemOptions = {
  loaders: TurboLoaderItem[];
  as?: string;
};

export type TurboRuleConfigItem =
  | TurboRuleConfigItemOptions
  | { [condition: string]: TurboRuleConfigItem }
  | false;

export interface ProjectOptions {
  /**
   * A root path from which all files must be nested under. Trying to access
   * a file outside this root will fail. Think of this as a chroot.
   */
  rootPath: string;

  /**
   * A path inside the root_path which contains the app/pages directories.
   */
  projectPath: string;

  entry: NapiEntryOptions[];

  /**
   * The path to the .next directory.
   */
  distDir: string;

  /**
   * The next.config.js contents.
   */
  config: ConfigComplete;

  /**
   * Jsconfig, or tsconfig contents.
   *
   * Next.js implicitly requires to read it to support few options
   * https://nextjs.org/docs/architecture/nextjs-compiler#legacy-decorators
   * https://nextjs.org/docs/architecture/nextjs-compiler#importsource
   */
  jsConfig: {
    compilerOptions: object;
  };

  /**
   * A map of environment variables to use when compiling code.
   */
  env: Record<string, string>;

  defineEnv: DefineEnv;

  /**
   * Whether to watch the filesystem for file changes.
   */
  watch: {
    enable: boolean;
    pollIntervalMs?: number;
  };

  /**
   * The mode in which Next.js is running.
   */
  dev: boolean;

  /**
   * The build id.
   */
  buildId: string;

  /**
   * The browserslist query to use for targeting browsers.
   */
  browserslistQuery: string;

  /**
   * When the code is minified, this opts out of the default mangling of local
   * names for variables, functions etc., which can be useful for
   * debugging/profiling purposes.
   */
  noMangling: boolean;
}

export interface Project {
  update(options: Partial<ProjectOptions>): Promise<void>;

  entrypointsSubscribe(): AsyncIterableIterator<
    TurbopackResult<RawEntrypoints>
  >;

  hmrEvents(identifier: string): AsyncIterableIterator<TurbopackResult<Update>>;

  hmrIdentifiersSubscribe(): AsyncIterableIterator<
    TurbopackResult<HmrIdentifiers>
  >;

  getSourceForAsset(filePath: string): Promise<string | null>;

  getSourceMap(filePath: string): Promise<string | null>;
  getSourceMapSync(filePath: string): string | null;

  traceSource(
    stackFrame: StackFrame,
    currentDirectoryFileUrl: string,
  ): Promise<StackFrame | null>;

  updateInfoSubscribe(
    aggregationMs: number,
  ): AsyncIterableIterator<TurbopackResult<NapiUpdateMessage>>;

  shutdown(): Promise<void>;

  onExit(): Promise<void>;
}

export interface RawEntrypoints {
  libraries?: Endpoint[];
}

export interface Endpoint {
  /** Write files for the endpoint to disk. */
  writeToDisk(): Promise<TurbopackResult<NapiWrittenEndpoint>>;

  /**
   * Listen to client-side changes to the endpoint.
   * After clientChanged() has been awaited it will listen to changes.
   * The async iterator will yield for each change.
   */
  clientChanged(): Promise<AsyncIterableIterator<TurbopackResult>>;

  /**
   * Listen to server-side changes to the endpoint.
   * After serverChanged() has been awaited it will listen to changes.
   * The async iterator will yield for each change.
   */
  serverChanged(
    includeIssues: boolean,
  ): Promise<AsyncIterableIterator<TurbopackResult>>;
}

export type StyledString =
  | {
      type: "text";
      value: string;
    }
  | {
      type: "code";
      value: string;
    }
  | {
      type: "strong";
      value: string;
    }
  | {
      type: "stack";
      value: StyledString[];
    }
  | {
      type: "line";
      value: StyledString[];
    };
