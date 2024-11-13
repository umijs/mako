import { pathToFileURL } from 'url';
export async function rustPluginResolver(plugins: Array<[string, any]>) {
  const resolved: Array<[string, any]> = [];
  for (const [plugin, options] of plugins) {
    let pluginPath = require.resolve(plugin);
    if (process.platform === 'win32') {
      pluginPath = (await import(pathToFileURL(pluginPath).toString())).default;
    } else {
      pluginPath = await import(pluginPath).then((m) => m.default);
    }
    resolved.push([pluginPath, JSON.stringify(options)]);
  }
  return resolved;
}
