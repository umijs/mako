in bundler the ignore just treat it as not found

how to produce it in umi project

```js
// plugin.ts
export default function (api) {
  api.modifyWebpackConfig((webpackConfig, { env, webpack }) => {
    webpackConfig.plugins.push(
      new webpack.IgnorePlugin({
        resourceRegExp: /antd/,
      })
    );
  });
}
```
