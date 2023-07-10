/// <reference types="cypress" />

describe('HMR', () => {
  beforeEach(() => {
    // Cypress starts out with a blank slate for each test
    // so we must tell it to visit our website with the `cy.visit()` command.
    // Since we want to visit the same URL at the start of all our tests,
    // we include it in our beforeEach function so that it runs before each test
    cy.visit('http://127.0.0.1:3000/');
  });

  it('render local component', () => {
    cy.get('[data-test-id="local"]').should('exist');
  });

  it('renders dynamic loaded component', () => {
    cy.get('[data-test-id="dynamic-counter"]').should('exist');
  });

  beforeEach(() => {
    cy.exec('git checkout .');
  });

  afterEach(() => {
    cy.exec('git checkout .');
  });

  it('respones to write updadte', () => {
    cy.get('[data-test-id="dynamic-counter"]').should(
      'contain.text',
      'count: 123',
    );

    cy.writeFile(
      'lazy.tsx',
      `import React from 'react';
export default function LazyComponent() {
  const [text, setText] = React.useState('Initial State');
  const [count, setCount] = React.useState(123456);

  React.useEffect(() => {
    setTimeout(() => {
      setText('State updated!');
    }, 2000);
  }, []);

  return (
    <div>
      <h3>{text}</h3>
      <h3 data-test-id="dynamic-counter">count: [{count}]</h3>
      {/* rome-ignore lint/a11y/useButtonType: <explanation> */}
      <button
        onClick={() => {
          setCount(count + 1);
        }}
      >
        count
      </button>
    </div>
  );
}
`,
    );

    cy.get('[data-test-id="dynamic-counter"]').should(
      'contain.text',
      'count: [123456]',
    );
  });
});
