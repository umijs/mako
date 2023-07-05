import React from 'react';

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
      <h3>count: {count}</h3>
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
