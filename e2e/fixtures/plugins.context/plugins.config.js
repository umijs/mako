module.exports = [
  {
    async load(path) {
      if (path.endsWith('.hoo')) {
        console.log('----');
        console.log('load', path, this, this.error, this.root);
        console.log('----');
        return {
          content: `export default () => <Foooo>.hoo</Foooo>;`,
          type: 'jsx',
        };
      }
    }
  },
];
