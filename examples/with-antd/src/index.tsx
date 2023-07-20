import React from 'react';
import ReactDOM from 'react-dom/client';
import { createHashRouter, RouterProvider } from 'react-router-dom';
import { Layout } from './layout';
import { Home } from './pages/home';
import { Todos } from './pages/todos';
import { ReactQuery } from './pages/react-query';
import { MonacoEditor } from './pages/monaco-editor';
import { AntDesignIcons } from './pages/ant-design-icons';
import { AntDesignPro } from './pages/ant-design-pro';

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
