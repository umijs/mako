import { addTarget as _addTarget } from "./file1";

const createC = function () {
  const methods = [
    {
      key: "addTarget",
      value: function addTarget() {
        _addTarget();
        return "OK";
      },
    },
  ];

  return {
    [methods[0].key]: methods[0].value,
  };
};

const c = createC();

export { c };
