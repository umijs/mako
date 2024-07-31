import sass from 'sass';

export default {
  mfsu: false,
  mako: {},
  sassLoader: {
    functions: {
      // Note: in real code, you should use `math.pow()` from the built-in
      // `sass:math` module.
      'pow($base, $exponent)': function(args) {
        const base = args[0].assertNumber('base').assertNoUnits('base');
        const exponent =
            args[1].assertNumber('exponent').assertNoUnits('exponent');

        return new sass.SassNumber(Math.pow(base.value, exponent.value));
      }
    }
  },
};
