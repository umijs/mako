const replace = require('unplugin-replace');

module.exports = [
  replace.raw({
    values: [
      {
        find: 'FOOOO',
        replacement: '"fooooooo"',
      },
    ],
  }),
];
