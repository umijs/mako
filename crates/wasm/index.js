const wasm = require('./pkg/wasm_binding');
const fs = require('fs');

global.myfs = {
    file_size(t) {
        console.log('file size: ', t);
        try {
            const stat = fs.statSync(t);
            return BigInt(stat.size);
        } catch (e) {
            console.log(e);
            return 0n;
        }
    },

    file_exists(t) {
        console.log('file exists: ', t);
        return fs.existsSync(t);
    },

    file_write(t, buf) {
        console.log('file write: ', t);
        return fs.writeFileSync(t, buf);
    },

    file_read(t) {
        console.log('file read: ', t);
        return fs.readFileSync(t);
    },

    file_create_dir_all(t) {
        console.log('file create dir all: ', t);
        fs.mkdirSync(t, {
            recursive: true,
        });
    },

    is_file(t) {
        const stats = fs.statSync(t);
        return stats.isFile()
    }
};

(async () => {
    const res = await wasm.greet("/Users/killa/workspace/mako/e2e/fixtures/config.entry");
    console.log('res: ', res);
})();
