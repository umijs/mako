import React from 'react';
import { proxy, useSnapshot } from 'valtio';
import Checkbox from 'antd/es/checkbox';
// import styled from "styled-components";

const todos = proxy({
  list: [{ id: 1, text: 'hello', done: false }],
  actions: {
    toggleChecked(id: number) {
      for (const item of todos.list) {
        if (item.id === id) {
          item.done = !item.done;
        }
      }
    },
  },
});

// const Wrapper = styled.div`
// 	.list div {
// 		display: flex;
// 	}
// `;

export function Todos() {
  const snap = useSnapshot(todos);
  return (
    <div>
      <h2>Todos</h2>
      <div className="list">
        {snap.list.map((todo) => {
          return (
            <div key={todo.id}>
              <Checkbox checked={todo.done} onChange={() => todos.actions.toggleChecked(todo.id)}>
                {todo.text}
              </Checkbox>
            </div>
          );
        })}
      </div>
    </div>
  );
}
