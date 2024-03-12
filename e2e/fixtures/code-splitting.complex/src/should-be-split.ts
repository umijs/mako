import React from 'react';
import ReactDOM from 'react-dom';
import * as less from 'less';
import * as overlay from 'react-error-overlay';
import * as refresh from 'react-refresh';
// @ts-ignore
import * as antd from 'antd';
import * as icons from '@ant-design/icons';
import context from './context';
import common from './common';
// make sure `should-be-split-self` has 2+ parents
import other from './should-not-be-common';

console.log(React, ReactDOM, less, overlay, refresh, antd, icons, context, common, other);

import('less/lib/less-browser/utils').then((m) => console.log(m));

export default {
  React,
  ReactDOM,
  less,
  overlay,
  refresh,
  antd,
  icons,
  context,
  common,
};
