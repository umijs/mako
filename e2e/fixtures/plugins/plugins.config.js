
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
      if (source === 'resolve_id') {
        console.log('resolveId', source, importer);
        return { id: require('path').join(__dirname, 'resolve_id_mock.js') };
      }
      return null;
    }
  },
];
