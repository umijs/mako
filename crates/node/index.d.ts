/* tslint:disable */
/* eslint-disable */

/* auto-generated by NAPI-RS */

export function build(
  root: string,
  config: {
    entry?: Record<string, string>;
    output?: { path: string };
    resolve?: {
      alias?: Record<string, string>;
      extensions?: string[];
    };
    mode?: 'development' | 'production';
    devtool?: 'source-map' | 'inline-source-map';
    externals?: Record<string, string>;
    copy?: string[];
    public_path?: string;
    data_url_limit?: number;
    targets?: Record<string, number>;
  },
): void;
