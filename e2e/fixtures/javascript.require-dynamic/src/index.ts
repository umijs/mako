function loadLang(lang) {
  return import(`./i18n/${lang}.json`);
}

function loadLang2(lang) {
  return import(`./i18n/zh-${lang}.json`);
}

function loadLangExt(lang, ext) {
  // nested dynamic require + with then callback
  return import(`./i18n/${lang}.${(require(`./ext/${ext}`)).default}`).then(m => m);
}


function loadFile(file) {
  return require('@/i18n' + file);
}

function loadFile2(file) {
  return require('./fake.js' + file);
}

function loadFile3(file) {
  return require('./fake.js/a' + file);
}
loadLang('zh-CN').then(console.log);
loadLang2('CN').then(console.log);

loadLangExt('zh-CN', 'json').then(console.log);

console.log(loadFile('/zh-CN.json'));
console.log(loadFile2('/a.js'));
console.log(loadFile3('a.js'));

