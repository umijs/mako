module.exports = {
  plugins: [
    require('postcss-px-to-viewport-8-plugin')({
      unitToConvert: 'px',
      viewportWidth: 375,
      propList: ['*'],
    }),
  ],
};
