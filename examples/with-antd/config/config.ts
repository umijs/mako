import { defineConfig } from 'umi';

export default defineConfig({
  esbuildMinifyIIFE: true,
  codeSplitting: {
    jsStrategy: 'granularChunks',
  },
});
