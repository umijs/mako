/* tslint:disable */
/* eslint-disable */

/* auto-generated by NAPI-RS */

export interface JsHooks {
  name?: string;
  load?: (
    filePath: string,
  ) => Promise<{ content: string; type: 'css' | 'js' } | void> | void;
  generateEnd?: (data: {
    isFirstCompile: boolean;
    time: number;
    stats: {
      startTime: number;
      endTime: number;
    };
  }) => void;
  onGenerateFile?: (path: string, content: Buffer) => Promise<void>;
  buildStart?: () => Promise<void>;
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
        };
    stats?: boolean;
    hash?: boolean;
    autoCSSModules?: boolean;
    ignoreCSSParserErrors?: boolean;
    dynamicImportToRequire?: boolean;
    umd?: false | string;
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
    };
    watch?: {
      ignoredPaths?: string[];
      nodeModulesRegexes?: string[];
    };
  };
  plugins: Array<JsHooks>;
  watch: boolean;
}
export function build(buildParams: BuildParams): Promise<void>;
