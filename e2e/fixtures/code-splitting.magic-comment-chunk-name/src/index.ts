import(/* webpackChunkName: 'chunk_a' */  "./lazy_a_0");

import(/* webpackChunkName: 'chunk_a' */  "./lazy_a_1");

import(/* makoChunkName: 'chunk_b' */ "./lazy_b");

new Worker(/* makoChunkName: 'my_worker' */  new URL("./worker", import.meta.url))
