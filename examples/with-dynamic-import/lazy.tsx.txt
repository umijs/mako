import React from 'react';
import classNames from 'classnames';

console.log(classNames({ lazy: 0 }));

export default function LazyComponent() {
  const [text, setText] = React.useState('Initial State');
  const [count, setCount] = React.useState(123);

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
