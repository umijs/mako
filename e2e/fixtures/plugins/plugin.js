
module.exports = {
  async load(ctx,path) {
    ctx.warn("11111111");
    if (path.endsWith('.hoo')) {
      return {
        content: `export default () => <Foooo>.hoo</Foooo>;`,
        type: 'jsx',
      };
    }
  }
};
