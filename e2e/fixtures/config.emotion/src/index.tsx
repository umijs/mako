import { css } from "@emotion/react";
import styled from "@emotion/styled";
import ReactDOM from "react-dom/client";

const style = css`
  color: rgb(1, 1, 1);
`;

const DivContainer = styled.div({
  background: "rgb(2,2,2)",
});

const Child = styled.div`
  color: rgb(3, 3, 3);
`;

const Parent = styled.div`
  ${Child} {
    color: rgb(4, 4, 4);
  }
`;

const App = () => {
  return [
    <div css={style} id="first" data-testid="directions">
      Hello emotion
    </div>,
    <DivContainer id="second">red div</DivContainer>,
    <Child id="third">Red because I am not inside a Parent</Child>,
    <Parent>
      <Child id="forth">Green because I am inside a Parent</Child>
    </Parent>,
  ];
};

ReactDOM.createRoot(document.getElementById("root")!).render(<App />);
