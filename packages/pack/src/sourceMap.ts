import {
  findSourceMap as nativeFindSourceMap,
  type SourceMapPayload,
} from "module";

/**
 * https://tc39.es/source-map/#index-map
 */
interface IndexSourceMapSection {
  offset: {
    line: number;
    column: number;
  };
  map: ModernRawSourceMap;
}

interface IndexSourceMap {
  version: number;
  file: string;
  sections: IndexSourceMapSection[];
}

interface ModernRawSourceMap extends SourceMapPayload {
  ignoreList?: number[];
}

export type ModernSourceMapPayload = ModernRawSourceMap | IndexSourceMap;
