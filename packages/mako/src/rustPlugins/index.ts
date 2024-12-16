import { pathToFileURL } from 'url';
export async function rustPluginResolver(plugins: Array<[string, any]>) {
  const promises = plugins.map(([plugin]) => {
    let pluginPath = require.resolve(plugin);
    if (process.platform == 'win32')
      return import(pathToFileURL(pluginPath).toString());
    else return import(pluginPath);
  });
  const result = await Promise.all(promises);
  return result.map((resolved, i) => [resolved.default, plugins[i][1]]);
}
