// css module
import styles from "./index.css";
console.log(styles);


// non css module
import('./a.css');
require('./b.css');
import "./c.css";
const e = require('./e.css');
import('./f.css').then(f => f);

console.log (e);



