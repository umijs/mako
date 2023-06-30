import './render';
const onUpdate = () => {
  console.time('hmr');
  module.hot.check().then(() => {
    console.timeEnd('hmr');
  });
};
