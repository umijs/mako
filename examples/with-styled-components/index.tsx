import React from 'react';
import ReactDOM from 'react-dom/client';
import styled from 'styled-components';

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
    '3333',
    <DivContainer>red div</DivContainer>,
    <SpanContainer>yellow span</SpanContainer>,
    <Parent>
      <Child>Green because I am inside a Parent</Child>
    </Parent>,
    <Child>Red because I am not inside a Parent</Child>,
  ];
};

ReactDOM.createRoot(document.getElementById('root')).render(<App />);
