import React from 'react';
import ReactDOM from 'react-dom/client';
import { RouterProvider, createHashRouter } from 'react-router-dom';
import { Layout } from './layout';

const Todos = React.lazy(() =>
  import('./pages/todos').then((m) => ({ default: m.Todos })),
);
const ReactQuery = React.lazy(() =>
  import('./pages/react-query').then((m) => ({ default: m.ReactQuery })),
);
const MonacoEditor = React.lazy(() =>
  import('./pages/monaco-editor').then((m) => ({ default: m.MonacoEditor })),
);
const Home = React.lazy(() =>
  import('./pages/home').then((m) => ({ default: m.Home })),
);
const AntDesignPro = React.lazy(() =>
  import('./pages/ant-design-pro').then((m) => ({ default: m.AntDesignPro })),
);
const AntDesignIcons = React.lazy(() =>
  import('./pages/ant-design-icons').then((m) => ({
    default: m.AntDesignIcons,
  })),
);

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
