const fs = require('fs');
const path = require('path');

const total = 20;
const content = `
import react from 'react';
import reactDom from 'react-dom';
import three from 'three';
react, reactDom, three;
`;
const makoConfig = { entry: {} };

for (let i = 0; i < total; i++) {
  const name = `entry-${i}`;
  const filename = `${name}.js`;
  makoConfig.entry[name] = filename;
  fs.writeFileSync(path.join(__dirname, filename), content);
}
fs.writeFileSync(
  path.join(__dirname, 'mako.config.json'),
  JSON.stringify(makoConfig, null, 2) + '\n',
);
