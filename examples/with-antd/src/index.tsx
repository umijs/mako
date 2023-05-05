import React from 'react';
import ReactDOM from 'react-dom/client';
import { createHashRouter, RouterProvider } from 'react-router-dom';
import { Layout } from './layout';
import { Home } from './home';
import { Todos } from './todos';
import { ReactQuery } from './react-query';
import { MonacoEditor } from './monaco-editor';
import { AntDesignIcons } from './ant-design-icons';

const router = createHashRouter([
  {
    path: '/',
    element: <Layout />,
    children: [
      {
        path: '/todos',
        element: <Todos />,
      },
      {
        path: '/react-query',
        element: <ReactQuery />,
      },
      {
        path: '/monaco-editor',
        element: <MonacoEditor />,
      },
      {
        path: '/ant-design-icons',
        element: <AntDesignIcons />,
      },
      {
        path: '/',
        element: <Home />,
      },
    ],
  },
]);

function App() {
  return (
    <div>
      <RouterProvider router={router} />
    </div>
  );
}

ReactDOM.createRoot(document.getElementById('root')).render(<App />);
