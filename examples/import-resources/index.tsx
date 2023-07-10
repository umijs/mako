import React, { useEffect, useState } from 'react';
import ReactDOM from 'react-dom/client';
import json, { hello } from './index.json5';
import toml, { title } from './index.toml';
import yaml, { pi } from './index.yaml';
import xml from './index.xml';
import { add } from './add.wasm';

const num1 = 10;
const num2 = 20;

function App() {
  const [sum, setSum] = useState(0);

  useEffect(() => {
    setSum(add(num1, num2));
  }, []);

  return (
    <div>
      <h2>
        Test import .wasm file async: {num1} + {num2} = {sum}
      </h2>
      <div>
        <h2>Test import .toml file</h2>
        <pre>{JSON.stringify(toml, null, 2)}</pre>
        <p>The toml.title is {title}</p>
      </div>
      <div>
        <h2>Test import .yaml file</h2>
        <pre>{JSON.stringify(yaml, null, 2)}</pre>
        <p>The yaml.pi is {pi}</p>
      </div>
      <div>
        <h2>Test import .json5 file</h2>
        <pre>{JSON.stringify(json, null, 2)}</pre>
        <p>The json.hello is {hello}</p>
      </div>
      <div>
        <h2>Test import .xml file</h2>
        <pre>{JSON.stringify(xml, null, 2)}</pre>
      </div>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById('root')!).render(<App />);
