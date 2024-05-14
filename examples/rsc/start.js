const { Writable } = require('stream');
const server = require('./server');
const serverRuntime = require('./server/runtime');
// const ReactDOM = require('react-dom/server');
// const React = require('react');

const { rsdws, rsdwc, React, ReactDOM } = serverRuntime;
const { renderToPipeableStream } = rsdws;
const { createFromFetch } = rsdwc;

const clientStats = require('./dist/stats.json');
const { parseClientStats } = require('@makojs/rsc');
const { clientComponents } = parseClientStats(clientStats);

// console.log(clientComponents);

/**
{
  'src/Foo.tsx': {
    '*': {
      id: 'src/Foo.tsx',
      name: '*',
      chunks: [],
    },
  },
}
 */

(async () => {
  const chunks = await new Promise((resolve) => {
    const stream = renderToPipeableStream(
      React.createElement(server.default),
      clientComponents,
    );
    let chunks = [];
    const writable = new Writable({
      write(chunk, encoding, callback) {
        chunks.push(chunk.toString());
        callback();
      },
      final(callback) {
        callback();
        resolve(chunks.join(''));
      },
    });
    stream.pipe(writable);
  });
  console.log(chunks);

  global.__webpack_chunk_load__ = (chunkId) => {
    return new Promise((resolve) => {
      console.log('> load', chunkId);
      require('./dist');
      require(`./dist/${chunkId}`);
      console.log(globalThis.runtime);
      resolve();
    });
  };

  global.__webpack_require__ = (moduleId) => {
    console.log('> require', moduleId);
    const module = globalThis.__mako_require_module__(moduleId);
    return module.default;
    // return () => React.createElement('div', null, moduleId);
  };

  const chunk = createFromFetch(
    fetch(`data:text/plain;base64,${btoa(chunks)}`),
  );
  const App = () => {
    return React.use(chunk);
  };
  ReactDOM.renderToPipeableStream(
    React.createElement(App, null, null),
    {},
  ).pipe(process.stdout);
})();

// Notice: should not trim()
// const rscStr1 = `
// M1:{"id":"src/Foo.tsx","name":"*","chunks":[]}
// J0:["$","div",null,{"children":[["$","h1",null,{"children":"App"}],["$","@1",null,{}]]}]
// `;
// console.log(createFromFetch);
