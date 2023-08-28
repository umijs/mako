const BundleAnalyzerPlugin =
  require('webpack-bundle-analyzer').BundleAnalyzerPlugin;
const path = require('path');

module.exports = {
  mode: 'production',
  entry: './index.js',
  optimization: {
    concatenateModules: false,
    moduleIds: 'named',
  },
  plugins: [new BundleAnalyzerPlugin()],
  output: {
    filename: 'webpack-dist.js',
    path: path.resolve(__dirname, 'dist'),
  },
};
