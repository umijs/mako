#!/bin/bash

# 参数检查
if [ "$#" -ne 5 ]; then
    echo "Usage: $0 <package-name> <version> <binary-path> <os> <cpu>"
    exit 1
fi

NAME=$1
VERSION=$2
BINARY=$3
OS=$4
CPU=$5

# 创建临时工作目录
WORK_DIR=$(mktemp -d)
echo "Working in temporary directory: $WORK_DIR"

# 创建平台特定包目录
PLATFORM_DIR="$WORK_DIR/binary"
mkdir -p "$PLATFORM_DIR/bin"

# 复制并处理 binary package.json 模板
cp ../templates/binary.package.json.template "$PLATFORM_DIR/package.json"
sed -i '' "s|{{name}}|$NAME|g" "$PLATFORM_DIR/package.json"
sed -i '' "s|{{version}}|$VERSION|g" "$PLATFORM_DIR/package.json"
sed -i '' "s|{{platform}}|$OS-$CPU|g" "$PLATFORM_DIR/package.json"
sed -i '' "s|{{os}}|$OS|g" "$PLATFORM_DIR/package.json"
sed -i '' "s|{{cpu}}|$CPU|g" "$PLATFORM_DIR/package.json"

# 复制二进制文件
cp "$BINARY" "$PLATFORM_DIR/bin/$NAME"
chmod +x "$PLATFORM_DIR/bin/$NAME"

# 发布平台包
cd "$PLATFORM_DIR"
npm publish --dry-run

# 清理
rm -rf "$WORK_DIR"