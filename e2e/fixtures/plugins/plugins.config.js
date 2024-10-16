module.exports = [
  {
    async load(path) {
      if (path.endsWith("foo.bar")) {
        return {
          content: `export default () => <Foooo>foo.bar</Foooo>;`,
          type: "jsx",
        };
      }
    },
  },
  {
    async loadInclude(path) {
      return path.endsWith(".bar");
    },
    async load() {
      return {
        content: `export default () => <Foooo>.bar</Foooo>;`,
        type: "jsx",
      };
    },
  },
  {
    async resolveId(source, importer, options) {
      console.log("resolveId", source, importer, options);
      if (source === "resolve_id") {
        return {
          id: require("path").join(__dirname, "resolve_id_mock.js"),
          external: false,
        };
      }
      if (source === "resolve_id_external") {
        return { id: "resolve_id_external", external: true };
      }
      return null;
    },
  },
  {
    async transform(code, id) {
      if (id.endsWith("transform.ts")) {
        console.log("transform", code, id);
        return {
          content: code.replace("transform", "transform_1"),
          type: "ts",
        };
      }
    },
  },
  {
    enforce: "pre",
    async transform(code, id) {
      if (id.endsWith("transform.ts")) {
        console.log("transform", code, id);
        return {
          content: code.replace("transform", "transform_2"),
          type: "ts",
        };
      }
    },
  },
];
