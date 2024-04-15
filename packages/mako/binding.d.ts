/* tslint:disable */
/* eslint-disable */

/* auto-generated by NAPI-RS */

export interface JsHooks {
  load?: (filePath: string) => Promise<{ content: string; type: 'css' | 'js' }>;
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
      asciiOnly?: boolean;
      skipWrite?: boolean;
    };
    resolve?: {
      alias?: Record<string, string>;
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
    codeSplitting?: false | 'auto';
    providers?: Record<string, string[]>;
    publicPath?: string;
    inlineLimit?: number;
    targets?: Record<string, number>;
    platform?: 'node' | 'browser';
    hmr?: false | { host?: string; port?: number };
    px2rem?:
      | false
      | {
          root?: number;
          propBlackList?: string[];
          propWhiteList?: string[];
          selectorBlackList?: string[];
          selectorWhiteList?: string[];
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
    emitAssets: boolean;
    cssModulesExportOnlyLocales: boolean;
    inlineCSS?: false | {};
  };
  hooks: JsHooks;
  watch: boolean;
}
export function build(buildParams: BuildParams): Promise<void>;
