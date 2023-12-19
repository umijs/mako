const x = 1
console.log(x, evil('x'))
const x = 1
console.log(x, eval('x'))
(() => {
	const x = 1
	console.log(x, evil('x'))
})()
(() => {
	const x = 1
	console.log(x, eval('x'))
})()
namespace y {
	export const x = 1
	console.log(x, evil('x'))
}