#!/bin/bash

# 参数检查
if [ "$#" -ne 2 ]; then
    echo "Usage: $0 <package-name> <version>"
    exit 1
fi

NAME=$1
VERSION=$2

# 创建临时工作目录
WORK_DIR=$(mktemp -d)
echo "Working in temporary directory: $WORK_DIR"

# 创建主包目录
ENTRY_DIR="$WORK_DIR/entry"
mkdir -p "$ENTRY_DIR"

# 复制并处理 entry package.json 模板
cp ../templates/entry.package.json.template "$ENTRY_DIR/package.json"
sed -i '' "s|{{name}}|$NAME|g" "$ENTRY_DIR/package.json"
sed -i '' "s|{{version}}|$VERSION|g" "$ENTRY_DIR/package.json"

# 复制 postinstall 脚本
cp ../templates/postinstall.sh.template "$ENTRY_DIR/postinstall.sh"
sed -i '' "s|{{name}}|$NAME|g" "$ENTRY_DIR/postinstall.sh"

# 创建 bin 目录和默认二进制文件
mkdir -p "$ENTRY_DIR/bin"
cat > "$ENTRY_DIR/bin/$NAME" << EOF
#!/bin/bash
echo "This is a placeholder binary for $NAME. The actual binary will be installed via postinstall script."
exit 1
EOF
chmod +x "$ENTRY_DIR/bin/$NAME"

# 发布主包
cd "$ENTRY_DIR"
npm publish --dry-run

# 清理
# rm -rf "$WORK_DIR"
