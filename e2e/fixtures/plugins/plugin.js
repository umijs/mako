
module.exports = {
  async load(path) {
    if (path.endsWith('.hoo')) {
      return {
        content: `export default () => <Foooo>.hoo</Foooo>;`,
        type: 'jsx',
      };
    }
  }
};
