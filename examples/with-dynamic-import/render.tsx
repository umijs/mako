import * as React from 'react';
import * as ReactDOM from 'react-dom/client';
import './index.css';
import Local from './local';

const Lazy = React.lazy(async () => {
  return await import('./lazy');
});

function App() {
  return (
    <div>
      <Local />
      <Lazy />
    </div>
  );
}

let root = ReactDOM.createRoot(document.getElementById('root')!);

function render() {
  root.render(<App />);
}

console.log('start render');
render();

module.hot.accept();
module.hot.dispose(() => {
  root.unmount();
});
