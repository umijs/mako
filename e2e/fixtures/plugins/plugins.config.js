
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
  }
];
