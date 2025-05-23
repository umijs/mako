// import { bar } from 'opted-out-external-package';
import Word1 from 'esm-package-1/entry'
// const { bar } = require('opted-out-external-package');


export default function Index() {
    return Word1 + 'bar';
}