const assert = require("assert");

module.exports = (e) => {
  assert(e.message.includes(`from client components as server action`));
};
