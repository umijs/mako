import {keep1} from './lib'
console.log(keep1(), require('./cjs'))
import {keep2} from './lib'
export default keep2()
export let keep1 = () => 'keep1'
export let keep2 = () => 'keep2'
export let REMOVE = () => 'REMOVE'