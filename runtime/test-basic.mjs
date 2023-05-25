import { createRuntime } from './runtime.mjs';

createRuntime(
  {
    '/entry.js': function (module, exports, __mako_require__) {
      const foo = __mako_require__('/foo.js');
      console.log(`Hello ${foo}`);
    },
    '/foo.js': function (module, exports, __mako_require__) {
      module.exports = 'world';
    },
  },
  '/entry.js',
);
