const React = require('react');
const ReactDOM = require('react-dom/server');
const { createFromFetch } = require('react-server-dom-webpack/client');
const { use } = React;

const dataFromServer = `
J0:["$","div",null,{"children":[["$","h1",null,{"children":"rsccc"}],[["$","p",null,{"children":"Hello"}]]]}]
`;

// (async () => {

const chunk = createFromFetch(
  fetch(`data:text/plain;base64,${btoa(dataFromServer)}`),
);

const Container = () => {
  return use(chunk);
};

// const jsx = ReactDOM.renderToString(React.createElement(Container, null, null));
// console.log(jsx);
ReactDOM.renderToPipeableStream(
  React.createElement(Container, null, null),
  {},
).pipe(process.stdout);
// })();
