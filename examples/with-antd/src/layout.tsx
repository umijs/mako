import React from 'react';
import { Link, Outlet } from 'react-router-dom';
import styled from 'styled-components';

const Wrapper = styled.div`
	h1 {
		color: red;
	}
	h2 {
		color: blue;
	}
	nav {
		ul {
			padding-left: 1rem;
		}
	}
`;

export function Layout() {
  return (
    <Wrapper>
      <h1>with-antd</h1>
      <nav>
        <ul>
          <li>
            <Link to="/">Home</Link>
          </li>
          <li>
            <Link to="/todos">Todos</Link>
          </li>
          <li>
            <Link to="/react-query">React Query</Link>
          </li>
          <li>
            <Link to="/monaco-editor">Monaco Query</Link>
          </li>
          <li>
            <Link to="/ant-design-icons">Ant Design Icons</Link>
          </li>
          <li>
            <Link to="/ant-design-pro">Ant Design Pro</Link>
          </li>
        </ul>
      </nav>
      <div>
        <Outlet />
      </div>
    </Wrapper>
  );
}
