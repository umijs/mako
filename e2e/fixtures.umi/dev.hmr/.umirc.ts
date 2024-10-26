export default {
  mfsu: false,
  mako: {},
  lessLoader: {
    math: 'always',
  },
  proxy: {
    '/api': {
      'target': 'http://jsonplaceholder.typicode.com/',
      'changeOrigin': true,
      'pathRewrite': { '^/api' : '' },
    }
  }
};
