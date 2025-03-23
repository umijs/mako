#!/bin/bash

# args check
if [ "$#" -ne 2 ]; then
    echo "Usage: $0 <package-name> <version>"
    exit 1
fi

NAME=$1
VERSION=$2

# create temp dir
WORK_DIR=$(mktemp -d)
echo "Working in temporary directory: $WORK_DIR"

# create vendor dir
ENTRY_DIR="$WORK_DIR/entry"
mkdir -p "$ENTRY_DIR"

# render entry package.json template
cp ../templates/entry.package.json.template "$ENTRY_DIR/package.json"
cat "$ENTRY_DIR/package.json" | \
# use awk to replace placeholders since sed is different on macOS and Linux
    awk -v name="$NAME" \
        -v version="$VERSION" \
    '{
        gsub(/{{name}}/, name);
        gsub(/{{version}}/, version);
        print;
    }' > "$ENTRY_DIR/package.json.tmp" && mv "$ENTRY_DIR/package.json.tmp" "$ENTRY_DIR/package.json"

# do copy postinstall.sh
cp ../templates/postinstall.sh.template "$ENTRY_DIR/postinstall.sh"
cat "$ENTRY_DIR/postinstall.sh" | \
    awk -v name="$NAME" \
    '{
        gsub(/{{name}}/, name);
        print;
    }' > "$ENTRY_DIR/postinstall.sh.tmp" && mv "$ENTRY_DIR/postinstall.sh.tmp" "$ENTRY_DIR/postinstall.sh"

# create placeholder binary
mkdir -p "$ENTRY_DIR/bin"
cat > "$ENTRY_DIR/bin/$NAME" << EOF
#!/bin/bash
echo "This is a placeholder binary for $NAME. The actual binary will be installed via postinstall script."
exit 1
EOF
chmod +x "$ENTRY_DIR/bin/$NAME"

# do publish, --dry-run for test
cd "$ENTRY_DIR"
npm publish --dry-run
cat package.json

# clean up temp dir
rm -rf "$WORK_DIR"
