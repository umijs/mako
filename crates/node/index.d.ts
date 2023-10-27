/* tslint:disable */
/* eslint-disable */

/* auto-generated by NAPI-RS */

export function build(root: string, config:
{
entry?: Record<string, string>;
output?: {path: string; mode: "bundle" | "minifish" ;  esVersion?: string, };
resolve?: {
alias?: Record<string, string>;
extensions?: string[];
};
manifest?: boolean;
manifestConfig?: {
fileName: string;
basePath: string;
};
mode?: "development" | "production";
define?: Record<string, string>;
devtool?: "source-map" | "inline-source-map" | "none";
externals?: Record<
string,
string | {
root: string;
subpath: {
exclude?: string[];
rules: {
regex: string;
target: string | '$EMPTY';
targetConverter?: 'PascalCase';
}[];
};
},
>;
copy?: string[];
code_splitting: "auto" | "none";
providers?: Record<string, string[]>;
publicPath?: string;
inlineLimit?: number;
targets?: Record<string, number>;
platform?: "node" | "browser";
hmr?: boolean;
hmrPort?: string;
hmrHost?: string;
px2rem?: boolean;
px2remConfig?: {
root: number;
propBlackList: string[];
propWhiteList: string[];
selectorBlackList: string[];
selectorWhiteList: string[];
};
stats?: boolean;
hash?: boolean;
autoCssModules?: boolean;
ignoreCSSParserErrors?: boolean;
dynamicImportToRequire?: boolean;
umd?: string;
transformImport?: { library: string; libraryDirectory?: string; style?: boolean | string }[];
}, watch: boolean): Promise<void>
