/* tslint:disable */
/* eslint-disable */

/* auto-generated by NAPI-RS */

export interface JsHooks {
  name?: string;
  enforce?: string;
  load?: (
    filePath: string,
  ) => Promise<{ content: string; type: 'css' | 'js' } | void> | void;
  loadInclude?: (filePath: string) => Promise<bool> | bool;
  generateEnd?: (data: {
    isFirstCompile: boolean;
    time: number;
    stats: {
      hash: number;
      builtAt: number;
      rootPath: string;
      outputPath: string;
      assets: { type: string; name: string; path: string; size: number }[];
      chunkModules: {
        type: string;
        id: string;
        chunks: string[];
        size: number;
      }[];
      modules: Record<
        string,
        { id: string; dependents: string[]; dependencies: string[] }
      >;
      chunks: {
        type: string;
        id: string;
        files: string[];
        entry: boolean;
        modules: { type: string; id: string; size: number; chunks: string[] }[];
        siblings: string[];
        origin: {
          module: string;
          moduleIdentifier: string;
          moduleName: string;
          loc: string;
          request: string;
        }[];
      }[];
      entrypoints: Record<string, { name: string; chunks: string[] }>;
      rscClientComponents: { path; string; moduleId: string }[];
      rscCSSModules: { path; string; moduleId: string; modules: boolean }[];
      startTime: number;
      endTime: number;
    };
  }) => void;
  writeBundle?: () => Promise<void>;
  watchChanges?: (
    id: string,
    change: { event: 'create' | 'delete' | 'update' },
  ) => Promise<void> | void;
  onGenerateFile?: (path: string, content: Buffer) => Promise<void>;
  buildStart?: () => Promise<void>;
  buildEnd?: () => Promise<void>;
  resolveId?: (
    source: string,
    importer: string,
    { isEntry: bool },
  ) => Promise<{ id: string }>;
  transform?: (
    content: { content: string; type: 'css' | 'js' },
    path: string,
  ) => Promise<{ content: string; type: 'css' | 'js' } | void> | void;
  transformInclude?: (filePath: string) => Promise<bool> | bool;
}
export interface WriteFile {
  path: string;
  content: Buffer;
}
export interface LoadResult {
  content: string;
  type: string;
}
export interface WatchChangesParams {
  event: string;
}
export interface ResolveIdResult {
  id: string;
  external: boolean | null;
}
export interface ResolveIdParams {
  isEntry: boolean;
}
export interface TransformResult {
  content: string;
  type: string;
}
export interface BuildParams {
  root: string;
  config: {
    entry?: Record<string, string>;
    output?: {
      path: string;
      mode: 'bundle' | 'bundless';
      esVersion?: string;
      meta?: boolean;
      preserveModules?: boolean;
      preserveModulesRoot?: string;
      skipWrite?: boolean;
    };
    resolve?: {
      alias?: Array<[string, string]>;
      extensions?: string[];
    };
    manifest?:
      | false
      | {
          fileName: string;
          basePath: string;
        };
    mode?: 'development' | 'production';
    define?: Record<string, string>;
    devtool?: false | 'source-map' | 'inline-source-map';
    externals?: Record<
      string,
      | string
      | {
          root: string;
          script?: string;
          subpath?: {
            exclude?: string[];
            rules: {
              regex: string;
              target: string | '$EMPTY';
              targetConverter?: 'PascalCase';
            }[];
          };
        }
    >;
    copy?: string[];
    codeSplitting?:
      | false
      | {
          strategy: 'auto';
        }
      | {
          strategy: 'granular';
          options: {
            frameworkPackages: string[];
            libMinSize?: number;
          };
        }
      | {
          strategy: 'advanced';
          options: {
            minSize?: number;
            groups: {
              name: string;
              allowChunks?: 'all' | 'entry' | 'async';
              test?: string;
              minChunks?: number;
              minSize?: number;
              maxSize?: number;
              priority?: number;
            }[];
          };
        };
    providers?: Record<string, string[]>;
    publicPath?: string;
    inlineLimit?: number;
    inlineExcludesExtensions?: string[];
    targets?: Record<string, number>;
    platform?: 'node' | 'browser';
    hmr?: false | {};
    devServer?: false | { host?: string; port?: number };
    px2rem?:
      | false
      | {
          root?: number;
          propBlackList?: string[];
          propWhiteList?: string[];
          selectorBlackList?: string[];
          selectorWhiteList?: string[];
          selectorDoubleList?: string[];
          mediaQuery?: boolean;
        };
    stats?:
      | false
      | {
          modules?: boolean;
        };
    hash?: boolean;
    autoCSSModules?: boolean;
    ignoreCSSParserErrors?: boolean;
    dynamicImportToRequire?: boolean;
    umd?: false | string | { name: string; export?: string[] };
    cjs?: boolean;
    writeToDisk?: boolean;
    transformImport?: {
      libraryName: string;
      libraryDirectory?: string;
      style?: boolean | string;
    }[];
    clean?: boolean;
    nodePolyfill?: boolean;
    ignores?: string[];
    moduleIdStrategy?: 'hashed' | 'named';
    minify?: boolean;
    _minifish?:
      | false
      | {
          mapping: Record<string, string>;
          metaPath?: string;
          inject?: Record<
            string,
            | {
                from: string;
                exclude?: string;
                include?: string;
                preferRequire?: boolean;
              }
            | {
                from: string;
                named: string;
                exclude?: string;
                include?: string;
                preferRequire?: boolean;
              }
            | {
                from: string;
                namespace: true;
                exclude?: string;
                include?: string;
                preferRequire?: boolean;
              }
          >;
        };
    optimization?:
      | false
      | {
          skipModules?: boolean;
          concatenateModules?: boolean;
        };
    react?: {
      runtime?: 'automatic' | 'classic';
      pragma?: string;
      importSource?: string;
      pragmaFrag?: string;
    };
    emitAssets?: boolean;
    cssModulesExportOnlyLocales?: boolean;
    inlineCSS?: false | {};
    rscServer?:
      | false
      | {
          emitCSS: boolean;
          clientComponentTpl: string;
        };
    rscClient?:
      | false
      | {
          logServerComponent: 'error' | 'ignore';
        };
    experimental?: {
      webpackSyntaxValidate?: string[];
      rustPlugins?: Array<[string, any]>;
    };
    watch?: {
      ignoredPaths?: string[];
      _nodeModulesRegexes?: string[];
    };
    caseSensitiveCheck?: boolean;
  };
  plugins: Array<JsHooks>;
  watch: boolean;
}
export declare function build(buildParams: BuildParams): Promise<void>;
export class PluginContext {
  warn(msg: string): void;
  error(msg: string): void;
  emitFile(originPath: string, outputPath: string): void;
}
