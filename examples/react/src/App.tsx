import React from 'react';
import ClassCmp, { ChildFnCmp, ClassCmp2 } from './ClassCmp';
import { foo } from './foo';
export function App() {
  return (
    <h1>
      App {foo} <ClassCmp />
      <ChildFnCmp />
      <ClassCmp2 />
    </h1>
  );
}
