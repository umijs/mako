const fs = require("fs");
const path = require("path");

const distPath = path.join(__dirname, "dist");

const assets = fs
  .readdirSync(distPath)
  .filter(
    (p) =>
      fs.statSync(path.join(distPath, p)).isFile() &&
      (p.endsWith(".js") || p.endsWith(".css")),
  );

const html = `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>External Dependencies Test</title>
</head>
<body>
    <h1>External Dependencies Test</h1>
    <div id="output"></div>
    
    <script>
        // Mock external libraries that would normally be loaded from external URLs
        window.bar = { name: 'bar', type: 'global' };
        window.bar_script2 = { name: 'bar_script2', type: 'script', url: 'https://example.com/lib/script.js' };
        
        // Mock require function for CommonJS externals
        window.require = function(id) {
            if (id === 'bar_require2') {
                return { name: 'bar_require2', type: 'require' };
            }
            if (id === 'bar') {
                return { name: 'bar', type: 'require' };
            }
            throw new Error('Module not found: ' + id);
        };
        
        // Mock ES module dynamic import
        window.import = async function(id) {
            if (id === 'bar_import2') {
                return { default: { name: 'bar_import2', type: 'import' } };
            }
            if (id === 'bar') {
                return { default: { name: 'bar', type: 'import' } };
            }
            throw new Error('Module not found: ' + id);
        };
        
        console.log('Test environment setup complete');
    </script>
    
    <!-- Load CSS files -->
    ${assets
      .filter((a) => a.endsWith(".css"))
      .map((a) => `<link rel="stylesheet" href="${a}">`)
      .join("\n    ")}
    
    <!-- Load JavaScript files -->
    ${assets
      .filter((a) => a.endsWith(".js"))
      .map((a) => `<script src="${a}"></script>`)
      .join("\n    ")}
    
    <script>
        // Check console for output
        console.log('All scripts loaded');
    </script>
</body>
</html>
`;

fs.writeFileSync(path.join(distPath, "index.html"), html);
