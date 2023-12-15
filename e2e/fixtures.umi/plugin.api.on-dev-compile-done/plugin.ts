export default (api) => {
  console.log("plugin.api.onDevCompileDone");
  api.onDevCompileDone((args) => {
    console.log("dev compile done", args);
  });
};
