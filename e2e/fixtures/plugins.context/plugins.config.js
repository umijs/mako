module.exports = [
  {
    async loadInclude(path) {
      // this.warn('loadInclude: ' + path);
      path.endsWith('.hoo');
      return true;
    },
    async load(path) {
      if (path.endsWith('.hoo')) {
        // console.log('load', path, this, this.warn);
        this.warn('load: ' + path);
        this.warn({
          message: 'test warn with object',
        });
        this.error('error: ' + path);
        this.emitFile({
          fileName: 'test.txt',
          source: 'test',
        });
        return {
          content: `export default () => <Foooo>.hoo</Foooo>;`,
          type: 'jsx',
        };
      }
    }
  },
];
