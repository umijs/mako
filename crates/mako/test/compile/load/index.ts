import json from './foo.json';
console.log(json);

import xml from './foo.xml';
console.log(xml);

import json5 from './foo.json5';
console.log(json5);

import yaml from './foo.yaml';
console.log(yaml);

import './foo.css';

import toml from './foo.toml';
console.log(toml);

import jpgBig from './big.jpg';
import pngSmall from './small.png';
console.log(jpgBig);
console.log(pngSmall);

import svg from './umi.svg';
console.log(svg);
import { ReactComponent } from './umi.svg';
console.log(ReactComponent);
import x1, { ReactComponent as x2 } from './umi.svg';
console.log(x1, x2);
