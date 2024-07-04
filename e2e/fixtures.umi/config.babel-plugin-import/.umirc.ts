export default {
  mfsu: false,
  // mako: {},
  extraBabelPlugins: [
    [
      require.resolve('babel-plugin-import1'),
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
