import './scripts'
function fn(config) {
  config = arguments[1];
  config.url = arguments[0];
  return config;
}

it('should run fn successfully under strict mode with a esm module',()=>{
  // @ts-ignore
  expect(fn('url', { method: 'get' }).url.indexOf('u')).toBe(0)
});


