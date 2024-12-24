const replace = require('unplugin-replace');
const icons = require('unplugin-icons');

module.exports = [
  replace.raw({
    values: [
      {
        find: 'FOOOO',
        replacement: '"fooooooo"',
      },
    ],
  }),
  icons.default.raw({
    compiler: 'jsx',
    jsx: 'react',
    autoInstall: false,
  }),
];
