import { css } from '@emotion/react';
import styled from '@emotion/styled';
import ReactDOM from 'react-dom/client';
import React from 'react';

const style = css`
  color: hotpink;
`;

const DivContainer = styled.div({
  background: 'red',
});

const SpanContainer = styled('span')({
  background: 'yellow',
});

const Child = styled.div`
  color: red;
`;

const Parent = styled.div`
  ${Child} {
    color: green;
  }
`;

const App = () => {
  return [
    <div css={style}>Hello emotion</div>,
    <DivContainer>red div</DivContainer>,
    <SpanContainer>yellow span</SpanContainer>,
    <Parent>
      <Child>Green because I am inside a Parent</Child>
    </Parent>,
    <Child>Red because I am not inside a Parent</Child>,
  ];
};

ReactDOM.createRoot(document.getElementById('root')!).render(<App />);
