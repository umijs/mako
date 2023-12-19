import React from 'react';
import ReactDOM from 'react-dom/client';
import { RouterProvider, createHashRouter } from 'react-router-dom';
import { Layout } from './layout';
import { AntDesignIcons } from './pages/ant-design-icons';
import { AntDesignPro } from './pages/ant-design-pro';
import { Home } from './pages/home';
import { MonacoEditor } from './pages/monaco-editor';
import { ReactQuery } from './pages/react-query';
import { Todos } from './pages/todos';

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
        path: '/ant-design-pro',
        element: <AntDesignPro />,
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
    <div data-test-id="app">
      <RouterProvider router={router} />
    </div>
  );
}

ReactDOM.createRoot(document.getElementById('root')).render(<App />);
