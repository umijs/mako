module.exports = function loader(request) {
  const options = this.getOptions();

  const insert = !options.insert
    ? '"head"'
    : typeof options.insert === "string"
      ? JSON.stringify(options.insert)
      : options.insert.toString();
  const injectType = options.injectType || "styleTag";

  switch (injectType) {
    case "linkTag": {
      return `${`import api from ${JSON.stringify("@utoo/style-loader/src/runtime/injectStylesIntoLinkTag.js")};
var content = ${JSON.stringify(request)};`}

var options = ${JSON.stringify(options)};

options.insert = ${insert};

var update = api([[${JSON.stringify(this.resourcePath.replace(this.rootContext, ""))}, content, undefined, undefined]], options);


${"export default {}"}`;
    }

    case "lazyStyleTag":
    case "lazySingletonStyleTag": {
      const isSingleton = injectType === "lazySingletonStyleTag";

      return `${`import api from ${JSON.stringify("@utoo/style-loader/src/runtime/injectStylesIntoStyleTag.js")};
var content = ${JSON.stringify(request)};`}

var refs = 0;
var update;
var options = ${JSON.stringify(options)};

options.insert = ${insert};
options.singleton = ${isSingleton};

var exported = {};

exported.locals = content.locals || {};
exported.use = function() {
  if (!(refs++)) {
    update = api([[${JSON.stringify(this.resourcePath.replace(this.rootContext, ""))}, content, undefined, undefined]], options);
  }

  return exported;
};
exported.unuse = function() {
  if (refs > 0 && !--refs) {
    update();
    update = null;
  }
};

${"export default"} exported;`;
    }

    case "styleTag":
    case "singletonStyleTag":
    default: {
      const isSingleton = injectType === "singletonStyleTag";

      return `${`import api from ${JSON.stringify("@utoo/style-loader/src/runtime/injectStylesIntoStyleTag.js")};
var content = ${JSON.stringify(request)};`}

var options = ${JSON.stringify(options)};

options.insert = ${insert};
options.singleton = ${isSingleton};

var update = api([[${JSON.stringify(this.resourcePath.replace(this.rootContext, ""))}, content, undefined, undefined]], options);


${"export default"} content.locals || {};`;
    }
  }
};
