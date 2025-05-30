const fs = require('fs');
const path = require('path');

const total = 20;
const content = `import react from 'react';
import reactDom from 'react-dom';
import * as antd from 'antd';
import * as three from 'three';
import * as lodash from 'lodash';
import * as axios from 'axios';

window.React = react;
window.ReactDom = reactDom;
window.Antd = antd;
window.Three = three;
window.Lodash = lodash;
window.Axios = axios;
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
