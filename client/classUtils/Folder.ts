import BaseFolder from './BaseFolder';
import Module from './Module';
class Folder extends BaseFolder {
  get parsedSize() {
    return this.src ? this.src.length : 0;
  }
  get gzipSize() {
    if (!Object.prototype.hasOwnProperty.call(this, '_gzipSize')) {
      this._gzipSize = this.src ? _gzipSize.default.sync(this.src) : 0;
    }
    return this._gzipSize;
  }
  addModule(moduleData) {
    const loaders = moduleData.id.split('!');
    const parsedPath = loaders[loaders.length - 1]
      // Splitting module path into parts
      .split('/')
      // Removing first `.`
      .slice(1)
      // Replacing `~` with `node_modules`
      .map((part) => (part === '~' ? 'node_modules' : part));
    // 如果路径不存在，则结束
    if (!parsedPath) {
      return;
    }
    const [folders, fileName] = [
      parsedPath.slice(0, -1),
      parsedPath[parsedPath.length - 1],
    ];

    let currentFolder = this;
    folders.forEach((folderName) => {
      let childNode = currentFolder.getChild(folderName);
      if (
        // Folder is not created yet
        !childNode ||
        // In some situations (invalid usage of dynamic `require()`) webpack generates a module with empty require
        // context, but it's moduleId points to a directory in filesystem.
        // In this case we replace this `File` node with `Folder`.
        // See `test/stats/with-invalid-dynamic-require.json` as an example.
        !(childNode instanceof Folder)
      ) {
        childNode = currentFolder.addChildFolder(new Folder(folderName));
      }
      currentFolder = childNode;
    });

    const module = new Module(fileName, moduleData, this);
    currentFolder.addChildModule(module);
  }
  toChartData() {
    return {
      ...super.toChartData(),
      parsedSize: this.parsedSize,
      gzipSize: this.gzipSize,
    };
  }
}
export default Folder;
