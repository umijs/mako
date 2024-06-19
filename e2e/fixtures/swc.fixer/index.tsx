const a = { b: () => {console.log('b')} };

(a.b as Function)();