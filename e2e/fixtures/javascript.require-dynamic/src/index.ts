function loadLang(lang) {
  return import(`./i18n/${lang}.json`);
}

function loadFile(file) {
  return require('@/i18n' + file);
}

function loadFile2(file) {
  return require('./fake.js/' + file);
}

console.log(loadLang('zh-CN'));
console.log(loadFile('/zh-CN.json'));
console.log(loadFile2('a.js'));
