import { defineConfig } from 'umi';

export default defineConfig({
  esbuildMinifyIIFE: true,
  codeSplitting: {
    jsStrategy: 'granularChunks',
  },
  // chainWebpack(memo) {
  //   memo.merge({
  //     optimization: {
  //       chunkIds: "named",
  //       moduleIds: "named",
  //     },
  //   });
  // },
});
