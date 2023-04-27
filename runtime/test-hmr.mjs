import { createRuntime } from "./runtime.mjs";

const { _modulesRegistry } = createRuntime(
	{
		"/entry.js": function (module, exports, __mako_require__) {
			const foo = __mako_require__("/foo.js");
			console.log(`Hello ${foo}`);
			module.hot.accept();
		},
		"/foo.js": function (module, exports, __mako_require__) {
			module.exports = "world";
		},
	},
	"/entry.js"
);

console.log("Simulating an update...");
_modulesRegistry["/entry.js"].hot.apply({
	modules: {
		"/foo.js": function (module, exports, __mako_require__) {
			module.exports = "updated world";
		},
	},
});
