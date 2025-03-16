// @ts-nocheck
export default class LessImportPlugin {
  install(less, pluginManager) {
    pluginManager.addVisitor({
      isReplacing: true,
      isPreEvalVisitor: true,

      run(root, visitArgs) {
        return new less.visitors.Visitor({
          visitImport(node, visitArgs) {
            let pathValue;

            // 只处理 @import '*.less' | url('*.less')
            if (node.path instanceof less.tree.Quoted) {
              pathValue = node.path.value;
            } else if (node.path instanceof less.tree.URL) {
              if (node.path.value instanceof less.tree.Quoted) {
                pathValue = node.path.value.value;
              }
            }

            if (pathValue?.endsWith('.less')) {
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
