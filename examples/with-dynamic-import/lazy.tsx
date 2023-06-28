import React from 'react';

export default function LazyComponent() {
  const [text, setText] = React.useState('Initial State');

  React.useEffect(() => {
    setTimeout(() => {
      setText('State updated!');
    }, 2000);
  }, []);

  return (
    <div>
      <h3>{text}</h3>
    </div>
  );
}
