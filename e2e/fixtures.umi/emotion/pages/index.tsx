import React from 'react';
import styles from './index.less';
import { css } from "@emotion/react";
import styled from "@emotion/styled";

const style = css`
  color: rgb(55, 255, 0);
`;

const Foo = styled.div({
  background: "rgb(150, 255, 3)",
  color: '#fff',
});

1;

export default function Page() {
  return (
    <div css={style}>
      <Foo>Hello</Foo>
      <h1 className={styles.title}>hello</h1>
      <span>abcd</span>
    </div>
  );
}
