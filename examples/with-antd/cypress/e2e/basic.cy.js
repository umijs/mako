/// <reference types="cypress" />

describe('antd works', () => {
  beforeEach(() => {
    // Cypress starts out with a blank slate for each test
    // so we must tell it to visit our website with the `cy.visit()` command.
    // Since we want to visit the same URL at the start of all our tests,
    // we include it in our beforeEach function so that it runs before each test
    cy.visit('http://127.0.0.1:8080/');
  });

  it('render antd', () => {
    cy.get('[data-test-id="app"]').should('exist');
  });
});
