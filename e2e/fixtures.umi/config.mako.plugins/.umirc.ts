
export default {
  mako: {
    plugins: [
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
    ],
  },
  mfsu: false,
}
