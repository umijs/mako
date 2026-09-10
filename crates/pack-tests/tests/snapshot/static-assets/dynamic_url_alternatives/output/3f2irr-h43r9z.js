(globalThis["TURBOPACK"] || (globalThis["TURBOPACK"] = [])).push([typeof document === "object" ? document.currentScript : undefined,
"[project]/static-assets/dynamic_url_alternatives/input/index.js [client] (ecmascript)", ((__turbopack_context__, module, exports) => {

var __TURBOPACK__import$2e$meta__ = {
    get url () {
        return __turbopack_context__.F("input/index.js");
    },
    env: {
        DEV: false,
        PROD: true,
        MODE: "production",
        BASE_URL: "/",
        SSR: false
    }
};
globalThis.createWorker = ({ classWorkerURL, ...options } = {})=>{
    if (classWorkerURL) {
        return new Worker(new URL(classWorkerURL, __TURBOPACK__import$2e$meta__.url), options);
    }
};
}),
]);

//# sourceMappingURL=411l1ee5zmkys.js.map