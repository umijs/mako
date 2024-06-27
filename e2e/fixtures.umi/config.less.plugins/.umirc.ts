export default {
  mfsu: false,
  mako: {},
  lessLoader: {
    plugins: [
      [require.resolve("less-plugin-clean-css"), { roundingPrecision: 1 }]
    ],
  },
};
