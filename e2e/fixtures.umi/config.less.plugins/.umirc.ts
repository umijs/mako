export default {
  mfsu: false,
  lessLoader: {
    pluginsForMako: [
      [require.resolve("less-plugin-clean-css"), { roundingPrecision: 1 }]
    ],
  },
};
