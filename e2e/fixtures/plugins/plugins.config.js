
module.exports = [
  {
    async load(path) {
      if (path.endsWith('foo.bar')) {
        return {
          content: `export default () => <Foooo>foo.bar</Foooo>;`,
          type: 'jsx',
        };
      }
    }
  },
  {
    async load(path) {
      if (path.endsWith('.bar')) {
        return {
          content: `export default () => <Foooo>.bar</Foooo>;`,
          type: 'jsx',
        };
      }
    }
  },
  {
    async resolveId(source, importer) {
      console.log('resolveId', source, importer);
      if (source === 'resolve_id') {
        return { id: require('path').join(__dirname, 'resolve_id_mock.js'), external: false };
      }
      if (source === 'resolve_id_external') {
        return { id: 'resolve_id_external', external: true };
      }
      return null;
    }
  },
];
