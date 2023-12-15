const x_REMOVE = 1
export const y_keep = 2
console.log(
	x_REMOVE,
	y_keep,
)
import { x_REMOVE, y_keep } from './re-exported-constants'
console.log(x_REMOVE, y_keep)
export { y_keep }
export const x_REMOVE = 1
export const y_keep = 2
export { y_keep } from './re-exported-2-constants'
export const x_REMOVE = 1
export const y_keep = 2
export * from './re-exported-star-constants'
export const x_keep = 1
export const y_keep = 2
import { x_REMOVE, y_keep } from './cross-module-constants'
console.log(x_REMOVE, y_keep)
export const x_REMOVE = 1
foo()
export const y_keep = 1
export function foo() {
	return [x_REMOVE, y_keep]
}
import { foo, _bar } from './print-shorthand-constants'
// The inlined constants must still be present in the output! We don't
// want the printer to use the shorthand syntax here to refer to the
// name of the constant itself because the constant declaration is omitted.
console.log({ foo, _bar })
export const foo = 123
export const _bar = -321
import './circular-import-constants'