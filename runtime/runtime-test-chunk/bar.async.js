window.jsonpCallback([
  ['bar'],
  {
    bar: function (module, exports, __mako_require__) {
      console.log('bar');
      module.exports = 'bar';
    },
  },
]);
