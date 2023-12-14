export default {
  mfsu: false,
  extraBabelPlugins: [
    [
      require.resolve('babel-plugin-import'),
      {
        libraryName: 'antd',
      },
    ],
    [
      'import',
      {
        libraryName: 'antd1',
        libraryDirectory: 'es',
        style: true,
      },
    ],
  ],
};
