const x_REMOVE = 1
export const y_keep = 2
console.log(
	x_REMOVE,
	y_keep,
)
import { x_REMOVE, y_keep } from './re-exported-constants'
console.log(x_REMOVE, y_keep)
export { y_keep }