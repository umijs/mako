export default {
  mfsu: false,
  lessLoader: {
    plugins: [
      [require.resolve("less-plugin-clean-css"), { roundingPrecision: 1 }]
    ],
  },
};
