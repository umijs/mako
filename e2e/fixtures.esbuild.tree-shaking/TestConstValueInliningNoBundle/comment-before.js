// These should be kept because they are top-level and tree shaking is not enabled
const n_keep = null
const u_keep = undefined
const i_keep = 1234567
const f_keep = 123.456
const s_keep = ''

// Values should still be inlined
console.log(
	// These are doubled to avoid the "inline const/let into next statement if used once" optimization
	n_keep, n_keep,
	u_keep, u_keep,
	i_keep, i_keep,
	f_keep, f_keep,
	s_keep, s_keep,
)
{
	const REMOVE_n = null
	const REMOVE_u = undefined
	const REMOVE_i = 1234567
	const REMOVE_f = 123.456
	const s_keep = '' // String inlining is intentionally not supported right now
	console.log(
		// These are doubled to avoid the "inline const/let into next statement if used once" optimization
		REMOVE_n, REMOVE_n,
		REMOVE_u, REMOVE_u,
		REMOVE_i, REMOVE_i,
		REMOVE_f, REMOVE_f,
		s_keep, s_keep,
	)
}
function nested() {
	const REMOVE_n = null
	const REMOVE_u = undefined
	const REMOVE_i = 1234567
	const REMOVE_f = 123.456
	const s_keep = '' // String inlining is intentionally not supported right now
	console.log(
		// These are doubled to avoid the "inline const/let into next statement if used once" optimization
		REMOVE_n, REMOVE_n,
		REMOVE_u, REMOVE_u,
		REMOVE_i, REMOVE_i,
		REMOVE_f, REMOVE_f,
		s_keep, s_keep,
	)
}
namespace ns {
	const x_REMOVE = 1
	export const y_keep = 2
	console.log(
		x_REMOVE, x_REMOVE,
		y_keep, y_keep,
	)
}
{
	//! comment
	const REMOVE = 1
	x = [REMOVE, REMOVE]
}