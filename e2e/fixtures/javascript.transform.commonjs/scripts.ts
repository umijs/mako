'use action';
'use strict';
function fn(config) {
  config = arguments[1];
  config.url = arguments[0];
  return config;
}
// @ts-ignore
fn('url', { method: 'get' }).url.indexOf('u')


it('should run fn successfully under strict mode with scripts',()=>{
  // @ts-ignore
  expect(fn('url', { method: 'get' }).url.indexOf('u')).toBe(0)
});

