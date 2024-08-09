import React from 'react';

class NonExportClassCmp extends React.Component {
  render() {
    return <div>Hot update non-export class component123.</div>;
  }
}

export default class ClassCmp extends React.Component {
  render() {
    return (
      <div>
        Hot update default class component.
        <NonExportClassCmp />
      </div>
    );
  }
}

export class ClassCmp2 extends React.Component {
  render() {
    return (
      <div>
        Hot update export class component. <NonExportClassCmp />
      </div>
    );
  }
}

export function ChildFnCmp() {
  return <b>Hot update export function component.</b>;
}
