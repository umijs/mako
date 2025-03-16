// @ts-nocheck
export default class LessImportPlugin {
  install(less, pluginManager) {
    pluginManager.addVisitor({
      isReplacing: true,
      isPreEvalVisitor: true,

      run(root, visitArgs) {
        return new less.visitors.Visitor({
          visitImport(node, visitArgs) {
            if (node.path.value.endsWith('.less')) {
              node.options.value = 'css';
              node.options.less = false;
            }
            return node;
          },
        }).visit(root, visitArgs);
      },
    });
  }
}
