function loadLang(lang) {
  return import(`./i18n/${lang}.json`);
}

function loadLangExt(lang, ext) {
  // nested dynamic require + with then callback
  return import(`./i18n/${lang}.${(require(`./ext/${ext}`)).default}`).then(m => m);
}

function loadFile(file) {
  return require('@/i18n' + file);
}

function loadFile2(file) {
  return require('./fake.js/' + file);
}

console.log(loadLang('zh-CN'));
console.log(loadLangExt('zh-CN', 'json'));
console.log(loadFile('/zh-CN.json'));
console.log(loadFile2('a.js'));
