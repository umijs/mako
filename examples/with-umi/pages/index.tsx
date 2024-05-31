import React from 'react';
import { connect } from 'umi';
import styles from './index.css';

function mapStateToProps(state) {
  return {
    count: state.count,
  };
}

export default connect(mapStateToProps)(function Page(props) {
  return (
    <div>
      <h1 className={styles.title}>Page index</h1>
      {/* biome-ignore lint/a11y/useButtonType: <explanation> */}
      <button
        onClick={() => {
          props.dispatch({ type: 'count/add' });
        }}
      >
        {props.count}
      </button>
    </div>
  );
});
