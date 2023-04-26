export function createRuntime(makoModules, entryModuleId) {
	const modulesRegistry = {};

	function requireModule(moduleId) {
		if (modulesRegistry[moduleId] !== undefined) {
			return modulesRegistry[moduleId].exports;
		}

		const module = {
			exports: {},
		};
		modulesRegistry[moduleId] = module;

		try {
			let execOptions = {
				id: moduleId,
				module,
				factory: makoModules[moduleId],
				require: requireModule,
			};
			hmrHandler(execOptions);
			execOptions.factory(
				execOptions.module,
				execOptions.module.exports,
				execOptions.require
			);
		} catch (e) {
			console.error(`Error require module '${moduleId}':`, e);
			delete modulesRegistry[moduleId];
		}

		return module.exports;
	}

	// hmr
	let currentParents = [];
	let currentChildModule;
	let hmrHandler = (options) => {
		options.module.hot = createModuleHotObject(options.id, options.module);
		options.module.parents = currentParents;
		currentParents = [];
		options.module.children = [];
		options.require = createHmrRequire(options.require, options.id);
	};
	let createHmrRequire = (require, moduleId) => {
		let me = modulesRegistry[moduleId];
		if (!me) return require;
		let fn = (request) => {
			if (me.hot.active) {
				if (modulesRegistry[request]) {
					let parents = modulesRegistry[request].parents;
					if (!parents.includes(moduleId)) {
						parents.push(moduleId);
					}
				} else {
					currentParents = [moduleId];
					currentChildModule = request;
				}
				if (!me.children.includes(request)) {
					me.children.push(request);
				}
			} else {
				// TODO
			}
			return require(request);
		};
		// TODO: fn.ensure
		return fn;
	};
	let createModuleHotObject = (moduleId, me) => {
		let hot = {
			_acceptedDependencies: {},
			_declinedDependencies: {},
			_selfAccepted: false,
			_selfDeclined: false,
			_selfInvalidated: false,
			_disposeHandlers: [],
			_requireSelf: function () {
				currentParents = me.parents.slice();
				requireModule(moduleId);
			},
			active: true,
			accept() {
				this._selfAccepted = true;
			},
			dispose(callback) {
				this._disposeHandlers.push(callback);
			},
			invalidate() {},
			check() {
				fetch("/hot-update.json")
					.then((res) => {
						return res.json();
					})
					.then((update) => {
						if (update) {
							hot.apply(update);
						}
					});
			},
			apply(update) {
				const { modules, removedModules } = update;

				// get outdated modules
				let outdatedModules = [];
				for (let moduleId of Object.keys(modules)) {
					if (!modulesRegistry[moduleId]) continue;
					if (outdatedModules.includes(moduleId)) continue;
					outdatedModules.push(moduleId);
					let queue = [moduleId];
					while (queue.length) {
						let item = queue.pop();
						let module = modulesRegistry[item];
						if (!module) continue;
						if (module.hot._selfAccepted) {
							continue;
						}
						for (let parentModule of module.parents) {
							if (outdatedModules.includes(parentModule)) continue;
							outdatedModules.push(parentModule);
							queue.push(parentModule);
						}
					}
				}

				// get self accepted modules
				let outdatedSelfAcceptedModules = [];
				for (let moduleId of outdatedModules) {
					let module = modulesRegistry[moduleId];
					if (module.hot._selfAccepted) {
						outdatedSelfAcceptedModules.push(module);
					}
				}

				// dispose
				for (let moduleId of outdatedModules) {
					let module = modulesRegistry[moduleId];
					for (let handler of module.hot._disposeHandlers) {
						handler();
					}
					module.hot.active = false;
					delete modulesRegistry[moduleId];
					for (let childModule of module.children) {
						let child = modulesRegistry[childModule];
						if (!child) continue;
						let idx = child.parents.indexOf(moduleId);
						if (idx !== -1) {
							child.parents.splice(idx, 1);
						}
					}
				}

				// apply
				registerModules(modules);
				for (let module of outdatedSelfAcceptedModules) {
					module.hot._requireSelf();
				}
			},
		};
		return hot;
	};

	// chunk and async load
	let installedChunks = {};
	let ensure = (chunkId) => {
		let data = installedChunks[chunkId];
		if (data === 0) return Promise.resolve();
		if (data) {
			// [resolve, reject, promise]
			return data[2];
		} else {
			let promise = new Promise((resolve, reject) => {
				data = installedChunks[chunkId] = [resolve, reject];
			});
			data[2] = promise;
			// TODO: support public path
			let url = `/${chunkId}.async.js`;
			let error = new Error();
			let onLoadEnd = (event) => {
				data = installedChunks[chunkId];
				if (data !== 0) installedChunks[chunkId] = undefined;
				if (data) {
					let errorType = event?.type;
					let src = event?.target?.src;
					error.message = `Loading chunk ${chunkId} failed. (${errorType} : ${src})`;
					error.name = "ChunkLoadError";
					error.type = errorType;
					data[1](error);
				}
			};
			// load
			load(url, onLoadEnd, `chunk-${chunkId}`);
			return promise;
		}
	};

	let inProgress = {};
	let load = (url, done, key) => {
		if (inProgress[url]) {
			return inProgress[url].push(done);
		}
		let script = document.createElement("script");
		script.timeout = 120;
		script.src = url;
		inProgress[url] = [done];
		let onLoadEnd = (prev, event) => {
			clearTimeout(timeout);
			let doneFns = inProgress[url];
			delete inProgress[url];
			script.parentNode?.removeChild(script);
			doneFns &&
				doneFns.forEach(function (fn) {
					return fn(event);
				});
			if (prev) return prev(event);
		};
		// 可能不需要，有 timeout 属性了
		let timeout = setTimeout(
			onLoadEnd.bind(null, undefined, { type: "timeout", target: script }),
			120000
		);
		script.onerror = onLoadEnd.bind(null, script.onerror);
		script.onload = onLoadEnd.bind(null, script.onload);
		document.head.appendChild(script);
	};

	let jsonpCallback = (data) => {
		let chunkIds = data[0];
		let modules = data[1];
		if (chunkIds.some((id) => installedChunks[id] !== 0)) {
			registerModules(modules);
		}
		for (let id of chunkIds) {
			if (installedChunks[id]) {
				installedChunks[id][0]();
			}
			installedChunks[id] = 0;
		}
	};

	let registerModules = (modules) => {
		for (let id in modules) {
			makoModules[id] = modules[id];
		}
	};

	requireModule(entryModuleId);
	requireModule.ensure = ensure;

	return {
		requireModule,
		_modulesRegistry: modulesRegistry,
		_jsonpCallback: jsonpCallback,
	};
}
