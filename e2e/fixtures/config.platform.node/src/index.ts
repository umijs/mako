import { ENCODING } from './constants';
import fs1 from 'fs';
import fs2 from 'node:fs';
import { readFile } from 'fs/promises';

fs1.readFileSync(__filename, ENCODING);
fs2.readFileSync(__filename, ENCODING);
readFile(__filename, { encoding: ENCODING });

console.log('dirname', __dirname);

require('http2');

try {
  require('crypto')
  hasCrypto = true
} catch {
  hasCrypto = false
}

require('node:sqlite');
