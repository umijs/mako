const { build } = require('../');
const path = require('path');

const root = path.join(__dirname, '../../../examples/with-antd');

build(root, {
  entry: { index: 'index.tsx' },
  output: { path: path.join(root, 'dist') },
  resolve: {
    alias: {},
    extensions: ['.js', '.jsx', '.ts', '.tsx', '.json'],
  },
  mode: 'development',
  sourcemap: true,
  externals: { stream: 'stream' },
  copy: ['public'],
  data_url_limit: 10000,
  public_path: '/',
  devtool: 'source-map',
  targets: {},
});
