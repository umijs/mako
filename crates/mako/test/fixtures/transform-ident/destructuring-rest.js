
const { ...a } = {};
a;

const [...b] = [];
b;

function f ({ ...c }, [...d]) {
  c;
  d;
}

({ ...e }, [...f]) => { e; f; };
